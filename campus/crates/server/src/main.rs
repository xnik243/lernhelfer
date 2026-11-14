use campus_server::{start, Config};

#[tokio::main]
async fn main() {
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    let cfg = Config { port: env("CAMPUS_PORT", "8090").parse().unwrap_or(8090), db: env("CAMPUS_DB", "campus.db"), web: env("CAMPUS_WEB", "web"), lh: env("CAMPUS_LH", "127.0.0.1:8080") };
    let (app, addr) = match start(cfg).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Start fehlgeschlagen: {e}");
            std::process::exit(1);
        }
    };
    println!("Campus läuft auf {addr}");
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("Signal");
    tokio::select! { _ = term.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
    let saved = app.game.lock().unwrap().save();
    match saved {
        Ok(n) => println!("Beendet, {n} Stücke gesichert."),
        Err(e) => eprintln!("Sichern beim Beenden: {}", e.1),
    }
}
