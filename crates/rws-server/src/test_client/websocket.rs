//! WebSocket routes in tests: [`TestClient::websocket`](super::TestClient::websocket)
//! sends the opening handshake through the application, and when the route
//! answers with [`WebSocket::upgrade`]'s `101`, [`TestWebSocketRequest::connect`]
//! runs its handler on a thread of its own over an in-memory duplex — the
//! same handler the server would run, given the same reader and writer
//! types it expects, with no socket in between. The test side is a
//! [`TestWebSocket`]: it masks what it sends, as a browser does (the
//! handler's `recv` refuses unmasked frames), answers pings, and joins
//! fragmented messages.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use super::{TestRequest, TestResponse};
use crate::application::Application;
use crate::websocket::{close_payload, encode_frame, read_raw_frame, take_upgrade, Frame, Message, WebSocket};

/// How long a [`TestWebSocket`] waits for the handler by default: 5 s.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// The `Sec-WebSocket-Key` the test client sends (RFC 6455's sample nonce).
const KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

/// A WebSocket upgrade request being built; see
/// [`TestClient::websocket`](super::TestClient::websocket).
pub struct TestWebSocketRequest<'a, A: Application> {
    request: TestRequest<'a, A>,
    timeout: Duration,
}

impl<'a, A: Application> TestWebSocketRequest<'a, A> {
    pub(super) fn new(request: TestRequest<'a, A>) -> Self {
        TestWebSocketRequest { request, timeout: DEFAULT_TIMEOUT }
    }

    /// Add a request header (an `Authorization` or `Cookie` for a guarded
    /// route, a `Sec-WebSocket-Protocol`, …).
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.request = self.request.header(name, value);
        self
    }

    /// How long [`TestWebSocket::recv`] and [`close`](TestWebSocket::close)
    /// wait for the handler (default [`DEFAULT_TIMEOUT`]).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Send the handshake. A `101` from [`WebSocket::upgrade`] starts the
    /// handler and returns the connection; any other answer — `426` for a
    /// route that isn't a WebSocket, `401` from an auth layer, a `101` built
    /// by hand without a handler — comes back as the `Err` response.
    pub fn connect(self) -> Result<TestWebSocket, TestResponse> {
        let mut request = self.request;
        for (name, value) in [
            ("Host", "localhost"),
            ("Upgrade", "websocket"),
            ("Connection", "Upgrade"),
            ("Sec-WebSocket-Key", KEY),
            ("Sec-WebSocket-Version", "13"),
        ] {
            if !request.has_header(name) {
                request = request.header(name, value);
            }
        }
        let mut response = request.dispatch();
        let upgrade = take_upgrade(&mut response);
        let accepted = response
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("Sec-WebSocket-Accept"))
            .is_some_and(|h| *h.value == *WebSocket::accept_key(KEY));
        let handshake = TestResponse::from_response(response);
        let upgrade = match upgrade {
            Some(upgrade) if handshake.status() == 101 && accepted => upgrade,
            _ => return Err(handshake),
        };

        let (to_server, server_in) = mpsc::channel();
        let (server_out, from_server) = mpsc::channel();
        let (done_tx, done) = mpsc::channel::<()>();
        #[cfg(feature = "http2")]
        let runtime = tokio::runtime::Handle::try_current().ok();
        let spawned = std::thread::Builder::new().name("rws-test-websocket".to_string()).spawn(move || {
            // A handler that reaches for the test's tokio runtime finds it,
            // as it would on the server's blocking pool.
            #[cfg(feature = "http2")]
            let _entered = runtime.as_ref().map(|r| r.enter());
            let reader = PipeReader::new(server_in, None);
            let writer = PipeWriter(server_out);
            upgrade.run(Box::new(reader), Box::new(writer));
            let _ = done_tx.send(());
        });
        if spawned.is_err() {
            return Err(handshake);
        }
        Ok(TestWebSocket {
            handshake,
            to_server: Some(PipeWriter(to_server)),
            from_server: PipeReader::new(from_server, Some(self.timeout)),
            done,
            timeout: self.timeout,
            close: None,
            mask: 0x9E37_79B9,
        })
    }
}

/// The test side of an upgraded connection; see the module docs.
///
/// Dropping it ends the connection (the handler's `recv` returns an error),
/// without waiting for the handler.
pub struct TestWebSocket {
    handshake: TestResponse,
    to_server: Option<PipeWriter>,
    from_server: PipeReader,
    done: Receiver<()>,
    timeout: Duration,
    close: Option<(Option<u16>, String)>,
    mask: u32,
}

