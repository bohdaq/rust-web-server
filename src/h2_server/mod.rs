//! rws's own HTTP/2 server (RFC 9113), used for every `h2` connection.
//!
//! One task per connection reads frames, keeps the stream states and both
//! flow-control windows, and writes. A bodiless request that the app answers
//! inline (`Application::runs_inline`) is executed right there; anything else
//! runs in a spawned task (`http1_async::execute_request`) whose response
//! comes back over a channel. Everything a loop iteration produces — SETTINGS
//! ACKs, HEADERS and DATA of several streams — goes out in one write: with
//! the `h2` crate a 1 KiB HTTP/2 response cost more CPU in framing and HPACK
//! than all of nginx's request handling.
//!
//! Untrusted input is bounded everywhere: frames above our
//! `SETTINGS_MAX_FRAME_SIZE` and header blocks above
//! [`MAX_HEADER_BLOCK`] end the connection, decoded header lists above
//! [`MAX_HEADER_LIST_SIZE`] get `431`, at most [`MAX_CONCURRENT_STREAMS`]
//! streams are open, and a client that resets streams faster than
//! [`MAX_RESETS_PER_WINDOW`] per [`RESET_WINDOW`] ("rapid reset") is sent
//! `GOAWAY(ENHANCE_YOUR_CALM)`. Frame parsing is a loop over a byte buffer;
//! nothing recurses on input.
//!
//! The `h2` crate is still used as an HTTP/2 *client* (`H2ReverseProxy`).

pub(crate) mod hpack;
pub mod fuzz;
mod huffman;
mod huffman_table;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use crate::application::Application;
use crate::header::Header;
use crate::http::VERSION;
use crate::log::Log;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

/// `SETTINGS_MAX_CONCURRENT_STREAMS` advertised to every client: without it
/// one connection may open unlimited simultaneous streams. 250 matches Go's
/// net/http (nginx uses 128).
pub const MAX_CONCURRENT_STREAMS: u32 = 250;

/// `SETTINGS_MAX_HEADER_LIST_SIZE`: the largest decoded request header list
/// (`name + value + 32` per field). Larger ones get `431`. Also the HTTP/3
/// limit in `h3_handler`.
pub const MAX_HEADER_LIST_SIZE: u32 = 64 * 1024;

/// Largest encoded header block (HEADERS plus CONTINUATIONs) we buffer;
/// past it the connection is closed (a CONTINUATION flood).
pub const MAX_HEADER_BLOCK: usize = 2 * MAX_HEADER_LIST_SIZE as usize;

/// Our `SETTINGS_MAX_FRAME_SIZE` (the protocol minimum).
const MAX_FRAME_SIZE: usize = 16_384;

/// Stream resets tolerated per [`RESET_WINDOW`] before `ENHANCE_YOUR_CALM`:
/// the client's own (CVE-2023-44487, "rapid reset") and the ones it makes
/// the server send by breaking a stream's rules — a zero or overflowing
/// `WINDOW_UPDATE`, a bad `PRIORITY`, frames on a closed stream
/// (CVE-2025-8671, "MadeYouReset"). Either way the request's work may
/// already be running, so both count against one budget.
pub const MAX_RESETS_PER_WINDOW: u32 = 1000;
pub const RESET_WINDOW: Duration = Duration::from_secs(10);

/// Window sizes from the protocol (§6.9.2).
const DEFAULT_WINDOW: i64 = 65_535;
const MAX_WINDOW: i64 = (1 << 31) - 1;

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

mod frame {
    pub const DATA: u8 = 0x0;
    pub const HEADERS: u8 = 0x1;
    pub const PRIORITY: u8 = 0x2;
    pub const RST_STREAM: u8 = 0x3;
    pub const SETTINGS: u8 = 0x4;
    pub const PUSH_PROMISE: u8 = 0x5;
    pub const PING: u8 = 0x6;
    pub const GOAWAY: u8 = 0x7;
    pub const WINDOW_UPDATE: u8 = 0x8;
    pub const CONTINUATION: u8 = 0x9;

    pub const END_STREAM: u8 = 0x1;
    pub const ACK: u8 = 0x1;
    pub const END_HEADERS: u8 = 0x4;
    pub const PADDED: u8 = 0x8;
    pub const PRIORITY_FLAG: u8 = 0x20;
}

/// Error codes (§7).
mod code {
    pub const NO_ERROR: u32 = 0x0;
    pub const PROTOCOL_ERROR: u32 = 0x1;
    pub const INTERNAL_ERROR: u32 = 0x2;
    pub const FLOW_CONTROL_ERROR: u32 = 0x3;
    pub const STREAM_CLOSED: u32 = 0x5;
    pub const FRAME_SIZE_ERROR: u32 = 0x6;
    pub const REFUSED_STREAM: u32 = 0x7;
    pub const CANCEL: u32 = 0x8;
    pub const COMPRESSION_ERROR: u32 = 0x9;
    pub const ENHANCE_YOUR_CALM: u32 = 0xb;
}

// Headers forbidden in HTTP/2 messages (RFC 9113 §8.2.2).
const CONNECTION_SPECIFIC: &[&str] = &["connection", "keep-alive", "proxy-connection", "transfer-encoding", "upgrade", "te"];

/// A connection error: `GOAWAY` with this code, then close.
#[derive(Debug)]
struct ConnError(u32);

/// A request being received.
struct Incoming {
    method: String,
    path: String,
    headers: Vec<Header>,
    body: Vec<u8>,
    content_length: Option<u64>,
}

/// A response body waiting for flow-control window.
struct Outgoing {
    data: Vec<u8>,
    sent: usize,
    /// END_STREAM once `data` is out and (if any) the producer is done.
    finish: bool,
    producer: Option<mpsc::Receiver<Result<Vec<u8>, String>>>,
    /// Sent as a final HEADERS frame (with END_STREAM) after the body, when
    /// it holds fields by then.
    trailers: Option<crate::response::Trailers>,
}

struct Stream {
    /// Request not complete yet (no END_STREAM received).
    receiving: bool,
    incoming: Option<Incoming>,
    /// HEADERS of the response sent.
    responded: bool,
    /// END_STREAM sent.
    done_sending: bool,
    send_window: i64,
    /// Bytes of the client's DATA not yet credited back on this stream.
    recv_unacked: u32,
    outgoing: Option<Outgoing>,
    /// A body streamed to a handler that is already running (#31): DATA
    /// goes here, and the window is credited back only as the handler reads.
    streamed: Option<StreamedBody>,
}

