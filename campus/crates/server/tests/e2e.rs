//! Abnahme-Tests je Bauabschnitt: echter Dienst auf freiem Port, Lernhelfer als Attrappe, echte WebSockets.
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use campus_server::game::Game;
use campus_server::{start, App, Config};
use campus_shared::economy as eco;
use campus_shared::items;
use campus_shared::proto::{self, Warp, C2S, S2C};
use campus_shared::world::{self, GROUND, MID};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

#[derive(Default)]
struct Lh {
    ich: Mutex<HashMap<String, Value>>,
    verdienst: Mutex<HashMap<u32, Value>>,
}

async fn mock_lh() -> (Arc<Lh>, String) {
    let lh = Arc::new(Lh::default());
    let r = Router::new()
        .route(
            "/api/ich",
            get(|State(l): State<Arc<Lh>>, h: HeaderMap| async move {
                let c = h.get("cookie").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
                match c.strip_prefix("lh=").and_then(|t| l.ich.lock().unwrap().get(t).cloned()) {
                    Some(v) => (StatusCode::OK, Json(v)),
                    None => (StatusCode::UNAUTHORIZED, Json(json!({}))),
                }
            }),
        )
        .route(
            "/api/campus/verdienst",
            get(|State(l): State<Arc<Lh>>, Query(q): Query<HashMap<String, String>>| async move {
                let (konto, seit): (u32, i64) = (q["konto"].parse().unwrap(), q["seit"].parse().unwrap());
                let mut v = l.verdienst.lock().unwrap().get(&konto).cloned().unwrap_or(json!({ "ereignisse": [] }));
                let ev: Vec<Value> = v["ereignisse"].as_array().unwrap().iter().filter(|e| e["id"].as_i64().unwrap() > seit).cloned().collect();
                v["ereignisse"] = json!(ev);
                Json(v)
            }),
        )
        .with_state(lh.clone());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap().to_string();
    tokio::spawn(async move { axum::serve(l, r).await.unwrap() });
    (lh, addr)
}

static N: AtomicU32 = AtomicU32::new(0);
fn db_path() -> String {
    let p = std::env::temp_dir().join(format!("campus-test-{}-{}.db", std::process::id(), N.fetch_add(1, Ordering::SeqCst))).display().to_string();
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{p}{s}"));
    }
    p
}

async fn http(addr: &str, method: &str, path: &str, kopf: &str, body: Option<&Value>) -> (u16, Value) {
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    let b = body.map(|b| b.to_string()).unwrap_or_default();
    s.write_all(format!("{method} {path} HTTP/1.0\r\nHost: {addr}\r\n{kopf}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{b}", b.len()).as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf).to_string();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    (head.split(' ').nth(1).unwrap().parse().unwrap(), serde_json::from_str(body).unwrap_or(Value::String(body.to_string())))
}

struct T {
    app: App,
    addr: String,
    lh: Arc<Lh>,
    db: String,
}

async fn boot() -> T {
    let (lh, lh_addr) = mock_lh().await;
    let db = db_path();
    let (app, addr) = start(Config { port: 0, db: db.clone(), web: concat!(env!("CARGO_MANIFEST_DIR"), "/../../web").into(), lh: lh_addr }).await.unwrap();
    T { app, addr: addr.to_string(), lh, db }
}

impl T {
    /// Konto im Attrappen-Lernhelfer anlegen und im Campus einmal anmelden.
    async fn user(&self, id: u32, name: &str, admin: bool) -> String {
        let tok = format!("tok{id}");
        self.lh.ich.lock().unwrap().insert(tok.clone(), json!({ "id": id, "spielname": name, "admin": admin, "email": "x@example.com", "api": "sk-geheim" }));
        assert_eq!(self.get(&tok, "me").await.0, 200);
        tok
    }
    async fn get(&self, tok: &str, op: &str) -> (u16, Value) {
        http(&self.addr, "GET", &format!("/campus/api/{op}"), &format!("Cookie: lh={tok}\r\n"), None).await
    }
    async fn post(&self, tok: &str, op: &str, body: Value) -> (u16, Value) {
        http(&self.addr, "POST", &format!("/campus/api/{op}"), &format!("Cookie: lh={tok}\r\n"), Some(&body)).await
    }
    fn g(&self) -> MutexGuard<'_, Game> {
        self.app.game.lock().unwrap()
    }
    // Jede Abfrage sperrt nur kurz: nie zwei `g()` in einem Ausdruck (die Sperre ist nicht wiedereintrittsfähig).
    fn taler(&self, id: u32) -> i64 {
        self.g().taler(id)
    }
    fn inv(&self, id: u32, item: u16) -> u32 {
        self.g().inv(id, item)
    }
    fn owns(&self, id: u32, item: u16) -> bool {
        self.g().owns(id, item)
    }
    fn can_build(&self, id: u32, group: u32, x: i32, z: i32) -> bool {
        self.g().can_build(id, group, x, z)
    }
    fn verdienst(&self, id: u32, v: Value) {
        self.lh.verdienst.lock().unwrap().insert(id, v);
    }
    /// Summe aller Taler im Spiel: Konten, Gruppenkassen und reservierte Gebote.
    fn supply(&self) -> i64 {
        let g = self.g();
        let q = |sql: &str| -> i64 { g.db.query_row(sql, [], |r| r.get(0)).unwrap() };
        q("select coalesce(sum(taler), 0) from users") + q("select coalesce(sum(treasury), 0) from groups") + q("select coalesce(sum(bid), 0) from auctions where state = 0")
    }
    /// Spieler an einen Ort stellen und warten, bis er das Stück dort geladen hat.
    async fn place(&self, id: u32, p: [f32; 3]) {
        self.g().players.get_mut(&id).unwrap().p = p;
        for _ in 0..200 {
            if self.g().has_chunk(id, p[0] as i32, p[2] as i32) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("Stück kam nicht an");
    }
    /// Bauplatz freiräumen (Bäume der Gemeinschaftsfläche stehen sonst zufällig im Weg).
    fn clear(&self, x0: i32, z0: i32, x1: i32, z1: i32) {
        let mut g = self.g();
        for z in z0..=z1 {
            for x in x0..=x1 {
                for y in BY..BY + 12 {
                    if g.block(x, y, z) != 0 {
                        g.set_block(x, y, z, 0, 0);
                    }
                }
            }
        }
    }
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
struct Bot {
    ws: Ws,
    id: u32,
}
impl Bot {
    async fn connect(addr: &str, tok: &str) -> Bot {
        let mut r = format!("ws://{addr}/campus/ws").into_client_request().unwrap();
        r.headers_mut().insert("Cookie", format!("lh={tok}").parse().unwrap());
        let (ws, _) = tokio_tungstenite::connect_async(r).await.unwrap();
        let mut b = Bot { ws, id: 0 };
        b.id = b.wait(|m| if let S2C::Welcome { id, .. } = m { Some(*id) } else { None }).await;
        b.wait(|m| matches!(m, S2C::Spawn { .. }).then_some(())).await; // Ankunft folgt immer gleich danach
        b
    }
    async fn send(&mut self, m: C2S) {
        self.ws.send(Message::Binary(proto::enc(&m).into())).await.unwrap();
    }
    /// Liest, bis `f` etwas findet (höchstens 10 s insgesamt).
    async fn wait<R>(&mut self, mut f: impl FnMut(&S2C) -> Option<R>) -> R {
        let ende = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let msg = tokio::time::timeout_at(ende, self.ws.next()).await.expect("Zeit um").expect("Verbindung zu").unwrap();
            if let Message::Binary(b) = msg {
                if let Some(r) = proto::dec::<S2C>(&b).and_then(|m| f(&m)) {
                    return r;
                }
            }
        }
    }
    async fn block(&mut self, x: i32, y: i32, z: i32, b: u8) -> (u8, u32) {
        tokio::time::sleep(Duration::from_millis(110)).await; // Abklingzeit zwischen zwei Blöcken
        if b == 0 {
            // Abbauen dauert: erst Dig, dann so lange halten wie world::mine_secs verlangt (Stein von Hand 1,1 s → 620 ms genügen)
            self.send(C2S::Dig { x: x as i16, y: y as i16, z: z as i16 }).await;
            tokio::time::sleep(Duration::from_millis(700)).await;
        }
        self.send(C2S::Block { x: x as i16, y: y as i16, z: z as i16, b }).await;
        self.wait(|m| match m {
            S2C::Block { x: bx, y: by_, z: bz, b, by } if (*bx as i32, *by_ as i32, *bz as i32) == (x, y, z) => Some((*b, *by)),
            _ => None,
        })
        .await
    }
}

// Bauplatz: Mitte des Grundstücks auf der ersten Privatinsel (Platz (5, 0)), die das erste Konto bekommt.
// Im Hub-Archipel darf niemand bauen.
const BX: i32 = 5 * world::SLOT + 48;
const BZ: i32 = 48;
const BY: i32 = GROUND + 1;
fn stand(dx: i32, dz: i32) -> [f32; 3] {
    [(BX + dx) as f32 + 0.5, BY as f32, (BZ + dz) as f32 + 0.5]
}

// ---------- Abschnitt 0: Anmeldung über den Lernhelfer ----------
#[tokio::test]
async fn a0_anmeldung_und_seiten() {
    let t = boot().await;
    assert_eq!(http(&t.addr, "GET", "/campus/api/me", "", None).await.0, 401, "ohne Sitzung kein Zugang");
    assert_eq!(t.get("falsch", "me").await.0, 401);
    let tok = t.user(7, "Anna", false).await;
    let (_, me) = t.get(&tok, "me").await;
    assert_eq!((me["id"].as_u64(), me["name"].as_str(), me["admin"].as_bool()), (Some(7), Some("Anna"), Some(false)));
    assert_eq!(me["taler"], 0);
    assert!(!me.to_string().contains("geheim"), "nichts aus /api/ich wird durchgereicht");
    assert_eq!(me["inv"], json!([[world::BRETTER, 48], [world::ZIEGEL, 24], [world::GLAS, 8]]), "Starter-Vorrat");
    let (c, reg) = http(&t.addr, "GET", "/campus/api/registry", "", None).await;
    assert!(c == 200 && reg["bloecke"].as_array().unwrap().len() == world::BLOCKS.len() && reg["cosmetics"].as_array().unwrap().len() == items::COSMETICS.len());
    let (c, page) = http(&t.addr, "GET", "/campus/", "", None).await;
    assert!(c == 200 && page.as_str().unwrap().contains("Campus"));
    assert_eq!(http(&t.addr, "GET", "/campus/hilfe", "", None).await.0, 200);
    assert_eq!(http(&t.addr, "GET", "/campus/admin", &format!("Cookie: lh={tok}\r\n"), None).await.0, 403);
    let chef = t.user(1, "Chef", true).await;
    assert_eq!(http(&t.addr, "GET", "/campus/admin", &format!("Cookie: lh={chef}\r\n"), None).await.0, 200);
    for bad in ["/campus/..%2fCargo.toml", "/campus/../Cargo.toml", "/campus/geheim.db", "/internal/campus/export/7x"] {
        assert_ne!(http(&t.addr, "GET", bad, "", None).await.0, 200, "{bad}");
    }
    assert_eq!(http(&t.addr, "POST", "/campus/api/fund/open", &format!("Cookie: lh={tok}\r\nOrigin: https://boese.example\r\n"), None).await.0, 403, "fremde Seite");
    assert_eq!(http(&t.addr, "GET", "/internal/campus/export/7", "X-Forwarded-For: 203.0.113.5\r\n", None).await.0, 404, "intern nur ohne Caddy");
}

