//! HTTP/1.1 over tokio — the one connection handler for plain TCP and TLS in
//! the `http2` / `http3` builds (`Server::process` remains the synchronous
//! handler of the `http1`-only build).
//!
//! Each connection is a cheap task rather than a pool thread, so idle or slow
//! clients can't exhaust the server: 200 idle sockets used to occupy all 200
//! worker threads. Every read is bounded by [`READ_TIMEOUT`], TLS handshakes
//! by [`TLS_HANDSHAKE_TIMEOUT`], and the (synchronous) `Application` runs on
//! tokio's blocking pool so a slow handler or upstream never stalls the async
//! worker threads.

use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::application::Application;
use crate::entry_point::{get_ip_port_thread_count, get_request_allocation_size};
use crate::header::Header;
use crate::http::VERSION;
use crate::log::Log;
use crate::request::Request;
use crate::response::Response;
use crate::core::New;
use crate::server::sendfile::{self, SendfileSocket};
use crate::server::{framing, Address, ConnectionInfo, Server, REQUEST_HEAD_TIMEOUT};

/// Longest wait for a client to send a request head (or the next request on a
/// keep-alive connection) and for each read of a request body.
pub(crate) const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Longest wait for a TLS handshake to complete after the TCP accept.
pub(crate) const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Serve HTTP/1.1 requests on `stream` until the client closes, asks to close,
/// idles past [`READ_TIMEOUT`], or sends something unparseable.
/// `is_tls` adds the `Strict-Transport-Security` and `Alt-Svc` headers.
pub(crate) async fn serve<S, A>(
    mut stream: S,
    peer_addr: SocketAddr,
    sni_hostname: Option<String>,
    is_tls: bool,
    app: A,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + SendfileSocket + 'static,
    A: Application + Send + Clone + 'static,
{
    let (server_ip, server_port, _) = get_ip_port_thread_count();
    let request_allocation_size = get_request_allocation_size();
    let max_body_size = crate::entry_point::get_max_body_size();

    // Bytes read but not consumed yet: the rest of a request head, a body, or
    // the next pipelined request. See `super::framing`.
    let mut pending: Vec<u8> = Vec::new();
    let mut read_buffer = vec![0u8; request_allocation_size as usize];
    // One idle timer for the whole connection, pushed back before each read.
    // Moving a deadline later is a lock-free compare-and-swap in tokio;
    // creating a timer per read (`tokio::time::timeout`) took the timer
    // driver's lock, shared by all worker threads, twice per read.
    let idle = tokio::time::sleep(READ_TIMEOUT);
    tokio::pin!(idle);

    'session: loop {
        // Read until the request head is complete — it may span several reads,
        // but must finish within REQUEST_HEAD_TIMEOUT of its first byte.
        let mut head_deadline: Option<tokio::time::Instant> = None;
        let head_len = loop {
            framing::skip_leading_empty_lines(&mut pending);
            match framing::scan_head(&pending) {
                framing::HeadScan::Complete(n) => break Some(n),
                framing::HeadScan::TooLarge => {
                    write_and_flush(&mut stream, &Server::header_fields_too_large_response()).await;
                    return Err("request head too large".to_string());
                }
                framing::HeadScan::Incomplete => {}
            }
            if !pending.is_empty() && head_deadline.is_none() {
                head_deadline = Some(tokio::time::Instant::now() + REQUEST_HEAD_TIMEOUT);
            }
            if !read_more(&mut stream, &mut read_buffer, &mut pending, idle.as_mut(), head_deadline).await {
                // Closed, read error, or a timeout: end of the session. A head
                // that ran out its deadline gets a 408 first.
                if head_deadline.is_some_and(|d| tokio::time::Instant::now() >= d) {
                    write_and_flush(&mut stream, &Server::request_timeout_response()).await;
                }
                break None;
            }
            if head_deadline.is_none() {
                head_deadline = Some(tokio::time::Instant::now() + REQUEST_HEAD_TIMEOUT);
            }
        };
        let Some(head_len) = head_len else { break };

        let parsed = Request::parse(&pending[..head_len])
            .and_then(|request| framing::body_framing(&request).map(|f| (request, f)));
        pending.drain(..head_len);
        let (mut request, body_framing) = match parsed {
            Ok(parsed) => parsed,
            Err(message) => {
                write_and_flush(&mut stream, &Server::bad_request_response(message.clone())).await;
                return Err(message);
            }
        };

        // RFC 9110 §10.1.1: only `100-continue` is understood.
        let expects_continue = match request.get_header(Header::_EXPECT.to_string()) {
            Some(h) if h.value.trim().eq_ignore_ascii_case("100-continue") => true,
            Some(h) => {
                let raw = Server::expectation_failed_response(format!(
                    "417 Expectation Failed: unsupported Expect value '{}'", h.value.trim()
                ));
                write_and_flush(&mut stream, &raw).await;
                break;
            }
            None => false,
        };

        match body_framing {
            framing::BodyFraming::None => {}
            framing::BodyFraming::Length(declared_len) => {
                if max_body_size > 0 && declared_len > max_body_size {
                    let raw = Server::payload_too_large_response(format!(
                        "413 Payload Too Large: declared Content-Length {} exceeds the {} byte limit",
                        declared_len, max_body_size
                    ));
                    write_and_flush(&mut stream, &raw).await;
                    break;
                }
                if expects_continue
                    && (pending.len() as u64) < declared_len
                    && stream.write_all(&Server::continue_response()).await.is_err()
                {
                    break;
                }
                while (pending.len() as u64) < declared_len {
                    if !read_more(&mut stream, &mut read_buffer, &mut pending, idle.as_mut(), None).await {
                        break 'session; // closed or stalled mid-body
                    }
                }
                request.body = pending.drain(..declared_len as usize).collect();
            }
            framing::BodyFraming::Chunked => {
                if expects_continue
                    && pending.is_empty()
                    && stream.write_all(&Server::continue_response()).await.is_err()
                {
                    break;
                }
                let mut decoder = framing::ChunkedBody::new(max_body_size);
                loop {
                    match decoder.feed(&pending) {
                        Ok((consumed, done)) => {
                            pending.drain(..consumed);
                            if done {
                                break;
                            }
                        }
                        Err(framing::ChunkError::TooLarge) => {
                            let raw = Server::payload_too_large_response(format!(
                                "413 Payload Too Large: chunked body exceeds the {} byte limit", max_body_size
                            ));
                            write_and_flush(&mut stream, &raw).await;
                            break 'session;
                        }
                        Err(framing::ChunkError::Malformed(message)) => {
                            write_and_flush(&mut stream, &Server::bad_request_response(message.clone())).await;
                            return Err(message);
                        }
                    }
                    if !read_more(&mut stream, &mut read_buffer, &mut pending, idle.as_mut(), None).await {
                        break 'session;
                    }
                }
                request.body = decoder.body;
            }
        }

        let keep_alive = match request.get_header(Header::_CONNECTION.to_string()) {
            Some(h) => !h.value.eq_ignore_ascii_case("close"),
            None => request.http_version == VERSION.http_1_1,
        };

        let connection = ConnectionInfo {
            client: Address { ip: peer_addr.ip().to_string(), port: peer_addr.port() as i32 },
            server: Address { ip: server_ip.clone(), port: server_port },
            request_size: request_allocation_size,
            sni_hostname: sni_hostname.clone(),
        };

        let (request, result) = execute_request(app.clone(), request, connection)
            .await
            .map_err(|e| format!("request handler panicked: {}", e))?;

        let mut response = match result {
            Ok(r) => r,
            Err(message) => {
                write_and_flush(&mut stream, &Server::bad_request_response(message.clone())).await;
                return Err(message);
            }
        };

        crate::metrics::record_request();
        crate::compression::apply_gzip(&request, &mut response);
        if is_tls {
            response.headers.push(Header::get_hsts_header());
            #[cfg(feature = "http3")]
            let alt_svc = format!("h3=\":{}\"", server_port);
            #[cfg(not(feature = "http3"))]
            let alt_svc = format!("h2=\":{}\"", server_port);
            response.headers.push(Header { name: Header::_ALT_SVC.to_string(), value: alt_svc });
        }
        response.headers.push(Header {
            name: Header::_CONNECTION.to_string(),
            value: if keep_alive { "keep-alive" } else { "close" }.to_string(),
        });

        Log::log_access(&request, &response, &peer_addr);

        if response.stream_pipe.is_some() || response.stream_file.is_some() {
            if !write_file_zero_copy(&mut stream, &mut response, &request).await? {
                write_streaming(&mut stream, response, request).await?;
            }
        } else {
            let (head, body) = Response::generate_response_parts(response, request);
            stream.write_all(&head).await.map_err(|e| e.to_string())?;
            stream.write_all(&body).await.map_err(|e| e.to_string())?;
            stream.flush().await.map_err(|e| e.to_string())?;
        }

        if !keep_alive {
            break;
        }
    }
    let _ = stream.shutdown().await;
    Ok(())
}

