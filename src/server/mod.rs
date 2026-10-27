//! The listeners and connection loops.
//!
//! [`Server::setup`] binds the listener (and, in the `http1` build, creates the
//! thread pool); then [`Server::run`] (`http1`), `run_tls` (TLS with HTTP/2 via
//! ALPN, or plain HTTP/1.1 without a certificate), `run_quic` (HTTP/3) and
//! `run_redirect` (HTTP → HTTPS) accept connections, parse requests and hand
//! them to your [`Application`](crate::application::Application).
//! [`ConnectionInfo`] carries the client and server addresses and the TLS SNI
//! name. Examples: `src/server/example/`.

#[cfg(all(test, not(target_arch = "wasm32")))]
pub mod tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod example;
#[cfg(feature = "http2")]
pub(crate) mod http1_async;
#[cfg(feature = "http2")]
pub(crate) mod body_stream;
#[cfg(feature = "http2")]
pub(crate) mod body_pump;
#[cfg(feature = "http2")]
pub(crate) mod sendfile;
#[cfg(feature = "http2")]
pub mod per_core;
pub(crate) mod framing;
pub mod proxy_protocol;

// The TCP/thread-pool accept loop below (`Server::setup`/`run`/`process`/…) needs
// `std::net::TcpListener` and OS threads, neither available on `wasm32-wasip2`.
// `ConnectionInfo`/`Address` further down have no socket dependency and stay
// compiled on every target — see spec/WASM_SHIM.md for the guest-adapter that
// replaces this accept loop under WASI.
#[cfg(not(target_arch = "wasm32"))]
use std::io::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use std::borrow::Borrow;
#[cfg(not(target_arch = "wasm32"))]
use std::net::{IpAddr, SocketAddr, TcpListener};
#[cfg(not(target_arch = "wasm32"))]
use std::str::FromStr;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use crate::request::{METHOD, Request};
#[cfg(not(target_arch = "wasm32"))]
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
#[cfg(not(target_arch = "wasm32"))]
use crate::app::App;
#[cfg(not(target_arch = "wasm32"))]
use crate::application::Application;
#[cfg(not(target_arch = "wasm32"))]
use crate::entry_point::{bootstrap, get_ip_port_thread_count, get_request_allocation_size, set_default_values};
#[cfg(not(target_arch = "wasm32"))]
use crate::header::Header;
#[cfg(not(target_arch = "wasm32"))]
use crate::log::Log;
#[cfg(not(target_arch = "wasm32"))]
use crate::mime_type::MimeType;
#[cfg(not(target_arch = "wasm32"))]
use crate::range::{ContentRange, Range};
#[cfg(not(target_arch = "wasm32"))]
use crate::symbol::SYMBOL;
#[cfg(not(target_arch = "wasm32"))]
use crate::thread_pool::ThreadPool;

#[cfg(not(target_arch = "wasm32"))]
/// Longest time from the first byte of a request head to its end (nginx's
/// `client_header_timeout`). The idle timeout alone restarts on every read,
/// so a client sending one byte every few seconds could hold a connection
/// for weeks with a 64 KiB head.
pub const REQUEST_HEAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub struct Server {}
#[cfg(not(target_arch = "wasm32"))]
impl Server {
    pub fn process_request(mut stream: impl Read + Write + Unpin, peer_addr: SocketAddr) -> Vec<u8> {
        let request_allocation_size = get_request_allocation_size();
        let mut buffer = vec![0; request_allocation_size as usize];
        let boxed_read = stream.read(&mut buffer);
        if boxed_read.is_err() {
            let message = boxed_read.err().unwrap().to_string();
            eprintln!("unable to read TCP stream {}", &message);

            let raw_response = Server::bad_request_response(message);
            let boxed_stream = stream.write(raw_response.borrow());
            if boxed_stream.is_ok() {
                stream.flush().unwrap();
            };
            return raw_response;
        }

        boxed_read.unwrap();
        let request : &[u8] = &buffer;

        // let raw_request = String::from_utf8(Vec::from(request)).unwrap();
        // println!("\n\n______{}______\n\n", raw_request);


        let boxed_request = Request::parse_request(request);
        if boxed_request.is_err() {
            let message = boxed_request.err().unwrap();
            eprintln!("unable to parse request: {}", &message);

            let raw_response = Server::bad_request_response(message);
            let boxed_stream = stream.write(raw_response.borrow());
            if boxed_stream.is_ok() {
                stream.flush().unwrap();
            };
            return raw_response;
        }


        let request: Request = boxed_request.unwrap();
        let (response, request) = App::handle_request(request);


        let log_request_response = Log::combined(&request, &response, &peer_addr);
        println!("{}", log_request_response);
        let raw_response = Response::generate_response(response, request);

        let boxed_stream = stream.write(raw_response.borrow());
        if boxed_stream.is_ok() {
            stream.flush().unwrap();
        };

        raw_response
    }

    pub fn bad_request_response(message: String) -> Vec<u8> {
        let error_request = Request {
            method: METHOD.get.to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![],
            body: Default::default(),
        };

        let size = message.chars().count() as u64;
        let content_range = ContentRange {
            unit: Range::BYTES.to_string(),
            range: Range { start: 0, end: size },
            size: size.to_string(),
            body: Vec::from(message.as_bytes()),
            content_type: MimeType::TEXT_PLAIN.to_string(),
        };

        let header_list = Header::get_header_list(&error_request);
        let error_response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n400_bad_request,
            Some(header_list),
            Some(vec![content_range])
        );