// ---------- Abschnitt 1: Welt allein, Speichern über einen Neustart ----------
#[tokio::test]
async fn a1_bauen_und_neustart() {
    let t = boot().await;
    let tok = t.user(7, "Anna", false).await;
    let mut a = Bot::connect(&t.addr, &tok).await;
    let rle = a.wait(|m| if let S2C::Chunk { rle, .. } = m { Some(rle.clone()) } else { None }).await;
    let mut c = vec![0u8; world::VOL];
    assert!(world::unrle(&rle, &mut c), "Stücke kommen gepackt an");
    t.clear(BX - 3, BZ - 4, BX + 3, BZ + 6);
    t.place(7, stand(0, 0)).await;
    assert_eq!(a.block(BX + 1, BY, BZ, world::ZIEGEL).await, (world::ZIEGEL, 7));
    assert_eq!(a.wait(|m| if let S2C::Inv { item, n } = m { Some((*item, *n)) } else { None }).await, (world::ZIEGEL as u16, 23));
    assert_eq!(a.block(BX + 1, BY, BZ, world::ZIEGEL).await.0, world::ZIEGEL, "besetzt: bleibt, wie es ist");
    assert_eq!(a.block(BX + 1, BY, BZ + 1, world::MARMOR).await.0, 0, "ohne Vorrat kein Block");
    tokio::time::sleep(Duration::from_millis(80)).await;
    let ground = t.g().block(BX, GROUND, BZ + 1);
    assert_eq!(a.block(BX, GROUND, BZ + 1, 0).await, (0, 7), "Boden abbauen");
    assert_eq!(t.inv(7, ground as u16), 1, "abgebauter Block landet im Vorrat");
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert_eq!(a.block(BX + 30, BY + 20, BZ, world::ZIEGEL).await.0, 0, "außer Reichweite");
    tokio::time::sleep(Duration::from_millis(80)).await;
    let unten = t.g().block(BX, 0, BZ);
    assert_eq!(a.block(BX, 0, BZ, world::BRETTER).await.0, unten, "unterste Schicht ist tabu");
    assert!(t.g().save().unwrap() >= 1);
    assert_eq!(t.g().save().unwrap(), 0, "nur geänderte Stücke werden geschrieben");
    // „Neustart“: dieselbe Datenbank frisch öffnen
    let g2 = Game::open(&t.db).unwrap();
    assert_eq!(g2.block(BX + 1, BY, BZ), world::ZIEGEL);
    assert_eq!(g2.block(BX, GROUND, BZ + 1), 0);
    assert_eq!(g2.inv(7, world::ZIEGEL as u16), 23);
    assert_eq!(g2.db.query_row("pragma journal_mode", [], |r| r.get::<_, String>(0)).unwrap(), "wal");
}

