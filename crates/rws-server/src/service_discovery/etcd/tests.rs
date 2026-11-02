use super::*;
use crate::service_discovery::BackendPool;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

// ── prefix_range_end ──────────────────────────────────────────────────────────

#[test]
fn prefix_range_end_increments_last_byte() {
    assert_eq!(vec![b'a', b'c'], prefix_range_end(b"ab"));
}

#[test]
fn prefix_range_end_drops_trailing_0xff_bytes() {
    assert_eq!(vec![b'a' + 1], prefix_range_end(&[b'a', 0xff, 0xff]));
}

#[test]
fn prefix_range_end_all_0xff_yields_no_upper_bound() {
    assert_eq!(vec![0u8], prefix_range_end(&[0xff, 0xff]));
}

#[test]
fn prefix_range_end_empty_yields_no_upper_bound() {
    assert_eq!(vec![0u8], prefix_range_end(&[]));
}

// ── base64 round-trip ─────────────────────────────────────────────────────────

#[test]
fn base64_round_trips() {
    for input in ["", "a", "ab", "abc", "abcd", "/services/api/1"] {
        let encoded = base64_encode(input.as_bytes());
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(input.as_bytes().to_vec(), decoded, "round-trip failed for {:?}", input);
    }
}

#[test]
fn base64_decode_to_string_works() {
    let encoded = base64_encode(b"10.0.0.5:8080");
    assert_eq!(Some("10.0.0.5:8080".to_string()), base64_decode_to_string(&encoded));
}

#[test]
fn base64_decode_rejects_invalid_characters() {
    assert!(base64_decode("not valid base64!!").is_none());
}

// ── apply_watch_line ──────────────────────────────────────────────────────────

