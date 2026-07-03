//! WebSocket reverse proxy.
//!
//! [`WsProxy`] listens for incoming TCP connections, reads the initial HTTP
//! request, verifies it is a WebSocket upgrade, connects to a backend, performs
//! the WebSocket handshake end-to-end, and then bidirectionally tunnels raw
//! WebSocket bytes between the client and the backend.
//!
//! Plain (`ws://`) backends use two threads (one per direction) via
//! `std::io::copy`, identical to the original implementation.
//!
//! TLS (`wss://`) backends use a single-thread polling loop: both streams are
//! set to a 5 ms read timeout and the loop alternates between the two
//! directions, sleeping 1 ms when neither side has data.  This avoids the
//! deadlock that arises when trying to share a `rustls::StreamOwned` between
//! two blocking threads.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::ws_proxy::WsProxy;
//!
//! // Plain WebSocket — two backends, round-robin.
//! WsProxy::new(["ws://chat-backend:9000", "ws://chat-backend:9001"])
//!     .connect_timeout_ms(3000)
//!     .bind("0.0.0.0:8080")
//!     .unwrap();
//!
//! // TLS WebSocket (requires http-client or http2 feature).
//! WsProxy::new(["wss://chat-backend.internal:443"])
//!     .bind("0.0.0.0:8080")
//!     .unwrap();
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use crate::request::Request;
use crate::websocket::WebSocket;

/// WebSocket reverse proxy with round-robin load balancing.
///
/// Accepts HTTP/1.1 WebSocket upgrade requests and tunnels traffic to one of the
/// configured backends.
///
/// Backend URL schemes:
/// - `"host:port"` — plain TCP (no scheme)
/// - `"ws://host:port"` — plain TCP (port defaults to 80)
/// - `"wss://host:port"` — TLS (port defaults to 443); requires the
///   `http-client` or `http2` Cargo feature
///
/// Call [`WsProxy::bind`] to start. It blocks the calling thread indefinitely.
pub struct WsProxy {
    backends: Vec<WsBackend>,
    counter: Arc<AtomicUsize>,
    connect_timeout: Duration,
    read_timeout: Duration,
}

impl WsProxy {
    /// Create a proxy that distributes connections across `backends` in
    /// round-robin order.
    ///
    /// Each entry may be `"host:port"`, `"ws://host:port"`, or
    /// `"wss://host:port"`.  `wss://` requires the `http-client` or `http2`
    /// Cargo feature.
    pub fn new<I, S>(backends: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        WsProxy {
            backends: backends
                .into_iter()
                .filter_map(|b| WsBackend::parse(&b.into()))
                .collect(),
            counter: Arc::new(AtomicUsize::new(0)),
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(30),
        }
    }

    /// Override the TCP connect timeout to each backend (default: 5 s).
    pub fn connect_timeout_ms(mut self, ms: u64) -> Self {
        self.connect_timeout = Duration::from_millis(ms);
        self
    }

    /// Override the idle read timeout on client connections (default: 30 s).
    ///
    /// For `wss://` backends this controls the outer idle timeout on the
    /// client side; the internal polling interval is fixed at 5 ms.
    pub fn read_timeout_ms(mut self, ms: u64) -> Self {
        self.read_timeout = Duration::from_millis(ms);
        self
    }