        let response = Response::generate_response(error_response, error_request);
        return response;
    }

    /// Builds a `413 Payload Too Large` raw response for a request whose declared
    /// `Content-Length` exceeds `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES`. The connection
    /// is always closed after this is sent (see [`Server::process`]) — the client's
    /// unread body bytes are still queued on the socket, so keep-alive framing
    /// can't be trusted.
    pub fn payload_too_large_response(message: String) -> Vec<u8> {
        let error_request = Request {
            method: METHOD.get.to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![],
            body: Default::default(),
        };

        let size = message.chars().count() as u64;
        let content_range = ContentRange {
            unit: Range::BYTES.to_string(),
            range: Range { start: 0, end: size },
            size: size.to_string(),
            body: Vec::from(message.as_bytes()),
            content_type: MimeType::TEXT_PLAIN.to_string(),
        };

        let mut header_list = Header::get_header_list(&error_request);
        header_list.push(Header { name: Header::_CONNECTION.into(), value: "close".into() });
        let error_response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n413_payload_too_large,
            Some(header_list),
            Some(vec![content_range])
        );

        Response::generate_response(error_response, error_request)
    }

    /// Builds a `431 Request Header Fields Too Large` raw response for a
    /// request head longer than [`framing::MAX_HEAD_SIZE`]. The connection is
    /// closed after it: the rest of the oversized head is still unread.
    pub fn header_fields_too_large_response() -> Vec<u8> {
        Server::closing_error_response(
            STATUS_CODE_REASON_PHRASE.n431_request_header_fields_too_large,
            format!("431 Request Header Fields Too Large: the request head exceeds {} bytes", framing::MAX_HEAD_SIZE),
        )
    }

    /// Builds a `408 Request Timeout` raw response for a client that started
    /// a request head but didn't finish it within [`REQUEST_HEAD_TIMEOUT`].
    /// The connection is closed after it.
    pub fn request_timeout_response() -> Vec<u8> {
        Server::closing_error_response(
            STATUS_CODE_REASON_PHRASE.n408_request_timeout,
            format!("408 Request Timeout: the request head took longer than {} s", REQUEST_HEAD_TIMEOUT.as_secs()),
        )
    }

    /// A plain-text error response with `Connection: close`.
    fn closing_error_response(status: &'static crate::response::StatusCodeReasonPhrase, message: String) -> Vec<u8> {
        let error_request = Request {
            method: METHOD.get.to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![],
            body: Default::default(),
        };

        let size = message.chars().count() as u64;
        let content_range = ContentRange {
            unit: Range::BYTES.to_string(),
            range: Range { start: 0, end: size },
            size: size.to_string(),
            body: Vec::from(message.as_bytes()),
            content_type: MimeType::TEXT_PLAIN.to_string(),
        };

        let mut header_list = Header::get_header_list(&error_request);
        header_list.push(Header { name: Header::_CONNECTION.into(), value: "close".into() });
        let error_response: Response = Response::get_response(status, Some(header_list), Some(vec![content_range]));

        Response::generate_response(error_response, error_request)
    }

    /// Builds the `100 Continue` interim response (RFC 9110 §15.2.1) — no
    /// headers, no body, just the status line. Sent immediately after parsing
    /// a request with `Expect: 100-continue`, before reading its body, so the
    /// client (which is waiting for this before it uploads the body) doesn't
    /// stall or time out. See [`Server::process`] for where this is sent from.
    pub fn continue_response() -> Vec<u8> {
        let dummy_request = Request {
            method: METHOD.get.to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![],
            body: Default::default(),
        };
        let response = Response::get_response(STATUS_CODE_REASON_PHRASE.n100_continue, None, None);
        Response::generate_response(response, dummy_request)
    }

    /// Builds a `417 Expectation Failed` raw response for an `Expect` header
    /// value other than `100-continue` (RFC 9110 §10.1.1) — the only
    /// expectation this server understands. Sent instead of reading the
    /// request body, since the expectation can't be met either way.
    pub fn expectation_failed_response(message: String) -> Vec<u8> {
        let error_request = Request {
            method: METHOD.get.to_string(),
            request_uri: "".to_string(),
            http_version: "".to_string(),
            headers: vec![],
            body: Default::default(),
        };

        let size = message.chars().count() as u64;
        let content_range = ContentRange {
            unit: Range::BYTES.to_string(),
            range: Range { start: 0, end: size },
            size: size.to_string(),
            body: Vec::from(message.as_bytes()),
            content_type: MimeType::TEXT_PLAIN.to_string(),
        };

        let mut header_list = Header::get_header_list(&error_request);
        header_list.push(Header { name: Header::_CONNECTION.into(), value: "close".into() });
        let error_response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n417_expectation_failed,
            Some(header_list),
            Some(vec![content_range])
        );

        Response::generate_response(error_response, error_request)
    }

    pub fn process(mut stream: impl Read + Write + Unpin,
                   connection: ConnectionInfo,
                   app: impl Application) -> Result<(), String> {

        let request_allocation_size = connection.request_size;
        let client = connection.client.clone();
        let client_addr = SocketAddr::new(IpAddr::from_str(client.ip.as_str()).unwrap(), client.port as u16);

        let max_body_size = crate::entry_point::get_max_body_size();

        // Bytes read from the connection but not consumed yet: the rest of a
        // request head, a body, or the next pipelined request.
        let mut pending: Vec<u8> = Vec::new();
        let mut read_buffer = vec![0u8; request_allocation_size as usize];

        'session: loop {
            // Read until the request head is complete — it may span several
            // reads, but must finish within REQUEST_HEAD_TIMEOUT of its first
            // byte (checked after each read; every read is itself bounded by
            // the socket's read timeout).
            let mut head_started: Option<std::time::Instant> = None;
            let head_len = loop {
                framing::skip_leading_empty_lines(&mut pending);
                match framing::scan_head(&pending) {
                    framing::HeadScan::Complete(n) => break Some(n),
                    framing::HeadScan::TooLarge => {
                        let _ = stream.write_all(&Server::header_fields_too_large_response());
                        let _ = stream.flush();
                        return Err("request head too large".to_string());
                    }
                    framing::HeadScan::Incomplete => {}
                }
                if !pending.is_empty() && head_started.is_none() {
                    head_started = Some(std::time::Instant::now());
                }
                match stream.read(&mut read_buffer) {
                    // timeout or client closed — normal end of keep-alive session
                    Ok(0) | Err(_) => break None,
                    Ok(n) => pending.extend_from_slice(&read_buffer[..n]),
                }
                let started = *head_started.get_or_insert_with(std::time::Instant::now);
                if started.elapsed() > REQUEST_HEAD_TIMEOUT && framing::scan_head(&pending) == framing::HeadScan::Incomplete {
                    let _ = stream.write_all(&Server::request_timeout_response());
                    let _ = stream.flush();
                    return Err("request head timed out".to_string());
                }
            };
            let Some(head_len) = head_len else { break };

            let parsed = Request::parse(&pending[..head_len])
                .and_then(|request| framing::validate_head(&pending[..head_len], &request).map(|_| request))
                .and_then(|request| framing::body_framing(&request).map(|f| (request, f)));
            pending.drain(..head_len);
            let (mut request, body_framing) = match parsed {
                Ok(parsed) => parsed,
                Err(message) => {
                    let _ = stream.write_all(&Server::bad_request_response(message.clone()));
                    let _ = stream.flush();
                    return Err(message);
                }
            };

            // RFC 9110 §10.1.1 `Expect` — the only expectation this server
            // understands is `100-continue`; anything else fails fast with a
            // final status instead of reading a body we can't act on.
            let expects_continue = match request.header(Header::_EXPECT) {
                Some(h) if h.value.trim().eq_ignore_ascii_case("100-continue") => true,
                Some(h) => {
                    let raw_response = Server::expectation_failed_response(format!(
                        "417 Expectation Failed: unsupported Expect value '{}'", h.value.trim()
                    ));
                    let _ = stream.write_all(&raw_response);
                    let _ = stream.flush();
                    break;
                }
                None => false,
            };

            match body_framing {
                framing::BodyFraming::None => {}
                framing::BodyFraming::Length(declared_len) => {
                    if max_body_size > 0 && declared_len > max_body_size {
                        let raw_response = Server::payload_too_large_response(format!(
                            "413 Payload Too Large: declared Content-Length {} exceeds the {} byte limit",
                            declared_len, max_body_size
                        ));
                        let _ = stream.write_all(&raw_response);
                        let _ = stream.flush();
                        break;
                    }

                    // Tell the client to go ahead and send the body now, before we
                    // block waiting for it below.
                    if expects_continue && (pending.len() as u64) < declared_len {
                        if stream.write_all(&Server::continue_response()).and_then(|_| stream.flush()).is_err() {
                            break;
                        }
                    }

                    while (pending.len() as u64) < declared_len {
                        match stream.read(&mut read_buffer) {
                            Ok(0) | Err(_) => break 'session, // closed or stalled mid-body
                            Ok(k) => pending.extend_from_slice(&read_buffer[..k]),
                        }
                    }
                    request.body = pending.drain(..declared_len as usize).collect::<Vec<u8>>().into();
                }
                framing::BodyFraming::Chunked => {
                    if expects_continue && pending.is_empty() {
                        if stream.write_all(&Server::continue_response()).and_then(|_| stream.flush()).is_err() {
                            break;
                        }
                    }

                    let mut decoder = framing::ChunkedBody::new(max_body_size);
                    loop {
                        match decoder.feed(&pending) {
                            Ok((consumed, done)) => {
                                pending.drain(..consumed);
                                if done { break; }
                            }
                            Err(framing::ChunkError::TooLarge) => {
                                let raw_response = Server::payload_too_large_response(format!(
                                    "413 Payload Too Large: chunked body exceeds the {} byte limit", max_body_size
                                ));
                                let _ = stream.write_all(&raw_response);
                                let _ = stream.flush();
                                break 'session;
                            }
                            Err(framing::ChunkError::Malformed(message)) => {
                                let _ = stream.write_all(&Server::bad_request_response(message.clone()));
                                let _ = stream.flush();
                                return Err(message);
                            }
                        }
                        match stream.read(&mut read_buffer) {
                            Ok(0) | Err(_) => break 'session,
                            Ok(k) => pending.extend_from_slice(&read_buffer[..k]),
                        }
                    }
                    request.body = decoder.body.into();
                }
            }

            let keep_alive = framing::keep_alive(&request);

            let mut response = match app.execute(&request, &connection) {
                Ok(r) => r,
                Err(message) => {
                    let raw_response = Server::bad_request_response(message.clone());
                    let boxed_stream = stream.write(raw_response.borrow());
                    if boxed_stream.is_ok() { stream.flush().unwrap(); }
                    return Err(message);
                }
            };

            crate::metrics::record_request();
            crate::compression::apply_compression(&request, &mut response);
            response.drop_content_for_status();

            response.headers.push(Header {
                name: Header::_CONNECTION.into(),
                value: if keep_alive { "keep-alive".into() } else { "close".into() },
            });

            Log::log_access(&request, &response, &client_addr);

            if let Some(reader) = response.stream_pipe.take() {
                if let Err(e) = Server::pipe_stream(&mut stream, response, request, reader) {
                    return Err(e);
                }
            } else if let Some(ref filepath) = response.stream_file.clone() {
                if let Err(e) = Server::write_chunked_file(&mut stream, response, request, filepath) {
                    return Err(e);
                }
            } else {
                let raw_response = Response::generate_response(response, request);
                // write_all: a single write() may accept only part of a large response.
                if let Err(e) = stream.write_all(raw_response.borrow()) {
                    return Err(e.to_string());
                }
                stream.flush().unwrap();
            }

            if !keep_alive { break; }
        }

        Ok(())
    }

    /// Streams a file to `stream` using HTTP/1.1 chunked transfer encoding.
    /// The response headers are written first, then the file is read and written in 64 KB chunks.
    pub(crate) fn write_chunked_file(
        stream: &mut impl Write,
        response: Response,
        request: Request,
        filepath: &str,
    ) -> Result<(), String> {
        use std::fs::File;
        use std::io::Read as _;

        let (head, chunked) = Server::stream_file_head(response);
        stream.write_all(&head).map_err(|e| e.to_string())?;

        if request.method != METHOD.head {
            let mut file = File::open(filepath).map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 65536];
            loop {
                let n = file.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 { break; }
                if chunked {
                    // chunk header: hex size + CRLF
                    stream.write_all(format!("{:x}\r\n", n).as_bytes()).map_err(|e| e.to_string())?;
                    stream.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                    stream.write_all(b"\r\n").map_err(|e| e.to_string())?;
                } else {
                    stream.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                }
            }
            if chunked {
                // terminal chunk
                stream.write_all(b"0\r\n\r\n").map_err(|e| e.to_string())?;
            }
        }

        stream.flush().map_err(|e| e.to_string())
    }

    /// The status line and headers of a `stream_file` response, and whether
    /// its body is sent chunked. With a known Content-Length (static files
    /// set it) the body goes out as-is; Transfer-Encoding: chunked is only
    /// for an unknown length. Sending both is forbidden (RFC 9112 §6.1) —
    /// mismatched handling of the two by intermediaries is how request
    /// smuggling happens.
    pub(crate) fn stream_file_head(mut response: Response) -> (Vec<u8>, bool) {
        let chunked = !response.headers.iter().any(|h| h.name.eq_ignore_ascii_case(Header::_CONTENT_LENGTH));
        if chunked {
            response.headers.push(Header {
                name: Header::_TRANSFER_ENCODING.into(),
                value: "chunked".into(),
            });
        }

        // build status line + headers (no body)
        let status = [
            response.http_version.clone(),
            response.status_code.to_string(),
            Header::single_line(&response.reason_phrase).into_owned(),
        ].join(SYMBOL.whitespace);

        let mut headers_str = SYMBOL.new_line_carriage_return.to_string();
        for header in &response.headers {
            headers_str.push_str(&Header::single_line(&header.name));
            headers_str.push_str(Header::NAME_VALUE_SEPARATOR);
            headers_str.push_str(&Header::single_line(&header.value));
            headers_str.push_str(SYMBOL.new_line_carriage_return);
        }
        let head = format!("{}{}{}", status, headers_str, SYMBOL.new_line_carriage_return);
        (head.into_bytes(), chunked)
    }

    /// Streams a `Read` source to `stream` using HTTP/1.1 chunked transfer encoding.
    ///
    /// Called when `response.stream_pipe` is set (proxy passthrough for SSE, AI token
    /// streams, and large downloads). The response headers are written first (with
    /// `Transfer-Encoding: chunked` injected and any `Content-Length` stripped), then
    /// bytes are read from `reader` and forwarded in 8 KB chunks. Each chunk is flushed
    /// immediately so the client sees data as it arrives.
    pub(crate) fn pipe_stream(
        stream: &mut impl Write,
        mut response: Response,
        request: Request,
        mut reader: Box<dyn std::io::Read + Send>,
    ) -> Result<(), String> {
        // If the backend already uses chunked encoding we pass the raw chunk
        // frames through; otherwise we wrap the raw bytes in our own chunked
        // framing so the client can receive data incrementally.
        let backend_is_chunked = response.headers.iter().any(|h| {
            h.name.eq_ignore_ascii_case("transfer-encoding")
                && h.value.to_lowercase().contains("chunked")
        });

        response.headers.retain(|h| !h.name.eq_ignore_ascii_case("content-length"));
        if !backend_is_chunked {
            response.headers.push(Header {
                name: Header::_TRANSFER_ENCODING.into(),
                value: "chunked".into(),
            });
        }

        let status = [
            response.http_version.clone(),
            response.status_code.to_string(),
            Header::single_line(&response.reason_phrase).into_owned(),
        ].join(SYMBOL.whitespace);

        let mut head = format!("{}\r\n", status);
        for header in &response.headers {
            head.push_str(&Header::single_line(&header.name));
            head.push_str(Header::NAME_VALUE_SEPARATOR);
            head.push_str(&Header::single_line(&header.value));
            head.push_str(SYMBOL.new_line_carriage_return);
        }
        head.push_str(SYMBOL.new_line_carriage_return);
        stream.write_all(head.as_bytes()).map_err(|e| e.to_string())?;

        if request.method != METHOD.head {
            let mut buf = [0u8; 8192];
            if backend_is_chunked {
                // Passthrough: forward raw chunk frames as-is; the client
                // decodes them itself (avoids double-chunking).
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            stream.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                            stream.flush().map_err(|e| e.to_string())?;
                        }
                    }
                }
            } else {
                // Re-encode: wrap the raw byte stream in chunked framing so
                // the client can receive each fragment as it arrives (SSE,
                // large downloads).
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            stream
                                .write_all(format!("{:x}\r\n", n).as_bytes())
                                .map_err(|e| e.to_string())?;
                            stream.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                            stream.write_all(b"\r\n").map_err(|e| e.to_string())?;
                            stream.flush().map_err(|e| e.to_string())?;
                        }
                    }
                }
                stream.write_all(b"0\r\n\r\n").map_err(|e| e.to_string())?;
            }
        }

        stream.flush().map_err(|e| e.to_string())
    }

    /// Reads configuration (IP, port, thread count, TLS paths) from the layered config system
    /// and returns a bound `TcpListener` and a sized `ThreadPool`. Call once at startup.
    pub fn setup() -> Result<(TcpListener, ThreadPool), String> {
        let info = Log::info("Rust Web Server");
        println!("{}", info);

        let usage_info = Log::usage_information();
        println!("{}", usage_info);


        println!("RWS Configuration Start: \n");

        set_default_values();
        bootstrap();

        #[cfg(feature = "secrets")]
        crate::secrets::resolve_env_vars().map_err(|e| e.to_string())?;

        println!("\nRWS Configuration End\n\n");


        let (ip, port, thread_count) = get_ip_port_thread_count();


        let mut ip_readable = ip.to_string();

        if ip.contains(":") {
            ip_readable = [SYMBOL.opening_square_bracket, &ip, SYMBOL.closing_square_bracket].join("");
        }

        let bind_addr = [ip_readable, SYMBOL.colon.to_string(), port.to_string()].join(SYMBOL.empty_string);

        #[cfg(feature = "http2")]
        let protocol = {
            let cert = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE).unwrap_or_default();
            if cert.is_empty() { "http" } else { "https" }
        };
        #[cfg(not(feature = "http2"))]
        let protocol = "http";

        println!("Setting up {}://{}...", protocol, &bind_addr);

        let boxed_listener = TcpListener::bind(&bind_addr);
        if boxed_listener.is_err() {
            let message = format!("unable to set up TCP listener: {}", boxed_listener.err().unwrap());
            return Err(message);
        }

        let listener = boxed_listener.unwrap();
        let pool = ThreadPool::new(thread_count as usize);


        let server_url_thread_count = Log::server_url_thread_count(protocol, &bind_addr, thread_count);
        println!("{}", server_url_thread_count);

        Ok((listener, pool))
    }

    /// Accepts TCP connections in a loop and dispatches each to the thread pool.
    ///
    /// When built with the `http1` feature, Ctrl+C and SIGTERM stop the accept
    /// loop gracefully: `SERVER_READY` is cleared and the pool drains all
    /// in-flight connections before returning.
    ///
    /// For TLS/HTTP2/HTTP3 use [`Server::run_tls`].
    pub fn run(listener: TcpListener,
               pool: ThreadPool,
               app: impl Application + Send + 'static + Clone) {
        #[cfg(feature = "http1")]
        {
            use std::sync::Arc;
            use std::sync::atomic::{AtomicBool, Ordering};

            let shutdown = Arc::new(AtomicBool::new(false));
            let s = shutdown.clone();
            if let Err(e) = ctrlc::set_handler(move || {
                s.store(true, Ordering::SeqCst);
            }) {
                eprintln!("unable to install signal handler: {}", e);
            }
            crate::config_reload::install_sighup_handler();
            if let Err(e) = listener.set_nonblocking(true) {
                eprintln!("unable to set non-blocking listener: {}", e);
            }

            loop {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                if crate::config_reload::RELOAD_REQUESTED
                    .compare_exchange(true, false, Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
                {
                    crate::config_reload::reload();
                }
                match listener.accept() {
                    Ok((stream, peer_addr)) => {
                        Server::dispatch_connection(stream, peer_addr, &pool, app.clone());
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        eprintln!("accept error: {}", e);
                        break;
                    }
                }
            }

            crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
            println!("Shutting down — waiting for in-flight connections to finish");
            pool.join();
            println!("Server stopped");
        }

        #[cfg(not(feature = "http1"))]
        {
            for boxed_stream in listener.incoming() {
                match boxed_stream {
                    Err(e) => {
                        eprintln!("unable to get TCP stream: {}", e);
                        return;
                    }
                    Ok(stream) => {
                        let peer_addr = match stream.peer_addr() {
                            Ok(a) => a,
                            Err(e) => {
                                eprintln!("unable to read peer addr: {}", e);
                                return;
                            }
                        };
                        Server::dispatch_connection(stream, peer_addr, &pool, app.clone());
                    }
                }
            }
        }
    }

    fn dispatch_connection(
        stream: std::net::TcpStream,
        peer_addr: std::net::SocketAddr,
        pool: &ThreadPool,
        app: impl Application + Send + 'static + Clone,
    ) {
        print!("Connection established, ");
        if let Ok(local) = stream.local_addr() {
            print!("local addr: {}", local);
        }
        println!(", peer addr: {}", peer_addr);

        let (server_ip, server_port, _thread_count) = get_ip_port_thread_count();
        let connection = ConnectionInfo {
            client: Address {
                ip: peer_addr.ip().to_string(),
                port: peer_addr.port() as i32,
            },
            server: Address {
                ip: server_ip,
                port: server_port,
            },
            request_size: get_request_allocation_size(),
            sni_hostname: None,
        };

        if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(30))) {
            eprintln!("failed to set read timeout: {}", e);
        }

        pool.execute(move || {
            let mut stream = stream;
            let mut connection = connection;
            if proxy_protocol::enabled() {
                // Read on the worker: a slow load balancer mustn't hold the accept loop.
                match proxy_protocol::read_blocking(&mut stream) {
                    Ok(Some(source)) => {
                        connection.client = Address { ip: source.ip().to_string(), port: source.port() as i32 };
                    }
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("PROXY protocol header from {}: {}", peer_addr, e);
                        return;
                    }
                }
            }
            crate::metrics::connection_open();
            let result = Server::process(stream, connection, app);
            crate::metrics::connection_close();
            if let Err(msg) = result {
                crate::metrics::record_error();
                eprintln!("{}", msg);
            }
        });
    }

}