/// The connection's side of a streamed request body.
struct StreamedBody {
    tx: crate::server::body_pump::UnboundedBodySender,
    content_length: Option<u64>,
    received: u64,
}

impl Stream {
    fn closed(&self) -> bool {
        !self.receiving && self.done_sending
    }
}

/// A response finished by a spawned task: `None` for a 500.
type Completed = (u32, Option<(Response, bool)>);

struct Conn {
    /// Over TLS (not h2c).
    https: bool,
    out: Vec<u8>,
    decoder: hpack::Decoder,
    encoder: hpack::Encoder,
    streams: HashMap<u32, Stream>,
    /// Highest client stream id seen.
    last_stream_id: u32,
    /// Streams we reset recently: their in-flight frames are ignored.
    recently_reset: VecDeque<u32>,
    peer_max_frame_size: usize,
    peer_initial_window: i64,
    conn_send_window: i64,
    conn_recv_unacked: u32,
    /// Header block being assembled: stream id, END_STREAM of its HEADERS,
    /// bytes so far.
    continuation: Option<(u32, bool, Vec<u8>)>,
    got_settings: bool,
    goaway_received: bool,
    resets: (Instant, u32),
    max_body_size: u64,
    /// Requests running in spawned tasks.
    in_flight: usize,
    /// Streamed-body handlers report bytes read here: `(stream, bytes)`.
    credit_tx: mpsc::UnboundedSender<(u32, usize)>,
}

/// Serve one HTTP/2 connection (after ALPN chose `h2`).
pub async fn handle_connection<S>(
    io: S,
    peer_addr: SocketAddr,
    sni_hostname: Option<String>,
    app: impl Application + Send + Clone + 'static,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    serve(io, peer_addr, sni_hostname, true, app).await
}

/// [`handle_connection`] for a connection over TLS (`https`) or cleartext
/// (h2c): that decides `url.scheme` in the request metrics and whether
/// responses carry `Strict-Transport-Security`.
pub(crate) async fn serve<S>(
    mut io: S,
    peer_addr: SocketAddr,
    sni_hostname: Option<String>,
    https: bool,
    app: impl Application + Send + Clone + 'static,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (server_ip, server_port, _) = crate::entry_point::get_ip_port_thread_count();
    let connection = ConnectionInfo {
        client: crate::server::Address { ip: peer_addr.ip().to_string(), port: peer_addr.port() as i32 },
        server: crate::server::Address { ip: server_ip, port: server_port },
        request_size: 0,
        sni_hostname,
        tls: https,
    };
    let (credit_tx, mut credit_rx) = mpsc::unbounded_channel::<(u32, usize)>();
    let mut conn = Conn {
        out: Vec::with_capacity(16 * 1024),
        decoder: hpack::Decoder::new(hpack::DEFAULT_TABLE_SIZE),
        encoder: hpack::Encoder::new(),
        streams: HashMap::new(),
        last_stream_id: 0,
        recently_reset: VecDeque::new(),
        peer_max_frame_size: MAX_FRAME_SIZE,
        peer_initial_window: DEFAULT_WINDOW,
        conn_send_window: DEFAULT_WINDOW,
        conn_recv_unacked: 0,
        continuation: None,
        got_settings: false,
        goaway_received: false,
        resets: (Instant::now(), 0),
        max_body_size: crate::entry_point::get_max_body_size(),
        in_flight: 0,
        credit_tx,
        https,
    };
    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<Completed>();

    // Server preface: our SETTINGS.
    let settings: [(u16, u32); 2] = [(0x3, MAX_CONCURRENT_STREAMS), (0x6, MAX_HEADER_LIST_SIZE)];
    write_frame_header(&mut conn.out, settings.len() * 6, frame::SETTINGS, 0, 0);
    for (id, value) in settings {
        conn.out.extend_from_slice(&id.to_be_bytes());
        conn.out.extend_from_slice(&value.to_be_bytes());
    }

    // The client preface, then frames.
    let mut input: Vec<u8> = Vec::with_capacity(32 * 1024);
    let mut preface_seen = false;
    // Closes a connection that sends nothing for READ_TIMEOUT while the
    // server isn't working on anything for it: a silent client, a header
    // block or request body left half-sent, a client that never opens the
    // flow-control window. Pushed back on every read and every response.
    let idle = tokio::time::sleep(crate::server::http1_async::READ_TIMEOUT);
    tokio::pin!(idle);
    let result: Result<(), ConnError> = loop {
        if !conn.out.is_empty() {
            if io.write_all(&conn.out).await.is_err() || io.flush().await.is_err() {
                return Ok(());
            }
            conn.out.clear();
        }
        if conn.goaway_received && conn.streams.is_empty() && conn.in_flight == 0 {
            break Ok(());
        }

        // Unbiased, and finished responses are drained below whatever woke
        // us: a client that never stops sending can't starve them.
        let waiting_on_client = conn.waiting_on_client();
        let event = tokio::select! {
            read = io.read_buf(&mut input) => Event::Read(read),
            Some(done) = done_rx.recv() => Event::Done(done),
            chunk = next_chunk(&mut conn.streams) => Event::Chunk(chunk),
            Some((id, n)) = credit_rx.recv() => Event::Credit(id, n),
            _ = &mut idle, if waiting_on_client => Event::Idle,
        };
        if !matches!(event, Event::Idle) {
            idle.as_mut().reset(tokio::time::Instant::now() + crate::server::http1_async::READ_TIMEOUT);
        }
        let step = match event {
            Event::Read(Ok(0)) | Event::Read(Err(_)) => break Ok(()),
            Event::Idle => Err(ConnError(code::NO_ERROR)),
            Event::Read(Ok(_)) => {
                if !preface_seen {
                    if input.len() < PREFACE.len() {
                        if !PREFACE.starts_with(&input) {
                            break Err(ConnError(code::PROTOCOL_ERROR));
                        }
                        continue;
                    }
                    if &input[..PREFACE.len()] != PREFACE {
                        break Err(ConnError(code::PROTOCOL_ERROR));
                    }
                    input.drain(..PREFACE.len());
                    preface_seen = true;
                }
                conn.read_frames(&mut input, &app, &connection, peer_addr, &done_tx)
            }
            Event::Done((id, response)) => {
                conn.in_flight -= 1;
                conn.respond(id, response);
                Ok(())
            }
            Event::Chunk((id, chunk)) => {
                conn.take_chunk(id, chunk);
                Ok(())
            }
            Event::Credit(id, n) => {
                // A streamed body's handler read `n` bytes: the client may
                // send that much more on the stream.
                if let Some(s) = conn.streams.get_mut(&id) {
                    if s.receiving {
                        s.recv_unacked = s.recv_unacked.saturating_add(n as u32);
                    }
                }
                Ok(())
            }
        };
        if let Err(e) = step {
            break Err(e);
        }
        while let Ok((id, response)) = done_rx.try_recv() {
            conn.in_flight -= 1;
            conn.respond(id, response);
        }
        conn.credit_windows();
    };

    // Handlers still reading a body get an error, not a body that just ends.
    for s in conn.streams.values_mut() {
        if let Some(streamed) = s.streamed.take() {
            let _ = streamed.tx.send(Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "the HTTP/2 connection closed mid-body")));
        }
    }
    if let Err(ConnError(error)) = result {
        conn.goaway(error);
        let _ = io.write_all(&conn.out).await;
        let _ = io.flush().await;
    }
    let _ = io.shutdown().await;
    Ok(())
}

