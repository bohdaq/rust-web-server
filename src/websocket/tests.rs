use std::io::Cursor;

use crate::header::Header;
use crate::http::VERSION;
use crate::request::Request;
use crate::websocket::{Frame, WebSocket, base64_encode, sha1};

// ── SHA-1 ─────────────────────────────────────────────────────────────────────

#[test]
fn sha1_empty_input() {
    // SHA-1("") = da39a3ee5e6b4b0d3255bfef95601890afd80709
    let hash = sha1(b"");
    let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    assert_eq!("da39a3ee5e6b4b0d3255bfef95601890afd80709", hex);
}

#[test]
fn sha1_abc() {
    // SHA-1("abc") = a9993e364706816aba3e25717850c26c9cd0d89d
    let hash = sha1(b"abc");
    let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    assert_eq!("a9993e364706816aba3e25717850c26c9cd0d89d", hex);
}

// ── base64 ─────────────────────────────────────────────────────────────────────

#[test]
fn base64_empty() {
    assert_eq!("", base64_encode(b""));
}

#[test]
fn base64_single_byte() {
    assert_eq!("YQ==", base64_encode(b"a"));
}

#[test]
fn base64_two_bytes() {
    assert_eq!("YWI=", base64_encode(b"ab"));
}

#[test]
fn base64_three_bytes() {
    assert_eq!("YWJj", base64_encode(b"abc"));
}

#[test]
fn base64_longer_string() {
    assert_eq!("SGVsbG8sIFdvcmxkIQ==", base64_encode(b"Hello, World!"));
}

// ── accept key ────────────────────────────────────────────────────────────────

#[test]
fn accept_key_matches_rfc_6455_example() {
    // RFC 6455 Section 1.3 example
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
    assert_eq!(expected, WebSocket::accept_key(key));
}

// ── upgrade detection ─────────────────────────────────────────────────────────

fn upgrade_request() -> Request {
    Request {
        method: "GET".to_string(),
        request_uri: "/ws".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![
            Header { name: "Upgrade".to_string(),              value: "websocket".to_string() },
            Header { name: "Connection".to_string(),           value: "Upgrade".to_string() },
            Header { name: "Sec-WebSocket-Key".to_string(),    value: "dGhlIHNhbXBsZSBub25jZQ==".to_string() },
            Header { name: "Sec-WebSocket-Version".to_string(), value: "13".to_string() },
        ],
        body: vec![],
    }
}

