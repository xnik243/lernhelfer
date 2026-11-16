//! HTTP und WebSocket: Anmeldung über das Sitzungs-Cookie des Lernhelfers, JSON-Abrufe, Dateien der Oberfläche.
use crate::econ::{registry, Verdienst};
use crate::game::User;
use crate::{lh, no, App, E, R};
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, RawQuery, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get};
use axum::Router;
use campus_shared::proto::{self, C2S};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const CSP: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

pub fn router(app: App) -> Router {
    Router::new()
        .route("/campus", get(|| async { Redirect::permanent("/campus/") }))
        .route("/campus/", get(|s: State<App>, h: HeaderMap, q: RawQuery| page(s, Path(String::new()), h, q)))
        .route("/campus/ws", get(ws))
        .route("/campus/api/{*op}", get(api_get).post(api_post))
        .route("/campus/{*path}", get(page))
        .route("/internal/campus/user/{id}", delete(internal_delete))
        .route("/internal/campus/export/{id}", get(internal_export))
        .with_state(app)
}

fn reply(r: R<Value>) -> Response {
    let (code, v) = match r {
        Ok(v) => (StatusCode::OK, v),
        Err(E(c, m)) => (StatusCode::from_u16(c).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), json!({ "error": m })),
    };
    let mut res = (code, axum::Json(v)).into_response();
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res.headers_mut().insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    res
}

/// Konto über den Lernhelfer bestimmen: das Cookie `lh` geht an dessen `GET /api/ich`. Ergebnis 30 s gemerkt.
pub async fn auth(app: &App, h: &HeaderMap) -> R<User> {
    let cookie = h.get(header::COOKIE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let Some(tok) = cookie.split(';').filter_map(|p| p.trim().strip_prefix("lh=")).find(|t| !t.is_empty() && t.len() < 200 && t.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')) else {
        return no(401, "Bitte zuerst im Lernhelfer anmelden.");
    };
    if let Some((u, t)) = app.auth.lock().unwrap().get(tok) {
        if t.elapsed() < Duration::from_secs(30) {
            return Ok(u.clone());
        }
    }
    let (status, body) = lh::get(&app.cfg.lh, "/api/ich", Some(&format!("lh={tok}"))).await.map_err(|_| E(502, "Der Lernhelfer antwortet gerade nicht.".into()))?;
    if status != 200 {
        return no(401, "Bitte zuerst im Lernhelfer anmelden.");
    }
    let v: Value = serde_json::from_str(&body).map_err(|_| E(502, "Der Lernhelfer antwortet unlesbar.".into()))?;
    let Some(id) = v["id"].as_u64().filter(|&i| i > 0 && i < u32::MAX as u64) else { return no(502, "Der Lernhelfer kennt Campus noch nicht (Konto-Nummer fehlt).") };
    let name = v["spielname"].as_str().filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| format!("Spieler {id}"));
    let user = User { id: id as u32, name, admin: v["admin"] == json!(true) };
    let mut cache = app.auth.lock().unwrap();
    if cache.len() > 2000 {
        cache.clear();
    }
    cache.insert(tok.to_string(), (user.clone(), Instant::now()));
    Ok(user)
}

/// Schreibende Anfragen und WebSockets nur von der eigenen Seite.
fn same_origin(h: &HeaderMap) -> bool {
    let Some(o) = h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else { return true };
    let host = h.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
    o.split_once("://").is_some_and(|(_, rest)| rest == host)
}

async fn verdienst(app: &App, u: &User) -> R<Verdienst> {
    let seit: i64 = {
        let mut g = app.game.lock().unwrap();
        g.ensure_user(u)?;
        g.db.query_row("select last_ev from users where id = ?", [u.id], |r| r.get(0))?
    };
    let (status, body) = lh::get(&app.cfg.lh, &format!("/api/campus/verdienst?konto={}&seit={seit}", u.id), None).await.map_err(|_| E(502, "Der Lernhelfer antwortet gerade nicht.".into()))?;
    if status != 200 {
        return no(502, "Der Lernhelfer liefert gerade keinen Verdienst.");
    }
    serde_json::from_str(&body).map_err(|_| E(502, "Der Lernhelfer antwortet unlesbar.".into()))
}

async fn api_get(State(app): State<App>, Path(op): Path<String>, h: HeaderMap) -> Response {
    reply(
        async {
            if op == "registry" {
                return Ok(registry());
            }
            let u = auth(&app, &h).await?;
            if op == "zahltag" {
                let v = verdienst(&app, &u).await?;
                return app.game.lock().unwrap().claim(u.id, &v, false);
            }
            let mut g = app.game.lock().unwrap();
            g.ensure_user(&u)?;
            g.get(&u, &op)
        }
        .await,
    )
}