    /// Bind on `addr` and start proxying WebSocket connections. Blocks indefinitely.
    pub fn bind(self, addr: &str) -> Result<(), String> {
        if self.backends.is_empty() {
            return Err("WsProxy: no backends configured".to_string());
        }
        let listener = TcpListener::bind(addr)
            .map_err(|e| format!("WsProxy: bind on {} failed: {}", addr, e))?;
        println!("WsProxy: listening on {}", addr);
        let proxy = Arc::new(self);
        for incoming in listener.incoming() {
            let client = match incoming {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("WsProxy: accept error: {}", e);
                    continue;
                }
            };
            let p = Arc::clone(&proxy);
            std::thread::spawn(move || {
                if let Err(e) = p.handle(client) {
                    eprintln!("WsProxy: {}", e);
                }
            });
        }
        Ok(())
    }

    fn pick_backend(&self) -> &WsBackend {
        let i = self.counter.fetch_add(1, Ordering::Relaxed) % self.backends.len();
        &self.backends[i]
    }

    fn handle(&self, mut client: TcpStream) -> Result<(), String> {
        client.set_read_timeout(Some(self.read_timeout)).ok();

        // Read the initial HTTP request.
        let mut buf = vec![0u8; 8192];
        let n = client.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(());
        }

        let request = Request::parse(&buf[..n])
            .map_err(|e| format!("WsProxy: invalid HTTP request: {}", e))?;

        if !WebSocket::is_upgrade_request(&request) {
            let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
            return Err(format!(
                "WsProxy: not a WebSocket upgrade — method={}, uri={}",
                request.method, request.request_uri
            ));
        }

        let backend = self.pick_backend();
        let addr_str = &backend.addr;
        let sock_addr = addr_str
            .to_socket_addrs()
            .map_err(|e| format!("WsProxy: DNS lookup for {} failed: {}", addr_str, e))?
            .next()
            .ok_or_else(|| format!("WsProxy: no address for {}", addr_str))?;

        let tcp = TcpStream::connect_timeout(&sock_addr, self.connect_timeout)
            .map_err(|e| format!("WsProxy: connect to {} failed: {}", addr_str, e))?;

        // Use backend.host (no port) for the Host header and TLS SNI.
        let upgrade_req = build_upgrade_request(&request, &backend.host);

        if backend.tls {
            self.handle_tls(client, tcp, &request, &backend.host, upgrade_req, addr_str)
        } else {
            handle_plain(client, tcp, &request, upgrade_req, addr_str)
        }
    }

    fn handle_tls(
        &self,
        mut client: TcpStream,
        tcp: TcpStream,
        request: &Request,
        host: &str,
        upgrade_req: Vec<u8>,
        addr_str: &str,
    ) -> Result<(), String> {
        #[cfg(any(feature = "http-client", feature = "http2"))]
        {
            use rustls::pki_types::ServerName;
            use rustls::ClientConfig;
            use std::sync::Arc;

            let root_store = rustls::RootCertStore::from_iter(
                webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
            );
            let config = Arc::new(
                ClientConfig::builder()
                    .with_root_certificates(root_store)
                    .with_no_client_auth(),
            );
            let server_name = ServerName::try_from(host)
                .map_err(|e| format!("WsProxy: invalid hostname '{}': {}", host, e))?
                .to_owned();
            let conn = rustls::ClientConnection::new(config, server_name)
                .map_err(|e| format!("WsProxy: TLS init failed: {}", e))?;
            let mut tls = rustls::StreamOwned::new(conn, tcp);

            // Send WebSocket upgrade request over TLS.
            tls.write_all(&upgrade_req)
                .map_err(|e| format!("WsProxy: write upgrade to {} failed: {}", addr_str, e))?;

            // Read backend's 101 Switching Protocols.
            let mut resp_buf = vec![0u8; 4096];
            let m = tls
                .read(&mut resp_buf)
                .map_err(|e| format!("WsProxy: read 101 from {} failed: {}", addr_str, e))?;
            let preview = &resp_buf[..m.min(20)];
            if !preview.starts_with(b"HTTP/1.1 101") && !preview.starts_with(b"HTTP/1.0 101") {
                return Err(format!(
                    "WsProxy: backend {} did not send 101 (got {:?})",
                    addr_str,
                    std::str::from_utf8(&resp_buf[..m.min(80)]).unwrap_or("?")
                ));
            }

            // Forward 101 to client.
            let response_101 = WebSocket::handshake_response(request)?;
            let raw_101 = format_response_head(&response_101);
            client
                .write_all(&raw_101)
                .map_err(|e| format!("WsProxy: write 101 to client failed: {}", e))?;

            // Bidirectional relay via single-thread poll loop.
            // Set both sides to 5 ms polling timeout.
            tls.sock.set_read_timeout(Some(Duration::from_millis(5))).ok();
            client.set_read_timeout(Some(Duration::from_millis(5))).ok();
            relay_tls(client, tls);
            Ok(())
        }

        #[cfg(not(any(feature = "http-client", feature = "http2")))]
        {
            let _ = (tcp, request, host, upgrade_req, addr_str);
            let _ = client.write_all(
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n",
            );
            Err("WsProxy: wss:// upstreams require the http-client or http2 Cargo feature".to_string())
        }
    }
}