fn kv_event(event_type: Option<&str>, key: &str, value: Option<&str>) -> String {
    let key_b64 = base64_encode(key.as_bytes());
    let type_field = event_type.map(|t| format!(r#""type":"{}","#, t)).unwrap_or_default();
    let value_field = value.map(|v| format!(r#","value":"{}""#, base64_encode(v.as_bytes()))).unwrap_or_default();
    format!(r#"{{"result":{{"events":[{{{}"kv":{{"key":"{}"{}}}}}]}}}}"#, type_field, key_b64, value_field)
}

#[test]
fn apply_watch_line_put_without_type_field_defaults_to_put() {
    let mut state = HashMap::new();
    let pool = BackendPool::r#static(vec![]);
    let line = kv_event(None, "/services/api/1", Some("10.0.0.5:8080"));
    let parsed = json_lite::parse(&line).unwrap();

    apply_watch_line(&parsed, &mut state, &pool);

    assert_eq!(Some(&"10.0.0.5:8080".to_string()), state.get("/services/api/1"));
    assert_eq!(vec!["10.0.0.5:8080".to_string()], pool.backends());
}

#[test]
fn apply_watch_line_explicit_put_updates_state() {
    let mut state = HashMap::new();
    let pool = BackendPool::r#static(vec![]);
    let line = kv_event(Some("PUT"), "/services/api/1", Some("10.0.0.5:8080"));
    let parsed = json_lite::parse(&line).unwrap();

    apply_watch_line(&parsed, &mut state, &pool);
    assert_eq!(Some(&"10.0.0.5:8080".to_string()), state.get("/services/api/1"));
}

#[test]
fn apply_watch_line_delete_removes_key() {
    let mut state = HashMap::new();
    state.insert("/services/api/1".to_string(), "10.0.0.5:8080".to_string());
    let pool = BackendPool::r#static(vec!["10.0.0.5:8080".to_string()]);

    let line = kv_event(Some("DELETE"), "/services/api/1", None);
    let parsed = json_lite::parse(&line).unwrap();
    apply_watch_line(&parsed, &mut state, &pool);

    assert!(state.is_empty());
    assert!(pool.backends().is_empty());
}

#[test]
fn apply_watch_line_delete_of_unknown_key_is_a_noop() {
    let mut state = HashMap::new();
    let pool = BackendPool::r#static(vec!["kept:80".to_string()]);
    // Prime the pool's list to something that must survive untouched.
    pool.update(vec!["kept:80".to_string()]);

    let line = kv_event(Some("DELETE"), "/services/api/nonexistent", None);
    let parsed = json_lite::parse(&line).unwrap();
    apply_watch_line(&parsed, &mut state, &pool);

    assert_eq!(vec!["kept:80".to_string()], pool.backends());
}

#[test]
fn apply_watch_line_no_events_key_is_ignored() {
    let mut state = HashMap::new();
    let pool = BackendPool::r#static(vec![]);
    let parsed = json_lite::parse(r#"{"result":{"created":true}}"#).unwrap();
    apply_watch_line(&parsed, &mut state, &pool);
    assert!(state.is_empty());
    assert!(pool.backends().is_empty());
}

// ── kv_range against a mock etcd gateway ──────────────────────────────────────

fn spawn_mock_etcd_kv_range(response_body: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock etcd server");
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });

    port
}

#[test]
fn kv_range_decodes_base64_keys_and_values() {
    let key_b64 = base64_encode(b"/services/api/1");
    let value_b64 = base64_encode(b"10.0.0.5:8080");
    let body = format!(r#"{{"kvs":[{{"key":"{}","value":"{}"}}]}}"#, key_b64, value_b64);
    let port = spawn_mock_etcd_kv_range(Box::leak(body.into_boxed_str()));

    let map = kv_range(&format!("127.0.0.1:{}", port), "/services/api/").unwrap();
    assert_eq!(Some(&"10.0.0.5:8080".to_string()), map.get("/services/api/1"));
}

#[test]
fn kv_range_empty_kvs_returns_empty_map() {
    let port = spawn_mock_etcd_kv_range("{}");
    let map = kv_range(&format!("127.0.0.1:{}", port), "/services/api/").unwrap();
    assert!(map.is_empty());
}

#[test]
fn kv_range_connection_failure_returns_error() {
    let result = kv_range("127.0.0.1:1", "/services/api/");
    assert!(result.is_err());
}

// ── run_once end-to-end against a two-request mock etcd server ───────────────

#[test]
fn run_once_applies_initial_list_and_one_watch_event_then_stream_ends() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock etcd server");
    let port = listener.local_addr().unwrap().port();
    let endpoint = format!("127.0.0.1:{}", port);

    let key1_b64 = base64_encode(b"/services/api/1");
    let value1_b64 = base64_encode(b"10.0.0.5:8080");
    let range_body = format!(r#"{{"kvs":[{{"key":"{}","value":"{}"}}]}}"#, key1_b64, value1_b64);

    let key2_b64 = base64_encode(b"/services/api/2");
    let value2_b64 = base64_encode(b"10.0.0.6:9090");
    let watch_event = format!(
        r#"{{"result":{{"events":[{{"kv":{{"key":"{}","value":"{}"}}}}]}}}}"#,
        key2_b64, value2_b64
    );

    thread::spawn(move || {
        // First connection: /v3/kv/range (one-shot, Connection: close).
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                range_body.len(),
                range_body
            );
            let _ = stream.write_all(resp.as_bytes());
        }

        // Second connection: /v3/watch — respond with chunked headers, one
        // chunk carrying the event, then the terminating zero-length chunk.
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let mut resp = Vec::new();
            resp.extend_from_slice(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n");
            let chunk = format!("{}\n", watch_event);
            resp.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
            resp.extend_from_slice(chunk.as_bytes());
            resp.extend_from_slice(b"\r\n0\r\n\r\n");
            let _ = stream.write_all(&resp);
        }
    });

    let pool = BackendPool::r#static(vec![]);
    run_once(&[endpoint], "/services/api/", &pool).unwrap();

    let backends = pool.backends();
    assert_eq!(2, backends.len(), "got {:?}", backends);
    assert!(backends.contains(&"10.0.0.5:8080".to_string()));
    assert!(backends.contains(&"10.0.0.6:9090".to_string()));
}

#[test]
fn run_once_returns_error_when_no_endpoints() {
    let pool = BackendPool::r#static(vec![]);
    let result = run_once(&[], "/services/api/", &pool);
    assert!(result.is_err());
}
