use std::io::Read;

use bytes::Bytes;
use http_body_util::BodyExt;

use super::*;
use crate::app::App;
use crate::core::New;
use crate::header::Header;
use crate::server::Address;

fn connection() -> ConnectionInfo {
    ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None }
}

fn request(method: &str, uri: &str, headers: &[(&str, &str)], body: &[u8]) -> Request {
    Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: "HTTP/2.0".to_string(),
        headers: headers.iter().map(|(n, v)| Header::new(n.to_string(), v.to_string())).collect(),
        body: body.to_vec().into(),
    }
}

fn read_all(response: &mut Response) -> Vec<u8> {
    if let Some(mut pipe) = response.stream_pipe.take() {
        let mut out = Vec::new();
        pipe.read_to_end(&mut out).unwrap();
        return out;
    }
    response.content_range_list.iter().flat_map(|r| r.body.clone()).collect()
}

fn trailer(response: &Response, name: &str) -> Option<String> {
    response.trailers.as_ref()?.get().into_iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.to_string())
}

// ── conversions ───────────────────────────────────────────────────────────────

#[test]
fn requests_cross_both_ways() {
    let original = request("PATCH", "/a/b?x=1", &[("X-Thing", "v"), ("x-latin", "caf\u{e9}")], b"body");
    let http = to_http_request(original.clone());
    assert_eq!(http::Method::PATCH, *http.method());
    assert_eq!("/a/b?x=1", http.uri().to_string());
    assert_eq!(http::Version::HTTP_2, http.version());
    assert_eq!("v", http.headers()["x-thing"]);
    let back = runtime().block_on(from_http_request(http)).unwrap();
    assert_eq!(("PATCH", "/a/b?x=1", "HTTP/2.0", b"body".as_slice()), (back.method.as_str(), back.request_uri.as_str(), back.http_version.as_str(), &*back.body));
    assert!(back.headers.iter().any(|h| h.name == "x-latin" && h.value == "caf\u{e9}"), "obs-text kept: {:?}", back.headers);
}

#[test]
fn a_buffered_rws_response_becomes_one_frame_with_its_content_type_and_trailers() {
    let mut response = Response::new();
    response.status_code = 201;
    response.headers.push(Header::new("x-a".to_string(), "1".to_string()));
    response.content_range_list = vec![Range::get_content_range(b"hello".to_vec(), "text/plain".to_string())];
    response.trailers = Some(Trailers::with(vec![Header::new("grpc-status".to_string(), "0".to_string())]));
    let http = to_http_response(response);
    assert_eq!(201, http.status().as_u16());
    assert_eq!("text/plain", http.headers()["content-type"]);
    let collected = runtime().block_on(http.into_body().collect()).unwrap();
    assert_eq!("0", collected.trailers().unwrap()["grpc-status"]);
    assert_eq!(Bytes::from_static(b"hello"), collected.to_bytes());
}

#[test]
fn a_piped_rws_response_streams() {
    let mut response = Response::new();
    response.stream_pipe = Some(Box::new(std::io::Cursor::new(vec![7u8; 200_000])));
    let http = to_http_response(response);
    assert!(http_body::Body::size_hint(http.body()).exact().is_none(), "a stream, not a buffer");
    let bytes = runtime().block_on(http.into_body().collect()).unwrap().to_bytes();
    assert_eq!(200_000, bytes.len());
}

