//! Foundation + Phase 1 of spec/WASM_SHIM.md: a `wasi:http/proxy` guest
//! component that runs rws's existing `Application`/`Request`/`Response`
//! seam inside a WASM runtime (Wasmtime, Spin, Fastly Compute).
//!
//! The host owns the listening socket and TLS termination; it calls
//! [`Guest::handle`] once per request. This adapter's only job is
//! translating a `wasi:http` incoming request into a [`rust_web_server`]
//! [`Request`], calling [`App::execute`], and translating the resulting
//! [`Response`] back into a `wasi:http` outgoing response.
//!
//! Buffered bodies only (`Response::stream_file`/`stream_pipe` are not
//! wired up — that's spec/WASM_SHIM.md Phase 2 item 6). No real peer
//! address is available in this guest model, so [`ConnectionInfo`] gets a
//! placeholder client/server address and `sni_hostname: None` — the host
//! already terminated TLS before this component ever runs.
//!
//! Build: `cargo build --release --target wasm32-wasip2` (from this
//! directory). Run: `wasmtime serve target/wasm32-wasip2/release/rws_wasm_shim.wasm`.

use std::io::{Read, Write};

use wasip2::exports::http::incoming_handler::Guest;
use wasip2::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

use rust_web_server::app::App;
use rust_web_server::application::Application;
use rust_web_server::core::New;
use rust_web_server::header::Header;
use rust_web_server::request::Request as RwsRequest;
use rust_web_server::response::{Response as RwsResponse, STATUS_CODE_REASON_PHRASE};
use rust_web_server::range::Range;
use rust_web_server::mime_type::MimeType;
use rust_web_server::server::{Address, ConnectionInfo};

wasip2::http::proxy::export!(Shim);

struct Shim;

impl Guest for Shim {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let rws_request = to_rws_request(&request);
        let connection = guest_connection_info();

        let app = App::new();
        let rws_response = match app.execute(&rws_request, &connection) {
            Ok(response) => response,
            Err(message) => bad_request_response(message),
        };

        write_response(response_out, rws_response);
    }
}

/// No real peer/server address or SNI hostname is visible to a wasi-http
/// guest — the host already accepted the connection and terminated TLS.
/// `request_size` is unused off the socket read path, so any positive
/// value is fine; ConnectionInfo's shape still requires one.
fn guest_connection_info() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "0.0.0.0".to_string(), port: 0 },
        server: Address { ip: "0.0.0.0".to_string(), port: 0 },
        request_size: 0,
        sni_hostname: None,
    }
}

fn method_to_string(method: &Method) -> String {
    match method {
        Method::Get => "GET".to_string(),
        Method::Head => "HEAD".to_string(),
        Method::Post => "POST".to_string(),
        Method::Put => "PUT".to_string(),
        Method::Delete => "DELETE".to_string(),
        Method::Connect => "CONNECT".to_string(),
        Method::Options => "OPTIONS".to_string(),
        Method::Trace => "TRACE".to_string(),
        Method::Patch => "PATCH".to_string(),
        Method::Other(other) => other.to_uppercase(),
    }
}

fn to_rws_request(request: &IncomingRequest) -> RwsRequest {
    let method = method_to_string(&request.method());
    let request_uri = request.path_with_query().unwrap_or_else(|| "/".to_string());

    let headers = request
        .headers()
        .entries()
        .into_iter()
        .map(|(name, value)| Header {
            name,
            value: String::from_utf8_lossy(&value).to_string(),
        })
        .collect();

    RwsRequest {
        method,
        request_uri,
        // wasi:http abstracts the wire version away entirely — there is no
        // WIT field for it. HTTP/1.1 is the closest stand-in and matches
        // what most existing rws logic (e.g. keep-alive checks) expects.
        http_version: "HTTP/1.1".to_string(),
        headers,
        body: read_incoming_body(request),
    }
}

