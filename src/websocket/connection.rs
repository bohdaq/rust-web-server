//! WebSocket endpoints in an ordinary handler: [`WebSocket::upgrade`]
//! returns the `101 Switching Protocols` response, and once the server has
//! sent it, the connection goes to the handler as a [`WebSocketConnection`]
//! on a thread of its own (tokio's blocking pool in async builds, a new
//! thread in the `http1` build). HTTP/1.1 only: an HTTP/2 or HTTP/3 request
//! can't upgrade and gets `426 Upgrade Required`.
//!
//! How the handler reaches the server: the response carries an internal
//! header with a random token, and the handler waits under that token in a
//! process-wide table. The server removes the header, takes the handler and
//! hands it the socket (with any bytes the client sent after the request).
//! A response that never reaches a client (replaced by a middleware) leaves
//! its handler behind for at most [`UPGRADE_TTL`]. An upstream can't use
//! the header: tokens are random, and unknown ones do nothing.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::{close_payload, encode_frame, read_raw_frame, Frame, RawFrame, WebSocket};
use crate::core::New;
use crate::header::Header;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

/// The internal header carrying the token from handler to server.
const UPGRADE_HEADER: &str = "X-Rws-WebSocket-Upgrade";
/// How long a handler waits for its response to be sent.
pub const UPGRADE_TTL: Duration = Duration::from_secs(60);
/// The default limit on one message (all its fragments): 16 MiB.
pub const MAX_MESSAGE: usize = 16 * 1024 * 1024;

type Handler = Box<dyn FnOnce(WebSocketConnection) + Send>;

/// A complete message from the client: fragments joined, text checked to
/// be UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
}

/// An upgraded connection. [`recv`](Self::recv) returns the client's
/// messages, answering pings and the closing handshake itself; send with
/// the methods here or a [`WebSocketSender`] from [`sender`](Self::sender),
/// which another thread can use while this one waits in `recv`.
pub struct WebSocketConnection {
    reader: Box<dyn Read + Send>,
    sender: WebSocketSender,
    max_message: usize,
    closed: bool,
}

/// Sends to one WebSocket client; cheap to clone, usable from any thread.
#[derive(Clone)]
pub struct WebSocketSender {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    closed: Arc<AtomicBool>,
}

impl WebSocketSender {
    fn send(&self, frame: Frame) -> Result<(), String> {
        let closing = matches!(frame, Frame::Close(..));
        if self.closed.load(Ordering::Acquire) {
            return Err("the WebSocket is closed".to_string());
        }
        if closing {
            self.closed.store(true, Ordering::Release);
        }
        let bytes = encode_frame(frame);
        let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        writer.write_all(&bytes).and_then(|_| writer.flush()).map_err(|e| format!("write error: {e}"))
    }

    /// Send a text message.
    pub fn send_text(&self, text: &str) -> Result<(), String> {
        self.send(Frame::Text(text.to_string()))
    }

    /// Send a binary message.
    pub fn send_binary(&self, data: &[u8]) -> Result<(), String> {
        self.send(Frame::Binary(data.to_vec()))
    }

    /// Send a ping (at most 125 bytes); the client's pong is consumed by
    /// [`WebSocketConnection::recv`].
    pub fn ping(&self, payload: &[u8]) -> Result<(), String> {
        if payload.len() > 125 {
            return Err("a ping payload is at most 125 bytes".to_string());
        }
        self.send(Frame::Ping(payload.to_vec()))
    }

    /// Start the closing handshake with `code` (1000 = normal) and `reason`;
    /// nothing more can be sent after it.
    pub fn close(&self, code: u16, reason: &str) -> Result<(), String> {
        self.send(Frame::Close(Some(code), reason.to_string()))
    }

    /// Whether a close frame was sent (by either side's handshake).
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

/// A close code the client may send (RFC 6455 §7.4).
fn valid_close_code(code: u16) -> bool {
    matches!(code, 1000..=1003 | 1007..=1011 | 3000..=4999)
}

impl WebSocketConnection {
    /// A connection over `reader` (client → server) and `writer`.
    pub(crate) fn new(reader: Box<dyn Read + Send>, writer: Box<dyn Write + Send>) -> Self {
        WebSocketConnection {
            reader,
            sender: WebSocketSender { writer: Arc::new(Mutex::new(writer)), closed: Arc::new(AtomicBool::new(false)) },
            max_message: MAX_MESSAGE,
            closed: false,
        }
    }

    /// Refuse messages over `bytes` (default [`MAX_MESSAGE`]) with close
    /// code 1009.
    pub fn max_message_size(mut self, bytes: usize) -> Self {
        self.max_message = bytes;
        self
    }

    /// A sender for this connection, for other threads.
    pub fn sender(&self) -> WebSocketSender {
        self.sender.clone()
    }

    /// Send a text message.
    pub fn send_text(&self, text: &str) -> Result<(), String> {
        self.sender.send_text(text)
    }