/// Relay over a plain TCP backend using two blocking threads.
fn handle_plain(
    mut client: TcpStream,
    mut backend: TcpStream,
    request: &Request,
    upgrade_req: Vec<u8>,
    addr_str: &str,
) -> Result<(), String> {
    backend
        .write_all(&upgrade_req)
        .map_err(|e| format!("WsProxy: write upgrade to {} failed: {}", addr_str, e))?;

    let mut resp_buf = vec![0u8; 4096];
    let m = backend
        .read(&mut resp_buf)
        .map_err(|e| format!("WsProxy: read 101 from {} failed: {}", addr_str, e))?;
    let preview = &resp_buf[..m.min(20)];
    if !preview.starts_with(b"HTTP/1.1 101") && !preview.starts_with(b"HTTP/1.0 101") {
        return Err(format!(
            "WsProxy: backend {} did not send 101 (got {:?})",
            addr_str,
            std::str::from_utf8(&resp_buf[..m.min(80)]).unwrap_or("?")
        ));
    }

    let response_101 = WebSocket::handshake_response(request)?;
    let raw_101 = format_response_head(&response_101);
    client
        .write_all(&raw_101)
        .map_err(|e| format!("WsProxy: write 101 to client failed: {}", e))?;

    // Bidirectional tunnel — one thread per direction.
    let mut client_r = client.try_clone().map_err(|e| e.to_string())?;
    let mut backend_r = backend.try_clone().map_err(|e| e.to_string())?;
    let mut client_w = client;
    let mut backend_w = backend;

    let t1 = std::thread::spawn(move || {
        std::io::copy(&mut client_r, &mut backend_w).ok();
        let _ = backend_w.shutdown(std::net::Shutdown::Write);
    });
    let t2 = std::thread::spawn(move || {
        std::io::copy(&mut backend_r, &mut client_w).ok();
        let _ = client_w.shutdown(std::net::Shutdown::Write);
    });

    let _ = t1.join();
    let _ = t2.join();
    Ok(())
}

