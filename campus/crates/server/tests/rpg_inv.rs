//! Phase 16/20 Inventar, Truhen, Beutel, Ausrüstung, Ständer, Handel: nichts entsteht, nichts verschwindet.
use axum::body::Bytes;
use campus_server::game::{Game, User};
use campus_shared::inventory::{self as iv, Stack};
use campus_shared::{proto, rpg, world};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const A: u32 = 7001;
const B: u32 = 7002;

fn db_path() -> String {
    let p = std::env::temp_dir().join(format!("campus-inv-{}.db", std::process::id())).display().to_string();
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{p}{s}"));
    }
    p
}
fn post(g: &mut Game, uid: u32, op: &str, b: Value) -> Result<Value, String> {
    match g.inv_post(uid, op, &b) {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(e.1),
        None => Err(format!("{op} unbekannt")),
    }
}
fn rows(g: &Game, sql: &str, uid: u32) -> Vec<(u16, i64)> {
    g.db.prepare(sql).unwrap().query_map([uid], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect()
}
/// Was ein Konto besitzt: Zählung + Beutel + Ausrüstung.
fn own(g: &Game, uid: u32) -> BTreeMap<u16, i64> {
    let mut m = BTreeMap::new();
    for (i, n) in rows(g, "select item, n from inv where user = ?", uid).into_iter().chain(rows(g, "select item, n from player_stash where user = ?", uid)) {
        *m.entry(i).or_insert(0) += n;
    }
    for i in g.gear(uid).into_iter().filter(|&i| i != 0) {
        *m.entry(i).or_insert(0) += 1;
    }
    m
}
/// Alles in der Welt: beide Konten, alle Truhen (neu + alt), alle Ständer; dazu die Taler-Summe.
fn world_sum(g: &Game) -> (BTreeMap<u16, i64>, i64) {
    let mut m = BTreeMap::new();
    for u in [A, B] {
        for (k, v) in own(g, u) {
            *m.entry(k).or_insert(0) += v;
        }
    }
    let chests: Vec<Vec<u8>> = g.db.prepare("select data from island_chests").unwrap().query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
    for d in chests {
        let s: Vec<Stack> = proto::dec(&d).unwrap();
        for (k, v) in iv::totals(&s) {
            *m.entry(k).or_insert(0) += v as i64;
        }
    }
    let old: Vec<(u16, i64)> = g.db.prepare("select item, n from chest").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect();
    for (k, v) in old {
        *m.entry(k).or_insert(0) += v;
    }
    let stands: Vec<String> = g.db.prepare("select data from island_functional_blocks where kind = 120").unwrap().query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
    for d in stands {
        let s: Vec<u16> = serde_json::from_str(&d).unwrap_or_default();
        for i in s.into_iter().filter(|&i| i != 0) {
            *m.entry(i).or_insert(0) += 1;
        }
    }
    (m, g.taler(A) + g.taler(B))
}
/// Plätze == Zählung (für alles, was in Plätze gehört), keine Stapel über der Grenze.
fn synced(g: &Game, uid: u32) {
    let d: Vec<u8> = g.db.query_row("select data from inv_layout where user = ?", [uid], |r| r.get(0)).unwrap();
    let s: Vec<Stack> = proto::dec(&d).unwrap();
    assert_eq!(s.len(), iv::INV_SLOTS);
    assert!(s.iter().all(|x| x.is_empty() || x.n <= iv::stack_max(x.item)));
    let c: BTreeMap<u16, u64> = rows(g, "select item, n from inv where user = ?", uid).into_iter().filter(|r| iv::in_slots(r.0) && r.1 > 0).map(|(i, n)| (i, n as u64)).collect();
    assert_eq!(iv::totals(&s), c, "Plätze und Zählung von {uid} laufen auseinander");
}
fn slots(g: &Game, uid: u32) -> Vec<Stack> {
    let d: Vec<u8> = g.db.query_row("select data from inv_layout where user = ?", [uid], |r| r.get(0)).unwrap();
    proto::dec(&d).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nichts_entsteht_nichts_verschwindet() {
    let mut g = Game::open(&db_path()).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Bytes>(1 << 16);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    for (id, name) in [(A, "Anna"), (B, "Bert")] {
        g.join(&User { id, name: name.into(), admin: false }, tx.clone()).unwrap();
        g.pay(id, 1000, "test").unwrap();
    }
    // Truhen und Ständer auf Annas Insel, dazu eine alte Insel-Truhe, die beim ersten Öffnen übernommen wird
    let p = g.plots.iter().find(|p| p.owner == A).unwrap().clone();
    let sp = world::plot_spawn(p.slot);
    let (x0, y0, z0) = (sp[0] as i32, sp[1] as i32 + 1, sp[2] as i32);
    let small = (x0 + 2, y0, z0);
    let large = (x0 + 3, y0, z0);
    let stand = (x0 + 4, y0, z0);
    g.set_block(small.0, small.1, small.2, world::TRUHE, 0);
    g.set_block(large.0, large.1, large.2, world::GROSSE_TRUHE, 0);
    g.set_block(stand.0, stand.1, stand.2, world::STAENDER, 0);
    g.db.execute("insert into chest (plot, item, n) values (?, ?, 100), (?, ?, 3)", rusqlite::params![p.id, world::BRUCHSTEIN as u16, p.id, world::SAND as u16]).unwrap();

    // Vorrat: viele Sorten (Überlauf in den Beutel), Werkzeug, Rüstung, Talisman
    let gear: Vec<u16> = vec![rpg::armor_item(0, 0), rpg::armor_item(0, 1), rpg::armor_item(0, 2), rpg::armor_item(0, 3), rpg::armor_item(1, 0), rpg::ARTIFACT, rpg::ARTIFACT + 1];
    for u in [A, B] {
        g.give(u, world::BRUCHSTEIN as u16, 100).unwrap();
        for i in 1..=45u16 {
            g.give(u, i, 70).unwrap();
        }
        g.give(u, campus_shared::skyblock::SPITZHACKE, 2).unwrap();
        for &i in &gear {
            g.give(u, i, 1).unwrap();
        }
    }
    g.flush_notes();
    assert!(rows(&g, "select item, n from player_stash where user = ?", A).iter().map(|r| r.1).sum::<i64>() > 0, "Überlauf landet im Beutel");
    for u in [A, B] {
        synced(&g, u);
    }
    let start = world_sum(&g);

    // erstes Öffnen übernimmt die alte Truhe
    g.chest_open_new(A, small.0, small.1, small.2).unwrap();
    let alt: i64 = g.db.query_row("select count(*) from chest where plot = ?", [p.id], |r| r.get(0)).unwrap();
    assert_eq!(alt, 0, "alte Truhe übernommen");
    assert_eq!(world_sum(&g), start);
    assert!(g.chest_can_break(A, small.0, small.1, small.2).is_err(), "volle Truhe lässt sich nicht abbauen");
    assert!(g.chest_open_new(B, small.0, small.1, small.2).is_err(), "fremde Truhe ist gesperrt");

    // gezielter Handel: Sperre, Bereit-Rücksetzen, Ausführung
    post(&mut g, A, "trade/request", json!({ "name": "bert" })).unwrap();
    post(&mut g, B, "trade/accept", json!({ "von": A })).unwrap();
    let (a_stein, b_stein) = (g.inv(A, world::BRUCHSTEIN as u16), g.inv(B, world::BRUCHSTEIN as u16));
    let (ta, tb) = (g.taler(A), g.taler(B));
    post(&mut g, A, "trade/offer", json!({ "items": [[world::BRUCHSTEIN, 10]], "taler": 50 })).unwrap();
    post(&mut g, B, "trade/offer", json!({ "items": [], "taler": 0 })).unwrap();
    post(&mut g, A, "trade/ready", json!({ "an": true })).unwrap();
    post(&mut g, B, "trade/ready", json!({ "an": true })).unwrap();
    assert!(post(&mut g, A, "trade/confirm", json!({})).is_err(), "3-s-Sperre");
    g.clock_offset += 3001;
    post(&mut g, A, "trade/confirm", json!({})).unwrap();
    assert_eq!(g.inv(A, world::BRUCHSTEIN as u16), a_stein - 10);
    assert!(g.inv(B, world::BRUCHSTEIN as u16) <= b_stein + 10);
    assert_eq!((g.taler(A), g.taler(B)), (ta - 50, tb + 50));
    assert_eq!(world_sum(&g), start);

    let mut r = 0x2545_f491_u64;
    let mut rnd = move |n: u64| {
        r ^= r << 13;
        r ^= r >> 7;
        r ^= r << 17;
        r % n
    };
    let chest = |c: (i32, i32, i32), i: u64| json!({ "art": "truhe", "i": i, "x": c.0, "y": c.1, "z": c.2 });
    let inv = |i: u64| json!({ "art": "inv", "i": i });
    for case in 0..100 {
        let u = if rnd(4) == 0 { B } else { A }; // Bert darf Annas Truhen nicht benutzen
        let c = if rnd(2) == 0 { small } else { large };
        let cn = if c == small { 27 } else { 54 };
        let before = world_sum(&g);
        let (own_a, own_b) = (own(&g, A), own(&g, B));
        let mut abort = false;
        let _ = match rnd(12) {
            0 => post(&mut g, u, "inv/move", json!({ "von": inv(rnd(36)), "nach": inv(rnd(36)), "n": rnd(70) })),
            1 => post(&mut g, u, "inv/move", json!({ "von": inv(rnd(36)), "nach": chest(c, rnd(cn)), "n": rnd(70) })),
            2 => post(&mut g, u, "inv/move", json!({ "von": chest(c, rnd(cn)), "nach": inv(rnd(36)), "n": rnd(3) * 20 })),
            3 => {
                let ziel = if rnd(2) == 0 { json!({ "x": c.0, "y": c.1, "z": c.2 }) } else { Value::Null };
                post(&mut g, u, "inv/move", json!({ "von": inv(rnd(36)), "nach": null, "ziel": ziel }))
            }
            4 => post(&mut g, u, "inv/sort", if rnd(2) == 0 { json!({ "truhe": { "x": c.0, "y": c.1, "z": c.2 } }) } else { json!({}) }),
            5 => post(&mut g, u, "truhe/einlagern", json!({ "x": c.0, "y": c.1, "z": c.2 })),
            6 => post(&mut g, u, "stash/claim", json!({})),
            7 | 8 => {
                // Ausrüstung oder Ständer: Teil aus dem Inventar anlegen, ablegen oder schnell zurück
                let s = slots(&g, u);
                let mut to = if rnd(2) == 0 { json!({ "art": "gear", "i": 0 }) } else { json!({ "art": "staender", "i": 0, "x": stand.0, "y": stand.1, "z": stand.2 }) };
                match (rnd(3), s.iter().position(|x| !x.is_empty() && rpg::equip(x.item).is_some())) {
                    (0, Some(i)) => {
                        let e = rpg::equip(s[i].item).unwrap();
                        to["i"] = json!(if e.slot < 4 { e.slot as u64 } else { 4 + rnd(3) });
                        post(&mut g, u, "inv/move", json!({ "von": inv(i as u64), "nach": to, "n": 0 }))
                    }
                    (1, _) => {
                        to["i"] = json!(rnd(7));
                        post(&mut g, u, "inv/move", json!({ "von": to, "nach": if rnd(2) == 0 { inv(rnd(36)) } else { Value::Null }, "n": 0 }))
                    }
                    (_, Some(i)) => post(&mut g, u, "inv/move", json!({ "von": inv(i as u64), "nach": null, "ziel": to })),
                    _ => Ok(Value::Null),
                }
            }
            9 => {
                let item = [world::BRUCHSTEIN as u16, world::SAND as u16, 7, campus_shared::skyblock::SPITZHACKE][rnd(4) as usize];
                let n = rnd(150) as i64 - 75;
                post(&mut g, u, "truhe", json!({ "x": c.0, "y": c.1, "z": c.2, "item": item, "n": n }))
            }
            k => {
                // Handel; bei k == 11 Abbruch mitten drin (per Knopf oder Trennung) – dann bewegt sich nichts
                abort = k == 11;
                let _ = post(&mut g, A, "trade/request", json!({ "name": "Bert" }));
                let _ = post(&mut g, B, "trade/accept", json!({ "von": A }));
                let it = |n: u64| json!([1 + n % 45, 1 + n % 90]);
                let _ = post(&mut g, A, "trade/offer", json!({ "items": [it(rnd(1000)), it(rnd(1000))], "taler": rnd(30) }));
                let _ = post(&mut g, B, "trade/offer", json!({ "items": [it(rnd(1000))], "taler": rnd(30) }));
                let _ = post(&mut g, A, "trade/ready", json!({ "an": true }));
                let _ = post(&mut g, B, "trade/ready", json!({ "an": true }));
                g.clock_offset += 3001;
                if abort {
                    if rnd(2) == 0 {
                        let _ = post(&mut g, B, "trade/cancel", json!({}));
                    } else {
                        g.trade_abort(A, "Die andere Person hat den Campus verlassen.");
                    }
                    assert!(post(&mut g, A, "trade/confirm", json!({})).is_err(), "nach Abbruch kein Abschluss");
                    Ok(Value::Null)
                } else {
                    // Änderung nach „bereit“ setzt beide zurück
                    let _ = post(&mut g, B, "trade/offer", json!({ "items": [], "taler": 1 }));
                    assert!(post(&mut g, A, "trade/confirm", json!({})).is_err(), "Änderung setzt bereit zurück");
                    let _ = post(&mut g, A, "trade/ready", json!({ "an": true }));
                    let _ = post(&mut g, B, "trade/ready", json!({ "an": true }));
                    g.clock_offset += 3001;
                    let r = post(&mut g, B, "trade/confirm", json!({}));
                    let _ = post(&mut g, A, "trade/cancel", json!({}));
                    r
                }
            }
        };
        g.flush_notes();
        assert_eq!(world_sum(&g), before, "Fall {case}: Summe von Gegenständen/Talern geändert");
        if abort {
            assert_eq!((own(&g, A), own(&g, B)), (own_a, own_b), "Fall {case}: Abbruch hat etwas bewegt");
        }
        for u in [A, B] {
            synced(&g, u);
        }
    }
    assert_eq!(world_sum(&g), start);

    // Beutel ganz abholen, so weit Platz ist; Truhe ausräumen und dann abbauen
    let _ = post(&mut g, A, "stash/claim", json!({}));
    post(&mut g, A, "truhe/leeren", json!({ "x": small.0, "y": small.1, "z": small.2 })).unwrap();
    assert!(post(&mut g, B, "truhe/leeren", json!({ "x": large.0, "y": large.1, "z": large.2 })).is_err());
    assert!(g.chest_can_break(A, small.0, small.1, small.2).is_ok(), "leere Truhe darf weg");
    assert_eq!(world_sum(&g), start);
    let v = g.inv_get(A, "inv").unwrap().unwrap();
    assert_eq!(v["slots"].as_array().unwrap().len(), 36);
    assert_eq!(v["gear"].as_array().unwrap().len(), 7);
}
