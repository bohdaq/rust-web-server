//! HTTP/1.1 request framing (RFC 9112), shared by the synchronous
//! [`Server::process`](super::Server::process) loop and the async
//! [`http1_async::serve`](super::http1_async) loop.
//!
//! Both loops keep one connection buffer across requests: bytes are read into
//! it until the head is complete ([`scan_head`]), the head is parsed, then the
//! body is taken by `Content-Length` or decoded from chunked framing
//! ([`ChunkedBody`]). Whatever is left over belongs to the next pipelined
//! request. This replaces a single fixed-size read per request, which cut off
//! request heads larger than the read buffer (10 KB by default — a browser
//! with a few large cookies) and dropped pipelined requests.

use crate::header::Header;
use crate::request::Request;

/// Largest accepted request head (request line + headers), in bytes. Beyond
/// this the client gets `431 Request Header Fields Too Large`. Same limit as
/// the HTTP/2 and HTTP/3 `MAX_HEADER_LIST_SIZE`.
pub(crate) const MAX_HEAD_SIZE: usize = 64 * 1024;

/// Longest chunk-size or trailer line accepted in a chunked body.
const MAX_CHUNK_LINE: usize = 4096;

#[derive(Debug, PartialEq)]
pub(crate) enum HeadScan {
    /// No blank line yet: read more.
    Incomplete,
    /// The head is longer than [`MAX_HEAD_SIZE`].
    TooLarge,
    /// The head (including its terminating blank line) is this many bytes.
    Complete(usize),
}

/// Drop empty lines in front of a request line (RFC 9112 §2.2: a server
/// SHOULD ignore at least one), e.g. a stray CRLF after a previous body.
pub(crate) fn skip_leading_empty_lines(buf: &mut Vec<u8>) {
    let n = buf.iter().take_while(|&&b| b == b'\r' || b == b'\n').count();
    if n > 0 {
        buf.drain(..n);
    }
}

