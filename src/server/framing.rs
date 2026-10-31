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

/// RFC 9112 checks on a request head that `Request::parse` (also used
/// outside the connection loops) leaves lenient (#57). `head` is the raw
/// head, `request` its parse. Rejected, as `400`:
///
/// - a method that isn't one of the known ones in upper case (methods are
///   case-sensitive, §3.1: `get` is not `GET`);
/// - a version other than `HTTP/1.0` / `HTTP/1.1` on this wire (`HTTP/0.9`
///   and `HTTP/2.0` heads are not HTTP/1.1 messages);
/// - a field line starting with whitespace (obs-fold, §5.2), with whitespace
///   between the name and the colon (§5.1), or whose name isn't a token;
/// - an `HTTP/1.1` request without exactly one `Host`, or any request with
///   more than one (§3.2).
///
/// Each of these is a way for rws and a proxy in front of it to read the
/// same bytes differently.
pub(crate) fn validate_head(head: &[u8], request: &Request) -> Result<(), String> {
    use crate::request::METHOD;
    let methods = [METHOD.get, METHOD.head, METHOD.post, METHOD.put, METHOD.delete, METHOD.connect, METHOD.options, METHOD.trace, METHOD.patch, Request::PURGE];
    if !methods.contains(&request.method.as_str()) {
        return Err(format!("unknown method {:?} (methods are case-sensitive)", request.method));
    }
    let http_1_1 = request.http_version == "HTTP/1.1";
    if !http_1_1 && request.http_version != "HTTP/1.0" {
        return Err(format!("{} is not served on an HTTP/1.1 connection", request.http_version));
    }
    // The parser ends the head at a blank-looking line (`" \t"`): anything
    // it then left over as "body" was a field line it never read.
    if !request.body.is_empty() {
        return Err("malformed request head".to_string());
    }
    // Checked on the parsed names (no second pass over the head): a folded
    // line's name starts with whitespace, whitespace before the colon ends
    // the name with it — neither is a token.
    let mut hosts = 0;
    for header in &request.headers {
        let name = header.name.as_bytes();
        if name.is_empty() || !name.iter().all(|&b| TOKEN[b as usize]) {
            return Err(format!("invalid header name {:?} (obs-fold, or whitespace before the colon)", &*header.name));
        }
        if header.value.is_empty() && !has_colon_line(head, name) {
            return Err(format!("header line without a colon: {:?}", &*header.name));
        }
        if name.eq_ignore_ascii_case(Header::_HOST.as_bytes()) {
            hosts += 1;
        }
    }
    if hosts > 1 || (http_1_1 && hosts == 0) {
        return Err(format!("an HTTP/1.1 request needs exactly one Host header, got {hosts}"));
    }
    Ok(())
}

