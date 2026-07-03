//! WebSocket protocol support — RFC 6455.
//!
//! Provides the building blocks for adding WebSocket endpoints to a server:
//! upgrade detection, the opening handshake, and frame-level read/write.
//!
//! # Integration pattern
//!
//! WebSocket connections require taking over the raw TCP stream after sending
//! the 101 response, so they cannot be handled inside a normal
//! [`Controller::process`](crate::controller::Controller) call (which has no
//! access to the stream).  The recommended pattern is to bypass
//! [`Server::run`](crate::server::Server) for upgraded connections and drive
//! them with your own accept loop:
//!
//! ```rust,no_run
//! use std::net::TcpListener;
//! use rust_web_server::websocket::{WebSocket, Frame};
//! use rust_web_server::request::Request;
//! use rust_web_server::response::Response;
//!
//! let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
//! for stream in listener.incoming() {
//!     let mut stream = stream.unwrap();
//!     // peek / read request bytes, decide if it's a WS upgrade
//!     // (simplified — real code should read the full request first)
//!     let raw = vec![0u8; 4096];
//!     // ... parse into Request, then:
//!     //
//!     // if WebSocket::is_upgrade_request(&request) {
//!     //     let response = WebSocket::handshake_response(&request).unwrap();
//!     //     // write the 101 response to stream, then frame loop
//!     //     loop {
//!     //         match WebSocket::read_frame(&mut stream) { ... }
//!     //     }
//!     // } else {
//!     //     // normal Server::process
//!     // }
//! }
//! ```

#[cfg(test)]
mod tests;

use std::io::{Read, Write};

use crate::header::Header;
use crate::http::VERSION;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

/// A decoded WebSocket frame.
#[derive(Debug, PartialEq, Eq)]
pub enum Frame {
    /// A complete UTF-8 text message (opcode 0x1).
    Text(String),
    /// A binary message (opcode 0x2).
    Binary(Vec<u8>),
    /// A ping control frame (opcode 0x9). Server should respond with Pong.
    Ping(Vec<u8>),
    /// A pong control frame (opcode 0xA).
    Pong(Vec<u8>),
    /// A close control frame (opcode 0x8) with an optional status code and reason.
    Close(Option<u16>, String),
    /// A continuation fragment (opcode 0x0).
    Continuation { fin: bool, data: Vec<u8> },
}

/// WebSocket protocol utilities.
pub struct WebSocket;

impl WebSocket {
    const MAGIC: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    // ── Handshake ─────────────────────────────────────────────────────────────

    /// Returns `true` if `request` is a valid WebSocket upgrade request
    /// (has `Upgrade: websocket`, `Connection: upgrade`, and `Sec-WebSocket-Key`).
    pub fn is_upgrade_request(request: &Request) -> bool {
        let upgrade = request.get_header("Upgrade".to_string());
        let connection = request.get_header("Connection".to_string());
        let key = request.get_header("Sec-WebSocket-Key".to_string());

        upgrade.map(|h| h.value.to_lowercase() == "websocket").unwrap_or(false)
            && connection.map(|h| h.value.to_lowercase().contains("upgrade")).unwrap_or(false)
            && key.is_some()
    }

    /// Build the HTTP `101 Switching Protocols` response for a WebSocket
    /// opening handshake. Returns an error if `Sec-WebSocket-Key` is absent.
    ///
    /// Write the raw bytes from [`Response::generate_response`] to the stream,
    /// then transition to frame-level I/O with [`WebSocket::read_frame`] /
    /// [`WebSocket::write_frame`].
    pub fn handshake_response(request: &Request) -> Result<Response, String> {
        let key_header = request.get_header("Sec-WebSocket-Key".to_string())
            .ok_or_else(|| "missing Sec-WebSocket-Key header".to_string())?;
        let accept = Self::accept_key(&key_header.value);

        let mut response = Response {
            http_version: VERSION.http_1_1.to_string(),
            status_code: *STATUS_CODE_REASON_PHRASE.n101_switching_protocols.status_code,
            reason_phrase: STATUS_CODE_REASON_PHRASE.n101_switching_protocols.reason_phrase.to_string(),
            headers: vec![
                Header { name: "Upgrade".to_string(),              value: "websocket".to_string() },
                Header { name: "Connection".to_string(),           value: "Upgrade".to_string() },
                Header { name: "Sec-WebSocket-Accept".to_string(), value: accept },
            ],
            content_range_list: vec![],
            stream_file: None,
            stream_pipe: None,
        };

        if let Some(proto) = request.get_header("Sec-WebSocket-Protocol".to_string()) {
            response.headers.push(Header {
                name: "Sec-WebSocket-Protocol".to_string(),
                value: proto.value.split(',').next().unwrap_or("").trim().to_string(),
            });
        }

        Ok(response)
    }

    /// Compute the `Sec-WebSocket-Accept` value from the client's
    /// `Sec-WebSocket-Key` using SHA-1 and base64 as specified in RFC 6455.
    pub fn accept_key(client_key: &str) -> String {
        let mut data = client_key.as_bytes().to_vec();
        data.extend_from_slice(Self::MAGIC.as_bytes());
        let hash = sha1(&data);
        base64_encode(&hash)
    }

    // ── Frame I/O ─────────────────────────────────────────────────────────────

