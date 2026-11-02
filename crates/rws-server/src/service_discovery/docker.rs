//! Docker Engine API discovery — queries the local Docker daemon (over its
//! Unix domain socket, `/var/run/docker.sock` by default) for running
//! containers carrying a given label, using each container's *value* for
//! that label as the backend address directly, e.g. a container labeled
//! `rws.backend=10.0.0.5:8080`.
//!
//! Deliberately **not** "guess the address from published ports" — a
//! container can be reachable via its internal bridge-network IP, a
//! published host port, an overlay-network IP, or a reverse-proxy sidecar,
//! and there's no single correct answer without knowing the deployment
//! topology. Requiring an explicit label value sidesteps that ambiguity
//! entirely (the same tradeoff Traefik's and Caddy's simplest Docker
//! integrations make) and keeps this source's parsing logic trivial and
//! fully unit-testable.
//!
//! Unix-only (`std::os::unix::net::UnixStream`) — Docker Desktop on Windows
//! exposes a named pipe instead, which is out of scope here; `discover()`
//! logs a warning and returns empty on non-Unix targets.

#[cfg(test)]
mod tests;

use super::json_lite::{self, JsonValue};

#[cfg(unix)]
pub(super) fn discover(socket_path: &str, label: &str) -> Vec<String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let mut stream = match UnixStream::connect(socket_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("service_discovery: cannot connect to Docker socket {}: {}", socket_path, e);
            return Vec::new();
        }
    };

    let filters = format!(r#"{{"label":["{}"]}}"#, label);
    let encoded_filters = crate::url::URL::percent_encode(&filters);
    let request = format!(
        "GET /containers/json?filters={} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nAccept: application/json\r\n\r\n",
        encoded_filters
    );

    if let Err(e) = stream.write_all(request.as_bytes()) {
        eprintln!("service_discovery: Docker socket write failed: {}", e);
        return Vec::new();
    }

    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => data.extend_from_slice(&buf[..n]),
            Err(e) => {
                eprintln!("service_discovery: Docker socket read failed: {}", e);
                return Vec::new();
            }
        }
    }

    parse_http_response(&data, label)
}

#[cfg(not(unix))]
pub(super) fn discover(_socket_path: &str, _label: &str) -> Vec<String> {
    eprintln!("service_discovery: Docker discovery requires a Unix domain socket and is not supported on this platform");
    Vec::new()
}

/// Splits headers from body and, if `Transfer-Encoding: chunked`, decodes
/// the body before parsing — the Docker Engine API uses chunked responses
/// for some endpoints/versions even for a small, complete JSON array.
fn parse_http_response(data: &[u8], label: &str) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    let Some((header_str, body_str)) = text.split_once("\r\n\r\n") else {
        eprintln!("service_discovery: malformed HTTP response from Docker socket");
        return Vec::new();
    };

    let status_line = header_str.lines().next().unwrap_or("");
    let status: u16 = status_line.splitn(3, ' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    if !(200..300).contains(&status) {
        eprintln!("service_discovery: Docker API returned status {}", status);
        return Vec::new();
    }

    let is_chunked = header_str
        .lines()
        .any(|l| l.split_once(':').is_some_and(|(k, v)| k.trim().eq_ignore_ascii_case("transfer-encoding") && v.to_lowercase().contains("chunked")));

    let body_bytes = data[header_str.len() + 4..].to_vec();
    let body = if is_chunked {
        String::from_utf8_lossy(&decode_chunked(&body_bytes)).to_string()
    } else {
        body_str.to_string()
    };

    parse_containers(&body, label)
}

fn decode_chunked(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < bytes.len() {
        let Some(line_end) = bytes[pos..].windows(2).position(|w| w == b"\r\n").map(|p| pos + p) else { break };
        let size_line = String::from_utf8_lossy(&bytes[pos..line_end]);
        let size_str = size_line.split(';').next().unwrap_or("").trim();
        let Ok(size) = usize::from_str_radix(size_str, 16) else { break };
        if size == 0 {
            break;
        }
        let chunk_start = line_end + 2;
        let chunk_end = chunk_start + size;
        if chunk_end > bytes.len() {
            break;
        }
        out.extend_from_slice(&bytes[chunk_start..chunk_end]);
        pos = chunk_end + 2; // skip trailing \r\n after the chunk data
    }
    out
}

fn parse_containers(body: &str, label: &str) -> Vec<String> {
    let parsed = match json_lite::parse(body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("service_discovery: failed to parse Docker API response: {}", e);
            return Vec::new();
        }
    };

    let Some(containers) = parsed.as_array() else {
        eprintln!("service_discovery: Docker API response was not a JSON array");
        return Vec::new();
    };

    containers
        .iter()
        .filter_map(|c| container_backend(c, label))
        .collect()
}

fn container_backend(container: &JsonValue, label: &str) -> Option<String> {
    let value = container.get("Labels")?.get(label)?.as_str()?;
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
