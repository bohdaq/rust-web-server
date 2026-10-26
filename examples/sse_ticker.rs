//! Server-sent events streamed as they happen: a counter that ticks once a
//! second, ten times, then ends.
//!
//! ```sh
//! cargo run --example sse_ticker
//! curl -N localhost:7878/ticks
//! ```

use std::io::Read;
use std::time::Duration;

use rust_web_server::header::Header;
use rust_web_server::prelude::*;
use rust_web_server::sse::SseEvent;
use rust_web_server::state::AppWithState;

/// A body that yields one SSE event per tick, then ends.
struct Ticker {
    every: Duration,
    left: u32,
    sent: u32,
    pending: Vec<u8>,
}

impl Read for Ticker {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pending.is_empty() {
            if self.left == 0 {
                return Ok(0);
            }
            std::thread::sleep(self.every);
            self.sent += 1;
            self.left -= 1;
            self.pending = SseEvent::data(&format!(r#"{{"tick":{}}}"#, self.sent)).id(&self.sent.to_string()).event_type("tick").encode();
        }
        let n = self.pending.len().min(buf.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }
}

/// `GET /ticks` streams `count` events, one every `every`.
pub fn app(every: Duration, count: u32) -> AppWithState<(Duration, u32)> {
    App::with_state((every, count)).get("/ticks", |_, _, _, &(every, count)| {
        let mut response = Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, None);
        response.headers.push(Header::new(Header::_CONTENT_TYPE, "text/event-stream"));
        response.headers.push(Header::new(Header::_CACHE_CONTROL, "no-cache"));
        // The connection writer drives this reader: each event goes out as
        // soon as it is produced (HTTP/1.1, HTTP/2 and HTTP/3 alike).
        response.stream_pipe = Some(Box::new(Ticker { every, left: count, sent: 0, pending: Vec::new() }));
        response
    })
}

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, app(Duration::from_secs(1), 10)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::application::Application;
    use rust_web_server::server::Address;

    #[test]
    fn events_stream_then_end() {
        let request = Request {
            method: "GET".into(),
            request_uri: "/ticks".into(),
            http_version: "HTTP/1.1".into(),
            headers: vec![],
            body: Default::default(),
        };
        let connection = ConnectionInfo {
            client: Address { ip: "127.0.0.1".into(), port: 0 },
            server: Address { ip: "127.0.0.1".into(), port: 0 },
            request_size: 16000,
            sni_hostname: None,
        };
        let mut response = app(Duration::from_millis(1), 3).execute(&request, &connection).unwrap();
        assert_eq!(Some("text/event-stream"), response.header("content-type").map(|h| &*h.value));
        let mut events = String::new();
        response.stream_pipe.take().unwrap().read_to_string(&mut events).unwrap();
        assert_eq!(3, events.matches("event: tick").count(), "{events}");
        assert!(events.contains(r#"data: {"tick":3}"#), "{events}");
    }
}