enum Event {
    Read(std::io::Result<usize>),
    /// A streamed body's handler read this many bytes.
    Credit(u32, usize),
    /// Nothing from the client for READ_TIMEOUT while nothing was running.
    Idle,
    Done(Completed),
    Chunk((u32, Option<Result<Vec<u8>, String>>)),
}

/// The next chunk of any streaming body that has nothing queued, or
/// pending forever when there is none.
async fn next_chunk(streams: &mut HashMap<u32, Stream>) -> (u32, Option<Result<Vec<u8>, String>>) {
    std::future::poll_fn(|cx| {
        for (id, stream) in streams.iter_mut() {
            let Some(out) = stream.outgoing.as_mut() else { continue };
            if out.sent < out.data.len() {
                continue;
            }
            if let Some(producer) = out.producer.as_mut() {
                if let std::task::Poll::Ready(chunk) = producer.poll_recv(cx) {
                    return std::task::Poll::Ready((*id, chunk));
                }
            }
        }
        std::task::Poll::Pending
    })
    .await
}

fn write_frame_header(out: &mut Vec<u8>, len: usize, kind: u8, flags: u8, stream: u32) {
    out.extend_from_slice(&(len as u32).to_be_bytes()[1..]);
    out.push(kind);
    out.push(flags);
    out.extend_from_slice(&(stream & 0x7fff_ffff).to_be_bytes());
}

fn read_u32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

