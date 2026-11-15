//! Nur für Tests (cfg(test)): Spiel auf einer frischen Temp-Datenbank, Anmelden ohne Netz, Nachrichten auslesen.
//! Alle Tests der Phasen 21–25 stehen als `#[cfg(test)] mod tests` in ihrem Modul und nutzen diese Helfer
//! (ein gemeinsames Test-Programm statt je Datei ein eigenes – der Server hat nur 2 GB).
use crate::game::{Game, User};
use axum::body::Bytes;
use campus_shared::proto::{self, S2C};
use serde_json::Value;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc::{channel, Receiver};

static N: AtomicU32 = AtomicU32::new(0);

pub fn game() -> Game {
    let p = std::env::temp_dir().join(format!("campus-p21-{}-{}.db", std::process::id(), N.fetch_add(1, Ordering::SeqCst))).display().to_string();
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{p}{s}"));
    }
    Game::open(&p).unwrap()
}
/// Meldet ein Konto an (legt es an) und liefert seinen Nachrichten-Kanal.
pub fn join(g: &mut Game, id: u32, name: &str, admin: bool) -> Receiver<Bytes> {
    let (tx, rx) = channel::<Bytes>(100_000);
    g.join(&User { id, name: name.into(), admin }, tx).unwrap();
    rx
}
pub fn user(id: u32, name: &str, admin: bool) -> User {
    User { id, name: name.into(), admin }
}
/// Setzt die Figur (ohne Tempo-Prüfung).
pub fn put(g: &mut Game, id: u32, p: [f32; 3]) {
    g.players.get_mut(&id).unwrap().p = p;
}
/// Alle bisher angekommenen Nachrichten (leert den Kanal).
pub fn msgs(rx: &mut Receiver<Bytes>) -> Vec<S2C> {
    let mut v = Vec::new();
    while let Ok(b) = rx.try_recv() {
        if let Some(m) = proto::dec::<S2C>(&b) {
            v.push(m);
        }
    }
    v
}
/// Alle bisher angekommenen Hud-Nachrichten als JSON (leert den Kanal).
pub fn huds(rx: &mut Receiver<Bytes>) -> Vec<Value> {
    msgs(rx).into_iter().filter_map(|m| if let S2C::Hud(s) = m { serde_json::from_str(&s).ok() } else { None }).collect()
}
/// System-Zeilen des Chats (Antworten auf Befehle), leert den Kanal.
pub fn says(rx: &mut Receiver<Bytes>) -> Vec<String> {
    msgs(rx).into_iter().filter_map(|m| if let S2C::Said { id: 0, text, .. } = m { Some(text) } else { None }).collect()
}