impl TestWebSocket {
    /// The `101` response, for its headers (`Sec-WebSocket-Protocol`, …).
    pub fn handshake(&self) -> &TestResponse {
        &self.handshake
    }

    /// Send a text message. Panics if the handler's side is gone.
    #[track_caller]
    pub fn send_text(&mut self, text: &str) {
        self.send(Frame::Text(text.to_string())).expect("send a WebSocket text message");
    }

    /// Send a binary message. Panics if the handler's side is gone.
    #[track_caller]
    pub fn send_binary(&mut self, data: &[u8]) {
        self.send(Frame::Binary(data.to_vec())).expect("send a WebSocket binary message");
    }

    /// Send a ping; the handler's pong is returned by
    /// [`recv_frame`](Self::recv_frame) (and skipped by [`recv`](Self::recv)).
    pub fn ping(&mut self, payload: &[u8]) -> Result<(), String> {
        self.send(Frame::Ping(payload.to_vec()))
    }

    /// Send `frame`, masked as a client must.
    pub fn send(&mut self, frame: Frame) -> Result<(), String> {
        let masked = mask_frame(encode_frame(frame), self.next_mask());
        self.send_raw(&masked)
    }

    /// Send bytes as they are — for testing how the handler copes with
    /// frames a well-behaved client wouldn't send (unmasked, bad opcodes).
    pub fn send_raw(&mut self, bytes: &[u8]) -> Result<(), String> {
        match self.to_server.as_mut() {
            Some(writer) => writer.write_all(bytes).map_err(|e| format!("write error: {e}")),
            None => Err("the test side already closed the connection".to_string()),
        }
    }

    fn next_mask(&mut self) -> [u8; 4] {
        // Masks only need to vary, not to be secret (they're for proxies).
        self.mask = self.mask.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.mask.to_be_bytes()
    }

    /// The next frame from the handler, waiting at most the timeout. Pings
    /// are answered (and returned); nothing is joined.
    pub fn recv_frame(&mut self) -> Result<Frame, String> {
        self.recv_frame_fin().map(|(_, frame)| frame)
    }

    /// [`recv_frame`](Self::recv_frame) with the frame's FIN bit.
    fn recv_frame_fin(&mut self) -> Result<(bool, Frame), String> {
        let raw = read_raw_frame(&mut self.from_server, WebSocket::MAX_FRAME_PAYLOAD)?;
        let fin = raw.fin;
        let frame = match raw.opcode {
            0x0 => Frame::Continuation { fin: raw.fin, data: raw.payload },
            0x1 => Frame::Text(String::from_utf8(raw.payload).map_err(|_| "a text frame isn't UTF-8".to_string())?),
            0x2 => Frame::Binary(raw.payload),
            0x8 => {
                let (code, reason) = close_payload(&raw.payload)?;
                self.close = Some((code, reason.clone()));
                Frame::Close(code, reason)
            }
            0x9 => {
                let _ = self.send(Frame::Pong(raw.payload.clone()));
                Frame::Ping(raw.payload)
            }
            0xA => Frame::Pong(raw.payload),
            other => return Err(format!("unknown opcode 0x{other:X} from the handler")),
        };
        Ok((fin, frame))
    }

    /// The next message from the handler: `Ok(None)` once it sent a close
    /// frame (answered here, see [`close_code`](Self::close_code)), `Err` on
    /// a timeout or when the handler returned without closing. Pings and
    /// pongs are skipped, fragments joined.
    pub fn recv(&mut self) -> Result<Option<Message>, String> {
        if self.close.is_some() {
            return Ok(None);
        }
        let mut message: Option<(bool, Vec<u8>)> = None;
        loop {
            let (fin, frame) = self.recv_frame_fin()?;
            match frame {
                Frame::Text(text) => message = Some((true, text.into_bytes())),
                Frame::Binary(data) => message = Some((false, data)),
                Frame::Continuation { data, .. } => match message.as_mut() {
                    Some((_, buf)) => buf.extend_from_slice(&data),
                    None => return Err("a continuation frame with no message".to_string()),
                },
                Frame::Close(code, _) => {
                    // Answer the handler's close, as a client would.
                    let _ = self.send(Frame::Close(code.or(Some(1000)), String::new()));
                    return Ok(None);
                }
                Frame::Ping(_) | Frame::Pong(_) => continue,
            }
            // A text or binary frame without FIN waits for its continuations.
            if !fin {
                continue;
            }
            return match message.take() {
                Some((true, bytes)) => String::from_utf8(bytes).map(|t| Some(Message::Text(t))).map_err(|_| "a text message isn't UTF-8".to_string()),
                Some((false, bytes)) => Ok(Some(Message::Binary(bytes))),
                None => continue,
            };
        }
    }