/// Network context for the current connection, passed into every [`Controller`](crate::controller::Controller).
#[derive(Clone)]
pub struct ConnectionInfo {
    /// Client (peer) address.
    pub client: Address,
    /// Server (local) address.
    pub server: Address,
    /// Bytes allocated for reading the request.
    pub request_size: i64,
    /// SNI hostname sent by the client during the TLS handshake, if any.
    /// `None` for plain-HTTP connections or when the client omits SNI.
    pub sni_hostname: Option<String>,
}

/// IP address and port pair.
#[derive(Clone)]
pub struct Address {
    pub ip: String,
    pub port: i32
}

impl ConnectionInfo {
    /// Parse the client address into a [`std::net::SocketAddr`], if the stored
    /// IP and port are valid. Returns `None` if parsing fails.
    pub fn peer_addr(&self) -> Option<std::net::SocketAddr> {
        self.client.to_socket_addr()
    }
}

impl Address {
    /// Parse this address into a [`std::net::SocketAddr`]. Returns `None` if
    /// the IP string or port value cannot be converted.
    pub fn to_socket_addr(&self) -> Option<std::net::SocketAddr> {
        let ip: std::net::IpAddr = self.ip.parse().ok()?;
        let port = u16::try_from(self.port).ok()?;
        Some(std::net::SocketAddr::new(ip, port))
    }
}