async fn api_post(State(app): State<App>, Path(op): Path<String>, h: HeaderMap, body: Bytes) -> Response {
    reply(
        async {
            if !same_origin(&h) {
                return no(403, "Anfrage von fremder Seite.");
            }
            let u = auth(&app, &h).await?;
            let b: Value = if body.is_empty() { Value::Null } else { serde_json::from_slice(&body).map_err(|_| E(400, "Ungültige Anfrage.".into()))? };
            if op == "zahltag" {
                let v = verdienst(&app, &u).await?;
                return app.game.lock().unwrap().claim(u.id, &v, true);
            }
            if op == "admin/ki_test" {
                // OpenRouter-Probe: Einstellungen unter der Sperre lesen, den Aufruf ohne Sperre abwarten (LAW 1), Ergebnis eintragen
                if !u.admin {
                    return no(403, "Nur für die Verwaltung.");
                }
                let thema = crate::duel_ai::normalize_topic(b.get("thema").and_then(Value::as_str).unwrap_or("Allgemeinwissen"));
                let cfg = app.game.lock().unwrap().ki_cfg();
                let res = crate::duel_ai::fetch(cfg, thema.clone(), String::new(), crate::duel_ai::VORAB_MS, |_| {}).await;
                return app.game.lock().unwrap().ki_test_done(&thema, res);
            }
            let mut g = app.game.lock().unwrap();
            g.ensure_user(&u)?;
            g.post(&u, &op, &b)
        }
        .await,
    )
}

async fn ws(State(app): State<App>, h: HeaderMap, up: WebSocketUpgrade) -> Response {
    if !same_origin(&h) {
        return reply(no(403, "Anfrage von fremder Seite."));
    }
    match auth(&app, &h).await {
        Ok(u) => up.max_message_size(1024).on_upgrade(move |sock| client(app, u, sock)),
        Err(e) => reply(Err(e)),
    }
}

async fn client(app: App, u: User, sock: WebSocket) {
    let (mut sink, mut stream) = sock.split();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Bytes>(1024);
    let joined = app.game.lock().unwrap().join(&u, tx);
    let session = match joined {
        Ok(s) => s,
        Err(E(_, m)) => {
            let _ = sink.send(Message::Binary(Bytes::from(proto::enc(&proto::S2C::Kick(m))))).await;
            return;
        }
    };
    let writer = tokio::spawn(async move {
        while let Some(b) = rx.recv().await {
            if sink.send(Message::Binary(b)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Binary(b) => {
                if let Some(m) = proto::dec::<C2S>(&b) {
                    let mut g = app.game.lock().unwrap();
                    if !g.players.get(&u.id).is_some_and(|p| p.session == session) {
                        break;
                    }
                    g.handle(u.id, m);
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    app.game.lock().unwrap().leave(u.id, session);
    writer.abort();
}

async fn page(State(app): State<App>, Path(path): Path<String>, h: HeaderMap, RawQuery(q): RawQuery) -> Response {
    let file = match path.as_str() {
        "" => "index.html",
        "hilfe" => "hilfe.html",
        "admin" => match auth(&app, &h).await {
            Ok(u) if u.admin => "admin.html",
            _ => return (StatusCode::FORBIDDEN, "Nur für die Verwaltung.").into_response(),
        },
        p => p,
    };
    if file.contains("..") || file.starts_with('/') || !file.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-/".contains(&c)) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let ext = file.rsplit('.').next().unwrap_or("");
    let ct = match ext {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let Ok(mut data) = tokio::fs::read(std::path::Path::new(&app.cfg.web).join(file)).await else { return StatusCode::NOT_FOUND.into_response() };
    if matches!(ext, "html" | "js") && !file.starts_with("pkg/") {
        data = String::from_utf8_lossy(&data).replace("__V__", &app.ver).into_bytes();
    }
    let cache = if q.is_some_and(|q| q.contains("v=")) { "public, max-age=31536000, immutable" } else { "no-cache" };
    let mut res = (StatusCode::OK, data).into_response();
    let hm = res.headers_mut();
    hm.insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
    hm.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    hm.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    hm.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    if ext == "html" {
        hm.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    }
    res
}

/// Nur vom selben Rechner und nicht über Caddy (der setzt X-Forwarded-For).
fn local(addr: &SocketAddr, h: &HeaderMap) -> bool {
    addr.ip().is_loopback() && !h.contains_key("x-forwarded-for")
}
async fn internal_delete(State(app): State<App>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Path(id): Path<u32>, h: HeaderMap) -> Response {
    if !local(&addr, &h) {
        return StatusCode::NOT_FOUND.into_response();
    }
    app.auth.lock().unwrap().clear();
    reply(app.game.lock().unwrap().delete_user(id))
}
async fn internal_export(State(app): State<App>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Path(id): Path<u32>, h: HeaderMap) -> Response {
    if !local(&addr, &h) {
        return StatusCode::NOT_FOUND.into_response();
    }
    reply(app.game.lock().unwrap().export_user(id))
}