/// Bidirectional relay between `client` (plain TCP) and a TLS backend.
///
/// Uses a single-thread polling loop to avoid the deadlock that arises when
/// sharing a `rustls::StreamOwned` between two blocking threads (the reader
/// thread would hold the TLS lock while waiting for data, blocking the writer).
///
/// Both streams are set to a 5 ms read timeout before this function is called.
/// The loop reads from each side in turn; when neither has data it sleeps 1 ms.
#[cfg(any(feature = "http-client", feature = "http2"))]
fn relay_tls(
    mut client: TcpStream,
    mut backend: rustls::StreamOwned<rustls::ClientConnection, TcpStream>,
) {
    use std::io::ErrorKind::{TimedOut, WouldBlock};
    let mut buf = [0u8; 8192];

    loop {
        let mut active = false;

        // client → TLS backend
        let cn = match client.read(&mut buf) {
            Ok(0) => break, // client closed
            Ok(n) => n,
            Err(ref e) if e.kind() == TimedOut || e.kind() == WouldBlock => 0,
            Err(_) => break,
        };
        if cn > 0 {
            if backend.write_all(&buf[..cn]).is_err() {
                break;
            }
            active = true;
        }

        // TLS backend → client
        let bn = match backend.read(&mut buf) {
            Ok(0) => break, // backend closed
            Ok(n) => n,
            Err(ref e) if e.kind() == TimedOut || e.kind() == WouldBlock => 0,
            Err(_) => break,
        };
        if bn > 0 {
            if client.write_all(&buf[..bn]).is_err() {
                break;
            }
            active = true;
        }

        if !active {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn build_upgrade_request(request: &Request, backend_host: &str) -> Vec<u8> {
    let mut req = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\n",
        request.method, request.request_uri, backend_host
    );
    for header in &request.headers {
        if header.name.to_lowercase() == "host" {
            continue;
        }
        req.push_str(&format!("{}: {}\r\n", header.name, header.value));
    }
    req.push_str("\r\n");
    req.into_bytes()
}

fn format_response_head(response: &crate::response::Response) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status_code, response.reason_phrase
    )
    .into_bytes();
    for h in &response.headers {
        out.extend_from_slice(h.name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(h.value.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out
}

// ── Backend URL parsing ───────────────────────────────────────────────────────

struct WsBackend {
    /// `"host:port"` — passed to `to_socket_addrs()` for TCP connect.
    addr: String,
    /// Bare hostname (no port) — used for the `Host` header and TLS SNI.
    host: String,
    /// `true` when the URL scheme was `wss://`.
    tls: bool,
}

impl WsBackend {
    fn parse(url: &str) -> Option<Self> {
        let (rest, tls, default_port) = if let Some(r) = url.strip_prefix("wss://") {
            (r, true, 443u16)
        } else if let Some(r) = url.strip_prefix("ws://") {
            (r, false, 80u16)
        } else {
            (url, false, 80u16)
        };

        // Drop any path component.
        let host_port = rest.split('/').next().unwrap_or(rest);

        let (host, port) = if let Some(colon) = host_port.rfind(':') {
            let port_str = &host_port[colon + 1..];
            if let Ok(p) = port_str.parse::<u16>() {
                (host_port[..colon].to_string(), p)
            } else {
                (host_port.to_string(), default_port)
            }
        } else {
            (host_port.to_string(), default_port)
        };

        if host.is_empty() {
            return None;
        }

        Some(WsBackend {
            addr: format!("{}:{}", host, port),
            host,
            tls,
        })
    }
}

// ── WsBackend::parse unit tests ───────────────────────────────────────────────

#[cfg(test)]
mod backend_parse_tests {
    use super::WsBackend;

    fn parse(url: &str) -> Option<(String, String, bool)> {
        WsBackend::parse(url).map(|b| (b.addr, b.host, b.tls))
    }

    #[test]
    fn bare_host_port() {
        assert_eq!(
            Some(("chat:9000".into(), "chat".into(), false)),
            parse("chat:9000")
        );
    }

    #[test]
    fn ws_scheme_plain() {
        assert_eq!(
            Some(("backend:3000".into(), "backend".into(), false)),
            parse("ws://backend:3000")
        );
    }

    #[test]
    fn ws_scheme_default_port() {
        assert_eq!(
            Some(("api.example.com:80".into(), "api.example.com".into(), false)),
            parse("ws://api.example.com")
        );
    }

    #[test]
    fn wss_scheme_sets_tls() {
        assert_eq!(
            Some(("secure.example.com:443".into(), "secure.example.com".into(), true)),
            parse("wss://secure.example.com")
        );
    }

    #[test]
    fn wss_scheme_explicit_port() {
        assert_eq!(
            Some(("secure.example.com:8443".into(), "secure.example.com".into(), true)),
            parse("wss://secure.example.com:8443")
        );
    }

    #[test]
    fn wss_default_port_is_443() {
        let b = WsBackend::parse("wss://api.example.com").unwrap();
        assert_eq!("api.example.com:443", b.addr);
        assert_eq!("api.example.com", b.host);
        assert!(b.tls);
    }

    #[test]
    fn ws_default_port_is_80() {
        let b = WsBackend::parse("ws://api.example.com").unwrap();
        assert_eq!("api.example.com:80", b.addr);
        assert!(!b.tls);
    }

    #[test]
    fn empty_host_returns_none() {
        assert_eq!(None, parse("wss://"));
    }

    #[test]
    fn bare_host_no_port_defaults_to_80() {
        assert_eq!(
            Some(("myhost:80".into(), "myhost".into(), false)),
            parse("myhost")
        );
    }

    #[test]
    fn path_component_is_ignored() {
        // URL paths after host:port are stripped — only host:port matters.
        let b = WsBackend::parse("ws://backend:9000/ws").unwrap();
        assert_eq!("backend:9000", b.addr);
        assert_eq!("backend", b.host);
    }
}