/// One accepted TLS connection: handshake (bounded by
/// `TLS_HANDSHAKE_TIMEOUT`), then HTTP/2 or HTTP/1.1 by ALPN.
#[cfg(feature = "http2")]
async fn serve_tls_connection(
    mut tcp_stream: tokio::net::TcpStream,
    peer_addr: std::net::SocketAddr,
    acceptor: tokio_rustls::TlsAcceptor,
    app: impl Application + Send + 'static + Clone,
) {
    let _ = tcp_stream.set_nodelay(true);
    let Some(peer_addr) = client_address(&mut tcp_stream, peer_addr).await else { return };
    let handshake = tokio::time::timeout(http1_async::TLS_HANDSHAKE_TIMEOUT, acceptor.accept(tcp_stream));
    match handshake.await {
        Err(_) => {} // client never finished the handshake
        Ok(Err(e)) => eprintln!("TLS handshake failed: {}", e),
        Ok(Ok(tls_stream)) => {
            let server_conn = tls_stream.get_ref().1;
            let sni = server_conn.server_name().map(|s| s.to_string());
            let protocol = server_conn.alpn_protocol().map(|p| p.to_vec());

            match protocol.as_deref() {
                // A TLS-ALPN-01 validation: the handshake was the answer.
                #[cfg(feature = "acme")]
                Some(crate::acme::tls_alpn::ALPN) => {}
                Some(b"h2") => {
                    if let Err(e) = crate::h2_server::handle_connection(tls_stream, peer_addr, sni, app).await {
                        eprintln!("H2 connection error: {}", e);
                    }
                }
                _ => {
                    crate::metrics::connection_open();
                    let result = http1_async::serve(tls_stream, peer_addr, sni, true, app).await;
                    crate::metrics::connection_close();
                    if let Err(e) = result {
                        crate::metrics::record_error();
                        eprintln!("H1 TLS error: {}", e);
                    }
                }
            }
        }
    }
}

