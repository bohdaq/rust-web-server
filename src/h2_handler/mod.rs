use bytes::Bytes;
use http::StatusCode;

use crate::application::Application;
use crate::entry_point::get_ip_port_thread_count;
use crate::header::Header;
use crate::http::VERSION;
use crate::log::Log;
use crate::request::Request;
use crate::response::Response;
use crate::server::{Address, ConnectionInfo};

// Headers forbidden in HTTP/2 responses per RFC 9113 §8.2.2
const FORBIDDEN_H2_RESPONSE_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "transfer-encoding",
    "upgrade",
    "proxy-connection",
    "te",
];

/// `SETTINGS_MAX_CONCURRENT_STREAMS` advertised to every HTTP/2 client.
/// Without it one connection may open unlimited simultaneous streams — a
/// resource-exhaustion vector. 250 matches Go's net/http (nginx uses 128);
/// HTTP/3 has the equivalent limit from quinn's default transport config.
pub const MAX_CONCURRENT_STREAMS: u32 = 250;

/// `SETTINGS_MAX_HEADER_LIST_SIZE`: the largest request header block (decoded)
/// a client may send. Without it the `h2` crate's own default (16 MiB) lets
/// one request make the server buffer megabytes of headers. 64 KiB is well
/// above real traffic (nginx's defaults allow ~32 KB) and matches the HTTP/3
/// limit in `h3_handler`.
pub const MAX_HEADER_LIST_SIZE: u32 = 64 * 1024;

