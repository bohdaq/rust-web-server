//! Live updates: a WebSocket chat echo, a server-pushed WebSocket ticker and
//! a Server-Sent Events stream, all in ordinary routes.
//!
//! ```sh
//! cargo run --example realtime
//! curl -N localhost:7878/events                 # three events, a second apart
//! # in a browser console on any page:
//! #   ws = new WebSocket("ws://localhost:7878/chat/lobby"); ws.onmessage = e => console.log(e.data); ws.send("hi")
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rust_web_server::prelude::*;
use rust_web_server::sse::{Sse, SseEvent};
use rust_web_server::state::AppWithState;
use rust_web_server::websocket::Message;

/// Messages relayed so far, across all rooms.
#[derive(Clone, Default)]
pub struct Stats {
    messages: Arc<AtomicU64>,
}

/// How long the SSE stream waits between events (`REALTIME_EVENT_MS`).
fn event_gap() -> Duration {
    Duration::from_millis(std::env::var("REALTIME_EVENT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1000))
}

pub fn app() -> AppWithState<Stats> {
    App::with_state(Stats::default())
        // Echo each message with the room name and a running count.
        .websocket("/chat/:room", |mut ws, params, stats| {
            let room = params.get("room").unwrap_or("lobby").to_string();
            while let Ok(Some(message)) = ws.recv() {
                let n = stats.messages.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = match message {
                    Message::Text(text) => ws.send_text(&format!("{room} #{n}: {text}")),
                    Message::Binary(data) => ws.send_binary(&data),
                };
            }
        })
        // The server talks first: three ticks from another thread, then a close.
        .websocket("/ticker", |mut ws, _params, _stats| {
            let sender = ws.sender();
            std::thread::spawn(move || {
                for n in 1..=3 {
                    if sender.send_text(&format!("tick {n}")).is_err() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                let _ = sender.close(1000, "done");
            });
            // Answer pings and finish the closing handshake.
            while let Ok(Some(_)) = ws.recv() {}
        })
        // Three events, `REALTIME_EVENT_MS` apart, then the stream ends.
        .get("/events", |_req, _params, _conn, _stats| {
            let (events, response) = Sse::stream();
            std::thread::spawn(move || {
                for n in 1..=3 {
                    let event = SseEvent::data(format!("event {n}")).id(n.to_string()).event_type("count");
                    if events.send(event).is_err() {
                        return;
                    }
                    std::thread::sleep(event_gap());
                }
            });
            response
        })
}

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, app()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    #[test]
    fn a_plain_get_on_a_websocket_route_is_told_to_upgrade() {
        let client = TestClient::new(app());
        assert_eq!(426, client.get("/chat/lobby").send().status());
    }
}
