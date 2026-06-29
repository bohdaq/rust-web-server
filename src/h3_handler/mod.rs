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
    app: impl Application + Send + Copy + 'static,
) -> Result<(), String> {
    let (server_ip, server_port, _) = get_ip_port_thread_count();

    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn))
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
                };
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
    app: impl Application,
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
                name: name.as_str().to_string(),
                value: v.to_string(),
            });
        }
    }

    let mut body: Vec<u8> = Vec::new();
    loop {
        match stream.recv_data().await {
            Ok(Some(mut chunk)) => {
                let bytes = chunk.copy_to_bytes(chunk.remaining());
                body.extend_from_slice(&bytes);
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
        body,
    };

    let rws_response = match app.execute(&rws_request, &connection) {
        Ok(r) => r,
        Err(message) => {
            eprintln!("App error on H3 stream: {}", message);
            send_error_response(stream).await;
            return;
        }
    };

    let mut rws_response = rws_response;
    crate::metrics::record_request();
    crate::compression::apply_gzip(&rws_request, &mut rws_response);
    rws_response.headers.push(Header::get_hsts_header());

    Log::log_access(&rws_request, &rws_response, &peer_addr);

    send_h3_response(stream, rws_response).await;
}

async fn send_h3_response(
    mut stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    mut rws_response: Response,
) {
    // Promote Content-Type and Content-Length from content_range_list into headers,
    // mirroring what Response::generate_response() does for HTTP/1.1.
    if rws_response.content_range_list.len() == 1 {
        let cr = &rws_response.content_range_list[0];
        rws_response.headers.push(Header {
            name: Header::_CONTENT_TYPE.to_string(),
            value: cr.content_type.clone(),
        });
        rws_response.headers.push(Header {
            name: Header::_CONTENT_LENGTH.to_string(),
            value: cr.body.len().to_string(),
        });
    } else if rws_response.content_range_list.len() > 1 {
        rws_response.headers.push(Header {
            name: Header::_CONTENT_TYPE.to_string(),
            value: crate::range::Range::MULTIPART_BYTERANGES_CONTENT_TYPE.to_string(),
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

    let body = Response::generate_body(rws_response.content_range_list);
    if !body.is_empty() {
        if let Err(e) = stream.send_data(Bytes::from(body)).await {
            eprintln!("H3 send data error: {}", e);
            return;
        }
    }

    if let Err(e) = stream.finish().await {
        eprintln!("H3 finish stream error: {}", e);
    }
}

async fn send_error_response(
    mut stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
) {
    let response = match http::Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(())
    {
        Ok(r) => r,
        Err(_) => return,
    };
    let _ = stream.send_response(response).await;
    let _ = stream.finish().await;
}