/// Find the end of the request head in `buf`: `\r\n\r\n`, or a bare `\n\n`.
pub(crate) fn scan_head(buf: &[u8]) -> HeadScan {
    let limit = buf.len().min(MAX_HEAD_SIZE + 4);
    let window = &buf[..limit];
    let end = window.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4);
    let bare = window.windows(2).position(|w| w == b"\n\n").map(|i| i + 2);
    match [end, bare].into_iter().flatten().min() {
        Some(n) if n <= MAX_HEAD_SIZE => HeadScan::Complete(n),
        Some(_) => HeadScan::TooLarge,
        None if buf.len() > MAX_HEAD_SIZE => HeadScan::TooLarge,
        None => HeadScan::Incomplete,
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum BodyFraming {
    /// No body (no `Content-Length`, no `Transfer-Encoding`).
    None,
    Length(u64),
    Chunked,
}

/// How the request body is delimited (RFC 9112 §6.3). A request carrying both
/// `Content-Length` and `Transfer-Encoding`, conflicting `Content-Length`
/// values, or a transfer coding other than `chunked` is rejected: a server and
/// a proxy in front of it that disagree on where a body ends is how request
/// smuggling works.
pub(crate) fn body_framing(request: &Request) -> Result<BodyFraming, String> {
    let mut length: Option<u64> = None;
    let mut transfer_encoding: Option<String> = None;
    for header in &request.headers {
        if header.name.eq_ignore_ascii_case(Header::_CONTENT_LENGTH) {
            let value: u64 = header.value.trim().parse()
                .map_err(|_| format!("invalid Content-Length: {:?}", header.value))?;
            if length.is_some_and(|l| l != value) {
                return Err("conflicting Content-Length headers".to_string());
            }
            length = Some(value);
        } else if header.name.eq_ignore_ascii_case(Header::_TRANSFER_ENCODING) {
            let joined = match transfer_encoding.take() {
                Some(prev) => format!("{}, {}", prev, header.value),
                None => header.value.clone().into_owned(),
            };
            transfer_encoding = Some(joined);
        }
    }
    match (length, transfer_encoding) {
        (Some(_), Some(_)) => Err("request has both Content-Length and Transfer-Encoding".to_string()),
        (None, Some(te)) => {
            if te.trim().eq_ignore_ascii_case("chunked") {
                Ok(BodyFraming::Chunked)
            } else {
                Err(format!("unsupported Transfer-Encoding: {:?}", te))
            }
        }
        (Some(n), None) => Ok(BodyFraming::Length(n)),
        (None, None) => Ok(BodyFraming::None),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ChunkState {
    Size,
    Data(u64),
    DataEnd,
    Trailer,
    Done,
}

/// Incremental decoder for a chunked request body. Feed it the connection
/// buffer after each read; it consumes what it can and keeps its place, so a
/// large body is decoded in one pass.
pub(crate) struct ChunkedBody {
    state: ChunkState,
    /// Decoded body bytes so far.
    pub(crate) body: Vec<u8>,
    /// Largest accepted decoded body; `0` is unlimited.
    max_body_size: u64,
}

#[derive(Debug, PartialEq)]
pub(crate) enum ChunkError {
    /// Malformed chunk framing: `400 Bad Request`.
    Malformed(String),
    /// The decoded body passed the configured limit: `413 Payload Too Large`.
    TooLarge,
}

impl ChunkedBody {
    pub(crate) fn new(max_body_size: u64) -> Self {
        ChunkedBody { state: ChunkState::Size, body: Vec::new(), max_body_size }
    }

    /// Decode from `buf`. Returns how many bytes of `buf` were consumed (the
    /// caller drains them) and whether the body is complete.
    pub(crate) fn feed(&mut self, buf: &[u8]) -> Result<(usize, bool), ChunkError> {
        let mut pos = 0;
        loop {
            match self.state {
                ChunkState::Done => return Ok((pos, true)),
                ChunkState::Size => {
                    let Some((line, len)) = next_line(&buf[pos..])? else { return Ok((pos, false)) };
                    let size_text = line.split(|&b| b == b';').next().unwrap_or(&[]);
                    let size_text = std::str::from_utf8(size_text)
                        .map_err(|_| ChunkError::Malformed("invalid chunk size".to_string()))?
                        .trim();
                    if size_text.is_empty() || size_text.len() > 16 {
                        return Err(ChunkError::Malformed(format!("invalid chunk size {:?}", size_text)));
                    }
                    let size = u64::from_str_radix(size_text, 16)
                        .map_err(|_| ChunkError::Malformed(format!("invalid chunk size {:?}", size_text)))?;
                    pos += len;
                    if size == 0 {
                        self.state = ChunkState::Trailer;
                    } else {
                        let total = (self.body.len() as u64).saturating_add(size);
                        if self.max_body_size > 0 && total > self.max_body_size {
                            return Err(ChunkError::TooLarge);
                        }
                        self.state = ChunkState::Data(size);
                    }
                }
                ChunkState::Data(remaining) => {
                    let available = (buf.len() - pos) as u64;
                    let take = remaining.min(available) as usize;
                    self.body.extend_from_slice(&buf[pos..pos + take]);
                    pos += take;
                    if (take as u64) < remaining {
                        self.state = ChunkState::Data(remaining - take as u64);
                        return Ok((pos, false));
                    }
                    self.state = ChunkState::DataEnd;
                }
                ChunkState::DataEnd => {
                    let Some((line, len)) = next_line(&buf[pos..])? else { return Ok((pos, false)) };
                    if !line.is_empty() {
                        return Err(ChunkError::Malformed("chunk data longer than its size".to_string()));
                    }
                    pos += len;
                    self.state = ChunkState::Size;
                }
                ChunkState::Trailer => {
                    // Trailer fields are read and discarded; an empty line ends the body.
                    let Some((line, len)) = next_line(&buf[pos..])? else { return Ok((pos, false)) };
                    pos += len;
                    if line.is_empty() {
                        self.state = ChunkState::Done;
                    }
                }
            }
        }
    }
}

/// The next `\n`-terminated line in `buf` without its `\r\n` / `\n`, and the
/// number of bytes it spans. `None` if the line is not complete yet.
fn next_line(buf: &[u8]) -> Result<Option<(&[u8], usize)>, ChunkError> {
    match buf.iter().position(|&b| b == b'\n') {
        Some(i) if i > MAX_CHUNK_LINE => Err(ChunkError::Malformed("chunk line too long".to_string())),
        Some(i) => {
            let line = &buf[..i];
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            Ok(Some((line, i + 1)))
        }
        None if buf.len() > MAX_CHUNK_LINE => Err(ChunkError::Malformed("chunk line too long".to_string())),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(headers: &[(&str, &str)]) -> Request {
        Request {
            method: "POST".to_string(),
            request_uri: "/".to_string(),
            http_version: "HTTP/1.1".to_string(),
            headers: headers.iter().map(|(n, v)| Header { name: n.to_string().into(), value: v.to_string().into() }).collect(),
            body: vec![],
        }
    }

    #[test]
    fn scan_head_finds_crlf_and_bare_lf_terminators() {
        assert_eq!(scan_head(b"GET / HTTP/1.1\r\nHost: x\r\n\r\nrest"), HeadScan::Complete(27));
        assert_eq!(scan_head(b"GET / HTTP/1.1\nHost: x\n\nrest"), HeadScan::Complete(24));
        assert_eq!(scan_head(b"GET / HTTP/1.1\r\nHost: x\r\n"), HeadScan::Incomplete);
        assert_eq!(scan_head(b""), HeadScan::Incomplete);
    }

    #[test]
    fn scan_head_accepts_a_head_larger_than_the_read_buffer() {
        let mut head = b"GET / HTTP/1.1\r\nCookie: ".to_vec();
        head.extend(std::iter::repeat(b'a').take(30_000));
        head.extend_from_slice(b"\r\n\r\n");
        assert_eq!(scan_head(&head), HeadScan::Complete(head.len()));
    }

    #[test]
    fn scan_head_rejects_heads_past_the_limit() {
        let unterminated = vec![b'a'; MAX_HEAD_SIZE + 1];
        assert_eq!(scan_head(&unterminated), HeadScan::TooLarge);

        let mut terminated_late = vec![b'a'; MAX_HEAD_SIZE];
        terminated_late.extend_from_slice(b"\r\n\r\n");
        assert_eq!(scan_head(&terminated_late), HeadScan::TooLarge);
    }

    #[test]
    fn skip_leading_empty_lines_strips_only_the_prefix() {
        let mut buf = b"\r\n\r\nGET / HTTP/1.1\r\n\r\n".to_vec();
        skip_leading_empty_lines(&mut buf);
        assert!(buf.starts_with(b"GET"));
        let mut empty = Vec::new();
        skip_leading_empty_lines(&mut empty);
        assert!(empty.is_empty());
    }

    #[test]
    fn body_framing_cases() {
        assert_eq!(body_framing(&request(&[])), Ok(BodyFraming::None));
        assert_eq!(body_framing(&request(&[("content-length", "5")])), Ok(BodyFraming::Length(5)));
        assert_eq!(body_framing(&request(&[("Content-Length", "5"), ("Content-Length", "5")])), Ok(BodyFraming::Length(5)));
        assert_eq!(body_framing(&request(&[("Transfer-Encoding", "Chunked")])), Ok(BodyFraming::Chunked));
    }

    #[test]
    fn body_framing_rejects_smuggling_shapes() {
        assert!(body_framing(&request(&[("Content-Length", "5"), ("Transfer-Encoding", "chunked")])).is_err());
        assert!(body_framing(&request(&[("Content-Length", "5"), ("Content-Length", "6")])).is_err());
        assert!(body_framing(&request(&[("Transfer-Encoding", "gzip, chunked")])).is_err());
        assert!(body_framing(&request(&[("Transfer-Encoding", "chunked"), ("Transfer-Encoding", "chunked")])).is_err());
        assert!(body_framing(&request(&[("Content-Length", "-1")])).is_err());
    }

    #[test]
    fn chunked_body_decodes_in_one_feed() {
        let mut decoder = ChunkedBody::new(0);
        let input = b"5\r\nhello\r\n6;ext=1\r\n world\r\n0\r\nX-Trailer: y\r\n\r\nNEXT";
        let (consumed, done) = decoder.feed(input).unwrap();
        assert!(done);
        assert_eq!(&input[consumed..], b"NEXT");
        assert_eq!(decoder.body, b"hello world");
    }

    #[test]
    fn chunked_body_decodes_across_split_reads() {
        let input = b"a\r\n0123456789\r\n3\r\nabc\r\n0\r\n\r\n";
        // Feed one byte at a time, draining what was consumed each time.
        let mut decoder = ChunkedBody::new(0);
        let mut pending: Vec<u8> = Vec::new();
        let mut done = false;
        for &b in input.iter() {
            pending.push(b);
            let (consumed, finished) = decoder.feed(&pending).unwrap();
            pending.drain(..consumed);
            done = finished;
        }
        assert!(done);
        assert!(pending.is_empty());
        assert_eq!(decoder.body, b"0123456789abc");
    }

    #[test]
    fn chunked_body_rejects_bad_framing() {
        assert!(matches!(ChunkedBody::new(0).feed(b"zz\r\n"), Err(ChunkError::Malformed(_))));
        assert!(matches!(ChunkedBody::new(0).feed(b"\r\n"), Err(ChunkError::Malformed(_))));
        assert!(matches!(ChunkedBody::new(0).feed(b"2\r\nabc\r\n"), Err(ChunkError::Malformed(_))));
        assert!(matches!(ChunkedBody::new(0).feed(b"fffffffffffffffff\r\n"), Err(ChunkError::Malformed(_))));
        assert!(matches!(ChunkedBody::new(0).feed(&vec![b'1'; MAX_CHUNK_LINE + 1]), Err(ChunkError::Malformed(_))));
    }

    #[test]
    fn chunked_body_enforces_the_body_limit() {
        assert_eq!(ChunkedBody::new(4).feed(b"5\r\nhello\r\n0\r\n\r\n"), Err(ChunkError::TooLarge));
        let mut ok = ChunkedBody::new(5);
        assert_eq!(ok.feed(b"5\r\nhello\r\n0\r\n\r\n"), Ok((15, true)));
    }
}
