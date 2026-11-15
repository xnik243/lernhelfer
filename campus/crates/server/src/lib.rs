//! Campus-Spielserver: eigener Dienst neben dem Lernhelfer, eigene SQLite-Datenbank.
pub mod architect;
pub mod catacombs;
pub mod catacombs_gen;
pub mod duel;
pub mod duel_ai;
pub mod duel_fallback_data;
pub mod dungeon_rooms_data;
pub mod econ;
pub mod game;
pub mod http;
pub mod instance;
pub mod inv;
pub mod island;
pub mod lh;
pub mod mobs;
pub mod rpg;
pub mod sky;
#[cfg(test)]
pub mod testkit;

use game::{Game, User};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct E(pub u16, pub String);
pub type R<T> = Result<T, E>;
pub fn no<T>(code: u16, msg: &str) -> R<T> {
    Err(E(code, msg.to_string()))
}
impl From<rusqlite::Error> for E {
    fn from(e: rusqlite::Error) -> E {
        E(500, format!("Datenbank: {e}"))
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub db: String,
    pub web: String,
    pub lh: String, // Adresse des Lernhelfers, z. B. 127.0.0.1:8080
}

pub struct AppState {
    pub game: Mutex<Game>,
    pub cfg: Config,
    pub auth: Mutex<HashMap<String, (User, Instant)>>,
    pub ver: String,
}
pub type App = Arc<AppState>;

/// Startet Dienst, Takt (10 Hz) und Hintergrundarbeit (Auktionen alle 5 s, Speichern alle 30 s).
pub async fn start(cfg: Config) -> Result<(App, SocketAddr), String> {
    let game = Game::open(&cfg.db).map_err(|e| e.1)?;
    let ver = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()).to_string();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", cfg.port)).await.map_err(|e| e.to_string())?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let app: App = Arc::new(AppState { game: Mutex::new(game), cfg, auth: Mutex::new(HashMap::new()), ver });
    let a = app.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_millis(100));
        t.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            t.tick().await;
            a.game.lock().unwrap().tick();
        }
    });
    let a = app.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_secs(5));
        let mut n = 0u32;
        loop {
            t.tick().await;
            n += 1;
            let mut g = a.game.lock().unwrap();
            if let Err(e) = g.auction_tick().and_then(|_| g.rollover()) {
                eprintln!("Hintergrundarbeit: {}", e.1);
            }
            g.weather_tick(); // Wetterwechsel und Ende des 2×-Taler-Events (architect.rs), nie im 100-ms-Takt
            if n % 6 == 0 {
                if let Err(e) = g.save() {
                    eprintln!("Speichern: {}", e.1);
                }
            }
        }
    });
    let router = http::router(app.clone());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).await;
    });
    Ok((app, addr))
}
