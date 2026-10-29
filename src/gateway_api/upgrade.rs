//! Upgraded connections (WebSocket) through a Gateway route: the request
//! goes to the backend over its own connection; a `101` hands the client's
//! connection to a byte relay with the backend, anything else is answered
//! as a normal response.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::core::New;
use crate::header::Header;
use crate::request::Request;
use crate::response::Response;
use crate::websocket::WebSocket;

/// The most a backend's response head may take.
const MAX_HEAD: usize = 64 * 1024;

/// Whether the request asks to switch protocols (`Connection: upgrade`
/// and an `Upgrade` header).
pub fn is_upgrade(request: &Request) -> bool {
    let connection = request.header("connection").map(|h| h.value.to_ascii_lowercase()).unwrap_or_default();
    request.header("upgrade").is_some() && connection.split(',').any(|t| t.trim() == "upgrade")
}

/// The request head as sent to the backend: the client's headers (with
/// `Host` = `host`) plus `X-Forwarded-For` and, unless the client sent one,
/// `X-Forwarded-Host` (the client's own `Host`).
pub fn request_head(request: &Request, client_ip: &str, host: &str) -> Vec<u8> {
    let mut head = format!("{} {} HTTP/1.1\r\nHost: {host}\r\n", request.method, request.request_uri);
    for header in &request.headers {
        if header.name.eq_ignore_ascii_case("host") || header.name.eq_ignore_ascii_case("content-length") {
            continue;
        }
        head.push_str(&format!("{}: {}\r\n", header.name, header.value));
    }
    head.push_str(&format!("X-Forwarded-For: {client_ip}\r\n"));
    if request.header("X-Forwarded-Host").is_none() {
        if let Some(original) = request.header("host").filter(|h| !h.value.is_empty()) {
            head.push_str(&format!("X-Forwarded-Host: {}\r\n", original.value));
        }
    }
    if !request.body.is_empty() {
        head.push_str(&format!("Content-Length: {}\r\n", request.body.len()));
    }
    head.push_str("\r\n");
    head.into_bytes()
}

/// Forward an upgrade request to `ip:port`.
pub fn forward(request: &Request, client_ip: &str, ip: &str, port: u16, host: &str, timeout: Duration) -> Result<Response, String> {
    let address = (ip, port).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let mut backend = TcpStream::connect_timeout(&address, timeout.min(Duration::from_secs(10))).map_err(|e| e.to_string())?;
    backend.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    backend.write_all(&request_head(request, client_ip, host)).map_err(|e| e.to_string())?;
    backend.write_all(&request.body).map_err(|e| e.to_string())?;

    let mut received = Vec::new();
    let head_end = loop {
        if let Some(at) = received.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        if received.len() > MAX_HEAD {
            return Err("backend response head too large".to_string());
        }
        let mut buffer = [0u8; 8192];
        let n = backend.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("backend closed before responding".to_string());
        }
        received.extend_from_slice(&buffer[..n]);
    };
    let head = String::from_utf8_lossy(&received[..head_end]).to_string();
    let status: i16 = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or("bad status line")?;
    if status != 101 {
        // Not switching: read the body it announces, and answer normally.
        let length = head
            .lines()
            .find_map(|l| l.split_once(':').filter(|(n, _)| n.trim().eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.trim().parse::<usize>().ok()))
            .unwrap_or(0);
        while received.len() < head_end + length {
            let mut buffer = [0u8; 8192];
            match backend.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => received.extend_from_slice(&buffer[..n]),
            }
        }
        return Ok(crate::proxy::normalize_upstream_response(Response::parse(&received)?));
    }
    let mut response = Response::new();
    response.status_code = 101;
    response.reason_phrase = "Switching Protocols".to_string();
    response.headers = head
        .lines()
        .skip(1)
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| Header::new(n.trim().to_string(), v.trim().to_string()))
        .collect();
    let leftover = received[head_end..].to_vec();
    backend.set_read_timeout(None).map_err(|e| e.to_string())?;
    let to_backend = backend.try_clone().map_err(|e| e.to_string())?;
    Ok(WebSocket::upgrade_raw(response, move |client_reader, client_writer| relay(client_reader, client_writer, backend, to_backend, leftover)))
}

/// Copy bytes both ways: backend → client here until the backend closes;
/// client → backend on a thread that ends when the client closes.
fn relay(mut client_reader: Box<dyn Read + Send>, mut client_writer: Box<dyn Write + Send>, mut from_backend: TcpStream, mut to_backend: TcpStream, leftover: Vec<u8>) {
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut client_reader, &mut to_backend);
        let _ = to_backend.shutdown(Shutdown::Write);
    });
    if client_writer.write_all(&leftover).is_ok() {
        let _ = std::io::copy(&mut from_backend, &mut client_writer);
    }
    let _ = client_writer.flush();
    let _ = from_backend.shutdown(Shutdown::Both);
}

#[cfg(test)]
mod tests;