impl Conn {
    /// Parse and handle every complete frame in `input`.
    fn read_frames<A: Application + Send + Clone + 'static>(
        &mut self,
        input: &mut Vec<u8>,
        app: &A,
        connection: &ConnectionInfo,
        peer: SocketAddr,
        done: &mpsc::UnboundedSender<Completed>,
    ) -> Result<(), ConnError> {
        let mut pos = 0;
        while input.len() - pos >= 9 {
            let h = &input[pos..pos + 9];
            let len = (h[0] as usize) << 16 | (h[1] as usize) << 8 | h[2] as usize;
            let (kind, flags) = (h[3], h[4]);
            let stream = read_u32(&h[5..9]) & 0x7fff_ffff;
            if len > MAX_FRAME_SIZE {
                return Err(ConnError(code::FRAME_SIZE_ERROR));
            }
            if input.len() - pos < 9 + len {
                break;
            }
            let payload = &input[pos + 9..pos + 9 + len];
            pos += 9 + len;
            if !self.got_settings && kind != frame::SETTINGS {
                return Err(ConnError(code::PROTOCOL_ERROR));
            }
            // A header block in progress admits only its CONTINUATIONs.
            if let Some((cont_stream, _, _)) = &self.continuation {
                if kind != frame::CONTINUATION || stream != *cont_stream {
                    return Err(ConnError(code::PROTOCOL_ERROR));
                }
            }
            self.frame(kind, flags, stream, payload, app, connection, peer, done)?;
            if self.resets.1 > MAX_RESETS_PER_WINDOW {
                return Err(ConnError(code::ENHANCE_YOUR_CALM));
            }
        }
        input.drain(..pos);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn frame<A: Application + Send + Clone + 'static>(
        &mut self,
        kind: u8,
        flags: u8,
        stream: u32,
        payload: &[u8],
        app: &A,
        connection: &ConnectionInfo,
        peer: SocketAddr,
        done: &mpsc::UnboundedSender<Completed>,
    ) -> Result<(), ConnError> {
        match kind {
            frame::SETTINGS => self.settings(flags, stream, payload),
            frame::PING => {
                if stream != 0 {
                    return Err(ConnError(code::PROTOCOL_ERROR));
                }
                if payload.len() != 8 {
                    return Err(ConnError(code::FRAME_SIZE_ERROR));
                }
                if flags & frame::ACK == 0 {
                    write_frame_header(&mut self.out, 8, frame::PING, frame::ACK, 0);
                    self.out.extend_from_slice(payload);
                }
                Ok(())
            }
            frame::GOAWAY => {
                if stream != 0 {
                    return Err(ConnError(code::PROTOCOL_ERROR));
                }
                if payload.len() < 8 {
                    return Err(ConnError(code::FRAME_SIZE_ERROR));
                }
                self.goaway_received = true;
                Ok(())
            }
            frame::WINDOW_UPDATE => self.window_update(stream, payload),
            frame::PRIORITY => {
                if stream == 0 {
                    return Err(ConnError(code::PROTOCOL_ERROR));
                }
                if payload.len() != 5 {
                    self.reset(stream, code::FRAME_SIZE_ERROR);
                    return Ok(());
                }
                if read_u32(payload) & 0x7fff_ffff == stream {
                    self.reset(stream, code::PROTOCOL_ERROR);
                }
                Ok(())
            }
            frame::RST_STREAM => {
                if stream == 0 {
                    return Err(ConnError(code::PROTOCOL_ERROR));
                }
                if payload.len() != 4 {
                    return Err(ConnError(code::FRAME_SIZE_ERROR));
                }
                if stream > self.last_stream_id {
                    return Err(ConnError(code::PROTOCOL_ERROR)); // idle
                }
                if let Some(mut removed) = self.streams.remove(&stream) {
                    if let Some(streamed) = removed.streamed.take() {
                        let _ = streamed.tx.send(Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "the client reset the stream mid-body")));
                    }
                    self.count_reset();
                }
                Ok(())
            }
            frame::PUSH_PROMISE => Err(ConnError(code::PROTOCOL_ERROR)),
            frame::HEADERS => self.headers(flags, stream, payload, app, connection, peer, done),
            frame::CONTINUATION => {
                let Some((cont_stream, end_stream, mut block)) = self.continuation.take() else {
                    return Err(ConnError(code::PROTOCOL_ERROR));
                };
                if block.len() + payload.len() > MAX_HEADER_BLOCK {
                    return Err(ConnError(code::ENHANCE_YOUR_CALM));
                }
                block.extend_from_slice(payload);
                if flags & frame::END_HEADERS != 0 {
                    self.header_block(cont_stream, end_stream, &block, app, connection, peer, done)
                } else {
                    self.continuation = Some((cont_stream, end_stream, block));
                    Ok(())
                }
            }
            frame::DATA => self.data(flags, stream, payload, app, connection, peer, done),
            _ => Ok(()), // unknown frame types are ignored (§4.1)
        }
    }

    fn settings(&mut self, flags: u8, stream: u32, payload: &[u8]) -> Result<(), ConnError> {
        if stream != 0 {
            return Err(ConnError(code::PROTOCOL_ERROR));
        }
        if flags & frame::ACK != 0 {
            if !payload.is_empty() {
                return Err(ConnError(code::FRAME_SIZE_ERROR));
            }
            return Ok(());
        }
        if payload.len() % 6 != 0 {
            return Err(ConnError(code::FRAME_SIZE_ERROR));
        }
        for setting in payload.chunks_exact(6) {
            let id = u16::from_be_bytes([setting[0], setting[1]]);
            let value = read_u32(&setting[2..]);
            match id {
                0x1 => self.encoder.set_peer_table_size(value as usize),
                0x2 if value > 1 => return Err(ConnError(code::PROTOCOL_ERROR)),
                0x4 => {
                    if value as i64 > MAX_WINDOW {
                        return Err(ConnError(code::FLOW_CONTROL_ERROR));
                    }
                    let delta = value as i64 - self.peer_initial_window;
                    self.peer_initial_window = value as i64;
                    for s in self.streams.values_mut() {
                        s.send_window += delta;
                        if s.send_window > MAX_WINDOW {
                            return Err(ConnError(code::FLOW_CONTROL_ERROR));
                        }
                    }
                }
                0x5 => {
                    if !(16_384..=16_777_215).contains(&value) {
                        return Err(ConnError(code::PROTOCOL_ERROR));
                    }
                    self.peer_max_frame_size = value as usize;
                }
                _ => {}
            }
        }
        self.got_settings = true;
        write_frame_header(&mut self.out, 0, frame::SETTINGS, frame::ACK, 0);
        // A larger window may let blocked bodies continue.
        self.pump_all();
        Ok(())
    }

    fn window_update(&mut self, stream: u32, payload: &[u8]) -> Result<(), ConnError> {
        if payload.len() != 4 {
            return Err(ConnError(code::FRAME_SIZE_ERROR));
        }
        let increment = (read_u32(payload) & 0x7fff_ffff) as i64;
        if stream == 0 {
            if increment == 0 {
                return Err(ConnError(code::PROTOCOL_ERROR));
            }
            self.conn_send_window += increment;
            if self.conn_send_window > MAX_WINDOW {
                return Err(ConnError(code::FLOW_CONTROL_ERROR));
            }
            self.pump_all();
            return Ok(());
        }
        if stream > self.last_stream_id {
            return Err(ConnError(code::PROTOCOL_ERROR)); // idle
        }
        if increment == 0 {
            self.reset(stream, code::PROTOCOL_ERROR);
            return Ok(());
        }
        let Some(s) = self.streams.get_mut(&stream) else { return Ok(()) };
        s.send_window += increment;
        if s.send_window > MAX_WINDOW {
            self.reset(stream, code::FLOW_CONTROL_ERROR);
            return Ok(());
        }
        self.pump(stream);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn headers<A: Application + Send + Clone + 'static>(
        &mut self,
        flags: u8,
        stream: u32,
        payload: &[u8],
        app: &A,
        connection: &ConnectionInfo,
        peer: SocketAddr,
        done: &mpsc::UnboundedSender<Completed>,
    ) -> Result<(), ConnError> {
        if stream == 0 || stream % 2 == 0 {
            return Err(ConnError(code::PROTOCOL_ERROR));
        }
        let mut block = payload;
        if flags & frame::PADDED != 0 {
            let pad = *block.first().ok_or(ConnError(code::FRAME_SIZE_ERROR))? as usize;
            if pad >= block.len() {
                return Err(ConnError(code::PROTOCOL_ERROR));
            }
            block = &block[1..block.len() - pad];
        }
        if flags & frame::PRIORITY_FLAG != 0 {
            if block.len() < 5 {
                return Err(ConnError(code::FRAME_SIZE_ERROR));
            }
            if read_u32(block) & 0x7fff_ffff == stream {
                // Depends on itself: a stream error, but the block must still
                // be decoded to keep HPACK in step.
                let rest = block[5..].to_vec();
                self.decode_and_discard(&rest)?;
                self.reset(stream, code::PROTOCOL_ERROR);
                return Ok(());
            }
            block = &block[5..];
        }
        let end_stream = flags & frame::END_STREAM != 0;
        if flags & frame::END_HEADERS != 0 {
            self.header_block(stream, end_stream, block, app, connection, peer, done)
        } else {
            if block.len() > MAX_HEADER_BLOCK {
                return Err(ConnError(code::ENHANCE_YOUR_CALM));
            }
            self.continuation = Some((stream, end_stream, block.to_vec()));
            Ok(())
        }
    }

    fn decode_and_discard(&mut self, block: &[u8]) -> Result<(), ConnError> {
        match self.decoder.decode(block, usize::MAX, |_, _| {}) {
            Err(hpack::DecodeError::Compression) => Err(ConnError(code::COMPRESSION_ERROR)),
            _ => Ok(()),
        }
    }

    /// A complete header block: a new request, or trailers.
    #[allow(clippy::too_many_arguments)]
    fn header_block<A: Application + Send + Clone + 'static>(
        &mut self,
        stream: u32,
        end_stream: bool,
        block: &[u8],
        app: &A,
        connection: &ConnectionInfo,
        peer: SocketAddr,
        done: &mpsc::UnboundedSender<Completed>,
    ) -> Result<(), ConnError> {
        // Trailers on an open stream.
        if let Some(s) = self.streams.get(&stream) {
            if !s.receiving {
                self.decode_and_discard(block)?;
                self.reset(stream, code::STREAM_CLOSED);
                return Ok(());
            }
            let mut pseudo = false;
            let decoded = self.decoder.decode(block, MAX_HEADER_LIST_SIZE as usize, |n, _| pseudo |= n.first() == Some(&b':'));
            if let Err(hpack::DecodeError::Compression) = decoded {
                return Err(ConnError(code::COMPRESSION_ERROR));
            }
            if pseudo || !end_stream {
                self.reset(stream, code::PROTOCOL_ERROR);
                return Ok(());
            }
            return self.request_complete(stream, app, connection, peer, done);
        }
        if stream <= self.last_stream_id {
            self.decode_and_discard(block)?;
            if self.recently_reset.contains(&stream) {
                return Ok(());
            }
            return Err(ConnError(code::STREAM_CLOSED));
        }
        self.last_stream_id = stream;

        let mut fields = Fields::default();
        let decoded = self.decoder.decode(block, MAX_HEADER_LIST_SIZE as usize, |n, v| fields.add(n, v));
        match decoded {
            Err(hpack::DecodeError::Compression) => return Err(ConnError(code::COMPRESSION_ERROR)),
            Err(hpack::DecodeError::ListTooLarge) => {
                self.open(stream, !end_stream);
                self.respond_status(stream, 431);
                return Ok(());
            }
            Ok(()) => {}
        }
        if self.goaway_received {
            self.reset_new(stream, code::REFUSED_STREAM);
            return Ok(());
        }
        let open = self.streams.values().filter(|s| !s.closed()).count();
        if open >= MAX_CONCURRENT_STREAMS as usize {
            self.reset_new(stream, code::REFUSED_STREAM);
            return Ok(());
        }
        let Some(incoming) = fields.into_request() else {
            self.reset_new(stream, code::PROTOCOL_ERROR);
            return Ok(());
        };
        self.open(stream, true);
        if !end_stream {
            let head = Request {
                method: incoming.method.clone(),
                request_uri: incoming.path.clone(),
                http_version: VERSION.http_2_0.to_string(),
                headers: incoming.headers.clone(),
                body: Default::default(),
            };
            if app.streams_body(&head, connection) {
                self.start_streamed(stream, head, incoming.content_length, app, connection, peer, done);
                return Ok(());
            }
        }
        if let Some(s) = self.streams.get_mut(&stream) {
            s.incoming = Some(incoming);
        }
        if end_stream {
            self.request_complete(stream, app, connection, peer, done)?;
        }
        Ok(())
    }

    /// Run a request whose body the application streams (#31) now, on the
    /// blocking pool, with DATA to come through a channel.
    #[allow(clippy::too_many_arguments)]
    fn start_streamed<A: Application + Send + Clone + 'static>(
        &mut self,
        stream: u32,
        mut request: Request,
        content_length: Option<u64>,
        app: &A,
        connection: &ConnectionInfo,
        peer: SocketAddr,
        done: &mpsc::UnboundedSender<Completed>,
    ) {
        let credit = self.credit_tx.clone();
        let (tx, body) = crate::server::body_pump::flow_controlled_body_channel(move |n| {
            let _ = credit.send((stream, n));
        });
        request.body = body;
        if let Some(s) = self.streams.get_mut(&stream) {
            s.streamed = Some(StreamedBody { tx, content_length, received: 0 });
        }
        self.in_flight += 1;
        let (app, connection, done, https) = (app.clone(), connection.clone(), done.clone(), self.https);
        let started = Instant::now();
        tokio::spawn(async move {
            let response = match crate::server::http1_async::execute_blocking(app, request, connection).await {
                Ok((request, result)) => finish(request, Ok(result), peer, started, https),
                Err(e) => {
                    eprintln!("Handler panicked on H2 stream: {}", e);
                    None
                }
            };
            let _ = done.send((stream, response));
        });
    }

    fn open(&mut self, stream: u32, receiving: bool) {
        self.streams.insert(
            stream,
            Stream {
                receiving,
                incoming: None,
                responded: false,
                done_sending: false,
                send_window: self.peer_initial_window,
                recv_unacked: 0,
                outgoing: None,
                streamed: None,
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn data<A: Application + Send + Clone + 'static>(
        &mut self,
        flags: u8,
        stream: u32,
        payload: &[u8],
        app: &A,
        connection: &ConnectionInfo,
        peer: SocketAddr,
        done: &mpsc::UnboundedSender<Completed>,
    ) -> Result<(), ConnError> {
        if stream == 0 {
            return Err(ConnError(code::PROTOCOL_ERROR));
        }
        // Flow control counts the whole payload, padding included, and is
        // credited back whatever happens to the stream.
        self.conn_recv_unacked += payload.len() as u32;
        let mut data = payload;
        if flags & frame::PADDED != 0 {
            let pad = *data.first().ok_or(ConnError(code::FRAME_SIZE_ERROR))? as usize;
            if pad >= data.len() {
                return Err(ConnError(code::PROTOCOL_ERROR));
            }
            data = &data[1..data.len() - pad];
        }
        if stream > self.last_stream_id {
            return Err(ConnError(code::PROTOCOL_ERROR)); // idle
        }
        let max_body = self.max_body_size;
        let Some(s) = self.streams.get_mut(&stream) else {
            if self.recently_reset.contains(&stream) {
                return Ok(());
            }
            return Err(ConnError(code::STREAM_CLOSED));
        };
        if !s.receiving {
            self.reset(stream, code::STREAM_CLOSED);
            return Ok(());
        }
        if let Some(streamed) = s.streamed.as_mut() {
            // Padding is credited now; the data as the handler reads it.
            s.recv_unacked += (payload.len() - data.len()) as u32;
            streamed.received += data.len() as u64;
            let end = flags & frame::END_STREAM != 0;
            let too_large = max_body > 0 && streamed.received > max_body;
            let wrong_length = end && streamed.content_length.is_some_and(|len| len != streamed.received);
            let error = if too_large {
                Some(format!("the request body exceeds the {} byte limit", max_body))
            } else if wrong_length {
                Some("the request body doesn't match its content-length".to_string())
            } else {
                None
            };
            let delivered = !data.is_empty() && error.is_none() && streamed.tx.send(Ok(data.to_vec())).is_ok();
            if !data.is_empty() && !delivered {
                // Nobody will read it: give the window back at once.
                s.recv_unacked += data.len() as u32;
            }
            if let Some(message) = error {
                let _ = streamed.tx.send(Err(std::io::Error::new(std::io::ErrorKind::InvalidData, message)));
                s.streamed = None;
                self.reset(stream, if too_large { code::CANCEL } else { code::PROTOCOL_ERROR });
                return Ok(());
            }
            if end {
                s.streamed = None; // the sender drops: the handler reads to the end
                s.receiving = false;
                s.recv_unacked = 0;
                self.close_if_done(stream);
            }
            return Ok(());
        }
        s.recv_unacked += payload.len() as u32;
        let Some(incoming) = s.incoming.as_mut() else {
            // A refused (431) request still sending its body.
            if flags & frame::END_STREAM != 0 {
                s.receiving = false;
                self.close_if_done(stream);
            }
            return Ok(());
        };
        incoming.body.extend_from_slice(data);
        if max_body > 0 && incoming.body.len() as u64 > max_body {
            s.incoming = None;
            self.respond_status(stream, 413);
            return Ok(());
        }
        if flags & frame::END_STREAM != 0 {
            self.request_complete(stream, app, connection, peer, done)?;
        }
        Ok(())
    }

    /// END_STREAM received: check the body against `content-length` and run
    /// the request.
    fn request_complete<A: Application + Send + Clone + 'static>(
        &mut self,
        stream: u32,
        app: &A,
        connection: &ConnectionInfo,
        peer: SocketAddr,
        done: &mpsc::UnboundedSender<Completed>,
    ) -> Result<(), ConnError> {
        let Some(s) = self.streams.get_mut(&stream) else { return Ok(()) };
        s.receiving = false;
        s.recv_unacked = 0; // nothing more to credit on a finished stream
        let Some(incoming) = s.incoming.take() else { return Ok(()) };
        if incoming.content_length.is_some_and(|len| len != incoming.body.len() as u64) {
            self.reset(stream, code::PROTOCOL_ERROR);
            return Ok(());
        }
        let request = Request {
            method: incoming.method,
            request_uri: incoming.path,
            http_version: VERSION.http_2_0.to_string(),
            headers: incoming.headers,
            body: incoming.body.into(),
        };
        // As on HTTP/1.1: a quick bodiless request the app answers inline
        // runs on this task; anything else (bodies, proxying, handlers that
        // may block) in its own.
        let started = Instant::now();
        if request.body.is_empty() && app.runs_inline(&request, connection) {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.execute(&request, connection)))
                .map_err(|_| "handler panicked".to_string());
            let response = finish(request, result, peer, started, self.https);
            self.respond(stream, response);
            return Ok(());
        }
        self.in_flight += 1;
        let (app, connection, done, https) = (app.clone(), connection.clone(), done.clone(), self.https);
        tokio::spawn(async move {
            let response = match crate::server::http1_async::execute_request(app, request, &connection).await {
                Ok((request, result)) => finish(request, Ok(result), peer, started, https),
                Err(e) => {
                    eprintln!("Handler panicked on H2 stream: {}", e);
                    None
                }
            };
            let _ = done.send((stream, response));
        });
        Ok(())
    }

    /// Send the app's response (or a 500) on `stream`.
    fn respond(&mut self, stream: u32, response: Option<(Response, bool)>) {
        let Some((mut response, is_head)) = response else {
            self.respond_status(stream, 500);
            return;
        };
        if !self.streams.contains_key(&stream) {
            return; // reset by the client meanwhile
        }
        let mut producer = crate::server::body_stream::spawn_producer(&mut response);
        if is_head {
            producer = None;
        }
        promote_body_headers(&mut response);
        let body = if is_head { Vec::new() } else { Response::generate_body(std::mem::take(&mut response.content_range_list)) };
        let trailers = if is_head { None } else { response.trailers.take() };
        let end_stream = body.is_empty() && producer.is_none() && trailers.is_none();

        let mut block = Vec::with_capacity(256);
        self.encoder.begin(&mut block);
        self.encoder.status(&mut block, u16::try_from(response.status_code).ok().filter(|s| (100..=999).contains(s)).unwrap_or(500));
        let mut name = Vec::with_capacity(32);
        for header in &response.headers {
            if CONNECTION_SPECIFIC.iter().any(|f| header.name.eq_ignore_ascii_case(f)) {
                continue;
            }
            name.clear();
            name.extend(header.name.bytes().map(|b| b.to_ascii_lowercase()));
            // Skip what HeaderName / HeaderValue would have refused.
            if name.is_empty() || !name.iter().all(|b| is_tchar(*b)) || !header.value.bytes().all(is_value_byte) {
                continue;
            }
            self.encoder.field(&mut block, &name, header.value.as_bytes(), changes_every_response(&header.name));
        }
        self.send_header_block(stream, &block, end_stream);
        if let Some(s) = self.streams.get_mut(&stream) {
            s.responded = true;
            if end_stream {
                s.done_sending = true;
            } else {
                s.outgoing = Some(Outgoing { data: body, sent: 0, finish: true, producer, trailers });
            }
        }
        if end_stream {
            self.close_if_done(stream);
        } else {
            self.pump(stream);
        }
    }

    /// A bodiless response with just a status (431, 413, 500).
    fn respond_status(&mut self, stream: u32, status: u16) {
        if !self.streams.contains_key(&stream) {
            return;
        }
        let mut block = Vec::with_capacity(8);
        self.encoder.begin(&mut block);
        self.encoder.status(&mut block, status);
        self.send_header_block(stream, &block, true);
        if let Some(s) = self.streams.get_mut(&stream) {
            s.responded = true;
            s.done_sending = true;
            s.incoming = None;
        }
        self.close_if_done(stream);
    }

    /// HEADERS (plus CONTINUATIONs beyond the client's frame size).
    fn send_header_block(&mut self, stream: u32, block: &[u8], end_stream: bool) {
        let max = self.peer_max_frame_size;
        let mut chunks = block.chunks(max).peekable();
        let first = chunks.next().unwrap_or(&[]);
        let mut flags = if end_stream { frame::END_STREAM } else { 0 };
        if chunks.peek().is_none() {
            flags |= frame::END_HEADERS;
        }
        write_frame_header(&mut self.out, first.len(), frame::HEADERS, flags, stream);
        self.out.extend_from_slice(first);
        while let Some(chunk) = chunks.next() {
            let flags = if chunks.peek().is_none() { frame::END_HEADERS } else { 0 };
            write_frame_header(&mut self.out, chunk.len(), frame::CONTINUATION, flags, stream);
            self.out.extend_from_slice(chunk);
        }
    }

    /// Send as much of `stream`'s queued body as the windows allow.
    fn pump(&mut self, stream: u32) {
        let max = self.peer_max_frame_size;
        let Some(s) = self.streams.get_mut(&stream) else { return };
        let Some(out) = s.outgoing.as_mut() else { return };
        // Trailers are complete once the producer has ended.
        let mut trailers = Vec::new();
        loop {
            let remaining = out.data.len() - out.sent;
            let last = out.finish && out.producer.is_none();
            if last && trailers.is_empty() {
                if let Some(slot) = out.trailers.as_ref() {
                    trailers = slot.get();
                }
            }
            // With trailers, their HEADERS frame ends the stream instead.
            let end_flag = if trailers.is_empty() { frame::END_STREAM } else { 0 };
            if remaining == 0 {
                if last && !s.done_sending {
                    if trailers.is_empty() {
                        write_frame_header(&mut self.out, 0, frame::DATA, frame::END_STREAM, stream);
                    }
                    s.done_sending = true;
                }
                break;
            }
            let window = self.conn_send_window.min(s.send_window);
            if window <= 0 {
                break;
            }
            let n = remaining.min(window as usize).min(max);
            let end = last && n == remaining;
            write_frame_header(&mut self.out, n, frame::DATA, if end { end_flag } else { 0 }, stream);
            self.out.extend_from_slice(&out.data[out.sent..out.sent + n]);
            out.sent += n;
            self.conn_send_window -= n as i64;
            s.send_window -= n as i64;
            if end {
                s.done_sending = true;
                break;
            }
        }
        if out.sent == out.data.len() && !out.data.is_empty() {
            out.data.clear();
            out.sent = 0;
        }
        if s.done_sending {
            s.outgoing = None;
            if !trailers.is_empty() {
                self.send_trailers(stream, &trailers);
            }
            self.close_if_done(stream);
        }
    }

    /// The trailer section: a HEADERS frame with END_STREAM, no pseudo-headers.
    fn send_trailers(&mut self, stream: u32, trailers: &[crate::header::Header]) {
        let mut block = Vec::with_capacity(64);
        self.encoder.begin(&mut block);
        let mut name = Vec::with_capacity(32);
        for header in trailers {
            if CONNECTION_SPECIFIC.iter().any(|f| header.name.eq_ignore_ascii_case(f)) {
                continue;
            }
            name.clear();
            name.extend(header.name.bytes().map(|b| b.to_ascii_lowercase()));
            if name.is_empty() || !name.iter().all(|b| is_tchar(*b)) || !header.value.bytes().all(is_value_byte) {
                continue;
            }
            self.encoder.field(&mut block, &name, header.value.as_bytes(), true);
        }
        self.send_header_block(stream, &block, true);
    }

    fn pump_all(&mut self) {
        let ids: Vec<u32> = self.streams.iter().filter(|(_, s)| s.outgoing.is_some()).map(|(id, _)| *id).collect();
        for id in ids {
            self.pump(id);
        }
    }

    /// A streaming body produced its next chunk (or ended).
    fn take_chunk(&mut self, stream: u32, chunk: Option<Result<Vec<u8>, String>>) {
        let Some(out) = self.streams.get_mut(&stream).and_then(|s| s.outgoing.as_mut()) else { return };
        match chunk {
            Some(Ok(data)) => {
                out.data = data;
                out.sent = 0;
            }
            Some(Err(e)) => {
                eprintln!("H2 streaming body error: {}", e);
                self.reset(stream, code::INTERNAL_ERROR);
                return;
            }
            None => out.producer = None,
        }
        self.pump(stream);
    }

    /// Forget a finished stream. Answered while the client is still
    /// sending (413, 431): `RST_STREAM(NO_ERROR)` tells it to stop (§8.1).
    fn close_if_done(&mut self, stream: u32) {
        match self.streams.get(&stream) {
            Some(s) if s.closed() => {
                self.streams.remove(&stream);
            }
            Some(s) if s.done_sending && s.incoming.is_none() => self.reset(stream, code::NO_ERROR),
            _ => {}
        }
    }

    /// `true` when no request is running and no streamed body is being
    /// produced: the connection can only move if the client does.
    fn waiting_on_client(&self) -> bool {
        // A handler reading a streamed body that is still arriving waits on
        // the client too: a client that stalls mid-upload times out.
        let waiting_for_body = self.streams.values().filter(|s| s.streamed.is_some() && s.receiving).count();
        self.in_flight <= waiting_for_body
            && self.streams.values().all(|s| s.outgoing.as_ref().is_none_or(|o| o.producer.is_none()))
    }

    /// One more stream reset in the current [`RESET_WINDOW`]; the frame
    /// loop closes the connection past [`MAX_RESETS_PER_WINDOW`].
    fn count_reset(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.resets.0) > RESET_WINDOW {
            self.resets = (now, 0);
        }
        self.resets.1 += 1;
    }

    /// RST_STREAM an existing stream. Anything but `NO_ERROR` (a stream we
    /// finished early) is the client's doing and counts toward the reset
    /// budget (MadeYouReset).
    fn reset(&mut self, stream: u32, error: u32) {
        if error != code::NO_ERROR {
            self.count_reset();
        }
        write_frame_header(&mut self.out, 4, frame::RST_STREAM, 0, stream);
        self.out.extend_from_slice(&error.to_be_bytes());
        self.streams.remove(&stream);
        self.recently_reset.push_back(stream);
        if self.recently_reset.len() > 64 {
            self.recently_reset.pop_front();
        }
    }

    /// RST_STREAM a stream we never opened (it still counts as used).
    fn reset_new(&mut self, stream: u32, error: u32) {
        self.reset(stream, error);
    }

    fn goaway(&mut self, error: u32) {
        write_frame_header(&mut self.out, 8, frame::GOAWAY, 0, 0);
        self.out.extend_from_slice(&self.last_stream_id.to_be_bytes());
        self.out.extend_from_slice(&error.to_be_bytes());
    }

    /// Give the client back the receive window its DATA used.
    fn credit_windows(&mut self) {
        if self.conn_recv_unacked > 0 {
            write_frame_header(&mut self.out, 4, frame::WINDOW_UPDATE, 0, 0);
            self.out.extend_from_slice(&self.conn_recv_unacked.to_be_bytes());
            self.conn_recv_unacked = 0;
        }
        for (id, s) in self.streams.iter_mut() {
            if s.recv_unacked > 0 && s.receiving {
                write_frame_header(&mut self.out, 4, frame::WINDOW_UPDATE, 0, *id);
                self.out.extend_from_slice(&s.recv_unacked.to_be_bytes());
                s.recv_unacked = 0;
            }
        }
    }
}

