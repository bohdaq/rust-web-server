//! Layer-4 UDP proxy.
//!
//! [`UdpProxy`] receives UDP datagrams from clients and forwards each one to a
//! backend server, then relays the backend's reply to the original sender.
//! This request-reply model covers protocols such as DNS, NTP, and RADIUS.
//!
//! Each datagram is handled in its own thread, so the main `bind` loop is
//! never blocked waiting for a backend reply.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::udp_proxy::UdpProxy;
//!
//! // Forward DNS queries round-robin across two resolvers.
//! UdpProxy::new(["8.8.8.8:53", "8.8.4.4:53"])
//!     .reply_timeout_ms(2000)
//!     .bind("0.0.0.0:53")
//!     .unwrap();
//! ```

use std::net::{ToSocketAddrs, UdpSocket};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

/// Layer-4 (raw UDP) reverse proxy with round-robin load balancing.
///
/// Each client datagram is forwarded to one backend; the backend's reply is
/// delivered back to the originating client address. No session state is kept
/// between datagrams.
///
/// Call [`UdpProxy::bind`] to start. It blocks the calling thread indefinitely.
pub struct UdpProxy {
    backends: Vec<String>,
    counter: Arc<AtomicUsize>,
    reply_timeout: Duration,
    buffer_size: usize,
}

impl UdpProxy {
    /// Create a proxy that distributes datagrams across `backends` in
    /// round-robin order. Each entry must be `"host:port"`.
    pub fn new<I, S>(backends: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        UdpProxy {
            backends: backends.into_iter().map(|b| b.into()).collect(),
            counter: Arc::new(AtomicUsize::new(0)),
            reply_timeout: Duration::from_secs(5),
            buffer_size: 65536,
        }
    }

    /// Override the timeout waiting for a backend reply (default: 5 s).
    pub fn reply_timeout_ms(mut self, ms: u64) -> Self {
        self.reply_timeout = Duration::from_millis(ms);
        self
    }

    /// Override the per-datagram buffer size (default: 65 536 B).
    pub fn buffer_size(mut self, bytes: usize) -> Self {
        self.buffer_size = bytes;
        self
    }

    /// Bind on `addr` and start forwarding datagrams. Blocks indefinitely.
    pub fn bind(self, addr: &str) -> Result<(), String> {
        if self.backends.is_empty() {
            return Err("UdpProxy: no backends configured".to_string());
        }
        let socket = UdpSocket::bind(addr)
            .map_err(|e| format!("UdpProxy: bind on {} failed: {}", addr, e))?;
        println!("UdpProxy: listening on {}", addr);
        let proxy = Arc::new(self);

        loop {
            let mut buf = vec![0u8; proxy.buffer_size];
            let (n, client_addr) = match socket.recv_from(&mut buf) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("UdpProxy: recv_from error: {}", e);
                    continue;
                }
            };
            let packet = buf[..n].to_vec();
            let backend_addr = proxy.pick_backend().to_string();
            let reply_socket = match socket.try_clone() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("UdpProxy: socket clone error: {}", e);
                    continue;
                }
            };
            let timeout = proxy.reply_timeout;
            let buf_size = proxy.buffer_size;

            std::thread::spawn(move || {
                let backend_sock_addr = match backend_addr.to_socket_addrs() {
                    Ok(mut a) => match a.next() {
                        Some(addr) => addr,
                        None => {
                            eprintln!("UdpProxy: no address for {}", backend_addr);
                            return;
                        }
                    },
                    Err(e) => {
                        eprintln!("UdpProxy: DNS lookup for {} failed: {}", backend_addr, e);
                        return;
                    }
                };

                let backend = match UdpSocket::bind("0.0.0.0:0") {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("UdpProxy: ephemeral socket error: {}", e);
                        return;
                    }
                };
                let _ = backend.set_read_timeout(Some(timeout));

                if let Err(e) = backend.send_to(&packet, backend_sock_addr) {
                    eprintln!("UdpProxy: send to {} failed: {}", backend_addr, e);
                    return;
                }

                let mut reply = vec![0u8; buf_size];
                match backend.recv_from(&mut reply) {
                    Ok((m, _)) => {
                        let _ = reply_socket.send_to(&reply[..m], client_addr);
                    }
                    Err(e) if e.kind() != std::io::ErrorKind::WouldBlock
                           && e.kind() != std::io::ErrorKind::TimedOut => {
                        eprintln!("UdpProxy: backend reply error from {}: {}", backend_addr, e);
                    }
                    _ => {} // timeout — backend didn't reply in time, drop silently
                }
            });
        }
    }

    fn pick_backend(&self) -> &str {
        let i = self.counter.fetch_add(1, Ordering::Relaxed) % self.backends.len();
        &self.backends[i]
    }
}
