//! Winziger HTTP-Abruf zum Lernhelfer auf demselben Rechner (HTTP/1.0, Verbindung schließt nach der Antwort).
use std::io;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn get(addr: &str, path: &str, cookie: Option<&str>) -> io::Result<(u16, String)> {
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "unlesbare Antwort");
    let fut = async {
        let mut s = tokio::net::TcpStream::connect(addr).await?;
        let c = cookie.map(|c| format!("Cookie: {c}\r\n")).unwrap_or_default();
        s.write_all(format!("GET {path} HTTP/1.0\r\nHost: {addr}\r\nAccept: application/json\r\n{c}\r\n").as_bytes()).await?;
        let mut buf = Vec::new();
        s.take(4_000_000).read_to_end(&mut buf).await?;
        io::Result::Ok(buf)
    };
    let buf = tokio::time::timeout(Duration::from_secs(5), fut).await.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Zeit abgelaufen"))??;
    let text = String::from_utf8_lossy(&buf);
    let (head, body) = text.split_once("\r\n\r\n").ok_or_else(bad)?;
    let status = head.split(' ').nth(1).and_then(|s| s.parse().ok()).ok_or_else(bad)?;
    Ok((status, body.to_string()))
}