/// Request fields as they're decoded, validated as RFC 9113 §8.3 asks.
#[derive(Default)]
struct Fields {
    method: Option<String>,
    scheme: bool,
    path: Option<String>,
    authority: bool,
    /// Its value: the request's `Host` when no `host` field came (RFC 9113 §8.3.1).
    authority_value: Option<String>,
    headers: Vec<Header>,
    content_length: Option<u64>,
    malformed: bool,
}

impl Fields {
    fn add(&mut self, name: &[u8], value: &[u8]) {
        if self.malformed {
            return;
        }
        if value.iter().any(|b| matches!(b, b'\0' | b'\r' | b'\n')) {
            self.malformed = true;
            return;
        }
        if let Some(pseudo) = name.strip_prefix(b":") {
            // Pseudo-headers come first, once each, and only these four.
            if !self.headers.is_empty() {
                self.malformed = true;
                return;
            }
            let text = || String::from_utf8_lossy(value).into_owned();
            let duplicate = match pseudo {
                b"method" => self.method.replace(text()).is_some(),
                b"path" => self.path.replace(text()).is_some(),
                b"scheme" => std::mem::replace(&mut self.scheme, true),
                b"authority" => {
                    self.authority_value = Some(text());
                    std::mem::replace(&mut self.authority, true)
                }
                _ => true,
            };
            self.malformed |= duplicate;
            return;
        }
        if name.is_empty() || !name.iter().all(|b| is_tchar(*b) && !b.is_ascii_uppercase()) {
            self.malformed = true;
            return;
        }
        if name == b"te" {
            if value != b"trailers" {
                self.malformed = true;
            } else {
                // Kept for the app: a gRPC upstream needs it forwarded.
                self.headers.push(Header { name: "te".into(), value: "trailers".into() });
            }
            return;
        }
        if CONNECTION_SPECIFIC.iter().any(|f| f.as_bytes() == name) {
            self.malformed = true;
            return;
        }
        if name == b"content-length" {
            match std::str::from_utf8(value).ok().and_then(|v| v.trim().parse::<u64>().ok()) {
                Some(len) if self.content_length.is_none_or(|l| l == len) => self.content_length = Some(len),
                _ => {
                    self.malformed = true;
                    return;
                }
            }
        }
        // As `HeaderValue::to_str`: values with bytes outside visible ASCII
        // (obs-text) aren't passed to the app.
        let Ok(value) = std::str::from_utf8(value) else { return };
        if !value.bytes().all(|b| b == b'\t' || (0x20..0x7f).contains(&b)) {
            return;
        }
        let Ok(name) = std::str::from_utf8(name) else { return };
        self.headers.push(Header { name: Header::name_from(name), value: value.to_string().into() });
    }