// ---------- Abschnitt 2: zwei Konten bauen gleichzeitig, Grundstücke, Sprüche ----------
#[tokio::test]
async fn a2_zwei_konten_bauen_gleichzeitig() {
    let t = boot().await;
    let (ta, tb) = (t.user(7, "Anna", false).await, t.user(8, "Ben", false).await);
    let mut a = Bot::connect(&t.addr, &ta).await;
    let mut b = Bot::connect(&t.addr, &tb).await;
    assert_eq!(a.wait(|m| if let S2C::Join { id: 8, name, .. } = m { Some(name.clone()) } else { None }).await, "Ben");
    assert_eq!(t.post(&ta, "plot/acl", json!({ "name": "Ben" })).await.0, 200, "Ben baut auf Annas Insel mit");
    t.clear(BX - 3, BZ - 4, BX + 3, BZ + 6);
    t.place(7, stand(0, 0)).await;
    t.place(8, stand(0, 2)).await;
    t.g().give(7, world::BRETTER as u16, 100).unwrap();
    t.g().give(8, world::BRETTER as u16, 100).unwrap();
    // beide setzen abwechselnd je 30 Blöcke, ohne aufeinander zu warten
    for i in 0..30 {
        a.send(C2S::Block { x: (BX + 1) as i16, y: (BY + i % 5) as i16, z: (BZ - 3 + i / 5) as i16, b: world::BRETTER }).await;
        b.send(C2S::Block { x: (BX - 1) as i16, y: (BY + i % 5) as i16, z: (BZ - 1 + i / 5) as i16, b: world::BRETTER }).await;
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    // jeder sieht alle 60 Änderungen
    for bot in [&mut a, &mut b] {
        let mut n = 0;
        bot.wait(|m| {
            if let S2C::Block { b, by, .. } = m {
                n += (*b == world::BRETTER && *by != 0) as u32;
            }
            (n == 60).then_some(())
        })
        .await;
    }
    for i in 0..30 {
        assert_eq!(t.g().block(BX + 1, BY + i % 5, BZ - 3 + i / 5), world::BRETTER, "Anna {i}");
        assert_eq!(t.g().block(BX - 1, BY + i % 5, BZ - 1 + i / 5), world::BRETTER, "Ben {i}");
    }
    assert_eq!((t.inv(7, world::BRETTER as u16), t.inv(8, world::BRETTER as u16)), (118, 118), "genau 30 je Konto verbraucht");
    // dieselbe Stelle im selben Augenblick: genau einer gewinnt, keiner verliert einen Block
    a.send(C2S::Block { x: BX as i16, y: (BY + 6) as i16, z: (BZ + 1) as i16, b: world::BRETTER }).await;
    b.send(C2S::Block { x: BX as i16, y: (BY + 6) as i16, z: (BZ + 1) as i16, b: world::BRETTER }).await;
    let who = a.wait(|m| if let S2C::Block { y, by, .. } = m { (*y as i32 == BY + 6 && *by != 0).then_some(*by) } else { None }).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(t.inv(7, world::BRETTER as u16) + t.inv(8, world::BRETTER as u16), 235);
    assert_eq!(t.inv(who, world::BRETTER as u16), 117);
    // Grundstück: nur Besitzer und eingetragene Mitbauer
    let (pa, pb) = { let g = t.g(); (g.plots.iter().find(|p| p.owner == 7).unwrap().clone(), g.plots.iter().find(|p| p.owner == 8).unwrap().clone()) };
    assert_eq!((pa.size, pb.size), (16, 16));
    assert_ne!(pa.slot, pb.slot);
    let rb = world::plot_rect(pb.slot, pb.size);
    let (px, pz) = (rb.0 + 4, rb.1 + 4);
    t.place(7, [px as f32 + 0.5, BY as f32, pz as f32 + 1.5]).await;
    t.place(8, [px as f32 + 0.5, BY as f32, pz as f32 + 2.5]).await;
    assert_eq!(a.block(px, BY, pz, world::BRETTER).await.0, 0, "fremdes Grundstück");
    assert_eq!(b.block(px, BY, pz, world::BRETTER).await, (world::BRETTER, 8));
    assert_eq!(a.block(px, BY, pz, 0).await.0, world::BRETTER, "auch nicht abbauen");
    assert_eq!(a.block(rb.0 - 2, BY, pz, world::BRETTER).await.0, 0, "außerhalb jedes Grundstücks");
    assert_eq!(t.post(&tb, "plot/acl", json!({ "name": "anna" })).await.0, 200);
    a.wait(|m| if let S2C::Plots(v) = m { v.iter().any(|p| p.id == pb.id && p.may).then_some(()) } else { None }).await; // Anna erfährt, dass sie darf
    assert_eq!(a.block(px + 1, BY, pz, world::BRETTER).await, (world::BRETTER, 7), "als Mitbauerin erlaubt");
    assert_eq!(t.post(&tb, "plot/acl", json!({ "name": "Anna", "add": false })).await.0, 200);
    assert_eq!(a.block(px + 2, BY, pz, world::BRETTER).await.0, 0);
    assert_eq!(t.post(&tb, "plot/acl", json!({ "name": "Niemand" })).await.0, 404);
    // Sprüche und Positionen
    a.send(C2S::Chat(2)).await;
    assert_eq!(b.wait(|m| if let S2C::Chat { id, n } = m { Some((*id, *n)) } else { None }).await, (7, 2));
    a.send(C2S::Chat(200)).await;
    a.send(C2S::Move { p: [px as f32 + 1.0, BY as f32, pz as f32 + 1.5], yaw: 1.0, anim: 1 }).await;
    let st = b.wait(|m| if let S2C::Players(v) = m { v.iter().find(|p| p.id == 7 && p.anim == 1).copied() } else { None }).await;
    assert_eq!(st.p[0], px as f32 + 1.0);
    a.send(C2S::Move { p: [5.0, BY as f32, 5.0], yaw: 0.0, anim: 0 }).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(t.g().players[&7].p[0], px as f32 + 1.0, "Teleport wird verworfen");
    // nichts geht verloren: speichern, neu öffnen, alles da
    t.g().save().unwrap();
    let g2 = Game::open(&t.db).unwrap();
    for i in 0..30 {
        assert_eq!(g2.block(BX + 1, BY + i % 5, BZ - 3 + i / 5), world::BRETTER);
        assert_eq!(g2.block(BX - 1, BY + i % 5, BZ - 1 + i / 5), world::BRETTER);
    }
    assert_eq!(g2.block(px, BY, pz), world::BRETTER);
    assert_eq!(g2.plots.len(), 2);
    // zweite Anmeldung desselben Kontos ersetzt die erste
    let _a2 = Bot::connect(&t.addr, &ta).await;
    assert_eq!(t.g().players.len(), 2);
}

// ---------- Abschnitt 2: Lasttest mit 50 Spielern ----------
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a2_last_50_spieler() {
    let t = boot().await;
    let mut toks = Vec::new();
    for i in 0..50u32 {
        toks.push(t.user(100 + i, &format!("Bot {i}"), false).await);
    }
    // jeder baut auf seinem eigenen Grundstück (im Hub darf niemand bauen)
    let mut tasks = Vec::new();
    for (i, tok) in toks.into_iter().enumerate() {
        let addr = t.addr.clone();
        let app = t.app.clone();
        tasks.push(tokio::spawn(async move {
            let bot = Bot::connect(&addr, &tok).await;
            let id = bot.id;
            let (hx, hz) = {
                let mut g = app.game.lock().unwrap();
                let r = g.plots.iter().find(|p| p.owner == id).map(|p| world::plot_rect(p.slot, p.size)).unwrap();
                let (hx, hz) = (r.0 + 4, r.1 + 6);
                for x in hx..=hx + 2 {
                    for y in BY..BY + 8 {
                        if g.block(x, y, hz) != 0 {
                            g.set_block(x, y, hz, 0, 0);
                        }
                    }
                }
                g.players.get_mut(&id).unwrap().p = [hx as f32 + 0.5, BY as f32, hz as f32 + 0.5];
                (hx, hz)
            };
            while !app.game.lock().unwrap().has_chunk(id, hx + 2, hz) || !app.game.lock().unwrap().has_chunk(id, hx, hz) {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let (mut tx, mut rx) = bot.ws.split();
            let ende = tokio::time::Instant::now() + Duration::from_secs(8);
            let reader = tokio::spawn(async move {
                let (mut got, mut mine) = (0u32, 0u32);
                while let Ok(Some(Ok(m))) = tokio::time::timeout_at(ende, rx.next()).await {
                    if let Message::Binary(b) = m {
                        got += 1;
                        if let Some(S2C::Block { b, by, .. }) = proto::dec::<S2C>(&b) {
                            mine += (by == id && b != 0) as u32;
                        }
                    }
                }
                (got, mine)
            });
            // 6 Sekunden: 10-mal je Sekunde bewegen, zweimal je Sekunde bauen (12 Blöcke, zwei Säulen)
            for k in 0..60 {
                let w = (k as f32 * 0.3 + i as f32).sin() * 0.4;
                tx.send(Message::Binary(proto::enc(&C2S::Move { p: [hx as f32 + 0.5 + w, BY as f32, hz as f32 + 0.5 + w], yaw: w, anim: 1 }).into())).await.unwrap();
                if k % 5 == 0 {
                    let j = k / 5;
                    tx.send(Message::Binary(proto::enc(&C2S::Block { x: (hx + 1 + j % 2) as i16, y: (BY + j / 2) as i16, z: hz as i16, b: world::BRETTER }).into())).await.unwrap();
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let (got, mine) = reader.await.unwrap();
            (hx, hz, got, mine)
        }));
    }
    let mut total = 0;
    for task in tasks {
        let (hx, hz, got, mine) = task.await.unwrap();
        assert_eq!(mine, 12, "alle 12 Blöcke bestätigt");
        assert!(got > 100, "Positionen und Stücke kommen an: {got}");
        for j in 0..12 {
            assert_eq!(t.g().block(hx + 1 + j % 2, BY + j / 2, hz), world::BRETTER, "kein Block verloren");
        }
        total += got;
    }
    let s = t.g().stats;
    let rss_mb = std::fs::read_to_string("/proc/self/status").unwrap().lines().find(|l| l.starts_with("VmRSS")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap() / 1024;
    println!("LAST 50 Spieler: Takt max {:.2} ms, Mittel {:.3} ms, {} Takte, {} Nachrichten, Speicher {} MB", s.max_us as f64 / 1000.0, s.avg_us / 1000.0, s.ticks, total, rss_mb);
    assert!(s.ticks > 50);
    assert!(s.max_us < 50_000, "Takt unter 50 ms: {} µs", s.max_us);
    assert!(rss_mb < 400, "Speicher im Rahmen: {rss_mb} MB");
}

// ---------- Abschnitt 3: Taler aus dem Lernhelfer, Laden, Grundstück ----------
#[tokio::test]
async fn a3_taler_auf_den_taler_genau() {
    let t = boot().await;
    let tok = t.user(7, "Anna", false).await;
    let _bot = Bot::connect(&t.addr, &tok).await;
    let ev = |id: i64, art: &str, menge: u32, tag: &str, serie: u32, pin: Value, note: Value| json!({ "id": id, "art": art, "menge": menge, "fach": "VWL", "tag": tag, "serie": serie, "pruefung_in": pin, "note": note });
    let n = Value::Null;
    t.verdienst(7, json!({ "xp": 2025, "serie": 3, "bester": 30, "checks": { "VWL": 100, "BWL": 99 }, "ereignisse": [
        ev(1, "fokus", 200, "2026-11-12", 0, n.clone(), n.clone()),      // 200
        ev(2, "fokus", 100, "2026-11-12", 0, n.clone(), n.clone()),      // nur noch 40 bis zur Tagesgrenze 240
        ev(3, "check", 1, "2026-11-12", 3, n.clone(), n.clone()),        // 15 × 1,15 = 17
        ev(4, "check", 1, "2026-11-12", 10, json!(14), n.clone()),       // 15 × 1,5 × 2 = 45
        ev(5, "karte", 59, "2026-11-12", 0, n.clone(), n.clone()),       // 118
        ev(6, "karte", 5, "2026-11-12", 0, n.clone(), n.clone()),        // nur noch 1 Karte: 2
        ev(7, "klausur", 1, "2026-11-12", 0, n.clone(), json!(1)),       // 100, Bauplan, Goldrahmen
        ev(8, "challenge", 1, "2026-11-12", 0, n.clone(), n.clone()),    // 25
        ev(9, "fokus", 30, "2026-11-13", 0, n.clone(), n.clone()),       // neuer Tag: 30
        ev(10, "unbekannt", 99, "2026-11-13", 0, n.clone(), n.clone()),  // zählt nicht
        ev(11, "fokus", 99, "kein-tag", 0, n.clone(), n.clone()),        // zählt nicht
    ] }));
    let want = 200 + 40 + 17 + 45 + 118 + 2 + 100 + 25 + 30;
    assert_eq!(want, 577);
    let (c, vor) = t.get(&tok, "zahltag").await;
    assert_eq!((c, vor["taler"].as_i64(), vor["funde"].as_i64()), (200, Some(577), Some(2)), "Zahltag-Hinweis rechnet vor");
    assert_eq!(t.taler(7), 0, "Vorrechnen schreibt nichts gut");
    let (c, z) = t.post(&tok, "zahltag", json!({})).await;
    assert_eq!((c, z["taler"].as_i64(), z["funde"].as_i64(), z["bauplaene"].as_i64(), z["stand"].as_i64()), (200, Some(577), Some(2), Some(1), Some(577)));
    let neu: Vec<&str> = z["neu"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    for c in [items::PLATE_GOLD, items::CAPE_L5, items::CAPE_L10, items::TRAIL_FLAMME_1, items::TRAIL_FLAMME_2, eco::subject_cape("VWL")] {
        assert!(neu.contains(&items::item_name(c)) && t.inv(7, c) == 1, "{}", items::item_name(c));
    }
    assert_eq!(neu.len(), 6, "kein Umhang für 99 Check-Fragen in BWL: {neu:?}");
    assert_eq!(t.post(&tok, "zahltag", json!({})).await.1["taler"], 0, "zweimal abholen bringt nichts");
    assert_eq!(t.taler(7), 577);
    let (_, me) = t.get(&tok, "me").await;
    assert_eq!((me["taler"].as_i64(), me["funde"].as_i64(), me["stufe"].as_i64(), me["serie"].as_i64()), (Some(577), Some(2), Some(10), Some(3)));
    assert_eq!(t.inv(7, items::BLUEPRINT), 1);
    // später kommt Neues dazu; die Tagesgrenze von gestern gilt weiter
    t.verdienst(7, json!({ "xp": 2025, "serie": 3, "bester": 30, "ereignisse": [ev(12, "fokus", 50, "2026-11-12", 0, n.clone(), n.clone()), ev(13, "karte", 3, "2026-11-13", 2, n.clone(), n.clone())] }));
    assert_eq!(t.post(&tok, "zahltag", json!({})).await.1["taler"], 6, "Fokus gedeckelt, 3 Karten × 2 × 1,1 = 6");
    assert_eq!(t.taler(7), 583);
    // Laden
    let week = t.g().week();
    let [w1, _] = eco::weekly_blocks(week);
    let nicht = world::BLOCKS.iter().find(|b| b.flags & world::F_WEEKLY != 0 && !eco::weekly_blocks(week).contains(&b.id)).unwrap().id;
    assert_eq!(t.post(&tok, "shop/buy", json!({ "item": world::ZIEGEL, "n": 10 })).await.1, json!({ "item": world::ZIEGEL, "n": 34, "taler": 563 }));
    let pw = world::block(w1).unwrap().price as i64;
    assert_eq!(t.post(&tok, "shop/buy", json!({ "item": w1, "n": 2 })).await.1["taler"], 563 - 2 * pw, "Wochenblock der Woche");
    assert_eq!(t.post(&tok, "shop/buy", json!({ "item": nicht, "n": 1 })).await.0, 404, "anderer Wochenblock");
    assert_eq!(t.post(&tok, "shop/buy", json!({ "item": world::GLUEH, "n": 1 })).await.0, 404, "Seltenes nie");
    assert_eq!(t.post(&tok, "shop/buy", json!({ "item": world::KUPFER, "n": 640 })).await.0, 402, "zu teuer");
    assert_eq!(t.post(&tok, "shop/buy", json!({ "item": world::KUPFER, "n": 0 })).await.0, 400);
    assert_eq!(t.post(&tok, "shop/buy", json!({ "item": items::HEAD_STROH, "n": 1 })).await.1["taler"], 563 - 2 * pw - 60);
    assert_eq!(t.post(&tok, "shop/buy", json!({ "item": items::HEAD_STROH, "n": 1 })).await.0, 409, "Cosmetic nur einmal");
    let stand = 563 - 2 * pw - 60;
    assert_eq!(t.taler(7), stand, "abgelehnte Käufe kosten nichts");
    // Funde: nur fürs Lernen, nie zu kaufen
    let (c, f) = t.post(&tok, "fund/open", json!({})).await;
    assert!(c == 200 && items::exists(f["item"].as_u64().unwrap() as u16) && f["menge"].as_u64().unwrap() > 0, "{f}");
    assert_eq!(t.post(&tok, "fund/open", json!({})).await.0, 200);
    assert_eq!(t.post(&tok, "fund/open", json!({})).await.0, 402, "kein Fund mehr");
    assert_eq!(t.taler(7), stand);
    // Grundstück ausbauen: Stufe 10 erreicht, 300 Taler, dann 900 Taler und ein Bauplan
    assert_eq!(t.post(&tok, "plot/expand", json!({})).await.1, json!({ "size": 24, "taler": stand - 300 }));
    assert_eq!(t.post(&tok, "plot/expand", json!({})).await.0, 402, "900 Taler fehlen");
    t.g().pay(7, 1000, "test").unwrap();
    assert_eq!(t.post(&tok, "plot/expand", json!({})).await.1, json!({ "size": 32, "taler": stand - 300 + 1000 - 900 }));
    assert_eq!(t.inv(7, items::BLUEPRINT), 0, "Bauplan verbraucht");
    assert_eq!(t.post(&tok, "plot/expand", json!({})).await.0, 409, "voll ausgebaut");
    // Kassenbuch stimmt mit dem Kontostand überein
    let (sum, n): (i64, i64) = t.g().db.query_row("select sum(delta), count(*) from ledger where user = 7", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(sum, t.taler(7));
    assert!(n >= 14);
    // niedrige Stufe: kein Ausbau
    let tb = t.user(8, "Ben", false).await;
    let _b = Bot::connect(&t.addr, &tb).await;
    t.g().pay(8, 5000, "test").unwrap();
    assert_eq!(t.post(&tb, "plot/expand", json!({})).await.0, 403, "Stufe 5 nötig");
}

// ---------- Abschnitt 4: Aussehen, Cosmetics auf drei Wegen, Emotes – live bei beiden ----------
#[tokio::test]
async fn a4_cosmetics_bei_beiden_sichtbar() {
    let t = boot().await;
    let (ta, tb) = (t.user(7, "Anna", false).await, t.user(8, "Ben", false).await);
    let mut a = Bot::connect(&t.addr, &ta).await;
    let mut b = Bot::connect(&t.addr, &tb).await;
    let look_of = |m: &S2C| if let S2C::Join { id: 7, look, .. } = m { Some(*look) } else { None };
    t.g().pay(7, 500, "test").unwrap();
    // Weg 2: gekauft
    assert_eq!(t.post(&ta, "equip", json!({ "slot": items::HEAD, "item": items::HEAD_DOKTOR })).await.0, 403, "nicht im Besitz");
    assert_eq!(t.post(&ta, "shop/buy", json!({ "item": items::HEAD_DOKTOR, "n": 1 })).await.0, 200);
    assert_eq!(t.post(&ta, "equip", json!({ "slot": items::CAPE, "item": items::HEAD_DOKTOR })).await.0, 400, "falscher Platz");
    while b.wait(look_of).await.slots != [0; 8] {}
    assert_eq!(t.post(&ta, "equip", json!({ "slot": items::HEAD, "item": items::HEAD_DOKTOR })).await.0, 200);
    assert_eq!(b.wait(look_of).await.slots[items::HEAD as usize], items::HEAD_DOKTOR, "Ben sieht den Hut sofort");
    assert_eq!(a.wait(|m| look_of(m).filter(|l| l.slots[1] != 0)).await.slots[items::HEAD as usize], items::HEAD_DOKTOR, "Anna selbst auch");
    // Figur
    assert_eq!(t.post(&ta, "look", json!({ "skin": 3, "hair": 2, "hair_col": 4, "outfit": 1, "col_a": 7, "col_b": 250 })).await.0, 200);
    let l = b.wait(look_of).await;
    assert_eq!((l.skin, l.hair, l.hair_col, l.outfit, l.col_a, l.col_b, l.slots[1]), (3, 2, 4, 1, 7, 250 % 12, items::HEAD_DOKTOR));
    // Weg 1: erlernt (Stufe 5 und Serie 7 aus dem Lernhelfer)
    t.verdienst(7, json!({ "xp": 400, "serie": 7, "bester": 7, "ereignisse": [] }));
    assert_eq!(t.post(&ta, "zahltag", json!({})).await.1["neu"], json!(["Umhang der Stufe 5", "Flammenspur I"]));
    assert_eq!(t.post(&ta, "equip", json!({ "slot": items::CAPE, "item": items::CAPE_L5 })).await.0, 200);
    assert_eq!(t.post(&ta, "equip", json!({ "slot": items::TRAIL, "item": items::TRAIL_FLAMME_1 })).await.0, 200);
    assert_eq!(t.post(&ta, "equip", json!({ "slot": items::CAPE, "item": items::CAPE_L10 })).await.0, 403, "Stufe 10 noch nicht erreicht");
    // Weg 3: gefunden
    t.g().give(7, items::PET_FUCHS, 1).unwrap();
    assert_eq!(t.post(&ta, "equip", json!({ "slot": items::COMPANION, "item": items::PET_FUCHS })).await.0, 200);
    let l = b.wait(|m| look_of(m).filter(|l| l.slots[items::COMPANION as usize] != 0)).await;
    assert_eq!((l.slots[0], l.slots[1], l.slots[3], l.slots[4]), (items::CAPE_L5, items::HEAD_DOKTOR, items::PET_FUCHS, items::TRAIL_FLAMME_1));
    // Emotes: Winken hat jeder, Jubeln muss man haben
    a.send(C2S::Emote(1)).await;
    tokio::time::sleep(Duration::from_millis(1600)).await;
    a.send(C2S::Emote(0)).await;
    assert_eq!(b.wait(|m| if let S2C::Emote { id, n } = m { Some((*id, *n)) } else { None }).await, (7, 0), "Jubeln ohne Besitz kam nicht an, Winken schon");
    assert_eq!(t.post(&ta, "shop/buy", json!({ "item": items::EMOTE_JUBELN, "n": 1 })).await.0, 200);
    tokio::time::sleep(Duration::from_millis(1600)).await;
    a.send(C2S::Emote(1)).await;
    assert_eq!(b.wait(|m| if let S2C::Emote { id, n } = m { Some((*id, *n)) } else { None }).await, (7, 1));
    // wer später dazukommt, sieht das Aussehen auch
    let tc = t.user(9, "Cem", false).await;
    let mut c = Bot::connect(&t.addr, &tc).await;
    assert_eq!(c.wait(look_of).await.slots[3], items::PET_FUCHS);
    // verkauftes Stück wird abgelegt
    assert_eq!(t.post(&ta, "auction/create", json!({ "item": items::PET_FUCHS, "menge": 1, "start": 10, "stunden": 24 })).await.0, 200);
    assert_eq!(b.wait(|m| look_of(m).filter(|l| l.slots[3] == 0)).await.slots[1], items::HEAD_DOKTOR);
    // Krone der Woche: die drei Besten der Vorwoche tragen sie 7 Tage
    let w = t.g().week();
    t.g().rollover().unwrap();
    t.g().db.execute("insert into earn_week (user, week, taler) values (7, ?1, 50), (8, ?1, 90), (9, ?1, 0)", [w]).unwrap();
    t.g().clock_offset += 7 * 86_400_000;
    t.g().rollover().unwrap();
    assert!(t.owns(7, items::HEAD_KRONE) && t.owns(8, items::HEAD_KRONE) && !t.owns(9, items::HEAD_KRONE));
    assert_eq!(t.post(&ta, "equip", json!({ "slot": items::HEAD, "item": items::HEAD_KRONE })).await.0, 200);
    assert_eq!(b.wait(look_of).await.slots[1], items::HEAD_KRONE);
    t.g().clock_offset += 7 * 86_400_000 + 60_000;
    t.g().rollover().unwrap();
    assert!(!t.owns(7, items::HEAD_KRONE));
    assert_eq!(b.wait(look_of).await.slots[1], 0, "abgelaufene Krone wird abgelegt");
}

// ---------- Abschnitt 5: Auktionshaus auf den Taler genau ----------
#[tokio::test]
async fn a5_auktionen_auf_den_taler_genau() {
    let t = boot().await;
    let (ts, ta, tb) = (t.user(1, "Sara", false).await, t.user(2, "Anna", false).await, t.user(3, "Ben", false).await);
    for id in 1..=3 {
        t.g().pay(id, 1000, "test").unwrap();
    }
    t.g().give(1, world::GLUEH as u16, 10).unwrap();
    t.g().give(1, items::BLUEPRINT, 2).unwrap();
    let bal = |t: &T| (t.taler(1), t.taler(2), t.taler(3));
    let h = 3_600_000i64;
    let mut fees = 0i64;
    // Anlegen
    assert_eq!(t.post(&ts, "auction/create", json!({ "item": world::BRETTER, "menge": 1, "start": 1, "stunden": 24 })).await.0, 400, "nur Seltenes");
    assert_eq!(t.post(&ts, "auction/create", json!({ "item": world::GLUEH, "menge": 4, "start": 100, "stunden": 48 })).await.0, 400, "nur 24 oder 72 Stunden");
    assert_eq!(t.post(&ts, "auction/create", json!({ "item": world::GLUEH, "menge": 4, "start": 100, "sofort": 99, "stunden": 24 })).await.0, 400);
    assert_eq!(t.post(&ts, "auction/create", json!({ "item": world::GLUEH, "menge": 11, "start": 100, "stunden": 24 })).await.0, 402, "mehr als im Vorrat");
    let a1 = t.post(&ts, "auction/create", json!({ "item": world::GLUEH, "menge": 4, "start": 100, "sofort": 500, "stunden": 24 })).await.1["id"].as_i64().unwrap();
    assert_eq!(t.inv(1, world::GLUEH as u16), 6, "Ware liegt im Auktionshaus");
    // Bieten: sofort reserviert, beim Überbieten sofort zurück
    assert_eq!(t.post(&ts, "auction/bid", json!({ "id": a1, "taler": 100 })).await.0, 403, "eigenes Angebot");
    assert_eq!(t.post(&ta, "auction/bid", json!({ "id": a1, "taler": 99 })).await.0, 400, "unter Startpreis");
    assert_eq!(t.post(&ta, "auction/bid", json!({ "id": a1, "taler": 100 })).await.0, 200);
    assert_eq!(bal(&t), (1000, 900, 1000));
    assert_eq!(t.post(&tb, "auction/bid", json!({ "id": a1, "taler": 104 })).await.0, 400, "mindestens 5 % mehr");
    assert_eq!(bal(&t), (1000, 900, 1000));
    assert_eq!(t.post(&tb, "auction/bid", json!({ "id": a1, "taler": 105 })).await.0, 200);
    assert_eq!(bal(&t), (1000, 1000, 895), "Anna hat ihr Gebot zurück");
    assert_eq!(t.post(&ta, "auction/bid", json!({ "id": a1, "taler": 200 })).await.0, 200);
    assert_eq!(bal(&t), (1000, 800, 1000));
    assert_eq!(t.post(&ta, "auction/bid", json!({ "id": a1, "taler": 300 })).await.0, 200, "eigenes Gebot erhöhen");
    assert_eq!(bal(&t), (1000, 700, 1000));
    // Gebot über dem Sofortpreis = Sofortkauf zum Sofortpreis (500, nicht 2000): Anna zurück, 5 % Gebühr = 25
    assert_eq!(t.post(&tb, "auction/bid", json!({ "id": a1, "taler": 2000 })).await.0, 200);
    fees += eco::auction_fee(500) as i64;
    assert_eq!(bal(&t), (1475, 1000, 500));
    assert_eq!(t.inv(3, world::GLUEH as u16), 4);
    // Ablauf nach 24 Stunden: Höchstgebot gewinnt, Verkäuferin bekommt Preis minus 5 %
    let a2 = t.post(&ts, "auction/create", json!({ "item": world::GLUEH, "menge": 2, "start": 10, "stunden": 24 })).await.1["id"].as_i64().unwrap();
    assert_eq!(t.post(&ta, "auction/bid", json!({ "id": a2, "taler": 10 })).await.0, 200);
    assert_eq!(t.post(&tb, "auction/bid", json!({ "id": a2, "taler": 21 })).await.0, 200);
    assert_eq!(t.post(&ta, "auction/bid", json!({ "id": a2, "taler": 5000 })).await.0, 402, "Gebot ohne Deckung");
    assert_eq!(bal(&t), (1475, 1000, 479), "abgelehntes Gebot ändert nichts, Ben führt weiter");
    t.g().clock_offset += 24 * h - 1000;
    assert_eq!(t.g().auction_tick().unwrap(), 0, "eine Sekunde vor Schluss läuft sie noch");
    t.g().clock_offset += 1001;
    assert_eq!(t.post(&ta, "auction/bid", json!({ "id": a2, "taler": 100 })).await.0, 409, "nach Schluss kein Gebot mehr");
    assert_eq!(t.g().auction_tick().unwrap(), 1);
    assert_eq!(t.g().auction_tick().unwrap(), 0, "nur einmal abgerechnet");
    fees += 2; // 5 % von 21 = 1,05 → aufgerundet 2
    assert_eq!(bal(&t), (1475 + 19, 1000, 479));
    assert_eq!((t.inv(3, world::GLUEH as u16), t.inv(1, world::GLUEH as u16)), (6, 4));
    // Ablauf ohne Gebot nach 72 Stunden: Ware zurück, niemand zahlt
    let a3 = t.post(&ts, "auction/create", json!({ "item": items::BLUEPRINT, "menge": 2, "start": 50, "stunden": 72 })).await.1["id"].as_i64().unwrap();
    assert_eq!(t.inv(1, items::BLUEPRINT), 0);
    t.g().clock_offset += 24 * h + 1;
    assert_eq!(t.g().auction_tick().unwrap(), 0, "72 Stunden laufen länger als 24");
    t.g().clock_offset += 48 * h;
    assert_eq!(t.g().auction_tick().unwrap(), 1);
    assert_eq!(t.inv(1, items::BLUEPRINT), 2);
    assert_eq!(t.post(&ta, "auction/bid", json!({ "id": a3, "taler": 50 })).await.0, 404);
    assert_eq!(bal(&t), (1494, 1000, 479));
    // Sofortkauf mit laufendem Gebot: Bieter zurück, Gebühr aufgerundet
    let a4 = t.post(&ts, "auction/create", json!({ "item": items::BLUEPRINT, "menge": 1, "start": 10, "sofort": 50, "stunden": 24 })).await.1["id"].as_i64().unwrap();
    assert_eq!(t.post(&tb, "auction/bid", json!({ "id": a4, "taler": 20 })).await.0, 200);
    assert_eq!(bal(&t), (1494, 1000, 459));
    assert_eq!(t.post(&ta, "auction/buyout", json!({ "id": a4 })).await.0, 200);
    fees += 3; // 5 % von 50 = 2,5 → 3
    assert_eq!(bal(&t), (1494 + 47, 950, 479));
    assert_eq!(t.post(&tb, "auction/buyout", json!({ "id": a4 })).await.0, 404, "schon verkauft");
    assert_eq!(t.inv(2, items::BLUEPRINT), 1);
    // kleinster Preis: 1 Taler, Gebühr 1, Verkäuferin bekommt 0
    let a5 = t.post(&ts, "auction/create", json!({ "item": world::GLUEH, "menge": 1, "start": 1, "sofort": 1, "stunden": 24 })).await.1["id"].as_i64().unwrap();
    assert_eq!(t.post(&tb, "auction/buyout", json!({ "id": a5 })).await.0, 200);
    fees += 1;
    assert_eq!(bal(&t), (1541, 950, 478));
    // Summe aller Taler: nur die Gebühren sind verschwunden, und zwar endgültig
    assert_eq!(fees, 25 + 2 + 3 + 1);
    let burned = |t: &T| -> i64 { t.g().db.query_row("select sum(delta) from ledger where user = 0 and reason = 'gebuehr'", [], |r| r.get(0)).unwrap() };
    assert_eq!(burned(&t), fees);
    assert_eq!(t.supply(), 3000 - fees);
    assert!(t.g().db.query_row("select count(*) from flags", [], |r| r.get::<_, i64>(0)).unwrap() == 0, "bis hierher nichts Auffälliges");
    // offene Gebote zählen zur Summe (reserviert, nicht weg)
    let a6 = t.post(&ts, "auction/create", json!({ "item": world::GLUEH, "menge": 1, "start": 560, "stunden": 24 })).await.1["id"].as_i64().unwrap();
    assert_eq!(t.post(&ta, "auction/bid", json!({ "id": a6, "taler": 560 })).await.0, 200);
    assert_eq!(t.supply(), 3000 - fees);
    assert_eq!(t.taler(2), 390);
    let (_, list) = t.get(&ta, "auctions").await;
    let e = list["liste"].as_array().unwrap().iter().find(|e| e["id"] == a6).unwrap().clone();
    assert_eq!((e["gebot"].as_i64(), e["fuehre"].as_bool(), e["mindest"].as_i64(), e["meins"].as_bool()), (Some(560), Some(true), Some(588), Some(false)));
    // Auffälligkeiten: 560 Taler für einen Glühstein (Richtwert 20) und einseitiger Geldfluss Anna → Sara (50 + 560 in 7 Tagen)
    t.g().clock_offset += 24 * h + 1;
    assert_eq!(t.g().auction_tick().unwrap(), 1);
    fees += 28;
    assert_eq!(bal(&t), (1541 + 532, 390, 478));
    assert_eq!((t.supply(), burned(&t)), (3000 - fees, fees));
    let flags: Vec<(String, u32, u32)> = t.g().db.prepare("select kind, a, b from flags order by id").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(|r| r.unwrap()).collect();
    assert_eq!(flags, vec![("preis".to_string(), 2, 1), ("einseitig".to_string(), 2, 1)]);
    // jedes Kassenbuch stimmt mit dem Kontostand überein
    for id in 1..=3 {
        let sum: i64 = t.g().db.query_row("select sum(delta) from ledger where user = ?", [id], |r| r.get(0)).unwrap();
        assert_eq!(sum, t.taler(id), "Konto {id}");
    }
}

// ---------- Abschnitt 6: Großprojekt von zwei Konten, Lerngruppe, Wochenziel ----------
#[tokio::test]
async fn a6_grossprojekt_gruppe_wochenziel() {
    let t = boot().await;
    let (ta, tb) = (t.user(7, "Anna", false).await, t.user(8, "Ben", false).await);
    let mut a = Bot::connect(&t.addr, &ta).await;
    let _b = Bot::connect(&t.addr, &tb).await;
    t.g().pay(7, 10_000, "test").unwrap();
    t.g().pay(8, 10_000, "test").unwrap();
    // Großprojekte stehen im Hub: Steg südlich hinter dem Spawn-Tempel, Ziegelturm im Minenviertel
    let (steg, turm) = ((240, 32, 340), (259, 35, 185));
    assert_eq!(t.g().block(steg.0, steg.1, steg.2), 0, "noch kein Steg");
    assert_eq!(t.post(&ta, "project/fund", json!({ "key": "turm", "taler": 100 })).await.0, 409, "erst die Brücke");
    assert_eq!(t.post(&ta, "project/fund", json!({ "key": "bruecke", "taler": 1000 })).await.1["stand"], 1000);
    let hud = a.wait(|m| if let S2C::Hud(s) = m { serde_json::from_str::<Value>(s).ok().filter(|v| v["t"] == "projekt") } else { None }).await;
    assert_eq!((hud["stand"].as_i64(), hud["ziel"].as_i64(), hud["fertig"].as_bool()), (Some(1000), Some(3000), Some(false)), "Balken läuft live mit");
    assert_eq!(t.post(&tb, "project/fund", json!({ "key": "bruecke", "taler": 1500 })).await.1["stand"], 2500);
    let (c, r) = t.post(&ta, "project/fund", json!({ "key": "bruecke", "taler": 9000 })).await;
    assert_eq!((c, r["eingezahlt"].as_i64(), r["fertig"].as_bool()), (200, Some(500), Some(true)), "nie mehr als noch fehlt");
    assert_eq!((t.taler(7), t.taler(8)), (8500, 8500));
    assert_eq!(a.wait(|m| if let S2C::World(w) = m { Some(w.projects) } else { None }).await, world::P_BRIDGE);
    assert_eq!(t.g().block(steg.0, steg.1, steg.2), world::BRETTER, "der Steg steht");
    assert_eq!(t.post(&ta, "project/fund", json!({ "key": "bruecke", "taler": 10 })).await.0, 409, "fertig ist fertig");
    let (_, p) = t.get(&tb, "projects").await;
    assert_eq!((p["liste"][0]["fertig"].as_bool(), p["liste"][0]["spender"].as_i64(), p["liste"][1]["offen"].as_bool(), p["liste"][2]["offen"].as_bool()), (Some(true), Some(2), Some(true), Some(false)));
    // Lerngruppe: eigenes Viertel, gemeinsame Kasse, Wochenwertung
    assert_eq!(t.post(&ta, "group/create", json!({ "name": "x" })).await.0, 400);
    let gid = t.post(&ta, "group/create", json!({ "name": "VWL-Runde" })).await.1["id"].as_u64().unwrap();
    assert_eq!(t.taler(7), 8300, "Gründen kostet 200");
    assert_eq!(t.post(&ta, "group/create", json!({ "name": "Zweite" })).await.0, 409);
    assert_eq!(t.post(&tb, "group/join", json!({ "id": gid })).await.0, 200);
    assert_eq!(t.post(&ta, "group/deposit", json!({ "taler": 3000 })).await.0, 200);
    assert_eq!(t.post(&tb, "group/deposit", json!({ "taler": 2500 })).await.0, 200);
    assert_eq!(t.post(&tb, "group/deposit", json!({ "taler": 99_999 })).await.0, 402);
    let gp = t.g().plots.iter().find(|p| p.group == gid as u32).unwrap().clone();
    assert_eq!(world::slot_kind(gp.slot.0 as i32, gp.slot.1 as i32, t.g().st), world::SlotKind::Group, "Grundstück im Gruppen-Viertel");
    let r = world::plot_rect(gp.slot, gp.size);
    assert!(t.can_build(8, gid as u32, r.0 + 1, r.1 + 1) && !t.can_build(9, 0, r.0 + 1, r.1 + 1), "Mitglieder bauen gemeinsam");
    // der Turm wird aus Gruppenkasse und eigener Tasche bezahlt
    assert_eq!(t.post(&tb, "project/fund", json!({ "key": "turm", "taler": 4000, "gruppe": true })).await.1["stand"], 4000);
    assert_eq!(t.g().block(turm.0, turm.1, turm.2), 0);
    assert_eq!(t.post(&ta, "project/fund", json!({ "key": "turm", "taler": 1000 })).await.1["fertig"], true);
    assert_eq!(t.g().block(turm.0, turm.1, turm.2), world::ZIEGEL, "der Turm steht");
    assert_eq!(t.g().st.projects, world::P_BRIDGE | world::P_TOWER);
    let (_, gl) = t.get(&ta, "groups").await;
    assert_eq!((gl["liste"][0]["kasse"].as_i64(), gl["liste"][0]["mitglieder"].as_i64(), gl["meine"].as_u64()), (Some(1500), Some(2), Some(gid)));
    assert_eq!(t.supply(), 20_000 - 3000 - 200 - 5000, "Projekte und Gründung sind echte Senken");
    // Wochenziel der Welt: gemeinsam 2000 Taler erlernt → neues Gebiet, höchstens eines je Woche
    let ev = |id: i64, min: u32, tag: &str| json!({ "id": id, "art": "fokus", "menge": min, "fach": "", "tag": tag, "serie": 0 });
    t.verdienst(7, json!({ "ereignisse": (1..=5).map(|i| ev(i, 240, &format!("2026-11-0{i}"))).collect::<Vec<_>>() }));
    assert_eq!(t.post(&ta, "zahltag", json!({})).await.1["taler"], 1200);
    assert_eq!(t.g().st.regions, 0, "1200 von 2000: noch zu");
    t.verdienst(8, json!({ "ereignisse": (1..=4).map(|i| ev(i, 240, &format!("2026-11-0{i}"))).collect::<Vec<_>>() }));
    assert_eq!(t.post(&tb, "zahltag", json!({})).await.1["taler"], 960);
    assert_eq!(t.g().st.regions, 1, "2160 gemeinsam erlernt: ein neues Gebiet");
    assert_eq!(a.wait(|m| if let S2C::World(w) = m { Some(w.regions).filter(|r| *r == 1) } else { None }).await, 1);
    let (_, w) = t.get(&ta, "world").await;
    assert_eq!((w["gelernt"].as_i64(), w["ziel"].as_i64(), w["erreicht"].as_bool(), w["gebiete"].as_i64()), (Some(2160), Some(2000), Some(true), Some(1)));
    t.verdienst(7, json!({ "ereignisse": (6..=9).map(|i| ev(i, 240, &format!("2026-11-0{i}"))).collect::<Vec<_>>() }));
    t.post(&ta, "zahltag", json!({})).await;
    assert_eq!(t.g().st.regions, 1, "in derselben Woche kein zweites Gebiet");
    let (_, rk) = t.get(&tb, "ranking").await;
    assert_eq!((rk["spieler"][0]["name"].as_str(), rk["spieler"][0]["taler"].as_i64(), rk["spieler"][1]["taler"].as_i64()), (Some("Anna"), Some(2160), Some(960)));
    // alles übersteht einen Neustart
    t.g().save().unwrap();
    let g2 = Game::open(&t.db).unwrap();
    assert_eq!((g2.st.regions, g2.st.projects), (1, world::P_BRIDGE | world::P_TOWER));
    assert_eq!(g2.block(steg.0, steg.1, steg.2), world::BRETTER);
    // letzte Person verlässt die Gruppe: Kasse geht an sie, Grundstück wird frei
    assert_eq!(t.post(&ta, "group/leave", json!({})).await.0, 200);
    let vor = t.taler(8);
    assert_eq!(t.post(&tb, "group/leave", json!({})).await.1["taler"], vor + 1500);
    assert!(t.g().plots.iter().all(|p| p.group == 0));
}

// ---------- Abschnitt 7: Melden, Admin setzt zurück und sperrt, Export und Löschen ----------
#[tokio::test]
async fn a7_melden_admin_datenschutz() {
    let t = boot().await;
    let (chef, ta, tb) = (t.user(1, "Chef", true).await, t.user(7, "Anna", false).await, t.user(8, "Ben", false).await);
    let mut a = Bot::connect(&t.addr, &ta).await;
    let mut b = Bot::connect(&t.addr, &tb).await;
    let pa = t.g().plots.iter().find(|p| p.owner == 7).unwrap().clone();
    let r = world::plot_rect(pa.slot, pa.size);
    let (x, z) = (r.0 + 3, r.1 + 3);
    t.place(7, [x as f32 + 0.5, BY as f32, z as f32 + 1.5]).await;
    for k in 0..4 {
        assert_eq!(a.block(x, BY + k, z, world::BRETTER).await.0, world::BRETTER);
    }
    assert_eq!(a.block(x + 1, GROUND, z, 0).await.0, 0, "Loch im Boden");
    t.g().save().unwrap();
    a.send(C2S::Chat(0)).await; // Marke: alles davor ist gelesen
    a.wait(|m| matches!(m, S2C::Chat { id: 7, .. }).then_some(())).await;
    // Melden
    assert_eq!(t.post(&tb, "report", json!({ "user": 0, "plot": pa.id, "grund": "Quatsch" })).await.0, 400, "nur feste Gründe");
    assert_eq!(t.post(&tb, "report", json!({ "user": 0, "plot": pa.id, "grund": "Beleidigender Bau" })).await.0, 200);
    assert_eq!(t.post(&tb, "report", json!({ "user": 7, "plot": 0, "grund": "Belästigung" })).await.0, 429, "nicht im Sekundentakt");
    assert_eq!(t.get(&tb, "admin/overview").await.0, 404, "Admin-Bereich nur für Admins");
    assert_eq!(t.post(&tb, "admin/rollback", json!({ "plot": pa.id })).await.0, 404);
    let (c, ov) = t.get(&chef, "admin/overview").await;
    assert_eq!((c, ov["meldungen"][0]["plot"].as_u64(), ov["meldungen"][0]["von"].as_str(), ov["meldungen"][0]["grund"].as_str()), (200, Some(pa.id as u64), Some("Ben"), Some("Beleidigender Bau")));
    assert!(ov["konten"].as_array().unwrap().len() == 3 && ov["takt"]["anzahl"].as_u64().is_some());
    // Admin setzt den Bau mit einem Klick zurück
    assert_eq!(t.post(&chef, "admin/rollback", json!({ "plot": pa.id })).await.0, 200);
    for k in 0..4 {
        assert_eq!(t.g().block(x, BY + k, z), 0, "Bau ist weg");
    }
    assert_eq!(t.g().block(x + 1, GROUND, z), world::WIESE, "Boden ist wieder da");
    let rle = a.wait(|m| if let S2C::Chunk { cx, cz, rle } = m { ((*cx as i32, *cz as i32) == (x >> 4, z >> 4)).then(|| rle.clone()) } else { None }).await;
    let mut c = vec![0u8; world::VOL];
    world::unrle(&rle, &mut c);
    assert_eq!(c[world::idx(x & 15, BY, z & 15)], 0, "die Spieler bekommen das zurückgesetzte Stück");
    assert_eq!(t.get(&chef, "admin/overview").await.1["meldungen"], json!([]), "Meldung erledigt");
    assert_eq!(Game::open(&t.db).unwrap().block(x, BY, z), 0, "auch nach einem Neustart");
    // Sperren
    assert_eq!(t.post(&chef, "admin/ban", json!({ "user": 8, "an": true })).await.0, 200);
    assert!(b.wait(|m| if let S2C::Kick(s) = m { Some(s.clone()) } else { None }).await.contains("gesperrt"));
    assert_eq!(t.get(&tb, "me").await.0, 403);
    assert!(!t.g().players.contains_key(&8));
    assert_eq!(t.post(&chef, "admin/ban", json!({ "user": 8, "an": false })).await.0, 200);
    assert_eq!(t.get(&tb, "me").await.0, 200);
    // Export und Löschen (ruft der Lernhelfer)
    t.g().pay(7, 123, "test").unwrap();
    let (c, ex) = http(&t.addr, "GET", "/internal/campus/export/7", "", None).await;
    assert_eq!((c, ex["name"].as_str(), ex["taler"].as_i64(), ex["kassenbuch"][0]["grund"].as_str()), (200, Some("Anna"), Some(123), Some("test")));
    assert!(ex["grundstueck"]["ort"].is_array() && ex["vorrat"].as_array().unwrap().len() >= 3);
    assert_eq!(http(&t.addr, "GET", "/internal/campus/export/999", "", None).await.1, Value::Null);
    assert_eq!(http(&t.addr, "DELETE", "/internal/campus/user/7", "X-Forwarded-For: 1.2.3.4\r\n", None).await.0, 404);
    assert_eq!(http(&t.addr, "DELETE", "/internal/campus/user/7", "", None).await.1, json!({ "ok": true, "war": true }));
    assert_eq!(http(&t.addr, "DELETE", "/internal/campus/user/7", "", None).await.1, json!({ "ok": true, "war": false }));
    for tab in ["users where id", "inv where user", "ledger where user", "plots where owner", "daily where user"] {
        let n: i64 = t.g().db.query_row(&format!("select count(*) from {tab} = 7"), [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "{tab}");
    }
    assert!(!t.g().players.contains_key(&7));
    assert!(t.g().plots.iter().all(|p| p.owner != 7));
}

// ---------- Phase 8: schwebende Inseln, Warp, Besuchsrechte, Generator, geschützter Hub ----------
#[tokio::test]
async fn b8_inseln_warp_generator() {
    let t = boot().await;
    let (ta, tb) = (t.user(7, "Anna", false).await, t.user(8, "Ben", false).await);
    let mut a = Bot::connect(&t.addr, &ta).await;
    let mut b = Bot::connect(&t.addr, &tb).await;
    let pa = t.g().plots.iter().find(|p| p.owner == 7).unwrap().clone();
    a.send(C2S::Warp(Warp::Home)).await;
    let p = a.wait(tele).await;
    assert_eq!(p, world::plot_spawn(pa.slot), "sicherer Boden vor der Starthütte");
    assert_eq!(t.g().players[&7].p, p, "Server kennt die neue Stelle sofort");

    // Bruchstein-Generator der Rohstoffinsel: abbauen gibt Ertrag, 2,5 s steht ABGEBAUT da, dann wächst er nach
    let (gx, gy, gz) = generator(&t, pa.slot);
    t.place(7, [gx as f32 - 1.5, GROUND as f32 + 1.0, gz as f32 + 0.5]).await;
    let vorher = t.inv(7, world::BRUCHSTEIN as u16);
    assert_eq!(a.block(gx, gy, gz, 0).await, (world::ABGEBAUT, 7));
    assert_eq!(t.inv(7, world::BRUCHSTEIN as u16), vorher + 1);
    assert_eq!(a.block(gx, gy, gz, 0).await.0, world::ABGEBAUT, "erschöpft: nichts zu holen");
    assert_eq!(t.inv(7, world::BRUCHSTEIN as u16), vorher + 1);
    let t0 = std::time::Instant::now();
    a.wait(|m| matches!(m, S2C::Block { b, .. } if *b == world::GENERATOR).then_some(())).await;
    assert!(t0.elapsed() >= Duration::from_millis(1000), "nicht sofort");
    assert_eq!(t.g().block(gx, gy, gz), world::GENERATOR, "nachgewachsen");
    assert_eq!(a.block(gx, gy, gz, 0).await, (world::ABGEBAUT, 7));
    assert_eq!(t.inv(7, world::BRUCHSTEIN as u16), vorher + 2);
    assert_eq!(t.g().regen.len(), 1);

    // Abbauen braucht Zeit: Dig und sofort Block → „Zu schnell.“, Boden bleibt
    let r = world::plot_rect(pa.slot, pa.size);
    let (wx, wz) = (r.0 + 2, r.1 + 2);
    t.place(7, [wx as f32 + 0.5, GROUND as f32 + 1.0, wz as f32 + 2.5]).await;
    let boden = t.g().block(wx, GROUND, wz);
    tokio::time::sleep(Duration::from_millis(110)).await;
    a.send(C2S::Dig { x: wx as i16, y: GROUND as i16, z: wz as i16 }).await;
    a.send(C2S::Block { x: wx as i16, y: GROUND as i16, z: wz as i16, b: 0 }).await;
    a.wait(|m| hud_has(m, "Zu schnell")).await;
    assert_eq!(t.g().block(wx, GROUND, wz), boden);

    // Truhe der Insel: Fenster per [E], Ein- und Auslagern nur mit Baurecht
    let (cx, cy, cz) = {
        let g = t.g();
        let (sx, sz) = (pa.slot.0 as i32 * world::SLOT, pa.slot.1 as i32 * world::SLOT);
        (sx..sx + world::SLOT).flat_map(|x| (sz..sz + world::SLOT).map(move |z| (x, GROUND + 1, z))).find(|&(x, y, z)| g.block(x, y, z) == world::TRUHE).expect("Truhe")
    };
    t.place(7, [cx as f32 + 0.5, GROUND as f32 + 1.0, cz as f32 + 2.5]).await;
    a.send(C2S::Chest { x: cx as i16, y: cy as i16, z: cz as i16 }).await;
    a.wait(|m| hud_has(m, "\"truhe\"")).await;
    let fach = |n: i64| json!({ "x": cx, "y": cy, "z": cz, "item": world::BRUCHSTEIN, "n": n });
    let belegt = |v: Value| v["truhe"]["slots"].as_array().unwrap().iter().filter(|x| !x.is_null()).cloned().collect::<Vec<_>>();
    assert_eq!(belegt(t.post(&ta, "truhe", fach(2)).await.1), vec![json!([world::BRUCHSTEIN, 2])]);
    assert_eq!(t.inv(7, world::BRUCHSTEIN as u16), vorher);
    assert_eq!(t.post(&tb, "truhe", fach(-1)).await.0, 403, "fremde Truhe");
    assert_eq!(t.post(&ta, "truhe", fach(-5)).await.0, 402);
    assert!(belegt(t.post(&ta, "truhe", fach(-2)).await.1).is_empty());
    assert_eq!(t.inv(7, world::BRUCHSTEIN as u16), vorher + 2);

    // Sturz ins Leere: ein Viertel der Geldbörse ist weg, die Bank bleibt; zurück auf die eigene Insel
    t.g().pay(7, 200, "test").unwrap();
    assert_eq!(t.post(&ta, "bank/deposit", json!({ "taler": 100 })).await.0, 200);
    let p = t.g().players[&7].p;
    a.send(C2S::Move { p: [p[0], -30.0, p[2]], yaw: 0.0, anim: 0 }).await;
    let back = a.wait(tele).await;
    a.wait(|m| hud_has(m, "\"leere\"")).await;
    assert_eq!(t.taler(7), 75, "25 % von 100 ungesicherten Talern");
    assert_eq!(t.get(&ta, "sky").await.1["bank"], 100, "die Bank ist sicher");
    assert_eq!(world::slot_at(back[0] as i32, back[2] as i32), Some(pa.slot));
    assert!(back[1] > GROUND as f32);

    // Besuch: öffentlich ja, bauen nie; privat gar nicht mehr
    b.send(C2S::Warp(Warp::Plot(pa.id))).await;
    assert_eq!(b.wait(tele).await, world::plot_spawn(pa.slot));
    assert!(!t.can_build(8, 0, gx - 2, gz));
    assert!(!t.g().may_edit(8, 0, gx, gz, world::GENERATOR, false), "Besuch baut auch den Generator nicht ab");
    assert_eq!(t.post(&ta, "plot/visit", json!({ "modus": 2 })).await.0, 200);
    b.send(C2S::Warp(Warp::Plot(pa.id))).await;
    b.wait(|m| matches!(m, S2C::Hud(s) if s.contains("nicht für Besuch")).then_some(())).await;
    let k = t.get(&tb, "islands").await.1;
    assert!(!k["inseln"].as_array().unwrap().iter().any(|i| i["id"] == pa.id));
    assert_eq!(t.post(&ta, "plot/visit", json!({ "modus": 7 })).await.0, 400);

    // Hub: niemand baut, nur Nachwachsendes wird abgebaut; Orte erreichbar auf festem Boden
    assert!(!t.can_build(8, 0, MID, MID + 20), "im Hub baut niemand");
    assert!(t.g().may_edit(8, 0, MID, MID, world::ERZ_KOHLE, false));
    assert!(!t.g().may_edit(8, 0, MID, MID, world::FELS, false));
    a.send(C2S::Warp(Warp::Spot(1))).await;
    let p = a.wait(tele).await;
    let s = world::SPOTS[1].1;
    assert!((p[0] - s[0]).abs() <= 4.0 && (p[2] - s[2]).abs() <= 4.0 && (p[1] - s[1]).abs() <= 3.0, "{p:?}");
    assert!(world::solid(t.g().block(p[0] as i32, p[1] as i32 - 1, p[2] as i32)), "auf festem Boden");
    a.send(C2S::View(12)).await;
}

// ---------- Phase 9–13: SkyBlock ----------
use campus_shared::skyblock::{self as sb, Goal};
use rusqlite::params;

fn said(m: &S2C) -> Option<(u32, u8, String)> {
    if let S2C::Said { id, ch, text, .. } = m { Some((*id, *ch, text.clone())) } else { None }
}
fn npc_stand(id: u8) -> [f32; 3] {
    let p = sb::npc_pos(sb::npc(id).unwrap());
    [p[0], p[1], p[2] + 2.0]
}
const HEUTE: &str = "2026-11-12";
fn lern(id: i64, art: &str, menge: u32, fach: &str) -> Value {
    json!({ "id": id, "art": art, "menge": menge, "fach": fach, "tag": HEUTE, "serie": 0 })
}
/// Taler insgesamt: Konten, Bank, hinterlegte Kaufaufträge
fn taler_gesamt(t: &T) -> i64 {
    let g = t.g();
    let q = |sql: &str| -> i64 { g.db.query_row(sql, [], |r| r.get(0)).unwrap() };
    q("select coalesce(sum(taler), 0) from users") + q("select coalesce(sum(bank), 0) from sky") + q("select coalesce(sum(price * open), 0) from orders where buy = 1")
}
fn verbrannt(t: &T) -> i64 {
    -t.g().db.query_row("select coalesce(sum(delta), 0) from ledger where reason = 'bazaar:gebuehr'", [], |r| r.get::<_, i64>(0)).unwrap()
}
fn hud_has(m: &S2C, s: &str) -> Option<()> {
    matches!(m, S2C::Hud(h) if h.contains(s)).then_some(())
}
/// Versetzt: Ankunft mit Blickrichtung (Warp, Portal, Leere) oder Sprungstab
fn tele(m: &S2C) -> Option<[f32; 3]> {
    match m {
        S2C::Teleport(p) | S2C::Spawn { p, .. } => Some(*p),
        _ => None,
    }
}
fn generators(t: &T, slot: (u8, u8)) -> Vec<(i32, i32, i32)> {
    let (sx, sz) = (slot.0 as i32 * world::SLOT, slot.1 as i32 * world::SLOT);
    let g = t.g();
    (sx..sx + world::SLOT).flat_map(|x| (sz..sz + world::SLOT).flat_map(move |z| (GROUND..GROUND + 3).map(move |y| (x, y, z)))).filter(|&(x, y, z)| g.block(x, y, z) == world::GENERATOR).collect()
}
fn generator(t: &T, slot: (u8, u8)) -> (i32, i32, i32) {
    *generators(t, slot).first().expect("Generator")
}

#[tokio::test]
async fn c9_chat_befehle_dekan_lernhelfer() {
    let t = boot().await;
    let (chef, ta, tb) = (t.user(1, "Chef", true).await, t.user(7, "Anna", false).await, t.user(8, "Ben", false).await);
    let mut a = Bot::connect(&t.addr, &ta).await;
    let mut b = Bot::connect(&t.addr, &tb).await;
    let pause = || tokio::time::sleep(Duration::from_millis(1600));

    a.send(C2S::Say { ch: 0, text: "Hallo <b>alle</b>".into() }).await;
    assert_eq!(b.wait(said).await, (7, 0, "Hallo ‹b›alle‹/b›".into()), "frei, aber entschärft");
    a.send(C2S::Say { ch: 0, text: "gleich nochmal".into() }).await;
    assert!(a.wait(|m| said(m).filter(|s| s.1 == 4)).await.2.contains("Langsam"), "1 Nachricht je 1,5 s");
    pause().await;
    a.send(C2S::Say { ch: 0, text: "Du Arschloch".into() }).await;
    assert_eq!(b.wait(said).await.2, "Du *********");
    pause().await;
    a.send(C2S::Say { ch: 0, text: "y".repeat(400) }).await;
    assert_eq!(b.wait(said).await.2.len(), 160);

    // Befehle
    let pa = t.g().plots.iter().find(|p| p.owner == 7).unwrap().clone();
    b.send(C2S::Say { ch: 0, text: "/visit anna".into() }).await;
    assert_eq!(b.wait(tele).await, world::plot_spawn(pa.slot));
    pause().await;
    b.send(C2S::Say { ch: 0, text: "/msg Anna psst".into() }).await;
    assert_eq!(a.wait(|m| said(m).filter(|s| s.1 == 3)).await, (8, 3, "psst".into()));
    pause().await;
    b.send(C2S::Say { ch: 0, text: "/bazaar".into() }).await;
    b.wait(|m| hud_has(m, "Bazaar")).await;

    // Stummschalten durch Admins
    assert_eq!(t.post(&ta, "admin/mute", json!({ "user": 8, "minuten": 10 })).await.0, 404);
    assert_eq!(t.post(&chef, "admin/mute", json!({ "user": 8, "minuten": 10 })).await.0, 200);
    pause().await;
    b.send(C2S::Say { ch: 0, text: "hallo?".into() }).await;
    b.wait(|m| said(m).filter(|s| s.1 == 4 && s.2.contains("stumm"))).await;

    // Dekan Weber: Startgeschenk genau einmal
    let vorher = t.taler(7);
    t.place(7, world::plot_spawn(pa.slot)).await;
    a.send(C2S::Talk(sb::DEKAN)).await;
    a.wait(|m| hud_has(m, "Geh näher")).await;
    t.place(7, npc_stand(sb::DEKAN)).await;
    a.send(C2S::Talk(sb::DEKAN)).await;
    a.wait(|m| hud_has(m, "\"geschenk\":true")).await;
    let schritt0 = sb::quest(1).unwrap().steps[0].taler as i64;
    let kap1 = campus_shared::questline_data::CHAPTERS[0].taler as i64;
    assert_eq!(t.taler(7), vorher + sb::STARTER_TALER + schritt0 + kap1, "50 Starter-Taler + erster Questschritt + Questbuch-Kapitel 1");
    assert_eq!((t.inv(7, sb::minion_item(0, 1)), t.inv(7, sb::HAMMER)), (1, 1));
    let h = t.get(&ta, "sky").await.1["treibstoff_h"].as_f64().unwrap();
    assert!((5.9..=6.0).contains(&h), "{h}");
    a.send(C2S::Talk(sb::DEKAN)).await;
    a.wait(|m| hud_has(m, "\"geschenk\":false")).await;
    assert_eq!(t.inv(7, sb::HAMMER), 1);

    // Lernhelfer: Fach-Splitter, Kristalle, Lern-Energie (Deckel 72 h), Fähigkeits-EP
    t.verdienst(7, json!({ "xp": 0, "serie": 0, "bester": 0, "ereignisse": [lern(1, "fokus", 15, "Mathematik"), lern(2, "karte", 20, "Business English"), lern(3, "fokus", 60, "BWL")] }));
    let r = t.post(&ta, "zahltag", json!({})).await.1;
    assert_eq!(r["sky"][0]["fach"], "math");
    assert_eq!(r["sky"][1]["fach"], "languages");
    let s = t.get(&ta, "sky").await.1;
    assert_eq!((s["splitter"]["math"].as_u64(), s["splitter"]["languages"].as_u64(), s["splitter"]["humanities"].as_u64()), (Some(3), Some(10), Some(12)));
    assert_eq!(s["kristalle"], 1 + 4);
    assert!(s["treibstoff_h"].as_f64().unwrap() > 71.9, "6 h + 24 h + 24 h + 96 h → Deckel 72 h");
    assert_eq!(s["faehigkeiten"][0]["ep"], 30);
    assert!(s["faehigkeiten"][4]["ep"].as_u64().unwrap() >= 150, "Fokus-EP aus 75 Minuten");
}

#[tokio::test]
async fn c10_faehigkeiten_sammlungen_minions_faul() {
    let t = boot().await;
    let ta = t.user(7, "Anna", false).await;
    let _a = Bot::connect(&t.addr, &ta).await;
    let pa = t.g().plots.iter().find(|p| p.owner == 7).unwrap().clone();
    let item = sb::minion_item(0, 1);
    t.g().give(7, item, 3).unwrap();
    let sp = world::plot_spawn(pa.slot);
    let (x, z) = (sp[0] as i32, sp[2] as i32);
    let ticks0 = t.g().stats.ticks;
    let setz = |x: i32, z: i32| json!({ "item": item, "x": x, "y": GROUND + 1, "z": z });
    assert_eq!(t.post(&ta, "minion/place", setz(x, z)).await.0, 200);
    assert_eq!(t.post(&ta, "minion/place", setz(x, z)).await.0, 409, "Platz belegt");
    assert_eq!(t.post(&ta, "minion/place", setz(x + 1, z)).await.0, 200);
    assert_eq!(t.post(&ta, "minion/place", setz(x + 2, z)).await.0, 409, "2 Plätze auf Stufe 1");
    assert_eq!(t.post(&ta, "minion/place", setz(MID, MID + 14)).await.0, 403, "nur eigene Insel");
    // 12 Stunden später, ohne Treibstoff (20 %): faul nachgerechnet, nicht im Takt
    t.g().clock_offset += 12 * sb::HOUR_MS;
    let m = t.get(&ta, &format!("minions/{}", pa.id)).await.1;
    let erwartet = sb::minion_step(0, 1, 0, 12 * sb::HOUR_MS, 0, 0, 0).0;
    assert_eq!(m["minions"][0]["lager"].as_u64().unwrap() as u32, erwartet, "{m}");
    let id = m["minions"][0]["id"].as_i64().unwrap();
    let r = t.post(&ta, "minion/collect", json!({ "id": id })).await.1;
    assert_eq!(r["menge"].as_u64().unwrap() as u32, erwartet);
    assert_eq!(t.inv(7, world::BRUCHSTEIN as u16), erwartet);
    assert_eq!(t.post(&ta, "minion/collect", json!({ "id": id })).await.1["menge"], 0, "nichts doppelt");
    assert_eq!(t.post(&ta, "minion/collect", json!({ "id": id, "abholen": true })).await.0, 200);
    assert_eq!(t.inv(7, item), 2);
    // Sammlung und freigeschaltete Rezepte
    let rc = t.get(&ta, "recipes").await.1;
    let c = rc["sammlungen"].as_array().unwrap().iter().find(|c| c["item"] == world::BRUCHSTEIN).unwrap().clone();
    assert_eq!(c["stufe"].as_u64().unwrap(), sb::coll_tier(erwartet as u64) as u64);
    assert!(rc["rezepte"].as_array().unwrap().iter().any(|r| r["out"] == sb::VERDICHTET && r["frei"] == true));
    // der Takt läuft, ohne Minions anzufassen
    let last: i64 = t.g().db.query_row("select last from minions limit 1", [], |r| r.get(0)).unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(t.g().stats.ticks > ticks0);
    assert_eq!(t.g().db.query_row("select last from minions limit 1", [], |r| r.get::<_, i64>(0)).unwrap(), last);
}

#[tokio::test]
async fn c11_buchhaltung_bazaar_verdichten_bank() {
    let t = boot().await;
    let (ta, tb) = (t.user(7, "Anna", false).await, t.user(8, "Ben", false).await);
    let stein = world::BRUCHSTEIN as u16;
    t.g().gain(7, stein, 400).unwrap();
    t.g().pay(8, 1000, "test").unwrap();
    let s0 = taler_gesamt(&t);
    let teile = |t: &T| -> i64 {
        let g = t.g();
        g.inv(7, stein) as i64 + g.inv(8, stein) as i64 + g.db.query_row("select coalesce(sum(open), 0) from orders where buy = 0", [], |r| r.get::<_, i64>(0)).unwrap()
    };
    let ok = |r: (u16, Value)| {
        assert_eq!(r.0, 200, "{}", r.1);
        r.1
    };
    ok(t.post(&ta, "bazaar/order", json!({ "item": stein, "kauf": false, "preis": 5, "menge": 100 })).await);
    let r = ok(t.post(&tb, "bazaar/order", json!({ "item": stein, "kauf": true, "preis": 6, "menge": 50 })).await);
    assert_eq!((r["gehandelt"].as_u64(), r["taler"].as_i64()), (Some(50), Some(250)), "zum Preis des wartenden Angebots");
    let r = ok(t.post(&tb, "bazaar/order", json!({ "item": stein, "kauf": true, "sofort": true, "menge": 30 })).await);
    assert_eq!(r["taler"], 150);
    ok(t.post(&tb, "bazaar/order", json!({ "item": stein, "kauf": true, "preis": 3, "menge": 20 })).await);
    let r = ok(t.post(&ta, "bazaar/order", json!({ "item": stein, "kauf": false, "sofort": true, "menge": 10 })).await);
    assert_eq!(r["gehandelt"], 10);
    assert_eq!(t.post(&ta, "bazaar/order", json!({ "item": sb::HAMMER, "kauf": false, "preis": 5, "menge": 1 })).await.0, 400);
    assert_eq!(t.post(&tb, "bazaar/order", json!({ "item": stein, "kauf": true, "preis": 10, "menge": 999 })).await.0, 402, "nicht genug Taler");
    let mine = ok(t.get(&tb, "bazaar").await)["meine"].clone();
    ok(t.post(&tb, "bazaar/cancel", json!({ "id": mine[0]["id"] })).await);
    assert_eq!(t.post(&tb, "bazaar/cancel", json!({ "id": mine[0]["id"] })).await.0, 404, "nicht doppelt zurück");
    assert_eq!(t.inv(8, stein), 90);
    assert_eq!(verbrannt(&t), 4 + 3 + 1, "1,5 % je Handel, aufgerundet");
    assert_eq!(taler_gesamt(&t) + verbrannt(&t), s0, "kein Taler entsteht oder verschwindet");
    assert_eq!(teile(&t), 400, "kein Stein entsteht oder verschwindet");

    // Verdichten: 160 → 1, gerechnet in Grundstücken bleibt alles gleich
    let einheiten = |t: &T| t.inv(7, stein) as i64 + 160 * t.inv(7, sb::VERDICHTET) as i64 + t.g().db.query_row("select coalesce(sum(open), 0) from orders where buy = 0 and user = 7", [], |r| r.get::<_, i64>(0)).unwrap();
    let e0 = einheiten(&t);
    assert_eq!(t.post(&ta, "craft", json!({ "item": sb::VERDICHTET, "mal": 1 })).await.0, 200);
    assert_eq!(t.inv(7, sb::VERDICHTET), 1);
    assert_eq!(einheiten(&t), e0);
    assert_eq!(t.post(&ta, "craft", json!({ "item": sb::VERDICHTET, "mal": 64 })).await.0, 402, "zu wenig: nichts passiert");
    assert_eq!(einheiten(&t), e0);
    assert_eq!(t.post(&ta, "craft", json!({ "item": sb::SUPER, "mal": 1 })).await.0, 403, "erst ab Sammlungsstufe 5");

    // Bank: Ein- und Auszahlen verschiebt nur; Zinsen nur nach Lernen in den letzten 48 h
    let vor = taler_gesamt(&t);
    ok(t.post(&tb, "bank/deposit", json!({ "taler": 500 })).await);
    assert_eq!(taler_gesamt(&t), vor);
    assert_eq!(t.post(&tb, "bank/withdraw", json!({ "taler": 501 })).await.0, 402);
    t.g().clock_offset += 3 * 24 * sb::HOUR_MS;
    assert_eq!(ok(t.get(&tb, "sky").await)["bank"], 500, "ohne Lernen keine Zinsen");
    let now = t.g().now();
    t.g().db.execute("update sky set last_study = ?, bank_at = ? where user = 8", params![now, now]).unwrap();
    t.g().clock_offset += 2 * 24 * sb::HOUR_MS;
    assert_eq!(ok(t.get(&tb, "sky").await)["bank"], 510, "500 → 505 → 510");
    assert_eq!(ok(t.get(&tb, "sky").await)["bank"], 510, "nicht doppelt verzinst");
    let zins: i64 = t.g().db.query_row("select sum(delta) from ledger where reason = 'bank:zins'", [], |r| r.get(0)).unwrap();
    assert_eq!(zins, 10);
    ok(t.post(&tb, "bank/withdraw", json!({ "taler": 510 })).await);
    assert_eq!(taler_gesamt(&t), vor + zins);
}

#[tokio::test]
async fn c12_werkzeuge_und_mana() {
    let t = boot().await;
    let ta = t.user(7, "Anna", false).await;
    let mut a = Bot::connect(&t.addr, &ta).await;
    let pa = t.g().plots.iter().find(|p| p.owner == 7).unwrap().clone();
    let r = world::plot_rect(pa.slot, pa.size);
    let (x, z, y) = (r.0 + 4, r.1 + 4, GROUND + 1);
    let plank = world::BRETTER;
    {
        let mut g = t.g();
        for dx in -1..=1 {
            g.set_block(x + dx, y, z, plank, 0);
            g.set_block(x + dx, y + 1, z, 0, 0);
        }
        g.give(7, sb::BAUSTAB[0], 1).unwrap();
        g.give(7, plank as u16, 10).unwrap();
    }
    let vorrat = t.inv(7, plank as u16);
    {
        let g = t.g();
        g.give(7, sb::HAKEN, 1).unwrap();
        g.give(7, sb::SPRUNGSTAB, 1).unwrap();
    }
    t.place(7, [x as f32 + 0.5, y as f32, z as f32 + 3.5]).await;
    let eye = [x as f32 + 0.5, y as f32 + 1.62, z as f32 + 3.5];
    let ziel = [x as f32 + 0.5, y as f32 + 0.9, z as f32 + 0.3];
    let dir = [ziel[0] - eye[0], ziel[1] - eye[1], ziel[2] - eye[2]];
    a.send(C2S::Ability { tool: sb::BAUSTAB[0], dir }).await;
    a.wait(|m| hud_has(m, "\"t\":\"mana\"")).await;
    for dx in -1..=1 {
        assert_eq!(t.g().block(x + dx, y + 1, z), plank, "Bau-Stab verlängert die ganze Fläche");
    }
    assert_eq!(t.inv(7, plank as u16), vorrat - 3);
    let mana = t.get(&ta, "sky").await.1["mana"].as_u64().unwrap();
    assert!((90..=92).contains(&mana), "10 Mana: {mana}");

    // Server prüft das Mana selbst
    let now = t.g().now();
    t.g().db.execute("update sky set mana = 5, mana_at = ? where user = 7", [now]).unwrap();
    tokio::time::sleep(Duration::from_millis(350)).await;
    a.send(C2S::Ability { tool: sb::BAUSTAB[0], dir }).await;
    a.wait(|m| hud_has(m, "Nicht genug Fokus-Mana")).await;
    assert_eq!(t.inv(7, plank as u16), vorrat - 3);

    // Enterhaken: kurz schneller erlaubt, 2 s Abklingzeit
    a.send(C2S::Ability { tool: sb::HAKEN, dir: [0.0, 0.3, -1.0] }).await;
    a.wait(|m| hud_has(m, "\"t\":\"mana\"")).await;
    let b = t.g().players[&7].boost_until;
    let n = t.g().now();
    assert!(b > n);
    a.send(C2S::Ability { tool: sb::HAKEN, dir: [0.0, 0.3, -1.0] }).await;
    a.wait(|m| hud_has(m, "Noch nicht wieder bereit")).await;

    // Werkzeug, das man nicht hat
    a.send(C2S::Ability { tool: sb::HAMMER, dir }).await;
    a.wait(|m| hud_has(m, "hast du nicht")).await;

    // Sprungstab: 6 Blöcke nach vorn
    tokio::time::sleep(Duration::from_millis(500)).await;
    let now = t.g().now();
    t.g().db.execute("update sky set mana = 100, mana_at = ? where user = 7", [now]).unwrap();
    t.place(7, [x as f32 - 3.5, y as f32, z as f32 + 9.5]).await;
    a.send(C2S::Ability { tool: sb::SPRUNGSTAB, dir: [1.0, 0.0, 0.0] }).await;
    let p = a.wait(tele).await;
    assert!((p[0] - (x as f32 - 3.5) - sb::BLINK).abs() < 0.3, "{p:?}");
    assert_eq!(t.g().players[&7].p, p);
}

#[tokio::test]
async fn c13_starterquest_komplett() {
    let t = boot().await;
    let ta = t.user(7, "Anna", false).await;
    let tb = t.user(8, "Ben", false).await;
    let mut a = Bot::connect(&t.addr, &ta).await;
    let pa = t.g().plots.iter().find(|p| p.owner == 7).unwrap().clone();
    let q = sb::quest(1).unwrap();
    let mut ev = 100;
    for (k, s) in q.steps.iter().enumerate() {
        match s.goal {
            Goal::Talk(n) => {
                t.place(7, npc_stand(n)).await;
                a.send(C2S::Talk(n)).await;
                a.wait(|m| hud_has(m, "dialog")).await;
            }
            Goal::Home => {
                a.send(C2S::Warp(Warp::Home)).await;
                assert_eq!(a.wait(tele).await, world::plot_spawn(pa.slot));
            }
            Goal::Mine(item, n) | Goal::MineMinion(item, n) => {
                assert_eq!(item, world::BRUCHSTEIN as u16);
                if matches!(s.goal, Goal::MineMinion(..)) {
                    let sp = world::plot_spawn(pa.slot);
                    assert_eq!(t.post(&ta, "minion/place", json!({ "item": sb::minion_item(0, 1), "x": sp[0] as i32, "y": GROUND + 1, "z": sp[2] as i32 })).await.0, 200);
                }
                // beide Generatoren abwechselnd; ein erschöpfter wächst nach 2,5 s nach
                let gens = generators(&t, pa.slot);
                assert!(gens.len() >= 2);
                t.place(7, [gens[0].0 as f32 - 1.5, GROUND as f32 + 1.0, gens[0].2 as f32 + 0.5]).await;
                let mut done = 0;
                while done < n {
                    let Some(&(x, y, z)) = gens.iter().find(|g| t.g().block(g.0, g.1, g.2) == world::GENERATOR) else {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    };
                    tokio::time::sleep(Duration::from_millis(110)).await;
                    a.send(C2S::Dig { x: x as i16, y: y as i16, z: z as i16 }).await;
                    tokio::time::sleep(Duration::from_millis(700)).await;
                    a.send(C2S::Block { x: x as i16, y: y as i16, z: z as i16, b: 0 }).await;
                    a.wait(|m| matches!(m, S2C::Block { x: bx, y: by_, z: bz, b, by: 7 } if (*bx as i32, *by_ as i32, *bz as i32) == (x, y, z) && *b == world::ABGEBAUT).then_some(())).await;
                    done += 1;
                }
            }
            Goal::Bazaar => {
                let stein = world::BRUCHSTEIN as u16;
                t.g().gain(8, stein, 5).unwrap();
                assert_eq!(t.post(&tb, "bazaar/order", json!({ "item": stein, "kauf": false, "preis": 2, "menge": 5 })).await.0, 200);
                assert_eq!(t.post(&ta, "bazaar/order", json!({ "item": stein, "kauf": true, "sofort": true, "menge": 1 })).await.0, 200);
            }
            Goal::PlaceMinion => {
                let sp = world::plot_spawn(pa.slot);
                assert_eq!(t.post(&ta, "minion/place", json!({ "item": sb::minion_item(0, 1), "x": sp[0] as i32, "y": GROUND + 1, "z": sp[2] as i32 })).await.0, 200);
            }
            Goal::Learn(n) | Goal::Check(n) => {
                let evs: Vec<Value> = (0..n).map(|i| lern(ev + i as i64, "check", 1, "Statistik")).collect();
                ev += n as i64;
                t.verdienst(7, json!({ "ereignisse": evs }));
                assert_eq!(t.post(&ta, "zahltag", json!({})).await.0, 200);
            }
            other => panic!("Startquest-Schritt {k} ({other:?}) im Test nicht vorgesehen"),
        }
        let s = t.get(&ta, "sky").await.1;
        if k + 1 < q.steps.len() {
            assert_eq!(s["ziel"].as_str().map(|z| z.starts_with(q.steps[k + 1].text)), Some(true), "nach Schritt {k}: {s}");
        }
    }
    assert!(t.g().db.query_row("select 1 from quests_done where user = 7 and quest = 1", [], |_| Ok(())).is_ok());
    assert!(t.get(&ta, "sky").await.1["quest"].is_null());
    let belohnt: i64 = t.g().db.query_row("select sum(delta) from ledger where user = 7 and reason like 'quest:1:%'", [], |r| r.get(0)).unwrap();
    assert_eq!(belohnt, q.steps.iter().map(|s| s.taler as i64).sum::<i64>());
    assert_eq!(t.inv(7, sb::ENERGIE), 5, "5× Minion-Turbo als Abschluss-Belohnung");
}