/// One accepted plain-HTTP connection.
#[cfg(feature = "http2")]
async fn serve_plain_connection(
    tcp_stream: tokio::net::TcpStream,
    peer_addr: std::net::SocketAddr,
    app: impl Application + Send + 'static + Clone,
) {
    // Head and body go out as separate writes: don't let Nagle hold the
    // body back waiting for an ACK.
    let mut tcp_stream = tcp_stream;
    let _ = tcp_stream.set_nodelay(true);
    let Some(peer_addr) = client_address(&mut tcp_stream, peer_addr).await else { return };
    if starts_with_h2_preface(&tcp_stream).await {
        if let Err(e) = crate::h2_server::handle_connection(tcp_stream, peer_addr, None, app).await {
            eprintln!("H2C connection error: {}", e);
        }
        return;
    }
    crate::metrics::connection_open();
    let result = http1_async::serve(tcp_stream, peer_addr, None, false, app).await;
    crate::metrics::connection_close();
    if let Err(e) = result {
        crate::metrics::record_error();
        eprintln!("{}", e);
    }
}

/// The client's address: the peer's, or with `proxy_protocol = true` the one
/// in the PROXY protocol header the connection starts with. `None`: no valid
/// header (in time) — close the connection.
#[cfg(feature = "http2")]
async fn client_address(tcp_stream: &mut tokio::net::TcpStream, peer_addr: std::net::SocketAddr) -> Option<std::net::SocketAddr> {
    if !proxy_protocol::enabled() {
        return Some(peer_addr);
    }
    match tokio::time::timeout(http1_async::TLS_HANDSHAKE_TIMEOUT, proxy_protocol::read_async(tcp_stream)).await {
        Ok(Ok(source)) => Some(source.unwrap_or(peer_addr)),
        Ok(Err(e)) => {
            eprintln!("PROXY protocol header from {}: {}", peer_addr, e);
            None
        }
        Err(_) => None,
    }
}