    fn into_request(self) -> Option<Incoming> {
        if self.malformed {
            return None;
        }
        let method = self.method?;
        let path = if method == "CONNECT" {
            if !self.authority || self.scheme || self.path.is_some() {
                return None;
            }
            "/".to_string()
        } else {
            let path = self.path.filter(|p| !p.is_empty())?;
            if !self.scheme {
                return None;
            }
            path
        };
        let mut headers = self.headers;
        if let Some(authority) = self.authority_value.filter(|a| !a.is_empty()) {
            if !headers.iter().any(|h| h.name.eq_ignore_ascii_case("host")) {
                headers.push(Header { name: Header::_HOST.into(), value: authority.into() });
            }
        }
        Some(Incoming { method, path, headers, body: Default::default(), content_length: self.content_length })
    }
}

/// RFC 9110 `tchar`.
fn is_tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

/// What `HeaderValue` accepts: visible ASCII, space, tab and obs-text.
fn is_value_byte(b: u8) -> bool {
    b == b'\t' || (b >= 0x20 && b != 0x7f)
}

/// Values that change on every response, kept out of the HPACK table:
/// indexing them would evict the headers that do repeat.
fn changes_every_response(name: &str) -> bool {
    [
        Header::_DATE,
        Header::_DATE_UNIX_EPOCH_NANOS,
        Header::_LAST_MODIFIED,
        Header::_LAST_MODIFIED_UNIX_EPOCH_NANOS,
        Header::_ETAG,
    ]
    .iter()
    .any(|volatile| name.eq_ignore_ascii_case(volatile))
}

