//! Server-Sent Events (SSE) response builder.
//!
//! [`Sse`] assembles a complete `text/event-stream` response body from a
//! sequence of [`SseEvent`] values. The full body is buffered before any bytes
//! are written to the socket, which suits pre-known event sequences (batch
//! updates, enumerable progress steps, static push payloads).
//!
//! For live streaming where events arrive over time (AI token output, real-time
//! sensor data), write `text/event-stream` headers and the raw event lines
//! directly to the TCP stream in a custom accept loop — the same approach used
//! for WebSocket connections.
//!
//! # Wire format
//!
//! Each event is a block of `field: value\n` lines terminated by a blank line:
//!
//! ```text
//! id: 1
//! event: update
//! data: {"count":42}
//!
//! ```
//!
//! Lines with no field name (starting with `:`) are comments and are ignored
//! by clients; they are used here as keep-alive pings.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::sse::{Sse, SseEvent};
//!
//! let response = Sse::new()
//!     .event("connected", "ready")
//!     .push(SseEvent::data(r#"{"count":1}"#).id("1").event_type("update"))
//!     .push(SseEvent::data(r#"{"count":2}"#).id("2").event_type("update"))
//!     .comment("keep-alive")
//!     .into_response();
//! ```

#[cfg(test)]
mod tests;

use crate::core::New;
use crate::header::Header;
use crate::range::Range;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

// ── SseEvent ─────────────────────────────────────────────────────────────────

/// A single Server-Sent Event.
///
/// Build with [`SseEvent::data`] and chain the optional setter methods.
/// Use [`Sse::push`] to add it to a response.
pub struct SseEvent {
    id: Option<String>,
    event: Option<String>,
    data: String,
    retry_ms: Option<u32>,
}

impl SseEvent {
    /// Create an event with the given data payload. Multi-line strings produce
    /// multiple `data:` lines, which the client concatenates with `\n`.
    pub fn data(data: impl Into<String>) -> Self {
        SseEvent { id: None, event: None, data: data.into(), retry_ms: None }
    }

    /// Set the `id` field. Clients use this to resume from the last event
    /// seen after a reconnection (`Last-Event-ID` request header).
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the event type (`event` field). Clients listen for specific types
    /// with `addEventListener("type", handler)`. Defaults to `"message"`.
    pub fn event_type(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    /// Set the `retry` reconnection delay in milliseconds. Overrides the
    /// client's default reconnect interval.
    pub fn retry(mut self, ms: u32) -> Self {
        self.retry_ms = Some(ms);
        self
    }

    /// Encode this event as SSE wire-format bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = String::new();
        if let Some(id) = &self.id {
            out.push_str("id: ");
            out.push_str(id);
            out.push('\n');
        }
        if let Some(event) = &self.event {
            out.push_str("event: ");
            out.push_str(event);
            out.push('\n');
        }
        for line in self.data.lines() {
            out.push_str("data: ");
            out.push_str(line);
            out.push('\n');
        }
        if self.data.is_empty() {
            out.push_str("data: \n");
        }
        if let Some(ms) = self.retry_ms {
            out.push_str("retry: ");
            out.push_str(&ms.to_string());
            out.push('\n');
        }
        out.push('\n');
        out.into_bytes()
    }
}

// ── Sse ──────────────────────────────────────────────────────────────────────

/// Builder for a buffered Server-Sent Events response.
///
/// Call [`Sse::into_response`] to obtain a [`Response`] with:
/// - `200 OK`
/// - `Content-Type: text/event-stream`
/// - `Cache-Control: no-cache`
/// - `X-Accel-Buffering: no` (disables nginx proxy buffering)
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::sse::Sse;
///
/// let response = Sse::new()
///     .event("open", "")
///     .data(r#"{"msg":"hello"}"#)
///     .retry(5000)
///     .into_response();
/// ```
pub struct Sse {
    chunks: Vec<Vec<u8>>,
}

impl Sse {
    /// Create an empty SSE response builder.
    pub fn new() -> Self {
        Sse { chunks: Vec::new() }
    }

    /// Append a named event with `data`.
    ///
    /// `event_type` becomes the `event:` field; `data` becomes the `data:` field.
    pub fn event(mut self, event_type: &str, data: &str) -> Self {
        self.chunks.push(
            SseEvent::data(data).event_type(event_type).encode(),
        );
        self
    }

    /// Append a data-only event (no `event:` type field; clients receive it as
    /// `"message"`).
    pub fn data(mut self, data: &str) -> Self {
        self.chunks.push(SseEvent::data(data).encode());
        self
    }

    /// Append a fully configured [`SseEvent`] (id, type, retry, data).
    pub fn push(mut self, event: SseEvent) -> Self {
        self.chunks.push(event.encode());
        self
    }

    /// Append a `retry:` directive that tells the client how many milliseconds
    /// to wait before reconnecting after the connection is lost.
    pub fn retry(mut self, ms: u32) -> Self {
        self.chunks.push(format!("retry: {}\n\n", ms).into_bytes());
        self
    }

    /// Append an SSE comment line (starts with `:`). Used as a keep-alive ping
    /// or annotation; browsers ignore comment content.
    pub fn comment(mut self, text: &str) -> Self {
        self.chunks.push(format!(": {}\n\n", text).into_bytes());
        self
    }

    /// Finalise the builder and return a [`Response`] with the correct SSE
    /// headers and the accumulated event body.
    pub fn into_response(self) -> Response {
        let body: Vec<u8> = self.chunks.into_iter().flatten().collect();

        let mut response = Response::new();
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase =
            STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.headers.push(Header {
            name: Header::_CACHE_CONTROL.to_string(),
            value: "no-cache".to_string(),
        });
        response.headers.push(Header {
            name: "X-Accel-Buffering".to_string(),
            value: "no".to_string(),
        });
        response.content_range_list = vec![Range::get_content_range(
            body,
            "text/event-stream".to_string(),
        )];
        response
    }
}