/// Run the `Application` for one request: directly on this task when the app
/// says the request is quick and non-blocking (`Application::runs_inline` —
/// health checks, small static files), otherwise on the blocking pool via
/// [`execute_blocking`]. The blocking-pool handoff (a shared lock and two
/// cross-thread wake-ups) cost more than serving a small file. A panic is
/// caught either way and costs only this request.
pub(crate) async fn execute_request<A: Application + Send + 'static>(
    app: A,
    request: Request,
    connection: ConnectionInfo,
) -> Result<(Request, Result<Response, String>), String> {
    let async_result = match app.execute_async(&request, &connection) {
        Some(future) => Some(CatchUnwind(future).await?),
        None => None,
    };
    if let Some(result) = async_result {
        return Ok((request, result));
    }
    if app.runs_inline(&request, &connection) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.execute(&request, &connection)))
            .map_err(panic_message)?;
        return Ok((request, result));
    }
    execute_blocking(app, request, connection).await
}

/// Polls a boxed future, turning a panic into an error (one request lost,
/// not the connection task).
struct CatchUnwind<F>(F);

impl<F: std::future::Future + Unpin> std::future::Future for CatchUnwind<F> {
    type Output = Result<F::Output, String>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let inner = &mut self.0;
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| std::pin::Pin::new(inner).poll(cx))) {
            Ok(std::task::Poll::Ready(output)) => std::task::Poll::Ready(Ok(output)),
            Ok(std::task::Poll::Pending) => std::task::Poll::Pending,
            Err(panic) => std::task::Poll::Ready(Err(panic_message(panic))),
        }
    }
}

fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".to_string())
}

/// Run the (synchronous) `Application` off the async scheduler — file reads
/// and upstream proxying must never hold an async worker thread hostage,
/// where a handful of slow requests would stall every connection. Used by the
/// HTTP/1.1, HTTP/2 and HTTP/3 handlers alike. A handler panic is returned
/// as an error, costing that one request.
pub(crate) async fn execute_blocking<A: Application + Send + 'static>(
    app: A,
    request: Request,
    connection: ConnectionInfo,
) -> Result<(Request, Result<Response, String>), String> {
    tokio::task::spawn_blocking(move || {
        let result = app.execute(&request, &connection);
        (request, result)
    })
    .await
    .map_err(|e| e.to_string())
}

/// One read (bounded by [`READ_TIMEOUT`], via the connection's `idle` timer,
/// and by `deadline` when given) appended to `pending`. `false` when the
/// peer closed, the read failed, or it timed out.
async fn read_more<S: AsyncRead + Unpin>(
    stream: &mut S,
    read_buffer: &mut [u8],
    pending: &mut Vec<u8>,
    mut idle: std::pin::Pin<&mut tokio::time::Sleep>,
    deadline: Option<tokio::time::Instant>,
) -> bool {
    let idle_until = tokio::time::Instant::now() + READ_TIMEOUT;
    idle.as_mut().reset(deadline.map_or(idle_until, |d| d.min(idle_until)));
    tokio::select! {
        read = stream.read(read_buffer) => match read {
            Ok(n) if n > 0 => {
                pending.extend_from_slice(&read_buffer[..n]);
                true
            }
            _ => false,
        },
        _ = idle => false,
    }
}

async fn write_and_flush<S: AsyncWrite + Unpin>(stream: &mut S, bytes: &[u8]) {
    if stream.write_all(bytes).await.is_ok() {
        let _ = stream.flush().await;
    }
}

