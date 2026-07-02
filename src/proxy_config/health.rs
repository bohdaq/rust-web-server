//! Background health-checker for upstream backends.
//!
//! Each `[[upstream]]` with a `[upstream.health_check]` section gets a
//! dedicated background thread that periodically sends `GET {path}` to every
//! backend and updates the shared `Arc<RwLock<Vec<String>>>` live-backend list.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::proxy_config::HealthCheckConfig;

/// Start a background health-checker thread.
///
/// The thread runs until the process exits. It periodically checks every
/// backend in `backends` by sending `GET {config.path} HTTP/1.1` and tracking
/// consecutive successes/failures. The `live` list is updated accordingly.
pub(crate) fn start_health_checker(
    upstream_name: String,
    backends: Vec<String>,
    live: Arc<RwLock<Vec<String>>>,
    config: HealthCheckConfig,
) {
    std::thread::Builder::new()
        .name(format!("health-{}", upstream_name))
        .spawn(move || {
            let interval = Duration::from_secs(config.interval_secs);
            let timeout = Duration::from_millis(config.timeout_ms);
            // Per-backend consecutive success/failure counters
            let mut successes: Vec<u32> = vec![0; backends.len()];
            let mut failures: Vec<u32> = vec![0; backends.len()];
            // Initial state: all backends considered alive
            let mut is_live: Vec<bool> = vec![true; backends.len()];

            loop {
                std::thread::sleep(interval);

                for (i, backend) in backends.iter().enumerate() {
                    let ok = check_backend(backend, &config.path, timeout);
                    if ok {
                        successes[i] += 1;
                        failures[i] = 0;
                        // Restore if we have enough consecutive successes
                        if !is_live[i] && successes[i] >= config.healthy_threshold {
                            is_live[i] = true;
                            eprintln!(
                                "[health] upstream={} backend={} restored ({}x ok)",
                                upstream_name, backend, successes[i]
                            );
                        }
                    } else {
                        failures[i] += 1;
                        successes[i] = 0;
                        // Remove if we have enough consecutive failures
                        if is_live[i] && failures[i] >= config.unhealthy_threshold {
                            is_live[i] = false;
                            eprintln!(
                                "[health] upstream={} backend={} removed ({}x fail)",
                                upstream_name, backend, failures[i]
                            );
                        }
                    }
                }

                // Update the live list
                let live_list: Vec<String> = backends
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| is_live[*i])
                    .map(|(_, b)| b.clone())
                    .collect();
                if let Ok(mut guard) = live.write() {
                    *guard = live_list;
                }
            }
        })
        .ok();
}

/// Send a minimal HTTP/1.1 GET request to `backend` (host:port) at `path`
/// with the given `timeout`. Returns `true` on a 2xx response.
fn check_backend(backend: &str, path: &str, timeout: Duration) -> bool {
    // Parse host:port
    let (host, port) = match parse_host_port(backend) {
        Some(hp) => hp,
        None => return false,
    };

    let addr_str = format!("{}:{}", host, port);
    let sock_addr = match addr_str.to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => return false,
    };

    let stream = match TcpStream::connect_timeout(&sock_addr, timeout) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host
    );

    let mut stream = stream;
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }

    // Read just the status line
    let mut buf = [0u8; 16];
    if stream.read(&mut buf).is_err() {
        return false;
    }

    // Expect "HTTP/1.1 2" at the start
    buf.starts_with(b"HTTP/1.1 2") || buf.starts_with(b"HTTP/1.0 2")
}

pub(crate) fn parse_host_port(backend: &str) -> Option<(String, u16)> {
    // Strip scheme prefixes
    let rest = backend
        .strip_prefix("https://")
        .or_else(|| backend.strip_prefix("http://"))
        .or_else(|| backend.strip_prefix("h2://"))
        .unwrap_or(backend);
    // Drop any path component
    let host_port = rest.split('/').next().unwrap_or(rest);
    if let Some(colon) = host_port.rfind(':') {
        let port_str = &host_port[colon + 1..];
        if let Ok(p) = port_str.parse::<u16>() {
            return Some((host_port[..colon].to_string(), p));
        }
    }
    // Default port 80
    if !host_port.is_empty() {
        Some((host_port.to_string(), 80))
    } else {
        None
    }
}
