//! etcd v3 discovery — an initial full listing via the gRPC-gateway's
//! `/v3/kv/range` JSON/HTTP endpoint, then (once [`super::BackendPool::start`]
//! is called) a dedicated long-lived connection to `/v3/watch` applying
//! `PUT`/`DELETE` events to the backend list incrementally as they arrive.
//!
//! Reuses [`crate::ingress::watch::read_chunked_lines`] — the same
//! "chunked, newline-delimited JSON event stream" shape the Kubernetes
//! Ingress watcher already solved, just applied to etcd's protocol instead
//! of the Kubernetes API server's. Unlike that watcher (which treats every
//! event as a plain re-list trigger because a `WatchEvent` there doesn't
//! carry enough to update one cached object in isolation), etcd's watch
//! events *do* carry a complete key+value per event, so they're applied as
//! real incremental deltas against a local `key -> value` map here.
//!
//! Plain HTTP only — no TLS support yet (etcd deployments that require it
//! need a plaintext sidecar/proxy in front, same limitation the Kubernetes
//! Ingress watcher had before `.from_service_account()`).

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use super::json_lite::{self, JsonValue};

/// Runs the reconnect-forever watch loop. Never returns; intended to be the
/// body of a dedicated background thread (see `BackendPool::start`).
pub(super) fn watch_forever(endpoints: &[String], prefix: &str, pool: &super::BackendPool) {
    loop {
        match run_once(endpoints, prefix, pool) {
            Ok(()) => {}
            Err(e) => eprintln!("service_discovery: etcd watch error: {}", e),
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

fn run_once(endpoints: &[String], prefix: &str, pool: &super::BackendPool) -> Result<(), String> {
    let endpoint = endpoints.first().ok_or("no etcd endpoints configured")?;

    let mut state = kv_range(endpoint, prefix)?;
    pool.update(state.values().cloned().collect());

    let mut stream = TcpStream::connect(endpoint).map_err(|e| format!("connect to {} failed: {}", endpoint, e))?;
    stream.set_read_timeout(Some(Duration::from_secs(120))).ok();

    let create_body = format!(
        r#"{{"create_request":{{"key":"{}","range_end":"{}","prefix":true}}}}"#,
        base64_encode(prefix.as_bytes()),
        base64_encode(&prefix_range_end(prefix.as_bytes())),
    );
    let request = format!(
        "POST /v3/watch HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        endpoint,
        create_body.len(),
        create_body
    );
    stream.write_all(request.as_bytes()).map_err(|e| format!("write failed: {}", e))?;

    crate::ingress::watch::read_chunked_lines(&mut stream, |line| {
        if let Ok(parsed) = json_lite::parse(line) {
            apply_watch_line(&parsed, &mut state, pool);
        }
    })
}

fn apply_watch_line(parsed: &JsonValue, state: &mut HashMap<String, String>, pool: &super::BackendPool) {
    let Some(result) = parsed.get("result") else { return };
    let Some(events) = result.get("events").and_then(JsonValue::as_array) else { return };

    let mut changed = false;
    for event in events {
        let Some(kv) = event.get("kv") else { continue };
        let Some(key) = kv.get("key").and_then(JsonValue::as_str).and_then(base64_decode_to_string) else { continue };

        // etcd's JSON mapping omits the `type` field entirely for its
        // zero-value variant, which is PUT (proto3 enum default) — absence
        // means PUT, not "unknown".
        let event_type = event.get("type").and_then(JsonValue::as_str).unwrap_or("PUT");

        if event_type == "DELETE" {
            if state.remove(&key).is_some() {
                changed = true;
            }
        } else {
            let value = kv
                .get("value")
                .and_then(JsonValue::as_str)
                .and_then(base64_decode_to_string)
                .unwrap_or_default();
            if state.get(&key) != Some(&value) {
                state.insert(key, value);
                changed = true;
            }
        }
    }

    if changed {
        pool.update(state.values().cloned().collect());
    }
}

/// One-shot full listing via `POST /v3/kv/range`, keyed by the raw etcd key
/// (decoded from base64) so the watch loop can apply deltas against it.
pub(super) fn kv_range(endpoint: &str, prefix: &str) -> Result<HashMap<String, String>, String> {
    let body = format!(
        r#"{{"key":"{}","range_end":"{}"}}"#,
        base64_encode(prefix.as_bytes()),
        base64_encode(&prefix_range_end(prefix.as_bytes())),
    );

    let response_body = http_post(endpoint, "/v3/kv/range", &body)?;
    let parsed = json_lite::parse(&response_body).map_err(|e| format!("bad JSON from /v3/kv/range: {}", e))?;

    let mut map = HashMap::new();
    if let Some(kvs) = parsed.get("kvs").and_then(JsonValue::as_array) {
        for kv in kvs {
            let key = kv.get("key").and_then(JsonValue::as_str).and_then(base64_decode_to_string);
            let value = kv.get("value").and_then(JsonValue::as_str).and_then(base64_decode_to_string).unwrap_or_default();
            if let Some(key) = key {
                map.insert(key, value);
            }
        }
    }
    Ok(map)
}

/// A plain, non-persistent request/response POST — `Connection: close`, read
/// until EOF. Used only for the one-shot `/v3/kv/range` listing; the watch
/// stream in [`run_once`] keeps its own connection open instead.
fn http_post(endpoint: &str, path: &str, body: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(endpoint).map_err(|e| format!("connect to {} failed: {}", endpoint, e))?;
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();

    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path,
        endpoint,
        body.len(),
        body
    );
    stream.write_all(request.as_bytes()).map_err(|e| format!("write failed: {}", e))?;

    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => data.extend_from_slice(&buf[..n]),
            Err(e) => return Err(format!("read failed: {}", e)),
        }
    }

    let text = String::from_utf8_lossy(&data).to_string();
    let (header_str, body) = text.split_once("\r\n\r\n").ok_or("malformed HTTP response")?;
    let status_line = header_str.lines().next().unwrap_or("");
    let status: u16 = status_line.splitn(3, ' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    if !(200..300).contains(&status) {
        return Err(format!("{} returned status {}", path, status));
    }
    Ok(body.to_string())
}

