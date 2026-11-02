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
            Header { name: "Upgrade".into(),              value: "websocket".into() },
            Header { name: "Connection".into(),           value: "Upgrade".into() },
            Header { name: "Sec-WebSocket-Key".into(),    value: "dGhlIHNhbXBsZSBub25jZQ==".into() },
            Header { name: "Sec-WebSocket-Version".into(), value: "13".into() },
        ],
        body: Default::default(),
    }
}

fn plain_request() -> Request {
    Request {
        method: "GET".to_string(),
        request_uri: "/index.html".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
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
        if h.name == "Upgrade" { h.value = "h2c".into(); }
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
fn a_close_reason_that_isnt_utf8_fails_the_frame() {
    // RFC 6455 §7.1.6. Decoded lossily it grew threefold past the payload
    // limit (found by the websocket_frame fuzz target).
    let payload = [0x03, 0xE8, 0xFF, 0xFE, 0xFD];
    let raw = masked_frame(0x8, &payload, true);
    let error = WebSocket::read_frame(&mut Cursor::new(raw)).unwrap_err();
    assert!(error.contains("invalid UTF-8"), "{error}");
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

// ── Payload limits: the length comes from the peer ──────────────────────────

#[test]
fn a_frame_claiming_a_huge_payload_is_refused_before_reading_it() {
    // FIN + binary, 64-bit length = u64::MAX, no payload behind it: this
    // used to try to allocate 2^64 - 1 bytes and abort the process.
    let mut frame = vec![0x82u8, 127];
    frame.extend_from_slice(&u64::MAX.to_be_bytes());
    let err = WebSocket::read_frame(&mut std::io::Cursor::new(frame)).unwrap_err();
    assert!(err.contains("exceeds"), "{err}");

    let mut over = vec![0x82u8, 127];
    over.extend_from_slice(&(WebSocket::MAX_FRAME_PAYLOAD + 1).to_be_bytes());
    assert!(WebSocket::read_frame(&mut std::io::Cursor::new(over)).is_err());
}

#[test]
fn a_frame_shorter_than_its_length_is_an_error() {
    // Claims 1000 bytes, carries 3.
    let mut frame = vec![0x82u8, 126];
    frame.extend_from_slice(&1000u16.to_be_bytes());
    frame.extend_from_slice(b"abc");
    let err = WebSocket::read_frame(&mut std::io::Cursor::new(frame)).unwrap_err();
    assert!(err.contains("ended inside a frame"), "{err}");
}

#[test]
fn read_frame_limited_applies_its_own_limit() {
    let mut buf = Vec::new();
    WebSocket::write_frame(&mut buf, Frame::Binary(vec![7u8; 300])).unwrap();
    assert!(WebSocket::read_frame_limited(&mut std::io::Cursor::new(buf.clone()), 299).is_err());
    match WebSocket::read_frame_limited(&mut std::io::Cursor::new(buf), 300).unwrap() {
        Frame::Binary(data) => assert_eq!(vec![7u8; 300], data),
        other => panic!("{other:?}"),
    }
}

// ── WebSocketConnection ───────────────────────────────────────────────────────

use std::sync::{Arc, Mutex};

use crate::websocket::{Message, WebSocketConnection};

/// A client frame: masked, with `fin` and `opcode` as given.
fn client_frame(fin: bool, opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mask = [0x12, 0x34, 0x56, 0x78];
    let mut out = vec![if fin { 0x80 } else { 0 } | opcode];
    match payload.len() {
        n @ 0..=125 => out.push(0x80 | n as u8),
        n => {
            out.push(0x80 | 126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
    }
    out.extend_from_slice(&mask);
    out.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    out
}

#[derive(Clone, Default)]
struct Sent(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Sent {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Sent {
    /// The server's frames, decoded.
    fn frames(&self) -> Vec<Frame> {
        let bytes = self.0.lock().unwrap().clone();
        let mut cursor = Cursor::new(bytes);
        let mut frames = Vec::new();
        while (cursor.position() as usize) < cursor.get_ref().len() {
            frames.push(WebSocket::read_frame(&mut cursor).unwrap());
        }
        frames
    }
}

fn connection(input: Vec<u8>) -> (WebSocketConnection, Sent) {
    let sent = Sent::default();
    (WebSocketConnection::new(Box::new(Cursor::new(input)), Box::new(sent.clone())), sent)
}

#[test]
fn messages_are_reassembled_and_pings_answered() {
    let snowman = "a☃b".as_bytes(); // the snowman's 3 bytes split across fragments
    let mut input = client_frame(false, 0x1, &snowman[..2]);
    input.extend(client_frame(true, 0x9, b"ping!"));
    input.extend(client_frame(true, 0x0, &snowman[2..]));
    input.extend(client_frame(true, 0x2, &[1, 2, 3]));
    input.extend(client_frame(true, 0x8, &[0x03, 0xE8]));
    let (mut ws, sent) = connection(input);
    assert_eq!(Some(Message::Text("a☃b".to_string())), ws.recv().unwrap());
    assert_eq!(Some(Message::Binary(vec![1, 2, 3])), ws.recv().unwrap());
    assert_eq!(None, ws.recv().unwrap(), "closed cleanly");
    assert_eq!(None, ws.recv().unwrap());
    assert_eq!(vec![Frame::Pong(b"ping!".to_vec()), Frame::Close(Some(1000), String::new())], sent.frames());
    assert!(ws.send_text("late").is_err(), "nothing after the close");
}

fn fails_with(input: Vec<u8>, code: u16) {
    let (mut ws, sent) = connection(input);
    assert!(ws.recv().is_err());
    assert_eq!(Some(&Frame::Close(Some(code), String::new())), sent.frames().last());
}

#[test]
fn protocol_errors_close_with_their_code() {
    let mut unmasked = client_frame(true, 0x1, b"hi");
    unmasked[1] &= 0x7F;
    unmasked.drain(2..6);
    for (i, b) in unmasked.iter_mut().skip(2).enumerate() {
        *b ^= [0x12, 0x34, 0x56, 0x78][i % 4];
    }
    fails_with(unmasked, 1002);
    fails_with(client_frame(true, 0x1, &[0xFF, 0xFE]), 1007);
    fails_with(client_frame(true, 0x0, b"orphan"), 1002);
    fails_with([client_frame(false, 0x1, b"a"), client_frame(true, 0x1, b"b")].concat(), 1002);
    fails_with(client_frame(false, 0x9, b""), 1002);
    fails_with(client_frame(true, 0x8, &[0x03, 0xED]), 1002); // 1005 is never sent
    fails_with(client_frame(true, 0x1 | 0x40, b"x"), 1002); // RSV1 without permessage-deflate
    let (ws, _) = connection(client_frame(true, 0x2, &[0u8; 200]));
    let mut ws = ws.max_message_size(100);
    assert!(ws.recv().is_err());
}

#[test]
fn a_server_close_ends_when_the_client_answers() {
    let (mut ws, sent) = connection(client_frame(true, 0x8, &[0x03, 0xE8, b'o', b'k']));
    ws.close(1001, "going away").unwrap();
    assert_eq!(None, ws.recv().unwrap());
    assert_eq!(vec![Frame::Close(Some(1001), "going away".to_string())], sent.frames(), "no second close");
}

#[test]
fn a_sender_works_from_another_thread() {
    let (ws, sent) = connection(Vec::new());
    let sender = ws.sender();
    std::thread::spawn(move || sender.send_text("from a thread").unwrap()).join().unwrap();
    assert_eq!(vec![Frame::Text("from a thread".to_string())], sent.frames());
}

// ── WebSocket::upgrade ────────────────────────────────────────────────────────

fn versioned_upgrade_request(version: &str) -> Request {
    let header = |name: &str, value: &str| Header { name: name.to_string().into(), value: value.to_string().into() };
    Request {
        method: "GET".to_string(),
        request_uri: "/ws".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![
            header("Upgrade", "websocket"),
            header("Connection", "Upgrade"),
            header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ=="),
            header("Sec-WebSocket-Version", version),
        ],
        body: Default::default(),
    }
}

#[test]
fn upgrade_answers_101_and_waits_for_the_server() {
    let mut response = WebSocket::upgrade(&versioned_upgrade_request("13"), |_ws| {});
    assert_eq!(101, response.status_code);
    let token = response.headers.iter().find(|h| h.name.eq_ignore_ascii_case("X-Rws-WebSocket-Upgrade")).unwrap().value.to_string();
    assert_eq!(32, token.len());
    let mut copy = WebSocket::upgrade(&versioned_upgrade_request("13"), |_ws| {});
    assert!(crate::websocket::take_upgrade(&mut response).is_some());
    assert!(!response.headers.iter().any(|h| h.name.eq_ignore_ascii_case("X-Rws-WebSocket-Upgrade")), "the header is removed");
    let head = String::from_utf8(crate::websocket::upgrade_head(&response)).unwrap();
    assert!(head.starts_with("HTTP/1.1 101 Switching Protocols\r\n") && head.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"), "{head}");
    // Each token once; one replayed onto another response does nothing.
    response.headers.push(Header { name: "X-Rws-WebSocket-Upgrade".into(), value: token.into() });
    assert!(crate::websocket::take_upgrade(&mut response).is_none());
    copy.status_code = 200;
    assert!(crate::websocket::take_upgrade(&mut copy).is_none(), "only a 101 hands over");
}

#[test]
fn a_request_that_cannot_upgrade_gets_426() {
    for request in [versioned_upgrade_request("8"), Request { headers: vec![], ..versioned_upgrade_request("13") }] {
        let response = WebSocket::upgrade(&request, |_ws| panic!("never runs"));
        assert_eq!(426, response.status_code);
        assert!(response.headers.iter().any(|h| &*h.name == "Sec-WebSocket-Version" && &*h.value == "13"));
    }
}