#[test]
fn http_responses_are_buffered_streamed_or_headers_only() {
    let small = http::Response::builder().header("content-type", "text/plain").header("content-length", "2").header("connection", "close").body(Body::full("hi")).unwrap();
    let mut small = runtime().block_on(from_http_response(small)).unwrap();
    assert!(small.stream_pipe.is_none());
    assert_eq!("text/plain", small.content_range_list[0].content_type);
    assert!(!small.headers.iter().any(|h| h.name.eq_ignore_ascii_case("content-length") || h.name.eq_ignore_ascii_case("connection")), "{:?}", small.headers);
    assert_eq!(b"hi".to_vec(), read_all(&mut small));

    let trailers = Trailers::with(vec![Header::new("grpc-status".to_string(), "0".to_string())]);
    let (tx, rx) = tokio::sync::mpsc::channel(2);
    tx.try_send(Ok(Bytes::from_static(b"\0\0\0\0\x02ab"))).unwrap();
    drop(tx);
    let grpc = http::Response::builder().header("content-type", "application/grpc").body(Body { inner: Inner::Channel { rx, trailers: Some(trailers) } }).unwrap();
    let mut grpc = runtime().block_on(from_http_response(grpc)).unwrap();
    assert!(grpc.stream_pipe.is_some(), "gRPC streams");
    assert_eq!(b"\0\0\0\0\x02ab".to_vec(), read_all(&mut grpc));
    assert_eq!(Some("0".to_string()), trailer(&grpc, "grpc-status"), "trailers set before the body ended");

    let only = http::Response::builder().header("content-type", "application/grpc").header("grpc-status", "12").body(Body::empty()).unwrap();
    let only = runtime().block_on(from_http_response(only)).unwrap();
    assert!(only.stream_pipe.is_none() && only.content_range_list.is_empty() && only.trailers.is_none());
    assert!(only.headers.iter().any(|h| h.name == "grpc-status" && h.value == "12"), "trailers-only: {:?}", only.headers);
}

// ── TowerApp with a real tonic service ────────────────────────────────────────

fn health() -> impl Application {
    let (_, service) = tonic_health::server::health_reporter();
    mount("/grpc.health.v1.Health/", service, App::new())
}

fn grpc(path: &str, message: &[u8]) -> Request {
    let mut frame = vec![0u8];
    frame.extend_from_slice(&(message.len() as u32).to_be_bytes());
    frame.extend_from_slice(message);
    request("POST", path, &[("content-type", "application/grpc"), ("te", "trailers")], &frame)
}

#[test]
fn a_tonic_service_answers_through_tower_app() {
    let app = health();
    let mut response = app.execute(&grpc("/grpc.health.v1.Health/Check", b""), &connection()).unwrap();
    assert_eq!(200, response.status_code);
    let body = read_all(&mut response);
    // HealthCheckResponse { status: SERVING (1) } = field 1 varint 1.
    assert_eq!(b"\0\0\0\0\x02\x08\x01".to_vec(), body);
    assert_eq!(Some("0".to_string()), trailer(&response, "grpc-status"));
}

#[test]
fn an_unknown_grpc_method_is_unimplemented_trailers_only() {
    let mut response = health().execute(&grpc("/grpc.health.v1.Health/Nope", b""), &connection()).unwrap();
    let status = response.headers.iter().find(|h| h.name.eq_ignore_ascii_case("grpc-status")).map(|h| h.value.to_string()).or_else(|| trailer(&response, "grpc-status"));
    assert_eq!(Some("12".to_string()), status, "{:?}", response.headers);
    assert!(read_all(&mut response).is_empty());
}

#[test]
fn other_paths_fall_through_and_async_execution_works() {
    let app = health();
    assert_eq!(200, app.execute(&request("GET", "/healthz", &[], b""), &connection()).unwrap().status_code);
    assert!(!app.runs_inline(&grpc("/grpc.health.v1.Health/Check", b""), &connection()));
    assert!(app.streams_body(&grpc("/grpc.health.v1.Health/Check", b""), &connection()));
    let check = grpc("/grpc.health.v1.Health/Check", b"");
    let conn = connection();
    let response = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        app.execute_async(&check, &conn).expect("routed to tower").await
    });
    let mut response = response.unwrap();
    assert_eq!(b"\0\0\0\0\x02\x08\x01".to_vec(), read_all(&mut response));
    assert_eq!(Some("0".to_string()), trailer(&response, "grpc-status"));
}

