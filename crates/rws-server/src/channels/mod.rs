//! Channels (18.35.0, #94): named topics that pages and other clients
//! subscribe to over SSE or WebSocket, and that server code publishes to —
//! "a comment was added", "the import finished". In-process: replicas
//! don't see each other's messages (publish from where the change happens,
//! or bridge a queue into [`Hub::publish`]).
//!
//! ```rust
//! # extern crate rws_server as rust_web_server;
//! use rust_web_server::app::App;
//! use rust_web_server::channels::Hub;
//!
//! let hub = Hub::new();
//! let app = App::with_state(hub.clone())
//!     // EventSource("/rooms/lobby/events") receives every message.
//!     .get("/rooms/:room/events", |_, params, _, hub| hub.sse(params.get("room").unwrap_or("lobby")))
//!     // A WebSocket client receives them too; what it sends is published.
//!     .websocket("/rooms/:room/ws", |ws, params, hub| {
//!         let room = params.get("room").unwrap_or("lobby").to_string();
//!         hub.serve_websocket(ws, &room, |hub, room, text| { hub.publish(room, text); });
//!     });
//!
//! let delivered = hub.publish("lobby", "hello");
//! assert_eq!(0, delivered); // nobody is listening yet
//! ```
//!
//! Delivery is best effort and never blocks on a slow SSE client: a
//! message that doesn't fit its buffer is dropped for that client.
//! WebSocket messages are written by the publishing thread. Subscribers
//! that have gone away are removed on the next publish to their topic.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::response::Response;
use crate::sse::{SseEvent, SseSender, SseStream};
use crate::websocket::{Message, WebSocketConnection, WebSocketSender};

enum Subscriber {
    Sse(SseSender),
    WebSocket(WebSocketSender),
    Channel(Sender<String>),
}

impl Subscriber {
    /// Deliver `message`: `Some(true)` delivered, `Some(false)` dropped
    /// (full), `None` the subscriber is gone.
    fn deliver(&self, topic: &str, message: &str) -> Option<bool> {
        match self {
            Subscriber::Sse(tx) => tx.try_send(SseEvent::data(message).event_type(topic)).ok(),
            Subscriber::WebSocket(ws) => (!ws.is_closed() && ws.send_text(message).is_ok()).then_some(true),
            Subscriber::Channel(tx) => tx.send(message.to_string()).ok().map(|_| true),
        }
    }
}

/// Topics and their subscribers; cheap to clone (all clones share them).
#[derive(Clone, Default)]
pub struct Hub {
    topics: Arc<Mutex<HashMap<String, Vec<Subscriber>>>>,
}

impl Hub {
    pub fn new() -> Self {
        Hub::default()
    }

    fn add(&self, topic: &str, subscriber: Subscriber) {
        let mut topics = self.topics.lock().unwrap_or_else(|e| e.into_inner());
        topics.entry(topic.to_string()).or_default().push(subscriber);
    }

    /// Send `message` to every subscriber of `topic`; the number it was
    /// delivered to. SSE clients get it as an event named after the topic
    /// (`EventSource.addEventListener(topic, …)`), WebSocket clients as a
    /// text message.
    pub fn publish(&self, topic: &str, message: &str) -> usize {
        // Take the subscribers out so a slow WebSocket write doesn't hold
        // the lock; put the live ones back after.
        let subscribers = {
            let mut topics = self.topics.lock().unwrap_or_else(|e| e.into_inner());
            match topics.get_mut(topic) {
                Some(list) => std::mem::take(list),
                None => return 0,
            }
        };
        let mut delivered = 0;
        let mut live = Vec::with_capacity(subscribers.len());
        for subscriber in subscribers {
            if let Some(sent) = subscriber.deliver(topic, message) {
                delivered += sent as usize;
                live.push(subscriber);
            }
        }
        let mut topics = self.topics.lock().unwrap_or_else(|e| e.into_inner());
        let list = topics.entry(topic.to_string()).or_default();
        // Subscribers that joined while this publish ran come after.
        live.append(list);
        if live.is_empty() {
            topics.remove(topic);
        } else {
            *list = live;
        }
        delivered
    }

    /// A streaming SSE response subscribed to `topic`: return it from the
    /// handler.
    pub fn sse(&self, topic: &str) -> Response {
        let (tx, response) = SseStream::new().start();
        self.add(topic, Subscriber::Sse(tx));
        response
    }

    /// Subscribe a WebSocket client (its sender) to `topic`.
    pub fn subscribe_websocket(&self, topic: &str, sender: WebSocketSender) {
        self.add(topic, Subscriber::WebSocket(sender));
    }

    /// Subscribe `ws` to `topic` and read its messages until it closes,
    /// handing each text message to `on_message(hub, topic, text)`. Run it
    /// in a `.websocket(…)` handler.
    pub fn serve_websocket<F>(&self, mut ws: WebSocketConnection, topic: &str, on_message: F)
    where
        F: Fn(&Hub, &str, &str),
    {
        self.subscribe_websocket(topic, ws.sender());
        while let Ok(Some(message)) = ws.recv() {
            if let Message::Text(text) = message {
                on_message(self, topic, &text);
            }
        }
        let _ = ws.close(1000, "");
    }

    /// Subscribe in-process: every message to `topic` arrives on the
    /// receiver until it's dropped.
    pub fn subscribe(&self, topic: &str) -> Receiver<String> {
        let (tx, rx) = channel();
        self.add(topic, Subscriber::Channel(tx));
        rx
    }

    /// How many subscribers `topic` has (some may have gone away since the
    /// last publish).
    pub fn subscribers(&self, topic: &str) -> usize {
        self.topics.lock().unwrap_or_else(|e| e.into_inner()).get(topic).map_or(0, Vec::len)
    }

    /// The topics that have subscribers, sorted.
    pub fn topics(&self) -> Vec<String> {
        let mut topics: Vec<String> = self.topics.lock().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect();
        topics.sort();
        topics
    }
}
