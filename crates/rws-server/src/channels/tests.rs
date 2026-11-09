use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use super::*;

/// A writer whose bytes the test can read back.
#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn channel_subscribers_receive_until_dropped() {
    let hub = Hub::new();
    let rx = hub.subscribe("news");
    let other = hub.subscribe("other");
    assert_eq!(1, hub.publish("news", "one"));
    assert_eq!("one", rx.recv().unwrap());
    assert!(other.try_recv().is_err());
    assert_eq!(vec!["news".to_string(), "other".to_string()], hub.topics());
    drop(rx);
    assert_eq!(0, hub.publish("news", "two"));
    assert_eq!(0, hub.subscribers("news"));
    assert_eq!(vec!["other".to_string()], hub.topics());
    assert_eq!(0, hub.publish("nobody", "x"));
}

#[test]
fn sse_subscribers_get_named_events() {
    let hub = Hub::new();
    let mut response = hub.sse("room");
    assert_eq!(1, hub.subscribers("room"));
    assert_eq!(1, hub.publish("room", "hi"));
    let mut reader = response.stream_pipe.take().expect("a streaming body");
    let mut buf = [0u8; 256];
    let n = reader.read(&mut buf).unwrap();
    let frame = String::from_utf8_lossy(&buf[..n]);
    assert!(frame.contains("event: room") && frame.contains("data: hi"), "{frame}");
    // The client goes away: the next publish removes it.
    drop(reader);
    assert_eq!(0, hub.publish("room", "again"));
    assert_eq!(0, hub.subscribers("room"));
}

#[test]
fn websocket_subscribers_get_text_and_closed_ones_are_dropped() {
    let hub = Hub::new();
    let written = Shared::default();
    let ws = WebSocketConnection::new(Box::new(std::io::Cursor::new(Vec::new())), Box::new(written.clone()));
    hub.subscribe_websocket("t", ws.sender());
    assert_eq!(1, hub.publish("t", "hello"));
    let bytes = written.0.lock().unwrap().clone();
    // An unmasked server text frame: FIN + opcode 1, length 5, payload.
    assert_eq!(vec![0x81, 5, b'h', b'e', b'l', b'l', b'o'], bytes);
    ws.close(1000, "").unwrap();
    assert_eq!(0, hub.publish("t", "after close"));
    assert_eq!(0, hub.subscribers("t"));
}

#[test]
fn serve_websocket_publishes_what_the_client_sends() {
    let hub = Hub::new();
    let listener = hub.subscribe("chat");
    // A masked client text frame "yo", then EOF.
    let mask = [1u8, 2, 3, 4];
    let mut frame = vec![0x81, 0x80 | 2];
    frame.extend_from_slice(&mask);
    frame.extend(b"yo".iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    let ws = WebSocketConnection::new(Box::new(std::io::Cursor::new(frame)), Box::new(Shared::default()));
    hub.serve_websocket(ws, "chat", |hub, topic, text| {
        hub.publish(topic, &format!("echo {text}"));
    });
    assert_eq!("echo yo", listener.recv().unwrap());
}