    /// Send a binary message.
    pub fn send_binary(&self, data: &[u8]) -> Result<(), String> {
        self.sender.send_binary(data)
    }

    /// Start the closing handshake; [`recv`](Self::recv) then returns
    /// `Ok(None)` when the client answers.
    pub fn close(&self, code: u16, reason: &str) -> Result<(), String> {
        self.sender.close(code, reason)
    }

    /// Fail the connection: a close frame with `code`, then `Err(reason)`.
    fn fail<T>(&mut self, code: u16, reason: String) -> Result<T, String> {
        self.closed = true;
        let _ = self.sender.close(code, "");
        Err(reason)
    }

    /// The next message; `Ok(None)` once the connection closed cleanly
    /// (the closing handshake done, whoever started it). Pings are
    /// answered and pongs dropped on the way. A protocol error — an
    /// unmasked frame, a bad fragment, invalid UTF-8, an oversized message
    /// — closes the connection with the matching code and is returned as
    /// `Err`, as is the client disappearing without a close frame.
    pub fn recv(&mut self) -> Result<Option<Message>, String> {
        if self.closed {
            return Ok(None);
        }
        let mut message: Option<(u8, Vec<u8>)> = None;
        loop {
            let assembled = message.as_ref().map_or(0, |(_, data)| data.len());
            let limit = self.max_message.saturating_sub(assembled) as u64;
            let RawFrame { fin, rsv, opcode, masked, payload } = match read_raw_frame(&mut self.reader, limit) {
                Ok(frame) => frame,
                Err(e) if e.contains("exceeds") => return self.fail(1009, format!("message over {} bytes", self.max_message)),
                Err(e) => {
                    self.closed = true;
                    return Err(e);
                }
            };
            if rsv != 0 {
                return self.fail(1002, "reserved bits set (no extension was negotiated)".to_string());
            }
            if !masked {
                return self.fail(1002, "a client frame wasn't masked".to_string());
            }
            if opcode >= 0x8 && (!fin || payload.len() > 125) {
                return self.fail(1002, "a control frame was fragmented or over 125 bytes".to_string());
            }
            match opcode {
                0x8 => {
                    let (code, _) = match close_payload(&payload) {
                        Ok(close) => close,
                        Err(e) => return self.fail(1007, e),
                    };
                    if payload.len() == 1 || code.is_some_and(|c| !valid_close_code(c)) {
                        return self.fail(1002, format!("invalid close code {code:?}"));
                    }
                    self.closed = true;
                    // Answer the client's close, unless this ends ours.
                    if !self.sender.is_closed() {
                        let _ = self.sender.close(code.unwrap_or(1000), "");
                    }
                    return Ok(None);
                }
                0x9 => {
                    let _ = self.sender.send(Frame::Pong(payload));
                }
                0xA => {}
                0x1 | 0x2 if message.is_some() => return self.fail(1002, "a new message began inside a fragmented one".to_string()),
                0x1 | 0x2 => message = Some((opcode, payload)),
                0x0 => match message.as_mut() {
                    Some((_, data)) => data.extend_from_slice(&payload),
                    None => return self.fail(1002, "a continuation frame with no message".to_string()),
                },
                other => return self.fail(1002, format!("unknown opcode 0x{other:X}")),
            }
            if fin && opcode < 0x8 {
                let Some((kind, data)) = message.take() else { continue };
                return match kind {
                    0x1 => match String::from_utf8(data) {
                        Ok(text) => Ok(Some(Message::Text(text))),
                        Err(_) => self.fail(1007, "a text message isn't UTF-8".to_string()),
                    },
                    _ => Ok(Some(Message::Binary(data))),
                };
            }
        }
    }
}

// ── Upgrade ───────────────────────────────────────────────────────────────────

/// A handler waiting for its connection.
pub(crate) struct Upgrade(Handler);

impl Upgrade {
    /// Run the handler on `reader` / `writer` (blocks for the connection's
    /// life).
    pub(crate) fn run(self, reader: Box<dyn Read + Send>, writer: Box<dyn Write + Send>) {
        (self.0)(WebSocketConnection::new(reader, writer))
    }
}

fn waiting() -> &'static Mutex<HashMap<String, (Instant, Handler)>> {
    static WAITING: OnceLock<Mutex<HashMap<String, (Instant, Handler)>>> = OnceLock::new();
    WAITING.get_or_init(Default::default)
}

/// 128 random bits as hex: SipHash with the process's random keys over a
/// counter and the time.
fn token() -> String {
    static KEYS: OnceLock<std::collections::hash_map::RandomState> = OnceLock::new();
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let keys = KEYS.get_or_init(Default::default);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let half = |salt: u8| {
        let mut hasher = keys.build_hasher();
        hasher.write_u8(salt);
        hasher.write_u64(n);
        hasher.write_u128(nanos);
        hasher.finish()
    };
    format!("{:016x}{:016x}", half(1), half(2))
}