fn read_incoming_body(request: &IncomingRequest) -> Vec<u8> {
    let mut buf = Vec::new();
    if let Ok(incoming_body) = request.consume() {
        if let Ok(mut stream) = incoming_body.stream() {
            let _ = stream.read_to_end(&mut buf);
        }
    }
    buf
}

fn bad_request_response(message: String) -> RwsResponse {
    RwsResponse::get_response(
        STATUS_CODE_REASON_PHRASE.n400_bad_request,
        None,
        Some(vec![Range::get_content_range(
            message.into_bytes(),
            MimeType::TEXT_PLAIN.to_string(),
        )]),
    )
}

fn to_wasi_headers(headers: &[Header]) -> Fields {
    let fields = Fields::new();
    for header in headers {
        let _ = fields.append(&header.name, header.value.as_bytes());
    }
    fields
}

fn write_response(response_out: ResponseOutparam, response: RwsResponse) {
    let wasi_headers = to_wasi_headers(&response.headers);
    let outgoing_response = OutgoingResponse::new(wasi_headers);
    let _ = outgoing_response.set_status_code(response.status_code as u16);

    let body = RwsResponse::generate_body(response.content_range_list.clone());

    let outgoing_body = outgoing_response
        .body()
        .expect("outgoing-response body handle taken twice");

    ResponseOutparam::set(response_out, Ok(outgoing_response));

    let mut out = outgoing_body
        .write()
        .expect("outgoing-body write stream taken twice");
    let _ = out.write_all(&body);
    let _ = out.flush();
    drop(out);

    let _ = OutgoingBody::finish(outgoing_body, None);
}

// Only the translation logic that doesn't touch a real `wasi:http` resource
// is testable here: constructing `Fields`/`OutgoingResponse`/etc. requires an
// actual WASI host underneath and panics when run natively (they link fine —
// wit-bindgen's generated imports aren't wasm32-gated — but there's no
// runtime behind them off-target). `to_wasi_headers`/`write_response`/
// `handle` were instead verified against a real `wasmtime serve` process
// (see spec/WASM_SHIM.md's Phase 1 item 4) — build the component, run
// `wasmtime serve target/wasm32-wasip2/release/rws_wasm_shim.wasm`, and curl
// it, since that mechanism can't run inside `cargo test`.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_to_string_maps_standard_methods() {
        assert_eq!("GET", method_to_string(&Method::Get));
        assert_eq!("HEAD", method_to_string(&Method::Head));
        assert_eq!("POST", method_to_string(&Method::Post));
        assert_eq!("PUT", method_to_string(&Method::Put));
        assert_eq!("DELETE", method_to_string(&Method::Delete));
        assert_eq!("CONNECT", method_to_string(&Method::Connect));
        assert_eq!("OPTIONS", method_to_string(&Method::Options));
        assert_eq!("TRACE", method_to_string(&Method::Trace));
        assert_eq!("PATCH", method_to_string(&Method::Patch));
    }

    #[test]
    fn method_to_string_uppercases_other_methods() {
        assert_eq!("PROPFIND", method_to_string(&Method::Other("propfind".to_string())));
    }

    #[test]
    fn bad_request_response_carries_status_and_message() {
        let response = bad_request_response("could not route request".to_string());
        assert_eq!(400, response.status_code);

        let body = RwsResponse::generate_body(response.content_range_list.clone());
        assert_eq!(b"could not route request".to_vec(), body);
    }

    #[test]
    fn guest_connection_info_has_no_sni_hostname_or_real_peer() {
        // Documents the real limitation from spec/WASM_SHIM.md: a wasi-http
        // guest is never told the client's address or the TLS SNI hostname
        // (the host already terminated TLS before invoking the guest).
        let connection = guest_connection_info();
        assert_eq!(None, connection.sni_hostname);
        assert_eq!("0.0.0.0", connection.client.ip);
        assert_eq!("0.0.0.0", connection.server.ip);
    }
}
