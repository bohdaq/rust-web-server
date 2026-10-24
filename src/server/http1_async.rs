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
use crate::server::{Address, ConnectionInfo, Server};

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
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    A: Application + Send + Clone + 'static,
{
    let (server_ip, server_port, _) = get_ip_port_thread_count();
    let request_allocation_size = get_request_allocation_size();
    let max_body_size = crate::entry_point::get_max_body_size();

    loop {
        let mut buffer = vec![0u8; request_allocation_size as usize];
        let n = match tokio::time::timeout(READ_TIMEOUT, stream.read(&mut buffer)).await {
            Ok(Ok(n)) if n > 0 => n,
            // Closed, read error, or idle past the timeout: end of the session.
            _ => break,
        };

        // Trim to the bytes actually read — the zero-padded rest of `buffer`
        // would otherwise become trailing NUL bytes in the body.
        let mut request = match Request::parse(&buffer[..n]) {
            Ok(r) => r,
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

        // A body longer than the first read keeps arriving in later reads.
        if let Some(declared_len) = request
            .get_header(Header::_CONTENT_LENGTH.to_string())
            .and_then(|h| h.value.trim().parse::<u64>().ok())
        {
            if max_body_size > 0 && declared_len > max_body_size {
                let raw = Server::payload_too_large_response(format!(
                    "413 Payload Too Large: declared Content-Length {} exceeds the {} byte limit",
                    declared_len, max_body_size
                ));
                write_and_flush(&mut stream, &raw).await;
                break;
            }
            if expects_continue && stream.write_all(&Server::continue_response()).await.is_err() {
                break;
            }
            while (request.body.len() as u64) < declared_len {
                let mut more = vec![0u8; request_allocation_size as usize];
                match tokio::time::timeout(READ_TIMEOUT, stream.read(&mut more)).await {
                    Ok(Ok(k)) if k > 0 => request.body.extend_from_slice(&more[..k]),
                    _ => break, // closed early or stalled: proceed with the short body
                }
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

        let (request, result) = execute_blocking(app.clone(), request, connection)
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
            write_streaming(&mut stream, response, request).await?;
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
}