/// Write a `stream_file` / `stream_pipe` response by running the existing
/// synchronous chunked writers (`Server::write_chunked_file`,
/// `Server::pipe_stream`) on the blocking pool, feeding their output to the
/// connection through a bounded channel — so a large file or a long SSE
/// stream never holds an async worker thread.
async fn write_streaming<S: AsyncWrite + Unpin>(
    stream: &mut S,
    mut response: Response,
    request: Request,
) -> Result<(), String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(8);
    let writer = tokio::task::spawn_blocking(move || {
        let mut sink = ChannelWriter(tx);
        if let Some(reader) = response.stream_pipe.take() {
            Server::pipe_stream(&mut sink, response, request, reader)
        } else {
            let filepath = response.stream_file.clone().unwrap_or_default();
            Server::write_chunked_file(&mut sink, response, request, &filepath)
        }
    });

    let mut client_error = None;
    while let Some(chunk) = rx.recv().await {
        let written = async {
            stream.write_all(&chunk).await?;
            stream.flush().await
        }
        .await;
        if let Err(e) = written {
            // Dropping the receiver makes the producer's next write fail.
            client_error = Some(e.to_string());
            break;
        }
    }
    drop(rx);

    let produced = writer.await.map_err(|e| format!("streaming writer panicked: {}", e))?;
    match client_error {
        Some(e) => Err(e),
        None => produced,
    }
}

/// A `stream_file` GET with a known `Content-Length` over plain TCP: write
/// the head, then let the kernel copy the file to the socket (`sendfile`).
/// `Ok(false)` — nothing written — when that doesn't apply (TLS, HEAD, a
/// chunked or piped body, an unsupported OS, a file that won't open); the
/// caller then uses [`write_streaming`].
async fn write_file_zero_copy<S: AsyncWrite + Unpin + SendfileSocket>(
    stream: &mut S,
    response: &mut Response,
    request: &Request,
) -> Result<bool, String> {
    if !sendfile::SUPPORTED || response.stream_pipe.is_some() || request.method != crate::request::METHOD.get {
        return Ok(false);
    }
    let Some(path) = response.stream_file.clone() else { return Ok(false) };
    let Some(len) = response
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(Header::_CONTENT_LENGTH))
        .and_then(|h| h.value.trim().parse::<u64>().ok())
    else {
        return Ok(false);
    };
    if stream.sendfile_socket().is_none() {
        return Ok(false);
    }
    let Ok(file) = std::fs::File::open(&path) else { return Ok(false) };

    let (head, _) = Server::stream_file_head(std::mem::replace(response, Response::new()));
    stream.write_all(&head).await.map_err(|e| e.to_string())?;
    stream.flush().await.map_err(|e| e.to_string())?;
    let socket = stream.sendfile_socket().expect("checked above");
    sendfile::send_file(socket, &file, len).await.map_err(|e| e.to_string())?;
    Ok(true)
}

/// `std::io::Write` that hands each buffer to the async writer task.
struct ChannelWriter(tokio::sync::mpsc::Sender<Vec<u8>>);

impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .blocking_send(buf.to_vec())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "client connection closed"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::STATUS_CODE_REASON_PHRASE;

    #[derive(Clone)]
    struct Hello;

    impl Application for Hello {
        fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
            if request.request_uri == "/panic" {
                panic!("handler bug");
            }
            Ok(Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "hello"))
        }
    }

    fn peer() -> SocketAddr {
        "127.0.0.1:5555".parse().unwrap()
    }

    async fn read_until_count(client: &mut tokio::io::DuplexStream, needle: &str, count: usize) -> String {
        let mut out = Vec::new();
        let mut buf = [0u8; 4096];
        while String::from_utf8_lossy(&out).matches(needle).count() < count {
            let n = client.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
        String::from_utf8_lossy(&out).to_string()
    }

    #[tokio::test]
    async fn keep_alive_serves_several_requests_on_one_connection() {
        let (mut client, server) = tokio::io::duplex(64 * 1024);
        let task = tokio::spawn(serve(server, peer(), None, false, Hello));
        client.write_all(b"GET /a HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let first = read_until_count(&mut client, "hello", 1).await;
        client.write_all(b"GET /b HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await.unwrap();
        let second = read_until_count(&mut client, "hello", 1).await;
        assert!(first.starts_with("HTTP/1.1 200") && first.contains("Connection: keep-alive"), "{first}");
        assert!(second.contains("Connection: close"), "{second}");
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn idle_connection_is_closed_after_the_read_timeout() {
        let (_client, server) = tokio::io::duplex(1024);
        let task = tokio::spawn(serve(server, peer(), None, false, Hello));
        // Paused clock: the timeout fires as soon as the runtime is idle.
        let result = tokio::time::timeout(READ_TIMEOUT + Duration::from_secs(1), task).await;
        assert!(result.expect("serve should return after READ_TIMEOUT").unwrap().is_ok());
    }

    #[tokio::test]
    async fn handler_panic_costs_the_request_not_the_runtime() {
        let (mut client, server) = tokio::io::duplex(1024);
        let task = tokio::spawn(serve(server, peer(), None, false, Hello));
        client.write_all(b"GET /panic HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let result = task.await.unwrap();
        assert!(result.unwrap_err().contains("panicked"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn multi_thread_runtime_serves_and_survives_handler_panics() {
        // The rws binary runs a multi-threaded runtime.
        let (mut client, server) = tokio::io::duplex(64 * 1024);
        let task = tokio::spawn(serve(server, peer(), None, false, Hello));
        client.write_all(b"GET /a HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        assert!(read_until_count(&mut client, "hello", 1).await.starts_with("HTTP/1.1 200"));
        client.write_all(b"GET /panic HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        assert!(task.await.unwrap().unwrap_err().contains("panicked"));
    }

    #[tokio::test]
    async fn tls_responses_carry_hsts_and_alt_svc() {
        let (mut client, server) = tokio::io::duplex(64 * 1024);
        let task = tokio::spawn(serve(server, peer(), Some("x".to_string()), true, Hello));
        client.write_all(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await.unwrap();
        let response = read_until_count(&mut client, "hello", 1).await;
        assert!(response.contains("Strict-Transport-Security"), "{response}");
        assert!(response.contains("Alt-Svc"), "{response}");
        task.await.unwrap().unwrap();
    }

    #[derive(Clone)]
    struct BigFile(String);

    impl Application for BigFile {
        fn execute(&self, _request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
            let mut response = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "");
            response.content_range_list.clear();
            response.stream_file = Some(self.0.clone());
            Ok(response)
        }
    }

    #[tokio::test]
    async fn stream_file_is_sent_chunked_and_complete() {
        let file = crate::test_env::TempPath::new("http1_async_big.bin");
        let content: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&*file, &content).unwrap();

        let (mut client, server) = tokio::io::duplex(16 * 1024);
        let task = tokio::spawn(serve(server, peer(), None, false, BigFile(file.as_str().to_string())));
        client.write_all(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await.unwrap();
        let mut raw = Vec::new();
        client.read_to_end(&mut raw).await.unwrap();
        task.await.unwrap().unwrap();

        let head_end = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        assert!(String::from_utf8_lossy(&raw[..head_end]).contains("Transfer-Encoding: chunked"));
        let mut decoded = Vec::new();
        crate::server::body_stream::ChunkedDecoder::new(&raw[head_end..]).read_to_end_sync(&mut decoded);
        assert_eq!(content, decoded);
    }

    trait ReadToEndSync {
        fn read_to_end_sync(self, out: &mut Vec<u8>);
    }
    impl<R: std::io::Read> ReadToEndSync for crate::server::body_stream::ChunkedDecoder<R> {
        fn read_to_end_sync(mut self, out: &mut Vec<u8>) {
            std::io::Read::read_to_end(&mut self, out).unwrap();
        }
    }

    #[derive(Clone)]
    struct Echo;

    impl Application for Echo {
        fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
            let text = format!("uri={} body={}", request.request_uri, String::from_utf8_lossy(&request.body));
            Ok(Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &text))
        }
    }

    async fn read_to_end(client: &mut tokio::io::DuplexStream) -> String {
        let mut out = Vec::new();
        client.read_to_end(&mut out).await.unwrap();
        String::from_utf8_lossy(&out).to_string()
    }

    #[tokio::test]
    async fn large_request_head_spanning_reads_is_served() {
        let (mut client, server) = tokio::io::duplex(1024);
        let task = tokio::spawn(serve(server, peer(), None, false, Echo));
        let mut raw = b"GET /big HTTP/1.1\r\nCookie: ".to_vec();
        raw.extend(std::iter::repeat(b'c').take(40_000));
        raw.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
        let writer = tokio::spawn(async move {
            client.write_all(&raw).await.unwrap();
            client
        });
        let mut client = writer.await.unwrap();
        let out = read_to_end(&mut client).await;
        assert!(out.starts_with("HTTP/1.1 200") && out.contains("uri=/big"), "{out}");
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn oversized_request_head_gets_431() {
        let (mut client, server) = tokio::io::duplex(256 * 1024);
        let task = tokio::spawn(serve(server, peer(), None, false, Echo));
        let mut raw = b"GET / HTTP/1.1\r\nCookie: ".to_vec();
        raw.extend(std::iter::repeat(b'c').take(framing::MAX_HEAD_SIZE + 10));
        client.write_all(&raw).await.unwrap();
        let out = read_to_end(&mut client).await;
        assert!(out.starts_with("HTTP/1.1 431"), "{out}");
        assert!(task.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn pipelined_and_chunked_requests_are_framed() {
        let (mut client, server) = tokio::io::duplex(64 * 1024);
        let task = tokio::spawn(serve(server, peer(), None, false, Echo));
        client.write_all(
            b"POST /a HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc\
              POST /b HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nhi\r\n0\r\n\r\n\
              GET /c HTTP/1.1\r\nConnection: close\r\n\r\n",
        ).await.unwrap();
        let out = read_to_end(&mut client).await;
        assert!(out.contains("uri=/a body=abc"), "{out}");
        assert!(out.contains("uri=/b body=hi"), "{out}");
        assert!(out.contains("uri=/c body="), "{out}");
        assert!(task.await.unwrap().is_ok());
    }


    /// Answers inline (opted in) or not, and records the thread it ran on.
    #[derive(Clone)]
    struct ThreadProbe {
        inline: bool,
        ran_on: std::sync::Arc<std::sync::Mutex<Option<std::thread::ThreadId>>>,
    }

    impl Application for ThreadProbe {
        fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
            if request.request_uri == "/panic" {
                panic!("inline handler bug");
            }
            *self.ran_on.lock().unwrap() = Some(std::thread::current().id());
            Ok(Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "ok"))
        }

        fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
            self.inline
        }
    }

    fn probe_connection() -> ConnectionInfo {
        ConnectionInfo {
            client: Address { ip: "127.0.0.1".to_string(), port: 1 },
            server: Address { ip: "127.0.0.1".to_string(), port: 2 },
            request_size: 1024,
            sni_hostname: None,
        }
    }

    fn probe_request(uri: &str) -> Request {
        Request { method: "GET".to_string(), request_uri: uri.to_string(), http_version: "HTTP/1.1".to_string(), headers: vec![], body: vec![] }
    }

    #[tokio::test]
    async fn inline_requests_run_on_the_calling_thread_others_on_the_blocking_pool() {
        let here = std::thread::current().id();
        for (inline, expect_here) in [(true, true), (false, false)] {
            let probe = ThreadProbe { inline, ran_on: Default::default() };
            let (_, result) = execute_request(probe.clone(), probe_request("/"), probe_connection()).await.unwrap();
            assert!(result.is_ok());
            let ran_on = probe.ran_on.lock().unwrap().unwrap();
            assert_eq!(ran_on == here, expect_here, "inline = {inline}");
        }
    }

    #[tokio::test]
    async fn inline_handler_panic_is_caught() {
        let probe = ThreadProbe { inline: true, ran_on: Default::default() };
        let err = execute_request(probe, probe_request("/panic"), probe_connection()).await.unwrap_err();
        assert!(err.contains("inline handler bug"), "{err}");
    }


    /// A `stream_file` response with a known length — the static-file shape.
    #[derive(Clone)]
    struct SizedFile(String, usize);

    impl Application for SizedFile {
        fn execute(&self, _request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
            let mut response = Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, None);
            response.headers.push(Header { name: Header::_CONTENT_LENGTH.to_string(), value: self.1.to_string() });
            response.stream_file = Some(self.0.clone());
            Ok(response)
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn plain_tcp_file_is_sent_with_sendfile_and_keep_alive_continues() {
        let file = crate::test_env::TempPath::new("http1_async_sendfile.bin");
        let content: Vec<u8> = (0..700_000u32).map(|i| (i % 249) as u8).collect();
        std::fs::write(&*file, &content).unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = SizedFile(file.as_str().to_string(), content.len());
        let server = tokio::spawn(async move {
            let (tcp, peer_addr) = listener.accept().await.unwrap();
            serve(tcp, peer_addr, None, false, app).await
        });

        let mut client = tokio::net::TcpStream::connect(addr).await.unwrap();
        client.write_all(b"GET /a HTTP/1.1\r\nHost: x\r\n\r\nGET /b HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await.unwrap();
        let mut raw = Vec::new();
        client.read_to_end(&mut raw).await.unwrap();
        assert!(server.await.unwrap().is_ok());

        // Two complete responses, each a head plus exactly Content-Length bytes.
        let mut rest = &raw[..];
        for _ in 0..2 {
            let head_end = rest.windows(4).position(|w| w == b"\r\n\r\n").expect("response head") + 4;
            let head = String::from_utf8_lossy(&rest[..head_end]).to_string();
            assert!(head.starts_with("HTTP/1.1 200"), "{head}");
            assert!(!head.contains("Transfer-Encoding"), "{head}");
            assert!(rest[head_end..].len() >= content.len());
            assert!(rest[head_end..head_end + content.len()] == content[..], "body differs from the file");
            rest = &rest[head_end + content.len()..];
        }
        assert!(rest.is_empty());
    }


    #[tokio::test(start_paused = true)]
    async fn idle_timer_restarts_on_every_body_read() {
        // Body pieces 20 s apart: 40 s in total, but never 30 s without a read.
        let (mut client, server) = tokio::io::duplex(1024);
        let task = tokio::spawn(serve(server, peer(), None, false, Echo));
        client.write_all(b"POST /slow HTTP/1.1\r\nContent-Length: 6\r\nConnection: close\r\n\r\nab").await.unwrap();
        tokio::time::sleep(Duration::from_secs(20)).await;
        client.write_all(b"cd").await.unwrap();
        tokio::time::sleep(Duration::from_secs(20)).await;
        client.write_all(b"ef").await.unwrap();
        let out = read_to_end(&mut client).await;
        assert!(out.contains("uri=/slow body=abcdef"), "{out}");
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn a_head_trickled_past_the_head_deadline_gets_408() {
        // Pieces 20 s apart never trip the idle timeout, but the head as a
        // whole must arrive within REQUEST_HEAD_TIMEOUT.
        let (mut client, server) = tokio::io::duplex(1024);
        let task = tokio::spawn(serve(server, peer(), None, false, Hello));
        client.write_all(b"GET /slow HTTP/1.1\r\n").await.unwrap();
        tokio::time::sleep(Duration::from_secs(20)).await;
        client.write_all(b"Host: x\r\n").await.unwrap();
        tokio::time::sleep(Duration::from_secs(20)).await;
        let _ = client.write_all(b"\r\n").await;
        let out = read_to_end(&mut client).await;
        assert!(out.starts_with("HTTP/1.1 408"), "{out}");
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_for_the_next_request_is_not_a_head_timeout() {
        // 25 s idle between keep-alive requests: only the idle timeout applies.
        let (mut client, server) = tokio::io::duplex(1024);
        let task = tokio::spawn(serve(server, peer(), None, false, Hello));
        client.write_all(b"GET /a HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let first = read_until_count(&mut client, "hello", 1).await;
        assert!(first.starts_with("HTTP/1.1 200"));
        tokio::time::sleep(Duration::from_secs(25)).await;
        client.write_all(b"GET /b HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await.unwrap();
        let second = read_until_count(&mut client, "hello", 1).await;
        assert!(second.starts_with("HTTP/1.1 200"), "{second}");
        assert!(task.await.unwrap().is_ok());
    }

}
