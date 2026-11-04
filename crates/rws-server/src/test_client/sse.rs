//! Server-Sent Events in tests: [`TestResponse::sse`](super::TestResponse::sse)
//! returns a [`TestSse`], which reads the response's `text/event-stream`
//! body — streamed (`Sse::stream`, `Response::stream_pipe`) as it arrives, or
//! buffered (`Sse::into_response`) at once — and hands out [`SseFrame`]s.
//!
//! [`SseParser`] is the parser underneath, usable on its own: it follows the
//! HTML Standard's event-stream rules (lines end in CRLF, LF or CR; `:`
//! starts a comment; `data` lines join with `\n`; a block without `data` is
//! not dispatched; the event type defaults to `message`). It loops over its
//! input, never recursing, and refuses an event (or a line) over
//! [`MAX_EVENT_BYTES`] instead of buffering without bound.

use std::io::Read;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// The most one event (all its lines) or one unfinished line may hold:
/// 8 MiB. Past it [`SseParser::feed`] returns an error.
pub const MAX_EVENT_BYTES: usize = 8 * 1024 * 1024;

/// One dispatched Server-Sent Event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseFrame {
    /// The `event` field; `"message"` when the event had none, as a
    /// browser's `EventSource` reports it.
    pub event: String,
    /// The `data` lines joined with `\n`.
    pub data: String,
    /// The `id` field of this event, if it had one (the stream's
    /// last-event-ID is the last `Some` seen).
    pub id: Option<String>,
    /// The `retry` field (milliseconds), if this event had a valid one.
    pub retry: Option<u64>,
}

/// An incremental `text/event-stream` parser: feed it bytes in any chunks,
/// get back the events they completed.
///
/// ```rust
/// # extern crate rws_server as rust_web_server;
/// use rust_web_server::test_client::SseParser;
///
/// let mut parser = SseParser::new();
/// assert!(parser.feed(b"event: tick\ndata: 1\n").unwrap().is_empty());
/// let events = parser.feed(b"id: 7\n\n").unwrap();
/// assert_eq!("tick", events[0].event);
/// assert_eq!("1", events[0].data);
/// assert_eq!(Some("7"), events[0].id.as_deref());
/// ```
#[derive(Debug, Default)]
pub struct SseParser {
    line: Vec<u8>,
    /// The previous byte was a CR (so an LF now ends nothing).
    after_cr: bool,
    /// The first bytes are still to be checked for a BOM.
    started: bool,
    event: Option<String>,
    data: String,
    has_data: bool,
    id: Option<String>,
    retry: Option<u64>,
    /// Bytes buffered in the current event's fields.
    pending: usize,
    comments: usize,
}

impl SseParser {
    /// A parser at the start of a stream.
    pub fn new() -> Self {
        SseParser::default()
    }

    /// Comment lines (`: …`, keep-alives) seen so far.
    pub fn comments(&self) -> usize {
        self.comments
    }

    /// Parse `bytes`, returning the events whose blank line they contained.
    /// An event or line over [`MAX_EVENT_BYTES`] is an error (the parser is
    /// then reset to the start of a stream).
    pub fn feed(&mut self, mut bytes: &[u8]) -> Result<Vec<SseFrame>, String> {
        let mut events = Vec::new();
        if !self.started && !bytes.is_empty() {
            // A UTF-8 BOM may open the stream; it isn't part of a field.
            // (A BOM split across chunks is rare enough to treat as data.)
            if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                bytes = &bytes[3..];
            }
            self.started = true;
        }
        for &byte in bytes {
            match byte {
                b'\n' if self.after_cr => self.after_cr = false,
                b'\n' | b'\r' => {
                    self.after_cr = byte == b'\r';
                    let line = std::mem::take(&mut self.line);
                    if let Some(event) = self.line_done(&line) {
                        events.push(event);
                    }
                    self.line = line;
                    self.line.clear();
                }
                _ => {
                    self.after_cr = false;
                    if self.line.len() + self.pending >= MAX_EVENT_BYTES {
                        *self = SseParser::default();
                        return Err(format!("an SSE event over {MAX_EVENT_BYTES} bytes"));
                    }
                    self.line.push(byte);
                }
            }
        }
        Ok(events)
    }

    /// Handle one complete line; a blank one dispatches the event.
    fn line_done(&mut self, line: &[u8]) -> Option<SseFrame> {
        if line.is_empty() {
            let event = self.event.take();
            let id = self.id.take();
            let retry = self.retry.take();
            let data = std::mem::take(&mut self.data);
            let has_data = std::mem::replace(&mut self.has_data, false);
            self.pending = 0;
            if !has_data {
                return None;
            }
            return Some(SseFrame { event: event.unwrap_or_else(|| "message".to_string()), data, id, retry });
        }
        if line[0] == b':' {
            self.comments += 1;
            return None;
        }
        let (name, value) = match line.iter().position(|&b| b == b':') {
            Some(colon) => {
                let value = &line[colon + 1..];
                (&line[..colon], value.strip_prefix(b" ").unwrap_or(value))
            }
            None => (line, &b""[..]),
        };
        // Every line counts (an empty `data:` still adds a `\n`), so a
        // flood of short lines is bounded as well as one long one.
        self.pending += line.len() + 1;
        let value = String::from_utf8_lossy(value).into_owned();
        match name {
            b"event" => self.event = Some(value),
            b"data" => {
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(&value);
                self.has_data = true;
            }
            b"id" if !value.contains('\0') => self.id = Some(value),
            b"retry" if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                self.retry = value.parse().ok();
            }
            _ => {}
        }
        None
    }
}