/// Whether a plain connection opens with the HTTP/2 connection preface —
/// cleartext HTTP/2 with prior knowledge (h2c), as `grpcurl -plaintext` and
/// gRPC clients without TLS speak it. Looks without consuming; an HTTP/1.1
/// request differs within its first two bytes.
#[cfg(feature = "http2")]
async fn starts_with_h2_preface(tcp_stream: &tokio::net::TcpStream) -> bool {
    const PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
    let mut seen = [0u8; 24];
    let deadline = tokio::time::Instant::now() + http1_async::READ_TIMEOUT;
    loop {
        let n = match tokio::time::timeout_at(deadline, tcp_stream.peek(&mut seen)).await {
            Ok(Ok(n)) => n,
            _ => return false,
        };
        if n == 0 || seen[..n] != PREFACE[..n] {
            return false;
        }
        if n == PREFACE.len() {
            return true;
        }
        // A preface arriving in pieces: look again shortly.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

/// An accept error (e.g. out of file descriptors): report it and back off
/// briefly instead of ending the loop.
#[cfg(feature = "http2")]
async fn accept_backoff(e: std::io::Error) {
    eprintln!("TCP accept error: {}", e);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

/// Set once by [`request_shutdown`]; every receiver sees it, however late.
#[cfg(feature = "http2")]
fn shutdown_requests() -> &'static tokio::sync::watch::Sender<bool> {
    static REQUESTS: std::sync::OnceLock<tokio::sync::watch::Sender<bool>> = std::sync::OnceLock::new();
    REQUESTS.get_or_init(|| tokio::sync::watch::channel(false).0)
}

/// Shut the server down gracefully, as SIGTERM does — for hosts that stop a
/// process some other way (a Windows service's Stop control). The accept
/// loops of `run_tls`, `run_plain`, `run_quic` and `run_redirect` end.
#[cfg(feature = "http2")]
pub fn request_shutdown() {
    shutdown_requests().send_replace(true);
}

/// Resolves when SIGTERM is received (Unix) or [`request_shutdown`] is
/// called. Enables a single `select!` branch to handle both, beside Ctrl+C.
#[cfg(feature = "http2")]
async fn sigterm() {
    let requested = async {
        let mut requests = shutdown_requests().subscribe();
        let _ = requests.wait_for(|stop| *stop).await;
    };
    #[cfg(unix)]
    {
        if let Ok(mut s) = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate()
        ) {
            tokio::select! {
                _ = s.recv() => {}
                _ = requested => {}
            }
        } else {
            requested.await
        }
    }
    #[cfg(not(unix))]
    requested.await
}

/// Returns a stream that fires on each SIGHUP on Unix; never fires elsewhere.
#[cfg(feature = "http2")]
async fn sighup() {
    #[cfg(unix)]
    {
        if let Ok(mut s) = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::hangup()
        ) {
            s.recv().await;
        } else {
            std::future::pending::<()>().await
        }
    }
    #[cfg(not(unix))]
    std::future::pending::<()>().await
}

/// Bind `port` for both IPv4 and IPv6 when `ip` is the wildcard `0.0.0.0`
/// (otherwise just `ip`). Let's Encrypt validates HTTP-01 over IPv6 when a
/// domain has an AAAA record, so a v4-only challenge/redirect listener fails
/// validation on dual-stack hosts. On Linux/macOS `[::]` already accepts
/// IPv4 too and the second bind reports "address in use", which is fine; on
/// Windows (`IPV6_V6ONLY` by default) both binds succeed.
#[cfg(feature = "http2")]
pub(crate) async fn bind_all_families(ip: &str, port: u16) -> Result<Vec<tokio::net::TcpListener>, String> {
    use tokio::net::TcpListener;
    if ip != "0.0.0.0" {
        let addr = if ip.contains(':') { format!("[{ip}]:{port}") } else { format!("{ip}:{port}") };
        return TcpListener::bind(&addr).await.map(|l| vec![l]).map_err(|e| format!("{addr}: {e}"));
    }
    let mut listeners = Vec::new();
    let mut errors = Vec::new();
    for addr in [format!("[::]:{port}"), format!("0.0.0.0:{port}")] {
        match TcpListener::bind(&addr).await {
            Ok(l) => listeners.push(l),
            Err(e) => errors.push(format!("{addr}: {e}")),
        }
    }
    if listeners.is_empty() {
        return Err(errors.join("; "));
    }
    Ok(listeners)
}

