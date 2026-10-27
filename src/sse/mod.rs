//! Server-Sent Events (SSE).
//!
//! [`Sse::stream`] answers with a `text/event-stream` response at once and
//! returns an [`SseSender`]: events sent through it reach the client as they
//! are sent — AI token output, progress, live updates — over HTTP/1.1,
//! HTTP/2 and HTTP/3 alike. The stream ends when every sender is dropped; a
//! client that went away shows up as [`SseClosed`] on the next send (a
//! keep-alive comment every 15 seconds finds it even when nothing is sent).
//!
//! [`Sse`] instead assembles a complete, buffered body from a known sequence
//! of [`SseEvent`]s.
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
//! # Streaming
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::sse::{Sse, SseEvent};
//!
//! let app = App::with_state(()).get("/ticks", |_req, _params, _conn, _state| {
//!     let (events, response) = Sse::stream();
//!     std::thread::spawn(move || {
//!         for n in 1.. {
//!             let event = SseEvent::data(n.to_string()).id(n.to_string());
//!             if events.send(event).is_err() {
//!                 break; // the client went away
//!             }
//!             std::thread::sleep(std::time::Duration::from_secs(1));
//!         }
//!     });
//!     response
//! });
//! ```
//!
//! # Buffered
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

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::time::Duration;

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

    /// Encode this event as SSE wire-format bytes. `data` is split into
    /// one `data:` line per line (at `\n`, `\r\n` or a lone `\r`, all line
    /// ends to the client); line breaks in `id` and the event type are
    /// dropped, so neither can add fields of its own.
    pub fn encode(&self) -> Vec<u8> {
        let one_line = |value: &str| value.replace(['\r', '\n'], "");
        let mut out = String::new();
        if let Some(id) = &self.id {
            out.push_str("id: ");
            out.push_str(&one_line(id));
            out.push('\n');
        }
        if let Some(event) = &self.event {
            out.push_str("event: ");
            out.push_str(&one_line(event));
            out.push('\n');
        }
        for line in self.data.replace("\r\n", "\n").split(['\n', '\r']) {
            out.push_str("data: ");
            out.push_str(line);
            out.push('\n');
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
        self.chunks.push(comment_bytes(text));
        self
    }

    /// A streaming response and the [`SseSender`] that feeds it: 32 events
    /// buffered, a keep-alive comment after 15 quiet seconds. See
    /// [`SseStream`] to change either.
    pub fn stream() -> (SseSender, Response) {
        SseStream::new().start()
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
            name: Header::_CACHE_CONTROL.into(),
            value: "no-cache".into(),
        });
        response.headers.push(Header {
            name: "X-Accel-Buffering".into(),
            value: "no".into(),
        });
        response.content_range_list = vec![Range::get_content_range(
            body,
            "text/event-stream".to_string(),
        )];
        response
    }
}

/// `: text` — one comment line per line of `text`.
fn comment_bytes(text: &str) -> Vec<u8> {
    let mut out = String::new();
    for line in text.replace("\r\n", "\n").split(['\n', '\r']) {
        out.push_str(": ");
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
    out.into_bytes()
}

/// The `text/event-stream` response headers: no caching, no proxy buffering.
fn stream_response(body: Option<Box<dyn std::io::Read + Send>>) -> Response {
    let mut response = Response::new();
    response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    response.headers.push(Header { name: Header::_CONTENT_TYPE.into(), value: "text/event-stream".into() });
    response.headers.push(Header { name: Header::_CACHE_CONTROL.into(), value: "no-cache".into() });
    response.headers.push(Header { name: "X-Accel-Buffering".into(), value: "no".into() });
    response.stream_pipe = body;
    response
}

// ── Streaming ─────────────────────────────────────────────────────────────────

/// Options for a streaming SSE response; [`Sse::stream`] uses the defaults.
///
/// ```rust,no_run
/// use std::time::Duration;
/// use rust_web_server::sse::SseStream;
///
/// let (events, response) = SseStream::new().buffer(256).keep_alive(Duration::from_secs(5)).start();
/// ```
pub struct SseStream {
    buffer: usize,
    keep_alive: Duration,
}

impl Default for SseStream {
    fn default() -> Self {
        SseStream::new()
    }
}

impl SseStream {
    /// 32 events buffered, keep-alive every 15 seconds.
    pub fn new() -> Self {
        SseStream { buffer: 32, keep_alive: Duration::from_secs(15) }
    }

    /// How many events may wait for a slow client before
    /// [`SseSender::send`] blocks (and [`SseSender::try_send`] drops).
    pub fn buffer(mut self, events: usize) -> Self {
        self.buffer = events.max(1);
        self
    }

    /// After this long with nothing sent, a `: keep-alive` comment goes out:
    /// proxies keep the connection, and a client that left is noticed.
    pub fn keep_alive(mut self, interval: Duration) -> Self {
        self.keep_alive = interval.max(Duration::from_millis(10));
        self
    }

    /// The sender and the response to return from the handler.
    pub fn start(self) -> (SseSender, Response) {
        let (tx, rx) = mpsc::sync_channel(self.buffer);
        let response = stream_response(Some(Box::new(SseReader::new(rx, self.keep_alive))));
        (SseSender { tx }, response)
    }
}

/// Sends events to one streaming SSE response; clone it to send from
/// several places. The stream ends when the last clone is dropped.
#[derive(Clone)]
pub struct SseSender {
    tx: SyncSender<Vec<u8>>,
}

/// The client is gone (or the response was never sent): stop sending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SseClosed;

impl std::fmt::Display for SseClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the SSE stream is closed")
    }
}

