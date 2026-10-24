//! Non-blocking upstream forwarding for plain-HTTP backends (`http2` builds).
//!
//! The synchronous forwarder (`forward_upstream`) holds a blocking-pool
//! thread for the whole round trip to the backend: under load most threads
//! sat in `recvfrom` waiting for upstream bytes, and each request paid two
//! cross-thread hand-offs. This one runs on the connection's own task with
//! tokio sockets. Same behaviour otherwise: pooled keep-alive connections
//! (per `host:port`), the same request bytes (`build_request`), a stale pooled
//! connection retried once for idempotent methods only, and streamed
//! responses (SSE, chunked, > 1 MB) handed to the existing streaming writer
//! with the socket converted to a blocking one.
//!
//! Used by config-driven proxy routes without middleware
//! (`DynamicProxy::execute_async`); everything else keeps the blocking path.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{
    build_request, is_idempotent, normalize_upstream_response, parse_status_and_headers, response_has_no_body, should_stream_response,
    ConcatReader, Pool, POOL_MAX_IDLE,
};
use crate::request::Request;
use crate::response::Response;

/// Idle async backend connections, per `host:port`.
fn pool() -> &'static Pool<TcpStream> {
    static POOL: std::sync::OnceLock<Pool<TcpStream>> = std::sync::OnceLock::new();
    POOL.get_or_init(|| Pool::new(POOL_MAX_IDLE, Duration::from_secs(60)))
}

enum Exchange {
    Done(Response),
    /// Failed before any response byte arrived: the backend may have closed
    /// a pooled connection, so an idempotent request can be retried.
    NoResponse(String),
    Failed(String),
}

/// Forward `request` to `host:port` and return the backend's response.
pub(crate) async fn forward(
    request: &Request,
    client_ip: &str,
    host: &str,
    port: u16,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<Response, String> {
    let key = format!("{}:{}", host, port);
    let pool = pool();
    if let Some(stream) = pool.acquire(&key) {
        match exchange(pool, &key, stream, request, host, client_ip, read_timeout).await {
            Exchange::Done(response) => return Ok(response),
            Exchange::NoResponse(_) if is_idempotent(&request.method) => {} // stale: retry fresh
            Exchange::NoResponse(e) | Exchange::Failed(e) => return Err(e),
        }
    }
    let stream = connect(&key, connect_timeout).await?;
    match exchange(pool, &key, stream, request, host, client_ip, read_timeout).await {
        Exchange::Done(response) => Ok(response),
        Exchange::NoResponse(e) | Exchange::Failed(e) => Err(e),
    }
}

/// Connect to the first address of `key` that accepts (`localhost` may
/// resolve to `::1` before `127.0.0.1`).
async fn connect(key: &str, connect_timeout: Duration) -> Result<TcpStream, String> {
    let addrs = tokio::net::lookup_host(key)
        .await
        .map_err(|e| format!("DNS lookup for {} failed: {}", key, e))?;
    let mut last_error = format!("no address resolved for {}", key);
    for addr in addrs {
        match tokio::time::timeout(connect_timeout, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Ok(Err(e)) => last_error = format!("connect to {} ({}) failed: {}", key, addr, e),
            Err(_) => last_error = format!("connect to {} ({}) timed out", key, addr),
        }
    }
    Err(last_error)
}

/// One request/response on `stream`. A fully read keep-alive response puts
/// the connection back in `pool`.
async fn exchange(
    pool: &Pool<TcpStream>,
    key: &str,
    mut stream: TcpStream,
    request: &Request,
    host: &str,
    client_ip: &str,
    read_timeout: Duration,
) -> Exchange {
    let request_bytes = build_request(request, host, client_ip, true);
    if let Err(e) = stream.write_all(&request_bytes).await {
        return Exchange::NoResponse(format!("write to backend failed: {}", e));
    }

    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut tmp = [0u8; 8192];

    // Response head, bounded by the read timeout as a whole.
    let head = tokio::time::timeout(read_timeout, async {
        loop {
            match stream.read(&mut tmp).await {
                Ok(0) => {
                    return Err((buf.is_empty(), "backend closed connection before headers were complete".to_string()))
                }
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(e) => return Err((buf.is_empty(), e.to_string())),
            }
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                return Ok(pos + 4);
            }
        }
    })
    .await;
    let header_end = match head {
        Ok(Ok(end)) => end,
        Ok(Err((true, e))) => return Exchange::NoResponse(e),
        Ok(Err((false, e))) => return Exchange::Failed(e),
        Err(_) => return Exchange::Failed("backend response timed out".to_string()),
    };
    let header_lower = std::str::from_utf8(&buf[..header_end]).unwrap_or("").to_ascii_lowercase();

    let no_body = response_has_no_body(&request.method, &header_lower);

    if !no_body && should_stream_response(&header_lower) {
        // Streaming: the existing writer reads the body from a blocking
        // socket on the blocking pool; the connection isn't reused.
        let body_prefix = buf.split_off(header_end);
        let std_stream = match stream.into_std().and_then(|s| {
            s.set_nonblocking(false)?;
            s.set_read_timeout(Some(read_timeout))?;
            Ok(s)
        }) {
            Ok(s) => s,
            Err(e) => return Exchange::Failed(e.to_string()),
        };
        return match parse_status_and_headers(&buf) {
            Ok(mut response) => {
                response.stream_pipe = Some(Box::new(ConcatReader::new(body_prefix, std_stream)));
                Exchange::Done(response)
            }
            Err(e) => Exchange::Failed(e),
        };
    }

    let connection_close = header_lower.lines().any(|l| l.starts_with("connection:") && l.contains("close"));
    let content_length: Option<usize> =
        header_lower.lines().find_map(|l| l.strip_prefix("content-length:")?.trim().parse().ok());

    let body = tokio::time::timeout(read_timeout, async {
        match content_length {
            _ if no_body => {
                buf.truncate(header_end);
                Ok(!connection_close)
            }
            Some(len) => {
                while buf.len() < header_end + len {
                    let n = stream.read(&mut tmp).await.map_err(|e| e.to_string())?;
                    if n == 0 {
                        return Err("backend closed connection mid-body".to_string());
                    }
                    buf.extend_from_slice(&tmp[..n]);
                }
                buf.truncate(header_end + len);
                Ok(!connection_close)
            }
            None => {
                // Delimited by the connection closing.
                loop {
                    let n = stream.read(&mut tmp).await.map_err(|e| e.to_string())?;
                    if n == 0 {
                        return Ok(false);
                    }
                    buf.extend_from_slice(&tmp[..n]);
                }
            }
        }
    })
    .await;
    let reusable = match body {
        Ok(Ok(reusable)) => reusable,
        Ok(Err(e)) => return Exchange::Failed(e),
        Err(_) => return Exchange::Failed("backend response timed out".to_string()),
    };
    if reusable {
        pool.release(key, stream);
    }
    match Response::parse(&buf) {
        Ok(response) => Exchange::Done(normalize_upstream_response(response)),
        Err(e) => Exchange::Failed(e),
    }
}

#[cfg(test)]
mod tests;