/// Content-Type, Content-Length (and Content-Range on 206) from
/// `content_range_list` into the headers, as `generate_response` does for
/// HTTP/1.1.
fn promote_body_headers(response: &mut Response) {
    if response.content_range_list.len() == 1 {
        let cr = &response.content_range_list[0];
        let content_type = cr.content_type.clone();
        let content_length = crate::ext::decimal::Decimal::new(cr.body.len() as u128).as_str().to_string();
        let content_range = crate::response::content_range_header_value(response.status_code, cr);
        response.headers.push(Header { name: Header::_CONTENT_TYPE.into(), value: content_type.into() });
        response.headers.push(Header { name: Header::_CONTENT_LENGTH.into(), value: content_length.into() });
        if let Some(value) = content_range {
            response.headers.push(Header { name: Header::_CONTENT_RANGE.into(), value: value.into() });
        }
    } else if response.content_range_list.len() > 1 {
        response.headers.push(Header {
            name: Header::_CONTENT_TYPE.into(),
            value: crate::range::Range::multipart_content_type(&response.content_range_list).into(),
        });
    }
}

/// After the app ran: metrics, gzip, HSTS (over TLS) and the access log;
/// `None` (a 500) for an error.
fn finish(request: Request, result: Result<Result<Response, String>, String>, peer: SocketAddr, started: Instant, https: bool) -> Option<(Response, bool)> {
    let mut response = match result {
        Ok(Ok(r)) => r,
        Ok(Err(message)) => {
            eprintln!("App error on H2 stream: {}", message);
            crate::metrics::http::observe(&request.method, 500, https, "HTTP/2", started);
            return None;
        }
        Err(e) => {
            eprintln!("Handler panicked on H2 stream: {}", e);
            crate::metrics::http::observe(&request.method, 500, https, "HTTP/2", started);
            return None;
        }
    };
    crate::metrics::record_request();
    crate::metrics::http::observe(&request.method, response.status_code, https, "HTTP/2", started);
    crate::compression::apply_compression(&request, &mut response);
    // 1xx/204/304: no DATA frame, no content-length (#54).
    response.drop_content_for_status();
    if https {
        response.headers.push(Header::get_hsts_header());
    }
    Log::log_access(&request, &response, &peer);
    let is_head = request.method == crate::request::METHOD.head;
    Some((response, is_head))
}