// ── TowerLayer with tower-http ────────────────────────────────────────────────

#[test]
fn tower_http_layers_wrap_an_rws_app() {
    use tower_http::set_header::SetResponseHeaderLayer;
    use tower_http::validate_request::ValidateRequestHeaderLayer;
    let app = App::with_state(())
        .get("/hello", |_, _, _, _| crate::response::Response::json(&crate::response::STATUS_CODE_REASON_PHRASE.n200_ok, b"{\"hi\":1}".to_vec()))
        .wrap(TowerLayer::new(SetResponseHeaderLayer::overriding(http::header::HeaderName::from_static("x-layer"), http::HeaderValue::from_static("tower-http"))))
        .wrap(TowerLayer::new(ValidateRequestHeaderLayer::has_header_value("authorization", "Bearer secret").unwrap()));
    let client = crate::test_client::TestClient::new(app);
    let denied = client.get("/hello").send();
    assert_eq!(403, denied.status(), "tower-http's answer, not the app's");
    let ok = client.get("/hello").header("Authorization", "Bearer secret").send();
    assert_eq!(200, ok.status());
    assert_eq!(Some("tower-http"), ok.header("x-layer"));
    assert_eq!(b"{\"hi\":1}", ok.body_bytes());
    assert_eq!(Some("application/json"), ok.header("content-type").map(|v| v.split(';').next().unwrap_or("")));
}

#[test]
fn a_layer_sees_what_the_app_changes_and_streams_stay_streams() {
    use tower_http::set_header::SetRequestHeaderLayer;
    let app = App::with_state(())
        .get("/echo", |request, _, _, _| {
            let mut r = crate::response::Response::new();
            let seen = request.header("x-added").map(|h| h.value.to_string()).unwrap_or_default();
            r.stream_pipe = Some(Box::new(std::io::Cursor::new(seen.into_bytes())));
            r
        })
        .wrap(TowerLayer::new(SetRequestHeaderLayer::overriding(http::header::HeaderName::from_static("x-added"), http::HeaderValue::from_static("by-layer"))));
    let mut response = crate::middleware::Middleware::handle(
        &TowerLayer::new(tower_layer::Identity::new()),
        &request("GET", "/echo", &[], b""),
        &connection(),
        &app,
    )
    .unwrap();
    assert!(response.stream_pipe.is_some(), "still a stream after the layers");
    assert_eq!(b"by-layer".to_vec(), read_all(&mut response));
}

// ── TowerService ──────────────────────────────────────────────────────────────

#[test]
fn an_rws_app_serves_as_a_tower_service() {
    use tower_service::Service;
    let app = App::with_state(())
        .post("/sum", |request, _, _, _| {
            let text = String::from_utf8_lossy(&request.body).to_string();
            let sum: i64 = text.split(',').filter_map(|n| n.trim().parse::<i64>().ok()).sum();
            crate::response::Response::json(&crate::response::STATUS_CODE_REASON_PHRASE.n200_ok, sum.to_string().into_bytes())
        });
    let mut service = TowerService::new(app);
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let (status, content_type, body) = runtime.block_on(async {
        let request = http::Request::post("/sum").body(http_body_util::Full::new(Bytes::from_static(b"1, 2, 39"))).unwrap();
        let response = service.call(request).await.unwrap();
        let status = response.status().as_u16();
        let content_type = response.headers()["content-type"].to_str().unwrap().to_string();
        (status, content_type, response.into_body().collect().await.unwrap().to_bytes())
    });
    assert_eq!((200, "application/json"), (status, content_type.split(';').next().unwrap()));
    assert_eq!(Bytes::from_static(b"42"), body);
    let missing = runtime.block_on(service.call(http::Request::get("/nope").body(http_body_util::Empty::<Bytes>::new()).unwrap())).unwrap();
    assert_eq!(404, missing.status().as_u16());
}
