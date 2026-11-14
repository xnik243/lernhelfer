//! Phasen 17/19/20 (island.rs): Ackerbau nur über Zeitstempel, Insel-Rechte, Co-op, Likes, Questbuch, Verzaubern.
//! Ohne WebSocket: Game direkt auf einer Temp-Datenbank, Spieler über join mit einem tokio-Kanal.
use axum::body::Bytes;
use campus_server::game::{Game, User};
use campus_shared::enchanting::{self as en, SCHUTZ};
use campus_shared::farming::{self, FERT_BASE};
use campus_shared::farming_data::CROPS;
use campus_shared::questline_data::CHAPTERS;
use campus_shared::rpg;
use campus_shared::skyblock::{self as sb, Goal};
use campus_shared::world::{self, ACKERBODEN, ALTAR_AT, SAAT_WEIZEN, SLOT};
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc::Receiver;

static N: AtomicU32 = AtomicU32::new(0);
fn game() -> Game {
    let p = std::env::temp_dir().join(format!("campus-insel-{}-{}.db", std::process::id(), N.fetch_add(1, Ordering::SeqCst))).display().to_string();
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{p}{s}"));
    }
    Game::open(&p).unwrap()
}
fn join(g: &mut Game, id: u32, name: &str) -> Receiver<Bytes> {
    let (tx, rx) = tokio::sync::mpsc::channel(100_000);
    g.join(&User { id, name: name.into(), admin: false }, tx).unwrap();
    rx
}
fn plot_of(g: &Game, uid: u32) -> (u32, (u8, u8)) {
    let p = g.plots.iter().find(|p| p.owner == uid).unwrap();
    (p.id, p.slot)
}
/// Ackerboden in der Luft auf der Insel von `uid` (y 40); liefert die Stelle der Saat darüber.
fn field(g: &mut Game, uid: u32, dx: i32) -> (i32, i32, i32) {
    let s = plot_of(g, uid).1;
    let (x, z) = (s.0 as i32 * SLOT + 10 + dx, s.1 as i32 * SLOT + 10);
    g.set_block(x, 40, z, ACKERBODEN, 0);
    (x, 41, z)
}
/// Wie edit() beim Setzen, ohne Vorrat/Abklingzeit.
fn plant(g: &mut Game, uid: u32, (x, y, z): (i32, i32, i32)) -> Result<(), String> {
    g.before_place(uid, x, y, z, SAAT_WEIZEN).map_err(|e| e.1)?;
    g.set_block(x, y, z, SAAT_WEIZEN, uid);
    g.after_place(uid, x, y, z, SAAT_WEIZEN);
    Ok(())
}
fn planted(g: &Game, (x, y, z): (i32, i32, i32)) -> (i64, u32) {
    g.db.query_row("select planted, fert from island_crops where x = ? and y = ? and z = ?", rusqlite::params![x, y, z], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
}

#[test]
fn ackerbau_nur_ueber_zeit() {
    let mut g = game();
    let _ra = join(&mut g, 1, "Anna");
    let spot = field(&mut g, 1, 0);
    let (x, y, z) = spot;
    assert!(g.before_place(1, x + 3, y, z, SAAT_WEIZEN).is_err(), "Saat ohne Ackerboden");
    let (seeds, crop0) = (g.inv(1, SAAT_WEIZEN as u16), g.inv(1, CROPS[0].product));

    // unreif abbauen: nur die Saat zurück
    plant(&mut g, 1, spot).unwrap();
    let (t0, fert) = planted(&g, spot);
    assert_eq!(fert, FERT_BASE);
    assert_eq!(farming::stage(t0, g.now(), farming::grow_ms(0, fert)), 0);
    assert!(g.before_break(1, x, y, z, SAAT_WEIZEN).unwrap());
    assert_eq!(g.block(x, y, z), 0);
    assert_eq!(g.inv(1, SAAT_WEIZEN as u16), seeds + 1);
    assert_eq!(g.inv(1, CROPS[0].product), crop0);

    // Weizen: 600 s, ohne einen einzigen Takt – nur die Uhr läuft
    plant(&mut g, 1, spot).unwrap();
    let (t0, fert) = planted(&g, spot);
    let grow = farming::grow_ms(0, fert);
    g.clock_offset += 599_000;
    assert_eq!(farming::stage(t0, g.now(), grow), 2);
    g.clock_offset += 1_000;
    assert_eq!(farming::stage(t0, g.now(), grow), 3);
    assert!(g.before_break(1, x, y, z, SAAT_WEIZEN).unwrap());
    let got = g.inv(1, CROPS[0].product) - crop0;
    assert!(got >= CROPS[0].yield_min, "Ernte gibt Ertrag ({got})");
    assert_eq!(g.inv(1, SAAT_WEIZEN as u16), seeds + 2, "Saat kommt zurück");
    let left: i64 = g.db.query_row("select count(*) from island_crops", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);
}

#[test]
fn fremde_nein_coop_ja() {
    let mut g = game();
    let _ra = join(&mut g, 1, "Anna");
    let _rb = join(&mut g, 2, "Bert");
    let spot = field(&mut g, 1, 0);
    let (x, y, z) = spot;
    assert!(!g.can_build(2, 0, x, z), "Fremde bauen nicht");
    assert!(plant(&mut g, 2, spot).is_err(), "Fremde pflanzen nicht");
    plant(&mut g, 1, spot).unwrap();
    g.clock_offset += 600_000;
    assert!(g.before_break(2, x, y, z, SAAT_WEIZEN).is_err(), "Fremde ernten nicht");
    assert_eq!(g.block(x, y, z), SAAT_WEIZEN);

    let r = g.island_post(1, "coop", &json!({ "name": "bert", "add": true })).unwrap().unwrap();
    assert_eq!(r["partner"], json!(["Bert"]));
    assert!(g.can_build(2, 0, x, z), "Co-op baut");
    assert!(g.before_break(2, x, y, z, SAAT_WEIZEN).unwrap(), "Co-op erntet");
    assert!(g.inv(2, CROPS[0].product) >= CROPS[0].yield_min);
    plant(&mut g, 2, spot).unwrap();
    // Co-op-Pflanzen wachsen nicht mit Bauern-Faktor des Besitzers
    assert_eq!(planted(&g, spot).1, FERT_BASE);

    g.island_post(1, "coop", &json!({ "name": "Bert", "add": false })).unwrap().unwrap();
    assert!(!g.can_build(2, 0, x, z));
    assert!(g.island_post(1, "coop", &json!({ "name": "Niemand", "add": true })).unwrap().is_err());
    assert!(g.island_post(1, "coop", &json!({ "name": "Anna", "add": true })).unwrap().is_err(), "nicht sich selbst");
}

#[test]
fn likes_einmal_je_besucher() {
    let mut g = game();
    let _ra = join(&mut g, 1, "Anna");
    let _rb = join(&mut g, 2, "Bert");
    let (pa, slot) = plot_of(&g, 1);
    let r = g.island_post(2, "like", &json!({ "plot": pa, "stempel": 2 })).unwrap().unwrap();
    assert_eq!(r["neu"], true);
    assert_eq!(r["likes"], 1);
    let r = g.island_post(2, "like", &json!({ "plot": pa, "stempel": 4 })).unwrap().unwrap();
    assert_eq!(r["neu"], false, "Stempel ändern zählt nicht neu");
    assert_eq!(r["likes"], 1);
    assert!(g.island_post(1, "like", &json!({ "plot": pa })).unwrap().is_err(), "eigene Insel");
    // /like auf der Insel: wieder kein zweiter Like
    g.players.get_mut(&2).unwrap().p = world::plot_spawn(slot);
    g.like_here(2).unwrap();
    let top = g.island_get(2, "top_inseln").unwrap().unwrap();
    let a = top.as_array().unwrap().iter().find(|v| v["id"] == pa).unwrap().clone();
    assert_eq!(a["likes"], 1);
    assert_eq!(a["stempel"][0], 1, "/like setzt Stempel 0");
    assert_eq!(a["besucht"], true);
    assert_eq!(top[0]["id"], pa, "meiste Likes zuerst");
}

#[test]
fn questbuch_rueckt_vor() {
    let mut g = game();
    let _ra = join(&mut g, 1, "Anna");
    assert_eq!(g.book_state(1)["kapitel"], 1);
    let (taler, shards) = (g.taler(1), g.inv(1, sb::SHARD + 4));
    g.quest_hit(1, |q| (q == Goal::Talk(sb::DEKAN)).then_some(1));
    let st = g.book_state(1);
    assert_eq!(st["kapitel"], 2);
    assert_eq!(st["schritt"], 0);
    assert_eq!(g.taler(1), taler + CHAPTERS[0].taler as i64);
    assert_eq!(g.inv(1, sb::SHARD + 4), shards + CHAPTERS[0].shards);
    // fremde Ziele zählen nicht
    g.quest_hit(1, |q| (q == Goal::Like).then_some(1));
    assert_eq!(g.book_state(1)["schritt"], 0);
    g.quest_hit(1, |q| (q == Goal::Home).then_some(1));
    assert_eq!(g.book_state(1)["schritt"], 1);
    g.quest_hit(1, |q| matches!(q, Goal::Mine(..)).then_some(10));
    let st = g.book_state(1);
    assert_eq!((st["stand"].as_u64(), st["von"].as_u64()), (Some(10), Some(32)));
    let b = g.island_get(1, "questbuch").unwrap().unwrap();
    assert_eq!(b["kapitel"][0]["status"], "erledigt");
    assert_eq!(b["kapitel"][1]["status"], "aktiv");
    assert_eq!(b["kapitel"][2]["status"], "offen");
    assert_eq!(b["kapitel"][1]["schritte"][0]["fertig"], true);
    assert_eq!(b["kapitel"][1]["schritte"][1]["stand"], 10);
}

#[test]
fn verzaubern_kostet_und_wirkt() {
    let mut g = game();
    let _ra = join(&mut g, 1, "Anna");
    let helm = rpg::armor_item(0, 0);
    g.db.execute("insert into player_equipment (user, slot, item) values (1, 0, ?)", [helm]).unwrap();
    g.give(1, sb::SHARD + 2, 100).unwrap();
    g.give(1, sb::FOKUS_KRISTALL, 10).unwrap();
    let body = json!({ "item": helm, "ench": SCHUTZ });
    g.players.get_mut(&1).unwrap().p = [10.0, 40.0, 10.0];
    assert!(g.island_post(1, "enchant", &body).unwrap().is_err(), "nur am Altar");
    g.players.get_mut(&1).unwrap().p = [ALTAR_AT[0] as f32 + 2.5, ALTAR_AT[1] as f32, ALTAR_AT[2] as f32 + 0.5];
    assert!(g.island_post(1, "enchant", &json!({ "item": helm, "ench": en::SCHAERFE })).unwrap().is_err(), "Schärfe passt nicht auf Rüstung");

    let def0 = g.stats(1).defense;
    let (fach, s1, k1) = en::cost(SCHUTZ, 1).unwrap();
    assert_eq!(fach, 2);
    g.island_post(1, "enchant", &body).unwrap().unwrap();
    assert_eq!(g.ench(1, helm)[SCHUTZ as usize], 1);
    assert_eq!(g.inv(1, sb::SHARD + 2), 100 - s1);
    assert_eq!(g.inv(1, sb::FOKUS_KRISTALL), 10 - k1);
    assert!((g.stats(1).defense - def0 - en::effect(SCHUTZ, 1)).abs() < 1e-3, "Schutz erhöht die Verteidigung");

    // Verzaubertes Buch ersetzt die Splitter, Kristalle kosten weiter
    g.give(1, rpg::VERZ_BUCH, 1).unwrap();
    let (_, _, k2) = en::cost(SCHUTZ, 2).unwrap();
    g.island_post(1, "enchant", &body).unwrap().unwrap();
    assert_eq!(g.ench(1, helm)[SCHUTZ as usize], 2);
    assert_eq!(g.inv(1, sb::SHARD + 2), 100 - s1);
    assert_eq!(g.inv(1, rpg::VERZ_BUCH), 0);
    assert_eq!(g.inv(1, sb::FOKUS_KRISTALL), 10 - k1 - k2);
    let list = g.island_get(1, "enchants").unwrap().unwrap();
    assert!(list["gegenstaende"].as_array().unwrap().iter().any(|v| v["item"] == helm && v["stufen"][SCHUTZ as usize] == 2));
}
