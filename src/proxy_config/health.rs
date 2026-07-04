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

/// Send a minimal HTTP(S)/1.1 GET request to `backend` at `path` with the
/// given `timeout`. Returns `true` on a 2xx response.
fn check_backend(backend: &str, path: &str, timeout: Duration) -> bool {
    let (host, port, tls) = match parse_backend_url(backend) {
        Some(t) => t,
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

    if tls {
        check_via_tls(stream, &host, req.as_bytes())
    } else {
        let mut stream = stream;
        if stream.write_all(req.as_bytes()).is_err() {
            return false;
        }
        let mut buf = [0u8; 16];
        if stream.read(&mut buf).is_err() {
            return false;
        }
        buf.starts_with(b"HTTP/1.1 2") || buf.starts_with(b"HTTP/1.0 2")
    }
}

#[cfg(any(feature = "http-client", feature = "http2"))]
fn check_via_tls(stream: TcpStream, host: &str, req: &[u8]) -> bool {
    use rustls::pki_types::ServerName;
    use rustls::ClientConfig;
    use std::sync::Arc;

    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    );
    let server_name = match ServerName::try_from(host.to_string()) {
        Ok(n) => n,
        Err(_) => return false,
    };
    let conn = match rustls::ClientConnection::new(config, server_name) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let mut tls = rustls::StreamOwned::new(conn, stream);
    if tls.write_all(req).is_err() {
        return false;
    }
    let mut buf = [0u8; 16];
    if tls.read(&mut buf).is_err() {
        return false;
    }
    buf.starts_with(b"HTTP/1.1 2") || buf.starts_with(b"HTTP/1.0 2")
}

// When rustls is not compiled in, silently skip TLS health checks.
#[cfg(not(any(feature = "http-client", feature = "http2")))]
fn check_via_tls(_stream: TcpStream, _host: &str, _req: &[u8]) -> bool {
    false
}

/// Parse a backend address that may include a scheme prefix.
///
/// Returns `(host, port, tls)`:
/// - `https://`, `wss://` → TLS=true, default port 443
/// - `http://`, `h2://`, `ws://`, or no scheme → TLS=false, default port 80
///
/// `ws://`/`wss://` are accepted (rather than requiring a caller to strip
/// them first) so this same function — and [`check_backend`] — can health
/// check `[[ws_proxy]]` backends, which are written with those schemes in
/// `rws.config.toml`; the TCP-connect-then-optionally-TLS-wrap check is
/// identical to an HTTP(S) backend once host/port/tls are known.
pub(crate) fn parse_backend_url(backend: &str) -> Option<(String, u16, bool)> {
    let (rest, tls, default_port) = if let Some(r) = backend.strip_prefix("https://") {
        (r, true, 443u16)
    } else if let Some(r) = backend.strip_prefix("wss://") {
        (r, true, 443u16)
    } else if let Some(r) = backend.strip_prefix("http://") {
        (r, false, 80u16)
    } else if let Some(r) = backend.strip_prefix("h2://") {
        (r, false, 80u16)
    } else if let Some(r) = backend.strip_prefix("ws://") {
        (r, false, 80u16)
    } else {
        (backend, false, 80u16)
    };

    // Drop any path component
    let host_port = rest.split('/').next().unwrap_or(rest);
    if host_port.is_empty() {
        return None;
    }

    // Handle IPv6 addresses like [::1]:8080
    let (host, port) = if host_port.starts_with('[') {
        // IPv6 literal: [host]:port or [host]
        let close = host_port.find(']')?;
        let host = host_port[1..close].to_string();
        let port = if host_port.len() > close + 1 && host_port.as_bytes()[close + 1] == b':' {
            host_port[close + 2..].parse::<u16>().unwrap_or(default_port)
        } else {
            default_port
        };
        (host, port)
    } else if let Some(colon) = host_port.rfind(':') {
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
    Some((host, port, tls))
}

#[cfg(test)]
mod tests {
    use super::parse_backend_url;

    #[test]
    fn https_scheme_is_tls_default_443() {
        assert_eq!(
            Some(("api.example.com".to_string(), 443, true)),
            parse_backend_url("https://api.example.com")
        );
    }

    #[test]
    fn http_scheme_is_plain_default_80() {
        assert_eq!(
            Some(("api.example.com".to_string(), 80, false)),
            parse_backend_url("http://api.example.com")
        );
    }

    #[test]
    fn h2_scheme_is_plain_with_explicit_port() {
        assert_eq!(
            Some(("grpc.example.com".to_string(), 9090, false)),
            parse_backend_url("h2://grpc.example.com:9090")
        );
    }

    #[test]
    fn wss_scheme_is_tls_default_443() {
        // wss:// backends (WsProxy) are health-checked with the same plain
        // HTTP GET as http(s):// backends — only the TLS-or-not decision
        // and default port depend on the scheme.
        assert_eq!(
            Some(("chat.example.com".to_string(), 443, true)),
            parse_backend_url("wss://chat.example.com")
        );
    }

    #[test]
    fn wss_scheme_explicit_port() {
        assert_eq!(
            Some(("chat.example.com".to_string(), 8443, true)),
            parse_backend_url("wss://chat.example.com:8443")
        );
    }

    #[test]
    fn ws_scheme_is_plain_default_80() {
        assert_eq!(
            Some(("chat.example.com".to_string(), 80, false)),
            parse_backend_url("ws://chat.example.com")
        );
    }

    #[test]
    fn ws_scheme_explicit_port() {
        assert_eq!(
            Some(("chat.example.com".to_string(), 9000, false)),
            parse_backend_url("ws://chat.example.com:9000")
        );
    }

    #[test]
    fn bare_host_port_no_scheme_is_plain() {
        assert_eq!(
            Some(("10.0.0.5".to_string(), 8080, false)),
            parse_backend_url("10.0.0.5:8080")
        );
    }

    #[test]
    fn ipv6_literal_with_port() {
        assert_eq!(
            Some(("::1".to_string(), 9000, false)),
            parse_backend_url("ws://[::1]:9000")
        );
    }

    #[test]
    fn path_component_is_dropped() {
        assert_eq!(
            Some(("chat.example.com".to_string(), 8080, false)),
            parse_backend_url("ws://chat.example.com:8080/ws")
        );
    }

    #[test]
    fn empty_host_returns_none() {
        assert_eq!(None, parse_backend_url("wss://"));
    }
}