    /// Read one WebSocket frame from `stream`.
    ///
    /// Handles client-to-server masking automatically. Returns an error if the
    /// stream closes unexpectedly or contains a protocol violation.
    pub fn read_frame(stream: &mut impl Read) -> Result<Frame, String> {
        let mut header = [0u8; 2];
        read_exact(stream, &mut header)?;

        let fin = (header[0] & 0x80) != 0;
        let opcode = header[0] & 0x0F;
        let masked = (header[1] & 0x80) != 0;
        let payload_len_byte = (header[1] & 0x7F) as usize;

        let payload_len: usize = match payload_len_byte {
            126 => {
                let mut ext = [0u8; 2];
                read_exact(stream, &mut ext)?;
                u16::from_be_bytes(ext) as usize
            }
            127 => {
                let mut ext = [0u8; 8];
                read_exact(stream, &mut ext)?;
                u64::from_be_bytes(ext) as usize
            }
            n => n,
        };

        let mask_key = if masked {
            let mut mk = [0u8; 4];
            read_exact(stream, &mut mk)?;
            Some(mk)
        } else {
            None
        };

        let mut payload = vec![0u8; payload_len];
        if payload_len > 0 {
            read_exact(stream, &mut payload)?;
        }

        if let Some(key) = mask_key {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= key[i % 4];
            }
        }

        let frame = match opcode {
            0x0 => Frame::Continuation { fin, data: payload },
            0x1 => {
                let text = String::from_utf8(payload)
                    .map_err(|_| "text frame contains invalid UTF-8".to_string())?;
                Frame::Text(text)
            }
            0x2 => Frame::Binary(payload),
            0x8 => {
                let code = if payload.len() >= 2 {
                    Some(u16::from_be_bytes([payload[0], payload[1]]))
                } else {
                    None
                };
                let reason = if payload.len() > 2 {
                    String::from_utf8_lossy(&payload[2..]).into_owned()
                } else {
                    String::new()
                };
                Frame::Close(code, reason)
            }
            0x9 => Frame::Ping(payload),
            0xA => Frame::Pong(payload),
            n   => return Err(format!("unknown opcode: 0x{:X}", n)),
        };

        Ok(frame)
    }

    /// Write a WebSocket frame to `stream` (server→client, unmasked).
    pub fn write_frame(stream: &mut impl Write, frame: Frame) -> Result<(), String> {
        let (opcode, payload, fin) = match frame {
            Frame::Text(s)          => (0x1u8, s.into_bytes(), true),
            Frame::Binary(b)        => (0x2, b, true),
            Frame::Ping(b)          => (0x9, b, true),
            Frame::Pong(b)          => (0xA, b, true),
            Frame::Close(code, reason) => {
                let mut payload = Vec::new();
                if let Some(c) = code {
                    payload.extend_from_slice(&c.to_be_bytes());
                    payload.extend_from_slice(reason.as_bytes());
                }
                (0x8, payload, true)
            }
            Frame::Continuation { fin, data } => (0x0, data, fin),
        };

        let fin_bit: u8 = if fin { 0x80 } else { 0x00 };
        let byte0 = fin_bit | opcode;

        let payload_len = payload.len();
        let mut header = Vec::with_capacity(10);
        header.push(byte0);
        match payload_len {
            0..=125 => header.push(payload_len as u8),
            126..=65535 => {
                header.push(126u8);
                header.extend_from_slice(&(payload_len as u16).to_be_bytes());
            }
            _ => {
                header.push(127u8);
                header.extend_from_slice(&(payload_len as u64).to_be_bytes());
            }
        }

        stream.write_all(&header).map_err(|e| format!("write error: {}", e))?;
        if !payload.is_empty() {
            stream.write_all(&payload).map_err(|e| format!("write error: {}", e))?;
        }
        stream.flush().map_err(|e| format!("flush error: {}", e))?;
        Ok(())
    }

    /// Convenience: send a text message.
    pub fn send_text(stream: &mut impl Write, text: &str) -> Result<(), String> {
        Self::write_frame(stream, Frame::Text(text.to_string()))
    }

    /// Convenience: send a close frame with a status code and reason.
    pub fn send_close(stream: &mut impl Write, code: u16, reason: &str) -> Result<(), String> {
        Self::write_frame(stream, Frame::Close(Some(code), reason.to_string()))
    }

    /// Convenience: reply to a ping with a pong carrying the same payload.
    pub fn send_pong(stream: &mut impl Write, payload: Vec<u8>) -> Result<(), String> {
        Self::write_frame(stream, Frame::Pong(payload))
    }
}

// ── Internal utilities ─────────────────────────────────────────────────────────

fn read_exact<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<(), String> {
    r.read_exact(buf).map_err(|e| format!("read error: {}", e))
}

/// SHA-1 digest of `data` (FIPS 180-4). Used for the WebSocket handshake only.
pub(crate) fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h = [0x67452301u32, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

    let msg_len = data.len();
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&((msg_len as u64) * 8).to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i*4], chunk[i*4+1], chunk[i*4+2], chunk[i*4+3]]);
        }
        for i in 16..80 {
            w[i] = (w[i-3] ^ w[i-8] ^ w[i-14] ^ w[i-16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);

        for i in 0..80 {
            let (f, k) = match i {
                0..=19  => ((b & c) | (!b & d),          0x5A827999u32),
                20..=39 => (b ^ c ^ d,                    0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _       => (b ^ c ^ d,                    0xCA62C1D6u32),
            };
            let temp = a.rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d; d = c; c = b.rotate_left(30); b = a; a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i*4..(i+1)*4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

/// Standard base64 encoding (RFC 4648 Table 1, with `=` padding).
pub(crate) fn base64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 0x3F) as usize]);
        out.push(T[((n >> 12) & 0x3F) as usize]);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 0x3F) as usize] } else { b'=' });
        out.push(if chunk.len() > 2 { T[(n & 0x3F) as usize] } else { b'=' });
    }
    String::from_utf8(out).unwrap()
}