impl WebSocket {
    /// Answer a WebSocket upgrade request: returns the `101` response to
    /// return from the handler, and runs `handler` with the connection once
    /// it's sent. A request that isn't a valid upgrade (or comes over HTTP/2
    /// or HTTP/3) gets `426 Upgrade Required` and `handler` never runs.
    ///
    /// ```rust,no_run
    /// use rust_web_server::app::App;
    /// use rust_web_server::websocket::{Message, WebSocket};
    ///
    /// let app = App::with_state(()).get("/echo", |request, _params, _conn, _state| {
    ///     WebSocket::upgrade(request, |mut ws| {
    ///         while let Ok(Some(message)) = ws.recv() {
    ///             let _ = match message {
    ///                 Message::Text(text) => ws.send_text(&text),
    ///                 Message::Binary(data) => ws.send_binary(&data),
    ///             };
    ///         }
    ///     })
    /// });
    /// ```
    pub fn upgrade<F>(request: &Request, handler: F) -> Response
    where
        F: FnOnce(WebSocketConnection) + Send + 'static,
    {
        let version = request.header("Sec-WebSocket-Version").map(|h| h.value.trim().to_string()).unwrap_or_default();
        let mut response = match WebSocket::handshake_response(request) {
            Ok(response) if Self::is_upgrade_request(request) && version == "13" && request.method == "GET" => response,
            _ => {
                let mut refused = Response::new();
                refused.status_code = *STATUS_CODE_REASON_PHRASE.n426_upgrade_required.status_code;
                refused.reason_phrase = STATUS_CODE_REASON_PHRASE.n426_upgrade_required.reason_phrase.to_string();
                refused.headers.push(Header { name: "Upgrade".into(), value: "websocket".into() });
                refused.headers.push(Header { name: "Sec-WebSocket-Version".into(), value: "13".into() });
                return refused;
            }
        };
        let token = token();
        let mut waiting = waiting().lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        waiting.retain(|_, (since, _)| now.duration_since(*since) < UPGRADE_TTL);
        waiting.insert(token.clone(), (now, Box::new(handler)));
        response.headers.push(Header { name: UPGRADE_HEADER.into(), value: token.into() });
        response
    }
}

/// The handler waiting for `response`, if it's a `101` from
/// [`WebSocket::upgrade`]; the internal header is removed either way.
pub(crate) fn take_upgrade(response: &mut Response) -> Option<Upgrade> {
    let at = response.headers.iter().position(|h| h.name.eq_ignore_ascii_case(UPGRADE_HEADER))?;
    let token = response.headers.remove(at).value;
    if response.status_code != 101 {
        return None;
    }
    let (_, handler) = waiting().lock().unwrap_or_else(|e| e.into_inner()).remove(token.as_ref())?;
    Some(Upgrade(handler))
}

/// The `101` response head as sent: the status line and the response's
/// headers — no body, no `Content-Length` (RFC 9110 §8.6).
pub(crate) fn upgrade_head(response: &Response) -> Vec<u8> {
    let mut head = format!("HTTP/1.1 101 {}\r\n", response.reason_phrase);
    for header in &response.headers {
        if header.name.eq_ignore_ascii_case("content-length") || header.name.eq_ignore_ascii_case("transfer-encoding") {
            continue;
        }
        head.push_str(&header.name);
        head.push_str(": ");
        head.push_str(&header.value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    head.into_bytes()
}

/// Blocking `Read`/`Write` over one half of an async stream, for a handler
/// on the blocking pool.
#[cfg(feature = "http2")]
struct Blocking<T> {
    inner: T,
    handle: tokio::runtime::Handle,
}

#[cfg(feature = "http2")]
impl<T: tokio::io::AsyncRead + Unpin> Read for Blocking<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.handle.block_on(tokio::io::AsyncReadExt::read(&mut self.inner, buf))
    }
}

#[cfg(feature = "http2")]
impl<T: tokio::io::AsyncWrite + Unpin> Write for Blocking<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.handle.block_on(tokio::io::AsyncWriteExt::write(&mut self.inner, buf))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.handle.block_on(tokio::io::AsyncWriteExt::flush(&mut self.inner))
    }
}

/// Give an async connection (plain or TLS) to `upgrade` on the blocking
/// pool, `pending` (bytes already read past the request) first.
#[cfg(feature = "http2")]
pub(crate) fn hand_over_async<S>(stream: S, pending: Vec<u8>, upgrade: Upgrade)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + 'static,
{
    let handle = tokio::runtime::Handle::current();
    let (read, write) = tokio::io::split(stream);
    let reader = std::io::Cursor::new(pending).chain(Blocking { inner: read, handle: handle.clone() });
    let writer = Blocking { inner: write, handle };
    tokio::task::spawn_blocking(move || upgrade.run(Box::new(reader), Box::new(writer)));
}
