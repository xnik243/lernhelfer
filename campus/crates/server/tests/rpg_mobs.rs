//! Phase 18: Zitadelle – Leerlauf ohne KI, Wesen erscheinen, Schlag (Reichweite/Abklingzeit/Rückstoß/Beute),
//! Boss-Beute nur ab 10 % Anteil, Tod ohne Verlust.
use axum::body::Bytes;
use campus_server::game::{Game, User};
use campus_shared::{rpg, world};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc::{channel, Receiver};

static N: AtomicU32 = AtomicU32::new(0);
const CIT: [f32; 3] = [230.5, 31.0, 78.5];

fn game() -> Game {
    let p = std::env::temp_dir().join(format!("campus-mobs-{}-{}.db", std::process::id(), N.fetch_add(1, Ordering::SeqCst))).display().to_string();
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{p}{s}"));
    }
    Game::open(&p).unwrap()
}
fn join(g: &mut Game, id: u32, name: &str) -> Receiver<Bytes> {
    let (tx, rx) = channel::<Bytes>(100_000);
    g.join(&User { id, name: name.into(), admin: false }, tx).unwrap();
    rx
}
fn put(g: &mut Game, id: u32, p: [f32; 3], shield: bool) {
    let pl = g.players.get_mut(&id).unwrap();
    pl.p = p;
    pl.shield_until = if shield { i64::MAX } else { 0 };
}

#[tokio::test]
async fn leerlauf_und_erscheinen() {
    let mut g = game();
    let _rx = join(&mut g, 1, "Anna");
    let t0 = g.now();
    for k in 0..1000 {
        g.mob_tick(t0 + k * 100);
    }
    assert_eq!(g.mobs.ai_steps, 0, "niemand in der Zitadelle → keine KI");
    assert!(g.mobs.list.is_empty());
    put(&mut g, 1, CIT, true);
    let mut k = 1000;
    while g.mobs.list.is_empty() && k < 1100 {
        g.mob_tick(t0 + k * 100);
        k += 1;
    }
    assert!(!g.mobs.list.is_empty() && g.mobs.ai_steps > 0, "Wesen erscheinen");
    assert!(g.mobs.list.iter().all(|m| m.kind != 3), "Boss erst nach BOSS_EVERY_MS");
    let v = g.mobs_get(1, "kampf").unwrap().unwrap();
    assert!(v["wesen"].as_u64().unwrap() > 0);
    // Zone leert sich → Wesen weg
    put(&mut g, 1, world::SPOTS[0].1, false);
    g.mob_tick(t0 + k * 100);
    assert!(g.mobs.list.is_empty());
}

#[tokio::test]
async fn schlag_reichweite_abklingzeit_beute() {
    let mut g = game();
    let _rx = join(&mut g, 1, "Anna");
    put(&mut g, 1, CIT, true);
    let t0 = g.now();
    let mut k = 0;
    while g.mobs.list.is_empty() && k < 100 {
        g.mob_tick(t0 + k * 100);
        k += 1;
    }
    let now = t0 + 100_000;
    let id = g.mobs.list[0].id;
    {
        let m = &mut g.mobs.list[0];
        m.hp = 1000.0;
        m.max = 1000.0;
        m.p = [CIT[0] + 10.0, world::CITADEL_Y, CIT[2]];
    }
    assert!(g.attack(1, id, now).is_err(), "zu weit weg");
    g.mobs.list[0].p = [CIT[0] + 1.5, world::CITADEL_Y, CIT[2]];
    g.attack(1, id, now).unwrap();
    assert!(g.mobs.list[0].hp < 1000.0);
    assert!(g.mobs.list[0].kb[0] > 0.0, "Rückstoß weg vom Spieler");
    assert!(g.attack(1, id, now + 100).is_err(), "Abklingzeit");
    let taler = g.taler(1);
    g.mobs.list[0].hp = 0.5;
    g.attack(1, id, now + 500).unwrap();
    assert!(g.mobs.list[0].dead_at > 0);
    assert!(g.taler(1) > taler, "Beute: Taler");
    assert!(g.attack(1, id, now + 1000).is_err(), "besiegtes Wesen nicht mehr angreifbar");
}

#[tokio::test]
async fn boss_beute_nur_ab_zehn_prozent() {
    let mut g = game();
    let _a = join(&mut g, 1, "Anna");
    let _b = join(&mut g, 2, "Ben");
    put(&mut g, 1, CIT, true);
    put(&mut g, 2, [CIT[0], CIT[1], CIT[2] + 2.0], true);
    g.boss_summon(1).unwrap();
    assert!(g.boss_summon(1).is_err(), "nur ein Boss");
    let i = g.mobs.list.iter().position(|m| m.kind == 3).unwrap();
    let id = g.mobs.list[i].id;
    g.mobs.list[i].p = [CIT[0] + 2.0, world::CITADEL_Y, CIT[2]];
    g.mobs.list[i].hp = 1.0;
    g.mobs.boss_dmg = vec![(1, 1000.0), (2, 50.0)]; // Ben < 10 %
    let tb = g.taler(2);
    g.attack(1, id, g.now() + 10_000).unwrap();
    assert!(g.inv(1, rpg::ZITADELLEN_FRAGMENT) >= 4, "Anna bekommt sichere Boss-Beute");
    assert_eq!(g.inv(2, rpg::ZITADELLEN_FRAGMENT), 0, "Ben unter 10 % geht leer aus");
    assert_eq!(g.taler(2), tb);
    assert!(g.mobs_get(1, "kampf").unwrap().unwrap()["boss"].is_null());
}

#[tokio::test]
async fn tod_ohne_verlust() {
    let mut g = game();
    let _rx = join(&mut g, 1, "Anna");
    put(&mut g, 1, CIT, false);
    let vorrat = g.inv(1, world::BRETTER as u16);
    let now = g.now();
    g.hurt_player(1, 1.0e6, now);
    let p = &g.players[&1];
    assert!(p.stats.max_hp > 0.0 && (p.hp - p.stats.max_hp).abs() < 1e-3, "volle HP");
    assert!(p.shield_until > now, "Schutzschild");
    assert!(!world::in_citadel(p.p[0], p.p[1], p.p[2]), "Wiederbelebung vor der Zitadelle");
    assert_eq!(g.inv(1, world::BRETTER as u16), vorrat, "kein Gegenstandsverlust");
    // Schild schützt
    let hp = g.players[&1].hp;
    g.hurt_player(1, 50.0, now + 10);
    assert_eq!(g.players[&1].hp, hp);
}