impl std::error::Error for SseClosed {}

impl SseSender {
    /// Send `event`, waiting while the buffer is full (a slow client slows
    /// the sender down rather than growing memory). Blocks the thread: in
    /// async code use [`Self::send_async`] or [`Self::try_send`].
    pub fn send(&self, event: SseEvent) -> Result<(), SseClosed> {
        self.tx.send(event.encode()).map_err(|_| SseClosed)
    }

    /// Send `event` if there's room: `Ok(false)` when the buffer is full and
    /// the event was dropped.
    pub fn try_send(&self, event: SseEvent) -> Result<bool, SseClosed> {
        match self.tx.try_send(event.encode()) {
            Ok(()) => Ok(true),
            Err(TrySendError::Full(_)) => Ok(false),
            Err(TrySendError::Disconnected(_)) => Err(SseClosed),
        }
    }

    /// [`Self::send`] without blocking the async runtime: waits for room
    /// with short sleeps.
    #[cfg(feature = "http2")]
    pub async fn send_async(&self, event: SseEvent) -> Result<(), SseClosed> {
        let mut frame = event.encode();
        loop {
            match self.tx.try_send(frame) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Disconnected(_)) => return Err(SseClosed),
                Err(TrySendError::Full(back)) => {
                    frame = back;
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            }
        }
    }

    /// Send a data-only event (the client's `message` event).
    pub fn data(&self, data: &str) -> Result<(), SseClosed> {
        self.send(SseEvent::data(data))
    }

    /// Send a comment line (ignored by clients).
    pub fn comment(&self, text: &str) -> Result<(), SseClosed> {
        self.tx.send(comment_bytes(text)).map_err(|_| SseClosed)
    }
}

/// The body of a streaming SSE response: a blocking [`std::io::Read`] over
/// the channel of encoded events, so the server's streaming writers (HTTP/1.1
/// chunked, HTTP/2 and HTTP/3 DATA) drive it. Waits for the next event;
/// after `keep_alive` with none, yields a `: keep-alive` comment instead;
/// at EOF when every sender is gone.
pub(crate) struct SseReader {
    rx: Receiver<Vec<u8>>,
    leftover: Vec<u8>,
    offset: usize,
    keep_alive: Duration,
}

impl SseReader {
    pub(crate) fn new(rx: Receiver<Vec<u8>>, keep_alive: Duration) -> Self {
        SseReader { rx, leftover: Vec::new(), offset: 0, keep_alive }
    }
}

impl std::io::Read for SseReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.offset >= self.leftover.len() {
            self.leftover = match self.rx.recv_timeout(self.keep_alive) {
                Ok(frame) => frame,
                Err(RecvTimeoutError::Timeout) => b": keep-alive\n\n".to_vec(),
                Err(RecvTimeoutError::Disconnected) => return Ok(0),
            };
            self.offset = 0;
        }
        let n = (self.leftover.len() - self.offset).min(buf.len());
        buf[..n].copy_from_slice(&self.leftover[self.offset..self.offset + n]);
        self.offset += n;
        Ok(n)
    }
}
