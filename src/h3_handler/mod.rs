use bytes::{Buf, Bytes};
use http::StatusCode;

use crate::application::Application;
use crate::entry_point::get_ip_port_thread_count;
use crate::header::Header;
use crate::http::VERSION;
use crate::log::Log;
use crate::request::Request;
use crate::response::Response;
use crate::server::{Address, ConnectionInfo};

// Headers forbidden in HTTP/3 responses per RFC 9114 §4.2
const FORBIDDEN_H3_RESPONSE_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "transfer-encoding",
    "upgrade",
    "proxy-connection",
    "te",
];

pub async fn handle_connection(
    conn: quinn::Connection,
    peer_addr: std::net::SocketAddr,
    app: impl Application + Send + Clone + 'static,
) -> Result<(), String> {
    let (server_ip, server_port, _) = get_ip_port_thread_count();

    // Extract the SNI hostname from the QUIC/TLS handshake data.
    let sni_hostname: Option<String> = conn
        .handshake_data()
        .and_then(|d| d.downcast::<quinn::crypto::rustls::HandshakeData>().ok())
        .and_then(|d| d.server_name.clone());

    let mut h3_conn = h3::server::builder()
        .max_field_section_size(crate::h2_server::MAX_HEADER_LIST_SIZE as u64)
        .build(h3_quinn::Connection::new(conn))
        .await
        .map_err(|e| format!("H3 connection error: {}", e))?;

    loop {
        match h3_conn.accept().await {
            Ok(Some(resolver)) => {
                let connection = ConnectionInfo {
                    client: Address {
                        ip: peer_addr.ip().to_string(),
                        port: peer_addr.port() as i32,
                    },
                    server: Address {
                        ip: server_ip.clone(),
                        port: server_port,
                    },
                    request_size: 0,
                    sni_hostname: sni_hostname.clone(),
                };
                let app = app.clone();
                tokio::spawn(async move {
                    match resolver.resolve_request().await {
                        Ok((request, stream)) => {
                            handle_stream(request, stream, connection, peer_addr, app).await;
                        }
                        Err(e) => eprintln!("H3 resolve request error: {}", e),
                    }
                });
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("H3 stream accept error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn handle_stream(
    request: http::Request<()>,
    mut stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    connection: ConnectionInfo,
    peer_addr: std::net::SocketAddr,
    app: impl Application + Send + 'static,
) {
    let method = request.method().to_string();
    let uri = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    let mut headers: Vec<Header> = Vec::new();
    for (name, value) in request.headers() {
        if let Ok(v) = value.to_str() {
            headers.push(Header {
                name: name.as_str().to_string().into(),
                value: v.to_string().into(),
            });
        }
    }

    let max_body_size = crate::entry_point::get_max_body_size();
    let mut body: Vec<u8> = Vec::new();
    loop {
        match stream.recv_data().await {
            Ok(Some(mut chunk)) => {
                let bytes = chunk.copy_to_bytes(chunk.remaining());
                body.extend_from_slice(&bytes);
                if max_body_size > 0 && body.len() as u64 > max_body_size {
                    send_error_response(stream, StatusCode::PAYLOAD_TOO_LARGE).await;
                    return;
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("H3 body read error: {}", e);
                break;
            }
        }
    }

    let rws_request = Request {
        method,
        request_uri: uri,
        http_version: VERSION.http_3_0.to_string(),
        headers,
        body: body.into(),
    };

    let started = std::time::Instant::now();
    let (rws_request, result) =
        match crate::server::http1_async::execute_request(app, rws_request, &connection).await {
            Ok(done) => done,
            Err(e) => {
                eprintln!("Handler panicked on H3 stream: {}", e);
                send_error_response(stream, StatusCode::INTERNAL_SERVER_ERROR).await;
                return;
            }
        };
    let rws_response = match result {
        Ok(r) => r,
        Err(message) => {
            eprintln!("App error on H3 stream: {}", message);
            send_error_response(stream, StatusCode::INTERNAL_SERVER_ERROR).await;
            return;
        }
    };

    let mut rws_response = rws_response;
    crate::metrics::record_request();
    crate::metrics::http::observe(&rws_request.method, rws_response.status_code, true, "HTTP/3", started);
    crate::compression::apply_compression(&rws_request, &mut rws_response);
    rws_response.drop_content_for_status();
    rws_response.headers.push(Header::get_hsts_header());

    Log::log_access(&rws_request, &rws_response, &peer_addr);

    let is_head = rws_request.method == crate::request::METHOD.head;
    send_h3_response(stream, rws_response, is_head).await;
}

async fn send_h3_response(
    mut stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    mut rws_response: Response,
    is_head: bool,
) {
    // stream_file / stream_pipe bodies: produced in chunks, sent below.
    let streamed_body = if is_head {
        None
    } else {
        crate::server::body_stream::spawn_producer(&mut rws_response)
    };

    // Promote Content-Type, Content-Length (and Content-Range on 206) from content_range_list into headers,
    // mirroring what Response::generate_response() does for HTTP/1.1.
    if rws_response.content_range_list.len() == 1 {
        let cr = &rws_response.content_range_list[0];
        rws_response.headers.push(Header {
            name: Header::_CONTENT_TYPE.into(),
            value: cr.content_type.clone().into(),
        });
        let content_range = crate::response::content_range_header_value(rws_response.status_code, cr);
        rws_response.headers.push(Header {
            name: Header::_CONTENT_LENGTH.into(),
            value: cr.body.len().to_string().into(),
        });
        if let Some(value) = content_range {
            rws_response.headers.push(Header {
                name: Header::_CONTENT_RANGE.into(),
                value: value.into(),
            });
        }
    } else if rws_response.content_range_list.len() > 1 {
        rws_response.headers.push(Header {
            name: Header::_CONTENT_TYPE.into(),
            value: crate::range::Range::multipart_content_type(&rws_response.content_range_list).into(),
        });
    }

    let status = StatusCode::from_u16(rws_response.status_code as u16)
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut builder = http::Response::builder().status(status);

    for header in &rws_response.headers {
        let name_lower = header.name.to_lowercase();
        if FORBIDDEN_H3_RESPONSE_HEADERS.contains(&name_lower.as_str()) {
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

    let h3_response = match builder.body(()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to build H3 response: {}", e);
            return;
        }
    };

    if let Err(e) = stream.send_response(h3_response).await {
        eprintln!("H3 send response error: {}", e);
        return;
    }

    if let Some(mut chunks) = streamed_body {
        // send_data awaits QUIC flow control, so memory stays bounded.
        while let Some(chunk) = chunks.recv().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("H3 streaming body error: {}", e);
                    return;
                }
            };
            if let Err(e) = stream.send_data(Bytes::from(chunk)).await {
                eprintln!("H3 send data error: {}", e);
                return;
            }
        }
    } else {
        let body = Response::generate_body(rws_response.content_range_list);
        if !body.is_empty() && !is_head {
            if let Err(e) = stream.send_data(Bytes::from(body)).await {
                eprintln!("H3 send data error: {}", e);
                return;
            }
        }
    }

    // Trailers (gRPC status): complete once the body has been produced.
    let trailers = if is_head { Vec::new() } else { rws_response.trailers.as_ref().map(|t| t.get()).unwrap_or_default() };
    if !trailers.is_empty() {
        let mut fields = http::HeaderMap::new();
        for header in &trailers {
            let lower = header.name.to_lowercase();
            if FORBIDDEN_H3_RESPONSE_HEADERS.contains(&lower.as_str()) {
                continue;
            }
            if let (Ok(name), Ok(value)) = (
                http::header::HeaderName::from_bytes(header.name.as_bytes()),
                http::header::HeaderValue::from_str(&header.value),
            ) {
                fields.append(name, value);
            }
        }
        if let Err(e) = stream.send_trailers(fields).await {
            eprintln!("H3 send trailers error: {}", e);
            return;
        }
    }

    if let Err(e) = stream.finish().await {
        eprintln!("H3 finish stream error: {}", e);
    }
}

async fn send_error_response(
    mut stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    status: StatusCode,
) {
    let response = match http::Response::builder()
        .status(status)
        .body(())
    {
        Ok(r) => r,
        Err(_) => return,
    };
    let _ = stream.send_response(response).await;
    let _ = stream.finish().await;
}

#[cfg(test)]
mod tests;