fn plain_request() -> Request {
    Request {
        method: "GET".to_string(),
        request_uri: "/index.html".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

#[test]
fn is_upgrade_request_detects_websocket() {
    assert!(WebSocket::is_upgrade_request(&upgrade_request()));
}

#[test]
fn is_upgrade_request_rejects_plain_get() {
    assert!(!WebSocket::is_upgrade_request(&plain_request()));
}

#[test]
fn is_upgrade_request_rejects_missing_key() {
    let mut req = upgrade_request();
    req.headers.retain(|h| h.name != "Sec-WebSocket-Key");
    assert!(!WebSocket::is_upgrade_request(&req));
}

#[test]
fn is_upgrade_request_rejects_wrong_upgrade_value() {
    let mut req = upgrade_request();
    for h in &mut req.headers {
        if h.name == "Upgrade" { h.value = "h2c".to_string(); }
    }
    assert!(!WebSocket::is_upgrade_request(&req));
}

// ── handshake response ────────────────────────────────────────────────────────

#[test]
fn handshake_response_returns_101() {
    let req = upgrade_request();
    let resp = WebSocket::handshake_response(&req).unwrap();
    assert_eq!(101, resp.status_code);
}

#[test]
fn handshake_response_includes_upgrade_header() {
    let req = upgrade_request();
    let resp = WebSocket::handshake_response(&req).unwrap();
    let upgrade = resp.headers.iter().find(|h| h.name == "Upgrade").unwrap();
    assert_eq!("websocket", upgrade.value);
}

#[test]
fn handshake_response_includes_correct_accept() {
    let req = upgrade_request();
    let resp = WebSocket::handshake_response(&req).unwrap();
    let accept = resp.headers.iter().find(|h| h.name == "Sec-WebSocket-Accept").unwrap();
    assert_eq!("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=", accept.value);
}

#[test]
fn handshake_response_errors_without_key() {
    let req = plain_request();
    assert!(WebSocket::handshake_response(&req).is_err());
}

// ── frame encode / decode ─────────────────────────────────────────────────────

/// Build a client→server masked frame from raw bytes.
fn masked_frame(opcode: u8, payload: &[u8], fin: bool) -> Vec<u8> {
    let mut frame = Vec::new();
    let fin_bit: u8 = if fin { 0x80 } else { 0x00 };
    frame.push(fin_bit | opcode);
    let len = payload.len();
    frame.push(0x80 | len as u8); // masked, short payload (test data is small)
    let mask = [0xAAu8, 0xBB, 0xCC, 0xDD];
    frame.extend_from_slice(&mask);
    for (i, b) in payload.iter().enumerate() {
        frame.push(b ^ mask[i % 4]);
    }
    frame
}

#[test]
fn read_frame_text() {
    let raw = masked_frame(0x1, b"hello", true);
    let mut cursor = Cursor::new(raw);
    let frame = WebSocket::read_frame(&mut cursor).unwrap();
    assert_eq!(Frame::Text("hello".to_string()), frame);
}

#[test]
fn read_frame_binary() {
    let raw = masked_frame(0x2, &[1, 2, 3], true);
    let mut cursor = Cursor::new(raw);
    let frame = WebSocket::read_frame(&mut cursor).unwrap();
    assert_eq!(Frame::Binary(vec![1, 2, 3]), frame);
}

#[test]
fn read_frame_ping() {
    let raw = masked_frame(0x9, b"ping-data", true);
    let mut cursor = Cursor::new(raw);
    let frame = WebSocket::read_frame(&mut cursor).unwrap();
    assert_eq!(Frame::Ping(b"ping-data".to_vec()), frame);
}

#[test]
fn read_frame_close_with_code() {
    // Close frame with status 1000 (Normal Closure)
    let payload = [0x03, 0xE8, b'b', b'y', b'e'];
    let raw = masked_frame(0x8, &payload, true);
    let mut cursor = Cursor::new(raw);
    let frame = WebSocket::read_frame(&mut cursor).unwrap();
    assert_eq!(Frame::Close(Some(1000), "bye".to_string()), frame);
}

#[test]
fn write_frame_text() {
    let mut buf: Vec<u8> = Vec::new();
    WebSocket::write_frame(&mut buf, Frame::Text("hi".to_string())).unwrap();
    // Byte 0: FIN=1, opcode=1 → 0x81
    // Byte 1: unmasked, length=2 → 0x02
    assert_eq!(&buf[..4], &[0x81, 0x02, b'h', b'i']);
}

#[test]
fn write_frame_ping() {
    let mut buf: Vec<u8> = Vec::new();
    WebSocket::write_frame(&mut buf, Frame::Ping(vec![])).unwrap();
    assert_eq!(&buf[..2], &[0x89, 0x00]);
}

#[test]
fn write_frame_close() {
    let mut buf: Vec<u8> = Vec::new();
    WebSocket::write_frame(&mut buf, Frame::Close(Some(1000), String::new())).unwrap();
    // 0x88 = FIN + opcode 0x8, length 2, status 1000 = 0x03 0xE8
    assert_eq!(&buf, &[0x88, 0x02, 0x03, 0xE8]);
}

#[test]
fn round_trip_text_frame() {
    let original = "Hello, WebSocket!";
    let mut buf: Vec<u8> = Vec::new();
    WebSocket::write_frame(&mut buf, Frame::Text(original.to_string())).unwrap();
    // Simulate as an unmasked server→server frame (server reads unmasked)
    let mut cursor = Cursor::new(buf);
    let frame = WebSocket::read_frame(&mut cursor).unwrap();
    assert_eq!(Frame::Text(original.to_string()), frame);
}

#[test]
fn round_trip_binary_frame() {
    let original = vec![10u8, 20, 30, 40, 50];
    let mut buf: Vec<u8> = Vec::new();
    WebSocket::write_frame(&mut buf, Frame::Binary(original.clone())).unwrap();
    let mut cursor = Cursor::new(buf);
    let frame = WebSocket::read_frame(&mut cursor).unwrap();
    assert_eq!(Frame::Binary(original), frame);
}

#[test]
fn send_text_convenience() {
    let mut buf: Vec<u8> = Vec::new();
    WebSocket::send_text(&mut buf, "hello").unwrap();
    let mut cursor = Cursor::new(buf);
    let frame = WebSocket::read_frame(&mut cursor).unwrap();
    assert_eq!(Frame::Text("hello".to_string()), frame);
}
