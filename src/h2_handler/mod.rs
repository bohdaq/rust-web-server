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

pub async fn handle_connection(
    stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    peer_addr: std::net::SocketAddr,
    app: impl Application + Send + Copy + 'static,
) -> Result<(), String> {
    let mut conn = h2::server::handshake(stream)
        .await
        .map_err(|e| format!("H2 handshake failed: {}", e))?;

    let (server_ip, server_port, _) = get_ip_port_thread_count();

    while let Some(result) = conn.accept().await {
        match result {
            Ok((request, respond)) => {
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
                tokio::spawn(handle_stream(request, respond, connection, peer_addr, app));
            }
            Err(e) => {
                eprintln!("H2 stream accept error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn handle_stream(
    request: http::Request<h2::RecvStream>,
    respond: h2::server::SendResponse<Bytes>,
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

    let mut body_stream = request.into_body();
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = body_stream.data().await {
        match chunk {
            Ok(data) => {
                let len = data.len();
                body.extend_from_slice(&data);
                let _ = body_stream.flow_control().release_capacity(len);
            }
            Err(e) => {
                eprintln!("H2 body read error: {}", e);
                break;
            }
        }
    }

    let rws_request = Request {
        method,
        request_uri: uri,
        http_version: VERSION.http_2_0.to_string(),
        headers,
        body,
    };

    let rws_response = match app.execute(&rws_request, &connection) {
        Ok(r) => r,
        Err(message) => {
            eprintln!("App error on H2 stream: {}", message);
            send_error_response(respond, StatusCode::INTERNAL_SERVER_ERROR);
            return;
        }
    };

    let mut rws_response = rws_response;
    rws_response.headers.push(crate::header::Header::get_hsts_header());

    let log = Log::combined(&rws_request, &rws_response, &peer_addr);
    println!("{}", log);

    send_h2_response(respond, rws_response);
}

fn send_h2_response(mut respond: h2::server::SendResponse<Bytes>, mut rws_response: Response) {
    // Promote Content-Type, Content-Range, and Content-Length from content_range_list
    // into response headers, mirroring what Response::generate_response() does for HTTP/1.1.
    if rws_response.content_range_list.len() == 1 {
        let cr = &rws_response.content_range_list[0];
        rws_response.headers.push(crate::header::Header {
            name: crate::header::Header::_CONTENT_TYPE.to_string(),
            value: cr.content_type.clone(),
        });
        rws_response.headers.push(crate::header::Header {
            name: crate::header::Header::_CONTENT_LENGTH.to_string(),
            value: cr.body.len().to_string(),
        });
    } else if rws_response.content_range_list.len() > 1 {
        rws_response.headers.push(crate::header::Header {
            name: crate::header::Header::_CONTENT_TYPE.to_string(),
            value: crate::range::Range::MULTIPART_BYTERANGES_CONTENT_TYPE.to_string(),
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
    let end_stream = body.is_empty();

    let h2_response = match builder.body(()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to build H2 response: {}", e);
            return;
        }
    };

    match respond.send_response(h2_response, end_stream) {
        Ok(mut send_stream) => {
            if !body.is_empty() {
                if let Err(e) = send_stream.send_data(Bytes::from(body), true) {
                    eprintln!("H2 send data error: {}", e);
                }
            }
        }
        Err(e) => eprintln!("H2 send response error: {}", e),
    }
}

fn send_error_response(mut respond: h2::server::SendResponse<Bytes>, status: StatusCode) {
    let response = match http::Response::builder().status(status).body(()) {
        Ok(r) => r,
        Err(_) => return,
    };
    let _ = respond.send_response(response, true);
}