#[cfg(feature = "http2")]
impl Server {
    pub async fn run_tls(
        listener: TcpListener,
        pool: ThreadPool,
        app: impl Application + Send + 'static + Clone,
    ) {
        use crate::tls::create_tls_acceptor_from_vhosts;

        let cert_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE)
            .unwrap_or_default();
        let key_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_KEY_FILE)
            .unwrap_or_default();

        if cert_path.is_empty() || key_path.is_empty() {
            println!("No TLS certificate configured — serving plain HTTP/1.1.");
            drop(pool);
            Server::run_plain(listener, app).await;
            return;
        }

        let vhosts = crate::entry_point::get_virtual_hosts();
        let mut tls_acceptor = match create_tls_acceptor_from_vhosts(&vhosts, &cert_path, &key_path) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("TLS setup failed: {}", e);
                return;
            }
        };
        // From now on this listener answers TLS-ALPN-01 challenges itself.
        #[cfg(feature = "acme")]
        crate::acme::tls_alpn::note_tls_listener();

        #[cfg(unix)]
        if per_core::enabled() {
            println!("Listening for TLS connections (HTTP/1.1 + HTTP/2), per-core runtimes...");
            let (acceptor_tx, acceptor_rx) = tokio::sync::watch::channel(tls_acceptor);
            let accept_loop = move |listener: tokio::net::TcpListener, mut stop: tokio::sync::watch::Receiver<bool>| {
                let app = app.clone();
                let acceptor_rx = acceptor_rx.clone();
                async move {
                    loop {
                        tokio::select! {
                            result = listener.accept() => match result {
                                Ok((tcp_stream, peer_addr)) => {
                                    let acceptor = acceptor_rx.borrow().clone();
                                    tokio::spawn(serve_tls_connection(tcp_stream, peer_addr, acceptor, app.clone()));
                                }
                                Err(e) => accept_backoff(e).await,
                            },
                            _ = per_core::stopped(&mut stop) => break,
                        }
                    }
                }
            };
            Server::run_per_core(listener, accept_loop, move || {
                let vhosts = crate::entry_point::get_virtual_hosts();
                if let Ok(new_acceptor) = create_tls_acceptor_from_vhosts(&vhosts, &cert_path, &key_path) {
                    let _ = acceptor_tx.send(new_acceptor);
                    println!("[TLS] Certificates reloaded ({} virtual hosts).", vhosts.len());
                }
            })
            .await;
            return;
        }

        listener
            .set_nonblocking(true)
            .expect("failed to set TCP listener to non-blocking");
        let tokio_listener = tokio::net::TcpListener::from_std(listener)
            .expect("failed to convert TCP listener to tokio");

        println!("Listening for TLS connections (HTTP/1.1 + HTTP/2)...");

        loop {
            tokio::select! {
                result = tokio_listener.accept() => {
                    match result {
                        Ok((tcp_stream, peer_addr)) => {
                            tokio::spawn(serve_tls_connection(tcp_stream, peer_addr, tls_acceptor.clone(), app.clone()));
                        }
                        Err(e) => eprintln!("TCP accept error: {}", e),
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down gracefully (SIGINT).");
                    break;
                }
                _ = sigterm() => {
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down gracefully (SIGTERM).");
                    break;
                }
                _ = sighup() => {
                    crate::config_reload::reload();
                    let vhosts = crate::entry_point::get_virtual_hosts();
                    if let Ok(new_acceptor) = create_tls_acceptor_from_vhosts(&vhosts, &cert_path, &key_path) {
                        tls_acceptor = new_acceptor;
                        println!("[TLS] Certificates reloaded ({} virtual hosts).", vhosts.len());
                    }
                }
            }
        }
    }

    /// Plain-HTTP accept loop of the async builds (used by `run_tls` when no
    /// certificate is configured): one tokio task per connection via
    /// `http1_async::serve`, so idle or slow clients can't exhaust a thread
    /// pool. An accept error (e.g. out of file descriptors) backs off briefly
    /// and keeps serving instead of ending the loop.
    pub async fn run_plain(listener: TcpListener, app: impl Application + Send + 'static + Clone) {
        #[cfg(unix)]
        if per_core::enabled() {
            let accept_loop = move |listener: tokio::net::TcpListener, mut stop: tokio::sync::watch::Receiver<bool>| {
                let app = app.clone();
                async move {
                    loop {
                        tokio::select! {
                            result = listener.accept() => match result {
                                Ok((tcp_stream, peer_addr)) => {
                                    tokio::spawn(serve_plain_connection(tcp_stream, peer_addr, app.clone()));
                                }
                                Err(e) => accept_backoff(e).await,
                            },
                            _ = per_core::stopped(&mut stop) => break,
                        }
                    }
                }
            };
            Server::run_per_core(listener, accept_loop, || {}).await;
            return;
        }

        listener
            .set_nonblocking(true)
            .expect("failed to set TCP listener to non-blocking");
        let tokio_listener = tokio::net::TcpListener::from_std(listener)
            .expect("failed to convert TCP listener to tokio");

        loop {
            tokio::select! {
                result = tokio_listener.accept() => {
                    match result {
                        Ok((tcp_stream, peer_addr)) => {
                            tokio::spawn(serve_plain_connection(tcp_stream, peer_addr, app.clone()));
                        }
                        Err(e) => accept_backoff(e).await,
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down gracefully (SIGINT).");
                    break;
                }
                _ = sigterm() => {
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down gracefully (SIGTERM).");
                    break;
                }
                _ = sighup() => {
                    crate::config_reload::reload();
                }
            }
        }
    }

    /// Per-core mode of `run_tls`/`run_plain` ([`per_core`]): rebinds
    /// `listener`'s address as one `SO_REUSEPORT` socket per runtime, runs
    /// `accept_loop` on each, and handles the signals here — SIGINT/SIGTERM
    /// stop every loop, SIGHUP reloads the config and calls `on_reload`.
    #[cfg(unix)]
    async fn run_per_core<F, Fut>(listener: TcpListener, accept_loop: F, on_reload: impl Fn())
    where
        F: Fn(tokio::net::TcpListener, tokio::sync::watch::Receiver<bool>) -> Fut + Send + Clone + 'static,
        Fut: std::future::Future<Output = ()>,
    {
        let addr = match listener.local_addr() {
            Ok(a) => a,
            Err(e) => return eprintln!("per-core runtimes: {}", e),
        };
        // The setup listener has no SO_REUSEPORT, so it can't share the port.
        drop(listener);
        let count = per_core::runtime_count();
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let handles = per_core::spawn(addr, count, stop_rx, accept_loop);
        println!("Started {} per-core runtimes on {}.", handles.len(), addr);

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down gracefully (SIGINT).");
                    break;
                }
                _ = sigterm() => {
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down gracefully (SIGTERM).");
                    break;
                }
                _ = sighup() => {
                    crate::config_reload::reload();
                    on_reload();
                }
            }
        }
        let _ = stop_tx.send(true);
        let _ = tokio::task::spawn_blocking(move || {
            for h in handles {
                let _ = h.join();
            }
        })
        .await;
    }

    /// Binds a plain-HTTP listener on the port in `RWS_CONFIG_HTTP_REDIRECT_PORT` and sends
    /// `301 Moved Permanently` to the HTTPS equivalent of every incoming URL.
    /// Returns immediately if TLS is not configured or the redirect port is not set.
    pub async fn run_redirect() {
        use std::env;

        let cert_path = env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE)
            .unwrap_or_default();
        if cert_path.is_empty() {
            return;
        }

        let redirect_port_str = env::var(crate::entry_point::Config::RWS_CONFIG_HTTP_REDIRECT_PORT)
            .unwrap_or_default();
        if redirect_port_str.is_empty() {
            return;
        }

        let redirect_port: u16 = match redirect_port_str.parse() {
            Ok(p) => p,
            Err(_) => {
                eprintln!("Invalid RWS_CONFIG_HTTP_REDIRECT_PORT: {}", redirect_port_str);
                return;
            }
        };

        let (server_ip, server_port, _) = get_ip_port_thread_count();
        let listeners = match bind_all_families(&server_ip, redirect_port).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("HTTP redirect listener error on port {}: {}", redirect_port, e);
                return;
            }
        };

        println!("HTTP→HTTPS redirect listening on http://{}:{}", server_ip, redirect_port);
        #[cfg(feature = "acme")]
        crate::acme::note_redirect_listener(redirect_port);

        for listener in listeners {
            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((stream, _peer)) => {
                            tokio::spawn(Server::handle_redirect_connection(stream, server_port));
                        }
                        Err(e) => eprintln!("HTTP redirect accept error: {}", e),
                    }
                }
            });
        }

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("\nShutting down HTTP redirect listener (SIGINT).");
                    break;
                }
                _ = sigterm() => {
                    println!("\nShutting down HTTP redirect listener (SIGTERM).");
                    break;
                }
                _ = sighup() => {
                    crate::config_reload::reload();
                }
            }
        }
    }

    /// One redirect-listener connection: answer a pending ACME HTTP-01
    /// challenge, or `301` to the HTTPS equivalent of the requested URL.
    pub(crate) async fn handle_redirect_connection(mut stream: tokio::net::TcpStream, https_port: i32) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut buf = vec![0u8; 4096];
        let n = match stream.read(&mut buf).await {
            Ok(n) => n,
            Err(_) => return,
        };
        let text = String::from_utf8_lossy(&buf[..n]);

        let uri = text.lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/")
            .to_string();

        // ACME HTTP-01 challenges must be answered over
        // plain HTTP, not redirected.
        #[cfg(feature = "acme")]
        if let Some(challenge) = crate::acme::challenge_http_response_shared(&uri).await {
            let _ = stream.write_all(challenge.as_bytes()).await;
            return;
        }

        let host_header = text.lines()
            .find(|l| l.to_lowercase().starts_with("host:"))
            .map(|l| l[5..].trim().to_string());

        let location = match host_header {
            Some(h) => {
                // strip existing port from Host header
                let h_no_port = if h.starts_with('[') {
                    // IPv6: [::1] or [::1]:port
                    h.find(']')
                        .map(|i| h[..=i].to_string())
                        .unwrap_or(h.clone())
                } else {
                    h.rfind(':')
                        .map(|i| h[..i].to_string())
                        .unwrap_or(h.clone())
                };
                if https_port == 443 {
                    format!("https://{}{}", h_no_port, uri)
                } else {
                    format!("https://{}:{}{}", h_no_port, https_port, uri)
                }
            }
            None => format!("https://localhost:{}{}", https_port, uri),
        };

        let response = format!(
            "HTTP/1.1 301 Moved Permanently\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            location
        );
        let _ = stream.write_all(response.as_bytes()).await;
    }

}