/// RFC 9110 §5.6.2 `tchar`, as a lookup table.
const TOKEN: [bool; 256] = {
    let mut table = [false; 256];
    let mut b = 0;
    while b < 256 {
        let c = b as u8;
        table[b] = c.is_ascii_alphanumeric() || matches!(c, b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~');
        b += 1;
    }
    table
};

/// `true` when a field line of `head` is `name` followed by a colon — for a
/// header the parser saw with an empty value, which is also how it records a
/// line with no colon at all. Only reached for empty values.
fn has_colon_line(head: &[u8], name: &[u8]) -> bool {
    head.split(|&b| b == b'\n').skip(1).any(|line| line.len() > name.len() && line.starts_with(name) && line[name.len()] == b':')
}

/// Whether the connection stays open after this request (RFC 9112 §9.3).
/// `Connection` is a comma-separated list of tokens (RFC 9110 §7.6.1):
/// `close` anywhere in it — `Connection: close, TE` — ends the connection
/// (#57; the whole value used to be compared with `close`). Otherwise
/// HTTP/1.1 stays open, and HTTP/1.0 only with a `keep-alive` token.
pub(crate) fn keep_alive(request: &Request) -> bool {
    let has_token = |wanted: &str| request.headers.iter()
        .filter(|h| h.name.eq_ignore_ascii_case(Header::_CONNECTION))
        .flat_map(|h| h.value.split(','))
        .any(|token| token.trim().eq_ignore_ascii_case(wanted));
    if has_token("close") {
        return false;
    }
    request.http_version == "HTTP/1.1" || has_token("keep-alive")
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
    /// Decoded body bytes so far (not yet taken by [`ChunkedBody::take_decoded`]).
    pub(crate) body: Vec<u8>,
    /// Decoded bytes already taken out — still counted against the limit.
    taken: u64,
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
        ChunkedBody { state: ChunkState::Size, body: Default::default(), taken: 0, max_body_size }
    }

    /// The bytes decoded since the last call, for a streamed body (#31);
    /// they still count against the size limit.
    pub(crate) fn take_decoded(&mut self) -> Vec<u8> {
        self.taken += self.body.len() as u64;
        std::mem::take(&mut self.body)
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
                        let total = (self.taken + self.body.len() as u64).saturating_add(size);
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

    /// The CL/TE smuggling corpus in fuzz/seeds (#8): every ambiguous framing
    /// is refused somewhere before a body is read, and the two accepted
    /// shapes (bare-LF chunks, a trailer naming Content-Length) end the body
    /// where the chunked framing says, leaving the rest as the next request.
    #[test]
    fn the_smuggling_corpus_is_refused_or_framed_consistently() {
        fn outcome(data: &[u8]) -> Result<Vec<u8>, String> {
            let mut pending = data.to_vec();
            skip_leading_empty_lines(&mut pending);
            let HeadScan::Complete(head_len) = scan_head(&pending) else { return Err("head incomplete".into()) };
            let request = crate::request::Request::parse(&pending[..head_len]).map_err(|e| e.to_string())?;
            validate_head(&pending[..head_len], &request).map_err(|e| format!("{e:?}"))?;
            match body_framing(&request)? {
                BodyFraming::Chunked => {
                    let (consumed, done) = ChunkedBody::new(1 << 20).feed(&pending[head_len..]).map_err(|e| format!("{e:?}"))?;
                    assert!(done, "chunked body not finished");
                    Ok(pending[head_len + consumed..].to_vec())
                }
                other => Err(format!("unexpected framing {other:?}")),
            }
        }
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/fuzz/seeds/http1_framing");
        let seed = |name: &str| std::fs::read(format!("{dir}/{name}")).unwrap();
        for refused in [
            "smuggle_cl_te", "smuggle_te_cl", "smuggle_te_te", "smuggle_te_obfuscated", "smuggle_two_cl",
            "smuggle_cl_list", "smuggle_chunk_ext", "smuggle_chunk_size_overflow", "smuggle_obs_fold",
        ] {
            assert!(outcome(&seed(refused)).is_err(), "{refused} was accepted");
        }
        assert_eq!(b"GET /x HTTP/1.1\r\n\r\n".to_vec(), outcome(&seed("smuggle_bare_lf")).unwrap());
        assert_eq!(b"GET /admin HTTP/1.1\r\n\r\n".to_vec(), outcome(&seed("smuggle_trailer")).unwrap());
    }
    use super::*;

    fn request(headers: &[(&str, &str)]) -> Request {
        Request {
            method: "POST".to_string(),
            request_uri: "/".to_string(),
            http_version: "HTTP/1.1".to_string(),
            headers: headers.iter().map(|(n, v)| Header { name: n.to_string().into(), value: v.to_string().into() }).collect(),
            body: Default::default(),
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

    #[test]
    fn taken_bytes_still_count_against_the_limit() {
        let mut decoder = ChunkedBody::new(8);
        assert_eq!(decoder.feed(b"5\r\nhello\r\n"), Ok((10, false)));
        assert_eq!(b"hello".to_vec(), decoder.take_decoded());
        assert!(decoder.body.is_empty());
        assert_eq!(decoder.feed(b"5\r\nworld\r\n"), Err(ChunkError::TooLarge));
    }

    // ── #57: strict request heads ──────────────────────────────────────────

    fn check(head: &[u8]) -> Result<(), String> {
        let request = Request::parse(head)?;
        validate_head(head, &request)
    }

    #[test]
    fn a_well_formed_head_passes() {
        assert!(check(b"GET / HTTP/1.1\r\nHost: x\r\nAccept: */*\r\n\r\n").is_ok());
        assert!(check(b"GET / HTTP/1.0\r\n\r\n").is_ok(), "HTTP/1.0 may omit Host");
        assert!(check(b"GET / HTTP/1.1\nHost: x\n\n").is_ok(), "bare LF");
        assert!(check(b"GET / HTTP/1.1\r\nhost:x\r\nX-A:  b c \r\n\r\n").is_ok());
    }

    #[test]
    fn host_must_appear_exactly_once() {
        assert!(check(b"GET / HTTP/1.1\r\n\r\n").is_err());
        assert!(check(b"GET / HTTP/1.1\r\nHost: a\r\nHost: b\r\n\r\n").is_err());
        assert!(check(b"GET / HTTP/1.0\r\nHost: a\r\nhost: b\r\n\r\n").is_err());
    }

    #[test]
    fn malformed_field_lines_are_rejected() {
        assert!(check(b"GET / HTTP/1.1\r\nHost: x\r\nX-A: 1\r\n folded\r\n\r\n").is_err(), "obs-fold");
        assert!(check(b"GET / HTTP/1.1\r\nHost: x\r\n\tX-A: 1\r\n\r\n").is_err(), "leading tab");
        assert!(check(b"GET / HTTP/1.1\r\nHost : x\r\n\r\n").is_err(), "space before colon");
        assert!(check(b"GET / HTTP/1.1\r\nHost: x\r\nX(A): 1\r\n\r\n").is_err(), "non-token name");
        assert!(check(b"GET / HTTP/1.1\r\nHost: x\r\nno colon here\r\n\r\n").is_err());
        assert!(check(b"GET / HTTP/1.1\r\nHost: x\r\nNoColon\r\n\r\n").is_err(), "no colon, no space");
        assert!(check(b"GET / HTTP/1.1\r\nHost: x\r\nX-Empty:\r\n\r\n").is_ok(), "an empty value is fine");
        assert!(check(b"GET / HTTP/1.1\r\nHost: x\r\n \t\r\nX-After: 1\r\n\r\n").is_err(), "whitespace-only line");
    }

    #[test]
    fn methods_are_case_sensitive_and_versions_are_http_1() {
        assert!(check(b"get / HTTP/1.1\r\nHost: x\r\n\r\n").is_err());
        assert!(check(b"PURGE /a/* HTTP/1.1\r\nHost: x\r\n\r\n").is_ok(), "the cache purge method");
        assert!(check(b"purge / HTTP/1.1\r\nHost: x\r\n\r\n").is_err());
        assert!(check(b"Post / HTTP/1.1\r\nHost: x\r\n\r\n").is_err());
        assert!(check(b"GET / HTTP/2.0\r\nHost: x\r\n\r\n").is_err());
        assert!(check(b"GET / HTTP/0.9\r\nHost: x\r\n\r\n").is_err());
    }

    #[test]
    fn connection_is_read_token_by_token() {
        let with = |version: &str, value: Option<&str>| {
            let mut r = request(&[]);
            r.http_version = version.to_string();
            if let Some(v) = value {
                r.headers.push(Header { name: "Connection".into(), value: v.to_string().into() });
            }
            keep_alive(&r)
        };
        assert!(with("HTTP/1.1", None));
        assert!(!with("HTTP/1.1", Some("close")));
        assert!(!with("HTTP/1.1", Some("close, TE")));
        assert!(!with("HTTP/1.1", Some("TE, Close")));
        assert!(with("HTTP/1.1", Some("TE")));
        assert!(!with("HTTP/1.0", None));
        assert!(with("HTTP/1.0", Some("Keep-Alive")));
        assert!(!with("HTTP/1.0", Some("TE")));
    }
}