/// What the reading thread sends: a chunk, the end, or a read error.
enum Chunk {
    Bytes(Vec<u8>),
    End,
    Failed(String),
}

/// Server-Sent Events read from a test response; see the module docs.
pub struct TestSse {
    parser: SseParser,
    ready: std::collections::VecDeque<SseFrame>,
    chunks: Option<Receiver<Chunk>>,
    ended: bool,
    error: Option<String>,
}

impl TestSse {
    /// Events from a buffered body.
    pub fn from_bytes(body: &[u8]) -> TestSse {
        let mut sse = TestSse::empty();
        sse.take_in(body);
        sse.ended = true;
        sse
    }

    /// Events from a streamed body, read on a thread of its own so that
    /// waits can time out. The thread ends with the stream, or at the
    /// first read after this `TestSse` is dropped (for `Sse::stream`, the
    /// next event or keep-alive comment).
    pub fn from_reader(mut reader: Box<dyn Read + Send>) -> TestSse {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("rws-test-sse".to_string()).spawn(move || {
            let mut buf = vec![0u8; 8192];
            loop {
                let chunk = match reader.read(&mut buf) {
                    Ok(0) => Chunk::End,
                    Ok(n) => Chunk::Bytes(buf[..n].to_vec()),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => Chunk::Failed(e.to_string()),
                };
                let last = !matches!(chunk, Chunk::Bytes(_));
                if tx.send(chunk).is_err() || last {
                    return;
                }
            }
        });
        let mut sse = TestSse::empty();
        match spawned {
            Ok(_) => sse.chunks = Some(rx),
            Err(e) => {
                sse.ended = true;
                sse.error = Some(format!("couldn't start the SSE reader thread: {e}"));
            }
        }
        sse
    }

    fn empty() -> TestSse {
        TestSse { parser: SseParser::new(), ready: Default::default(), chunks: None, ended: false, error: None }
    }

    fn take_in(&mut self, bytes: &[u8]) {
        match self.parser.feed(bytes) {
            Ok(events) => self.ready.extend(events),
            Err(e) => {
                self.error = Some(e);
                self.ended = true;
                self.chunks = None;
            }
        }
    }

    /// The next event, waiting at most `timeout`; `None` when the stream
    /// ended (see [`is_ended`](Self::is_ended)) or the time ran out.
    pub fn next_event(&mut self, timeout: Duration) -> Option<SseFrame> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(event) = self.ready.pop_front() {
                return Some(event);
            }
            if self.ended {
                return None;
            }
            let rx = self.chunks.as_ref()?;
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(Chunk::Bytes(bytes)) => self.take_in(&bytes),
                Ok(Chunk::End) | Err(RecvTimeoutError::Disconnected) => self.finish(None),
                Ok(Chunk::Failed(e)) => self.finish(Some(e)),
                Err(RecvTimeoutError::Timeout) => return None,
            }
        }
    }

    fn finish(&mut self, error: Option<String>) {
        self.ended = true;
        self.chunks = None;
        if error.is_some() {
            self.error = error;
        }
    }

    /// Up to `n` events, waiting at most `timeout` in all.
    pub fn events(&mut self, n: usize, timeout: Duration) -> Vec<SseFrame> {
        let deadline = Instant::now() + timeout;
        let mut events = Vec::new();
        while events.len() < n {
            match self.next_event(deadline.saturating_duration_since(Instant::now())) {
                Some(event) => events.push(event),
                None => break,
            }
        }
        events
    }

    /// Every event until the stream ends, waiting at most `timeout` in
    /// all. For a stream that never ends (`Sse::stream` with a sender kept
    /// alive) this waits the whole `timeout`: prefer [`events`](Self::events).
    pub fn collect_until_end(&mut self, timeout: Duration) -> Vec<SseFrame> {
        self.events(usize::MAX, timeout)
    }

    /// `true` once the stream has ended (and every event was read out of it).
    pub fn is_ended(&self) -> bool {
        self.ended && self.ready.is_empty()
    }

    /// Why the stream stopped, if it was a read or parse error.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Comment lines (keep-alives) seen so far.
    pub fn comments(&self) -> usize {
        self.parser.comments()
    }
}
