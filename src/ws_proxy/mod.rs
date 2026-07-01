//! WebSocket reverse proxy.
//!
//! [`WsProxy`] listens for incoming TCP connections, reads the initial HTTP
//! request, verifies it is a WebSocket upgrade, connects to a backend, performs
//! the WebSocket handshake end-to-end, and then bidirectionally tunnels raw
//! WebSocket bytes between the client and the backend.
//!
//! Two threads handle each live connection (one per direction), so neither side
//! is blocked waiting for the other.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::ws_proxy::WsProxy;
//!
//! // All WebSocket connections on port 8080 are forwarded to a chat backend.
//! WsProxy::new(["chat-backend:9000", "chat-backend:9001"])
//!     .connect_timeout_ms(3000)
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
/// Accepts plain HTTP/1.1 WebSocket upgrade requests, tunnels the handshake to
/// a backend, and relays all subsequent frames bidirectionally.
///
/// For TLS-terminated WebSocket proxying, place a TLS terminator in front (e.g.
/// another rws instance with TLS configured) and point it at this proxy.
///
/// Call [`WsProxy::bind`] to start. It blocks the calling thread indefinitely.
pub struct WsProxy {
    backends: Vec<String>,
    counter: Arc<AtomicUsize>,
    connect_timeout: Duration,
    read_timeout: Duration,
}

impl WsProxy {
    /// Create a proxy that distributes connections across `backends` in
    /// round-robin order. Each entry must be `"host:port"`.
    pub fn new<I, S>(backends: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        WsProxy {
            backends: backends.into_iter().map(|b| b.into()).collect(),
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

    fn pick_backend(&self) -> &str {
        let i = self.counter.fetch_add(1, Ordering::Relaxed) % self.backends.len();
        &self.backends[i]
    }

    fn handle(&self, mut client: TcpStream) -> Result<(), String> {
        client.set_read_timeout(Some(self.read_timeout)).ok();

        // Read the initial HTTP request
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

        // Connect to backend
        let backend_str = self.pick_backend().to_string();
        let backend_sock = backend_str
            .to_socket_addrs()
            .map_err(|e| format!("WsProxy: DNS lookup for {} failed: {}", backend_str, e))?
            .next()
            .ok_or_else(|| format!("WsProxy: no address for {}", backend_str))?;

        let mut backend = TcpStream::connect_timeout(&backend_sock, self.connect_timeout)
            .map_err(|e| format!("WsProxy: connect to {} failed: {}", backend_str, e))?;

        // Forward the HTTP upgrade request to the backend
        let upgrade_req = build_upgrade_request(&request, &backend_str);
        backend
            .write_all(&upgrade_req)
            .map_err(|e| format!("WsProxy: write upgrade to backend failed: {}", e))?;

        // Read backend's 101 response
        let mut resp_buf = vec![0u8; 4096];
        let m = backend
            .read(&mut resp_buf)
            .map_err(|e| format!("WsProxy: read 101 from backend failed: {}", e))?;
        let resp_preview = &resp_buf[..m.min(20)];
        if !resp_preview.starts_with(b"HTTP/1.1 101") && !resp_preview.starts_with(b"HTTP/1.0 101") {
            return Err(format!(
                "WsProxy: backend {} did not send 101 (got {:?})",
                backend_str,
                std::str::from_utf8(&resp_buf[..m.min(80)]).unwrap_or("?")
            ));
        }

        // Send 101 Switching Protocols to the client
        let response_101 = WebSocket::handshake_response(&request)?;
        let raw_101 = format_response_head(&response_101);
        client
            .write_all(&raw_101)
            .map_err(|e| format!("WsProxy: write 101 to client failed: {}", e))?;

        // Bidirectional byte tunnel — two threads, one per direction
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
}

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