    /// The next message, which must be text. Panics otherwise (binary, a
    /// close, a timeout), with what came instead.
    #[track_caller]
    pub fn recv_text(&mut self) -> String {
        match self.recv() {
            Ok(Some(Message::Text(text))) => text,
            other => panic!("expected a WebSocket text message, got {other:?}"),
        }
    }

    /// The next message, which must be binary. Panics otherwise.
    #[track_caller]
    pub fn recv_binary(&mut self) -> Vec<u8> {
        match self.recv() {
            Ok(Some(Message::Binary(data))) => data,
            other => panic!("expected a WebSocket binary message, got {other:?}"),
        }
    }

    /// The close code the handler sent, once it closed (`Some(None)` for a
    /// close frame without one).
    pub fn close_code(&self) -> Option<Option<u16>> {
        self.close.as_ref().map(|(code, _)| *code)
    }

    /// Run the closing handshake: send a close frame with `code` and
    /// `reason`, skip messages until the handler's close (or the timeout),
    /// then end the connection and wait (at most the timeout) for the
    /// handler to return. Returns the handler's close code, `None` when it
    /// didn't send one.
    pub fn close(mut self, code: u16, reason: &str) -> Option<u16> {
        if self.close.is_none() {
            let _ = self.send(Frame::Close(Some(code), reason.to_string()));
            while self.close.is_none() {
                if self.recv_frame().is_err() {
                    break;
                }
            }
        }
        self.to_server = None;
        let _ = self.done.recv_timeout(self.timeout);
        self.close.and_then(|(code, _)| code)
    }

    /// Wait at most `timeout` for the handler to return; `true` if it did.
    pub fn handler_finished(&self, timeout: Duration) -> bool {
        match self.done.recv_timeout(timeout) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => true,
            Err(RecvTimeoutError::Timeout) => false,
        }
    }
}

/// Set the mask bit on a server-style (unmasked) frame and mask its payload.
fn mask_frame(mut frame: Vec<u8>, key: [u8; 4]) -> Vec<u8> {
    let offset = match frame[1] & 0x7F {
        126 => 4,
        127 => 10,
        _ => 2,
    };
    frame[1] |= 0x80;
    for (i, byte) in frame[offset..].iter_mut().enumerate() {
        *byte ^= key[i % 4];
    }
    frame.splice(offset..offset, key);
    frame
}

/// One direction of the in-memory duplex: each write is a chunk.
struct PipeWriter(Sender<Vec<u8>>);

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .send(buf.to_vec())
            .map(|_| buf.len())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "the other side of the test WebSocket is gone"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The reading end: blocks for the next chunk (at most `timeout`, when
/// set); the writer gone is the end of the stream.
struct PipeReader {
    rx: Receiver<Vec<u8>>,
    buf: Vec<u8>,
    pos: usize,
    timeout: Option<Duration>,
}

impl PipeReader {
    fn new(rx: Receiver<Vec<u8>>, timeout: Option<Duration>) -> Self {
        PipeReader { rx, buf: Vec::new(), pos: 0, timeout }
    }
}

impl Read for PipeReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.buf.len() {
            let next = match self.timeout {
                Some(timeout) => self.rx.recv_timeout(timeout).map_err(|e| match e {
                    RecvTimeoutError::Timeout => Some(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("no WebSocket frame from the handler within {timeout:?}"),
                    )),
                    RecvTimeoutError::Disconnected => None,
                }),
                None => self.rx.recv().map_err(|_| None),
            };
            match next {
                Ok(chunk) => {
                    self.buf = chunk;
                    self.pos = 0;
                }
                Err(Some(e)) => return Err(e),
                Err(None) => return Ok(0),
            }
        }
        let n = out.len().min(self.buf.len() - self.pos);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}