/// Standard etcd "prefix range end": increment the last byte that isn't
/// `0xff`, dropping any trailing `0xff` bytes first. An all-`0xff` (or
/// empty) prefix means "no upper bound", encoded as a single `\0` byte —
/// mirrors etcd's own `clientv3.GetPrefixRangeEnd`.
fn prefix_range_end(prefix: &[u8]) -> Vec<u8> {
    let mut end = prefix.to_vec();
    while let Some(&last) = end.last() {
        if last < 0xff {
            *end.last_mut().unwrap() = last + 1;
            return end;
        }
        end.pop();
    }
    vec![0]
}

fn base64_decode_to_string(s: &str) -> Option<String> {
    base64_decode(s).map(|bytes| String::from_utf8_lossy(&bytes).to_string())
}

/// `crate::websocket::base64_encode` already implements standard
/// (non-URL-safe, padded) base64 encoding — etcd's gRPC-gateway JSON mapping
/// for protobuf `bytes` fields uses that same alphabet, so it's reused
/// as-is rather than re-implementing an encoder this module would otherwise
/// need only for symmetry with its own decoder.
fn base64_encode(input: &[u8]) -> String {
    crate::websocket::base64_encode(input)
}

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let input = input.trim_end_matches('=');
    let mut out = Vec::with_capacity(input.len() * 3 / 4 + 3);
    let mut buffer = 0u32;
    let mut bits = 0u32;

    for c in input.bytes() {
        let value = BASE64_CHARS.iter().position(|&b| b == c)? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }

    Some(out)
}