pub async fn handle_connection(
    stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    peer_addr: std::net::SocketAddr,
    sni_hostname: Option<String>,
    app: impl Application + Send + Clone + 'static,
) -> Result<(), String> {
    let mut conn = h2::server::Builder::new()
        .max_concurrent_streams(MAX_CONCURRENT_STREAMS)
        .max_header_list_size(MAX_HEADER_LIST_SIZE)
        .handshake(stream)
        .await
        .map_err(|e| format!("H2 handshake failed: {}", e))?;

    let (server_ip, server_port, _) = get_ip_port_thread_count();
    let connection = ConnectionInfo {
        client: Address {
            ip: peer_addr.ip().to_string(),
            port: peer_addr.port() as i32,
        },
        server: Address {
            ip: server_ip,
            port: server_port,
        },
        request_size: 0,
        sni_hostname,
    };

    while let Some(result) = conn.accept().await {
        match result {
            Ok((request, respond)) => {
                // A request without a body that the app answers inline
                // (`Application::runs_inline`: static files, health checks —
                // never anything that waits) is answered right here, as on
                // HTTP/1.1. A task per stream, then waking this task to write
                // the frames, cost about as much as the request itself. The
                // response is queued on the stream and written by the next
                // `accept()` poll.
                if request.body().is_end_stream() {
                    let rws_request = rws_request(&request, Vec::new());
                    if app.runs_inline(&rws_request, &connection) {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            app.execute(&rws_request, &connection)
                        }));
                        let result = result.map_err(|_| "handler panicked".to_string());
                        if let Some((respond, response, is_head)) = finish(rws_request, result, respond, peer_addr) {
                            if response.stream_file.is_some() || response.stream_pipe.is_some() {
                                // A streamed body waits on flow control: not here.
                                tokio::spawn(send_h2_response(respond, response, is_head));
                            } else {
                                send_h2_response(respond, response, is_head).await;
                            }
                        }
                        continue;
                    }
                }
                tokio::spawn(handle_stream(request, respond, connection.clone(), peer_addr, app.clone()));
            }
            Err(e) => {
                eprintln!("H2 stream accept error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// The rws `Request` for an HTTP/2 request (method, path and query,
/// headers) with `body`.
fn rws_request(request: &http::Request<h2::RecvStream>, body: Vec<u8>) -> Request {
    let method = request.method().to_string();
    let uri = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    let mut headers: Vec<Header> = Vec::with_capacity(request.headers().len());
    for (name, value) in request.headers() {
        if let Ok(v) = value.to_str() {
            headers.push(Header {
                name: name.as_str().to_string().into(),
                value: v.to_string().into(),
            });
        }
    }

    Request {
        method,
        request_uri: uri,
        http_version: VERSION.http_2_0.to_string(),
        headers,
        body,
    }
}

/// After the app has run: errors become a 500 here (`None`); otherwise
/// metrics, gzip, HSTS and the access log, and what to send.
fn finish(
    rws_request: Request,
    result: Result<Result<Response, String>, String>,
    respond: h2::server::SendResponse<Bytes>,
    peer_addr: std::net::SocketAddr,
) -> Option<(h2::server::SendResponse<Bytes>, Response, bool)> {
    let mut rws_response = match result {
        Ok(Ok(r)) => r,
        Ok(Err(message)) => {
            eprintln!("App error on H2 stream: {}", message);
            send_error_response(respond, StatusCode::INTERNAL_SERVER_ERROR);
            return None;
        }
        Err(e) => {
            eprintln!("Handler panicked on H2 stream: {}", e);
            send_error_response(respond, StatusCode::INTERNAL_SERVER_ERROR);
            return None;
        }
    };
    crate::metrics::record_request();
    crate::compression::apply_gzip(&rws_request, &mut rws_response);
    rws_response.headers.push(crate::header::Header::get_hsts_header());

    Log::log_access(&rws_request, &rws_response, &peer_addr);

    let is_head = rws_request.method == crate::request::METHOD.head;
    Some((respond, rws_response, is_head))
}

async fn handle_stream(
    request: http::Request<h2::RecvStream>,
    respond: h2::server::SendResponse<Bytes>,
    connection: ConnectionInfo,
    peer_addr: std::net::SocketAddr,
    app: impl Application + Send + 'static,
) {
    let head_only = rws_request(&request, Vec::new());

    let max_body_size = crate::entry_point::get_max_body_size();
    let mut body_stream = request.into_body();
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = body_stream.data().await {
        match chunk {
            Ok(data) => {
                let len = data.len();
                body.extend_from_slice(&data);
                let _ = body_stream.flow_control().release_capacity(len);
                if max_body_size > 0 && body.len() as u64 > max_body_size {
                    send_error_response(respond, StatusCode::PAYLOAD_TOO_LARGE);
                    return;
                }
            }
            Err(e) => {
                eprintln!("H2 body read error: {}", e);
                break;
            }
        }
    }

    let rws_request = Request { body, ..head_only };

    let (rws_request, result) = match crate::server::http1_async::execute_request(app, rws_request, &connection).await {
        Ok((rws_request, result)) => (rws_request, Ok(result)),
        Err(e) => (Request { method: String::new(), request_uri: String::new(), http_version: String::new(), headers: Vec::new(), body: Vec::new() }, Err(e)),
    };
    if let Some((respond, response, is_head)) = finish(rws_request, result, respond, peer_addr) {
        send_h2_response(respond, response, is_head).await;
    }
}

async fn send_h2_response(
    mut respond: h2::server::SendResponse<Bytes>,
    mut rws_response: Response,
    is_head: bool,
) {
    // stream_file / stream_pipe bodies: produced in chunks, sent below.
    let mut streamed_body = crate::server::body_stream::spawn_producer(&mut rws_response);
    if is_head {
        streamed_body = None;
    }

    // Promote Content-Type, Content-Length (and Content-Range on 206) from content_range_list
    // into response headers, mirroring what Response::generate_response() does for HTTP/1.1.
    if rws_response.content_range_list.len() == 1 {
        let cr = &rws_response.content_range_list[0];
        rws_response.headers.push(crate::header::Header {
            name: crate::header::Header::_CONTENT_TYPE.to_string().into(),
            value: cr.content_type.clone().into(),
        });
        let content_range = crate::response::content_range_header_value(rws_response.status_code, cr);
        rws_response.headers.push(crate::header::Header {
            name: crate::header::Header::_CONTENT_LENGTH.to_string().into(),
            value: cr.body.len().to_string().into(),
        });
        if let Some(value) = content_range {
            rws_response.headers.push(crate::header::Header {
                name: crate::header::Header::_CONTENT_RANGE.to_string().into(),
                value: value.into(),
            });
        }
    } else if rws_response.content_range_list.len() > 1 {
        rws_response.headers.push(crate::header::Header {
            name: crate::header::Header::_CONTENT_TYPE.to_string().into(),
            value: crate::range::Range::MULTIPART_BYTERANGES_CONTENT_TYPE.into(),
        });
    }

    let status = StatusCode::from_u16(rws_response.status_code as u16)
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut builder = http::Response::builder().status(status);

    for header in &rws_response.headers {
        let name_lower = header.name.to_lowercase();
        if FORBIDDEN_H2_RESPONSE_HEADERS.contains(&name_lower.as_str()) {
            continue;
        }
        match (
            http::header::HeaderName::from_bytes(header.name.as_bytes()),
            http::header::HeaderValue::from_str(&header.value),
        ) {
            (Ok(name), Ok(value)) => {
                builder = builder.header(name, value);
            }
            _ => {}
        }
    }

    let body = Response::generate_body(rws_response.content_range_list);
    let end_stream = (body.is_empty() || is_head) && streamed_body.is_none();

    let h2_response = match builder.body(()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to build H2 response: {}", e);
            return;
        }
    };

    match respond.send_response(h2_response, end_stream) {
        Ok(mut send_stream) => {
            if let Some(mut chunks) = streamed_body {
                while let Some(chunk) = chunks.recv().await {
                    let chunk = match chunk {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("H2 streaming body error: {}", e);
                            send_stream.send_reset(h2::Reason::INTERNAL_ERROR);
                            return;
                        }
                    };
                    if let Err(e) = send_with_flow_control(&mut send_stream, Bytes::from(chunk)).await {
                        eprintln!("H2 send data error: {}", e);
                        return;
                    }
                }
                if let Err(e) = send_stream.send_data(Bytes::new(), true) {
                    eprintln!("H2 send data error: {}", e);
                }
            } else if !end_stream {
                if let Err(e) = send_stream.send_data(Bytes::from(body), true) {
                    eprintln!("H2 send data error: {}", e);
                }
            }
        }
        Err(e) => eprintln!("H2 send response error: {}", e),
    }
}

/// Send `data` within the peer's flow-control window: reserve capacity, wait
/// until some is granted, send that much, repeat. Without this, `send_data`
/// buffers everything — a whole large file — in memory.
async fn send_with_flow_control(
    send_stream: &mut h2::SendStream<Bytes>,
    mut data: Bytes,
) -> Result<(), String> {
    while !data.is_empty() {
        send_stream.reserve_capacity(data.len());
        let granted = match std::future::poll_fn(|cx| send_stream.poll_capacity(cx)).await {
            Some(Ok(n)) => n,
            Some(Err(e)) => return Err(e.to_string()),
            None => return Err("stream closed".to_string()),
        };
        if granted == 0 {
            continue;
        }
        let part = data.split_to(granted.min(data.len()));
        send_stream.send_data(part, false).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn send_error_response(mut respond: h2::server::SendResponse<Bytes>, status: StatusCode) {
    let response = match http::Response::builder().status(status).body(()) {
        Ok(r) => r,
        Err(_) => return,
    };
    let _ = respond.send_response(response, true);
}