#[cfg(feature = "http3")]
impl Server {
    pub async fn run_quic(
        app: impl Application + Send + 'static + Clone,
    ) {
        use crate::tls::create_quinn_server_config_from_vhosts;
        use crate::h3_handler;

        let cert_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE)
            .unwrap_or_default();
        let key_path = std::env::var(crate::entry_point::Config::RWS_CONFIG_TLS_KEY_FILE)
            .unwrap_or_default();

        if cert_path.is_empty() || key_path.is_empty() {
            return;
        }

        let vhosts = crate::entry_point::get_virtual_hosts();
        let server_config = match create_quinn_server_config_from_vhosts(&vhosts, &cert_path, &key_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("QUIC TLS setup failed: {}", e);
                return;
            }
        };

        let (server_ip, server_port, _) = get_ip_port_thread_count();
        let bind_addr = format!("{}:{}", server_ip, server_port);
        let addr: std::net::SocketAddr = match bind_addr.parse() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("Invalid QUIC bind address '{}': {}", bind_addr, e);
                return;
            }
        };

        let endpoint = match quinn::Endpoint::server(server_config, addr) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("QUIC endpoint error: {}", e);
                return;
            }
        };

        println!("Listening for QUIC/HTTP3 on UDP {}:{}", server_ip, server_port);

        loop {
            tokio::select! {
                maybe = endpoint.accept() => {
                    match maybe {
                        Some(incoming) => {
                            let app = app.clone();
                            tokio::spawn(async move {
                                match incoming.await {
                                    Ok(conn) => {
                                        let peer_addr = conn.remote_address();
                                        if let Err(e) = h3_handler::handle_connection(conn, peer_addr, app).await {
                                            eprintln!("H3 connection error: {}", e);
                                        }
                                    }
                                    Err(e) => eprintln!("QUIC connection error: {}", e),
                                }
                            });
                        }
                        None => break,
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down QUIC (SIGINT).");
                    endpoint.close(0u32.into(), b"shutdown");
                    break;
                }
                _ = sigterm() => {
                    crate::metrics::SERVER_READY.store(false, std::sync::atomic::Ordering::SeqCst);
                    println!("\nShutting down QUIC (SIGTERM).");
                    endpoint.close(0u32.into(), b"shutdown");
                    break;
                }
                _ = sighup() => {
                    crate::config_reload::reload();
                }
            }
        }
    }
}


