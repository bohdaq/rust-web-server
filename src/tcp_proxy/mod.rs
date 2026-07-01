//! Layer-4 TCP proxy.
//!
//! [`TcpProxy`] accepts raw TCP connections and forwards them to backend servers,
//! bidirectionally tunneling bytes with one thread per direction.
//!
//! Unlike [`crate::proxy::ReverseProxy`] (which operates at the HTTP layer),
//! `TcpProxy` is protocol-agnostic: any TCP-based protocol (database wire formats,
//! custom binary protocols, raw TLS passthrough) is forwarded unchanged.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::tcp_proxy::TcpProxy;
//!
//! // Proxy raw TCP on port 5432 across two PostgreSQL backends.
//! TcpProxy::new(["backend-1:5432", "backend-2:5432"])
//!     .connect_timeout_ms(3000)
//!     .bind("0.0.0.0:5432")
//!     .unwrap();
//! ```

use std::io;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

/// Layer-4 (raw TCP) reverse proxy with round-robin load balancing.
///
/// Call [`TcpProxy::bind`] to start accepting connections. Each connection is
/// handled in its own thread pair (one thread per direction), so `bind` blocks
/// the calling thread indefinitely.
pub struct TcpProxy {
    backends: Vec<String>,
    counter: Arc<AtomicUsize>,
    connect_timeout: Duration,
}

impl TcpProxy {
    /// Create a proxy that distributes connections across `backends` in
    /// round-robin order. Each entry must be `"host:port"`.
    pub fn new<I, S>(backends: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        TcpProxy {
            backends: backends.into_iter().map(|b| b.into()).collect(),
            counter: Arc::new(AtomicUsize::new(0)),
            connect_timeout: Duration::from_secs(5),
        }
    }

    /// Override the TCP connect timeout to each backend (default: 5 s).
    pub fn connect_timeout_ms(mut self, ms: u64) -> Self {
        self.connect_timeout = Duration::from_millis(ms);
        self
    }

    /// Bind on `addr` and start proxying. Blocks until the listener is closed.
    pub fn bind(self, addr: &str) -> Result<(), String> {
        if self.backends.is_empty() {
            return Err("TcpProxy: no backends configured".to_string());
        }
        let listener = TcpListener::bind(addr)
            .map_err(|e| format!("TcpProxy: bind on {} failed: {}", addr, e))?;
        println!("TcpProxy: listening on {}", addr);
        let proxy = Arc::new(self);
        for incoming in listener.incoming() {
            let client = match incoming {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("TcpProxy: accept error: {}", e);
                    continue;
                }
            };
            let p = Arc::clone(&proxy);
            std::thread::spawn(move || {
                if let Err(e) = p.relay(client) {
                    eprintln!("TcpProxy: relay error: {}", e);
                }
            });
        }
        Ok(())
    }

    fn pick_backend(&self) -> &str {
        let i = self.counter.fetch_add(1, Ordering::Relaxed) % self.backends.len();
        &self.backends[i]
    }

    fn relay(&self, client: TcpStream) -> Result<(), String> {
        let addr_str = self.pick_backend().to_string();
        let sock_addr = addr_str
            .to_socket_addrs()
            .map_err(|e| format!("DNS lookup for {} failed: {}", addr_str, e))?
            .next()
            .ok_or_else(|| format!("no address resolved for {}", addr_str))?;

        let backend = TcpStream::connect_timeout(&sock_addr, self.connect_timeout)
            .map_err(|e| format!("TcpProxy: connect to {} failed: {}", addr_str, e))?;

        let mut client_r = client.try_clone().map_err(|e| e.to_string())?;
        let mut backend_r = backend.try_clone().map_err(|e| e.to_string())?;
        let mut client_w = client;
        let mut backend_w = backend;

        let t1 = std::thread::spawn(move || {
            io::copy(&mut client_r, &mut backend_w).ok();
            let _ = backend_w.shutdown(std::net::Shutdown::Write);
        });
        let t2 = std::thread::spawn(move || {
            io::copy(&mut backend_r, &mut client_w).ok();
            let _ = client_w.shutdown(std::net::Shutdown::Write);
        });

        let _ = t1.join();
        let _ = t2.join();
        Ok(())
    }
}
