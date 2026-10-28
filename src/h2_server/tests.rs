//! The server against the `h2` crate's client (and raw frames) over an
//! in-memory stream. Protocol conformance is `ci/e2e/h2spec.sh`.

use std::net::SocketAddr;

use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::application::Application;
use crate::header::Header;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

/// `GET /big` → 300 000 bytes, `GET /file` → a streamed file, `POST` →
/// echo of the body, `/panic` panics, anything else → "hello <path>".
#[derive(Clone)]
struct TestApp(Option<String>);

const BIG: usize = 300_000;

fn big_body() -> Vec<u8> {
    (0..BIG).map(|i| (i % 251) as u8).collect()
}

impl Application for TestApp {
    fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        if request.request_uri == "/panic" {
            panic!("test handler bug");
        }
        let mut response = if request.method == "POST" {
            let mut r = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "");
            r.content_range_list[0].body = request.body.to_vec();
            r
        } else if request.request_uri == "/trailers" {
            let mut r = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "body");
            r.trailers = Some(crate::response::Trailers::with(vec![Header::new("grpc-status", "0".to_string())]));
            r
        } else if request.request_uri == "/only-trailers" {
            let mut r = Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, None);
            r.trailers = Some(crate::response::Trailers::with(vec![Header::new("grpc-status", "5".to_string())]));
            r
        } else if request.request_uri == "/big" {
            let mut r = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "");
            r.content_range_list[0].body = big_body();
            r
        } else if request.request_uri == "/file" {
            let mut r = Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "");
            r.content_range_list.clear();
            r.stream_file = self.0.clone();
            r
        } else {
            Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &format!("hello {}", request.request_uri))
        };
        response.headers.push(Header::new("X-Seen-Agent", request.header("user-agent").map(|h| h.value.to_string()).unwrap_or_default()));
        response.headers.push(Header::new("X-Seen-Host", request.header("host").map(|h| h.value.to_string()).unwrap_or_default()));
        Ok(response)
    }

    fn runs_inline(&self, request: &Request, _connection: &ConnectionInfo) -> bool {
        request.method == "GET"
    }
}

fn peer() -> SocketAddr {
    "127.0.0.1:5555".parse().unwrap()
}

async fn client(app: TestApp) -> h2::client::SendRequest<Bytes> {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    tokio::spawn(super::handle_connection(server_io, peer(), None, app));
    let (send, connection) = h2::client::handshake(client_io).await.unwrap();
    tokio::spawn(connection);
    send
}

async fn get(send: &mut h2::client::SendRequest<Bytes>, path: &str) -> (http::response::Parts, Vec<u8>) {
    let request = http::Request::get(format!("https://x{}", path)).header("user-agent", "t/1").body(()).unwrap();
    let (response, _) = send.clone().ready().await.unwrap().send_request(request, true).unwrap();
    read(response).await
}

async fn read(response: h2::client::ResponseFuture) -> (http::response::Parts, Vec<u8>) {
    let (parts, mut body) = response.await.unwrap().into_parts();
    let mut out = Vec::new();
    while let Some(chunk) = body.data().await {
        let chunk = chunk.unwrap();
        let _ = body.flow_control().release_capacity(chunk.len());
        out.extend_from_slice(&chunk);
    }
    (parts, out)
}

#[tokio::test]
async fn many_requests_on_one_connection_keep_their_headers() {
    let mut send = client(TestApp(None)).await;
    for i in 0..20 {
        let (parts, body) = get(&mut send, &format!("/n{}", i)).await;
        assert_eq!(200, parts.status.as_u16());
        assert_eq!(format!("hello /n{}", i).as_bytes(), &body[..]);
        assert_eq!("t/1", parts.headers["x-seen-agent"]);
        assert_eq!(body.len().to_string(), parts.headers["content-length"]);
        assert!(parts.headers.contains_key("strict-transport-security"));
    }
}

#[tokio::test]
async fn concurrent_streams_and_a_large_body_respect_flow_control() {
    let mut send = client(TestApp(None)).await;
    let mut pending = Vec::new();
    for _ in 0..8 {
        let request = http::Request::get("https://x/big").body(()).unwrap();
        let (response, _) = send.clone().ready().await.unwrap().send_request(request, true).unwrap();
        pending.push(tokio::spawn(read(response)));
    }
    let expected = big_body();
    for p in pending {
        let (parts, body) = p.await.unwrap();
        assert_eq!(200, parts.status.as_u16());
        assert!(body == expected, "body differs ({} bytes)", body.len());
    }
    let _ = &mut send;
}

#[tokio::test]
async fn request_bodies_arrive_whole() {
    let send = client(TestApp(None)).await;
    let payload: Vec<u8> = (0..200_000u32).map(|i| (i % 249) as u8).collect();
    let request = http::Request::post("https://x/echo").header("content-length", payload.len()).body(()).unwrap();
    let (response, mut body) = send.ready().await.unwrap().send_request(request, false).unwrap();
    let mut rest = Bytes::from(payload.clone());
    while !rest.is_empty() {
        body.reserve_capacity(rest.len());
        let capacity = std::future::poll_fn(|cx| body.poll_capacity(cx)).await.unwrap().unwrap();
        let chunk = rest.split_to(capacity.min(rest.len()));
        body.send_data(chunk, rest.is_empty()).unwrap();
    }
    let (parts, echoed) = read(response).await;
    assert_eq!(200, parts.status.as_u16());
    assert!(echoed == payload);
}

#[tokio::test]
async fn streamed_files_are_sent_complete() {
    let file = crate::test_env::TempPath::new("h2_server_stream.bin");
    let content: Vec<u8> = (0..700_000u32).map(|i| (i % 241) as u8).collect();
    std::fs::write(&*file, &content).unwrap();
    let mut send = client(TestApp(Some(file.as_str().to_string()))).await;
    let (parts, body) = get(&mut send, "/file").await;
    assert_eq!(200, parts.status.as_u16());
    assert!(body == content, "{} of {} bytes", body.len(), content.len());
    // HEAD: headers only.
    let request = http::Request::head("https://x/file").body(()).unwrap();
    let (response, _) = send.clone().ready().await.unwrap().send_request(request, true).unwrap();
    let (parts, body) = read(response).await;
    assert_eq!(200, parts.status.as_u16());
    assert!(body.is_empty());
}

#[tokio::test]
async fn oversized_header_lists_get_431_and_the_connection_lives() {
    let mut send = client(TestApp(None)).await;
    let request = http::Request::get("https://x/").header("x-big", "a".repeat(70_000)).body(()).unwrap();
    let (response, _) = send.clone().ready().await.unwrap().send_request(request, true).unwrap();
    let (parts, _) = read(response).await;
    assert_eq!(431, parts.status.as_u16());
    let (parts, body) = get(&mut send, "/after").await;
    assert_eq!(200, parts.status.as_u16());
    assert_eq!(b"hello /after", &body[..]);
}

#[tokio::test]
async fn a_panicking_handler_costs_one_stream() {
    let mut send = client(TestApp(None)).await;
    let (parts, _) = get(&mut send, "/panic").await;
    assert_eq!(500, parts.status.as_u16());
    let (parts, _) = get(&mut send, "/ok").await;
    assert_eq!(200, parts.status.as_u16());
}

/// Raw frames: write `frames` after the preface and a SETTINGS frame, then
/// read until the server closes and return its GOAWAY error code, if any.
async fn goaway_code(frames: &[u8]) -> Option<u32> {
    let (mut client, server_io) = tokio::io::duplex(1 << 20);
    tokio::spawn(super::handle_connection(server_io, peer(), None, TestApp(None)));
    client.write_all(super::PREFACE).await.unwrap();
    client.write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0]).await.unwrap();
    client.write_all(frames).await.unwrap();
    let mut out = Vec::new();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), client.read_to_end(&mut out)).await;
    let mut pos = 0;
    while out.len() - pos >= 9 {
        let len = (out[pos] as usize) << 16 | (out[pos + 1] as usize) << 8 | out[pos + 2] as usize;
        if out[pos + 3] == super::frame::GOAWAY {
            let p = &out[pos + 9..pos + 9 + len];
            return Some(u32::from_be_bytes([p[4], p[5], p[6], p[7]]));
        }
        pos += 9 + len;
    }
    None
}

fn raw_frame(kind: u8, flags: u8, stream: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    super::write_frame_header(&mut out, payload.len(), kind, flags, stream);
    out.extend_from_slice(payload);
    out
}

#[tokio::test]
async fn protocol_violations_end_the_connection_with_the_right_code() {
    // DATA on stream 0.
    assert_eq!(Some(super::code::PROTOCOL_ERROR), goaway_code(&raw_frame(super::frame::DATA, 0, 0, b"x")).await);
    // A frame above our max frame size.
    let big = vec![0u8; super::MAX_FRAME_SIZE + 1];
    assert_eq!(Some(super::code::FRAME_SIZE_ERROR), goaway_code(&raw_frame(super::frame::DATA, 0, 1, &big)).await);
    // Garbage HPACK (index 0).
    let headers = raw_frame(super::frame::HEADERS, super::frame::END_HEADERS | super::frame::END_STREAM, 1, &[0x80]);
    assert_eq!(Some(super::code::COMPRESSION_ERROR), goaway_code(&headers).await);
    // A CONTINUATION flood: header block bytes past the limit without END_HEADERS.
    let mut flood = raw_frame(super::frame::HEADERS, 0, 1, &[0x82]);
    for _ in 0..=super::MAX_HEADER_BLOCK / 16_000 {
        flood.extend(raw_frame(super::frame::CONTINUATION, 0, 1, &vec![0x82; 16_000]));
    }
    assert_eq!(Some(super::code::ENHANCE_YOUR_CALM), goaway_code(&flood).await);
}

#[tokio::test]
async fn rapid_reset_is_cut_off() {
    // GET / on a new stream, then RST_STREAM(CANCEL) it, over and over.
    let mut frames = Vec::new();
    for i in 0..=super::MAX_RESETS_PER_WINDOW {
        let stream = 1 + 2 * i;
        // :method GET, :scheme https, :path / (static 2, 7, 4); no END_STREAM.
        frames.extend(raw_frame(super::frame::HEADERS, super::frame::END_HEADERS, stream, &[0x82, 0x87, 0x84]));
        frames.extend(raw_frame(super::frame::RST_STREAM, 0, stream, &8u32.to_be_bytes()));
    }
    assert_eq!(Some(super::code::ENHANCE_YOUR_CALM), goaway_code(&frames).await);
}

#[tokio::test]
async fn made_you_reset_is_cut_off() {
    // CVE-2025-8671: open a stream, then make the server reset it — here
    // with a WINDOW_UPDATE of 0 — over and over. The server's resets count
    // like the client's own.
    let mut frames = Vec::new();
    for i in 0..=super::MAX_RESETS_PER_WINDOW {
        let stream = 1 + 2 * i;
        frames.extend(raw_frame(super::frame::HEADERS, super::frame::END_HEADERS, stream, &[0x82, 0x87, 0x84]));
        frames.extend(raw_frame(super::frame::WINDOW_UPDATE, 0, stream, &0u32.to_be_bytes()));
    }
    assert_eq!(Some(super::code::ENHANCE_YOUR_CALM), goaway_code(&frames).await);
}

#[tokio::test]
async fn a_few_server_resets_are_tolerated() {
    // Below the budget the connection stays up (no GOAWAY).
    let mut frames = Vec::new();
    for i in 0..10u32 {
        let stream = 1 + 2 * i;
        frames.extend(raw_frame(super::frame::HEADERS, super::frame::END_HEADERS, stream, &[0x82, 0x87, 0x84]));
        frames.extend(raw_frame(super::frame::WINDOW_UPDATE, 0, stream, &0u32.to_be_bytes()));
    }
    assert_eq!(None, goaway_code(&frames).await);
}

/// Frames written after the preface and an empty SETTINGS; then nothing.
/// Returns what the server wrote before closing, and whether it closed.
async fn silent_after(frames: &[u8]) -> (Vec<u8>, bool) {
    let (mut client, server_io) = tokio::io::duplex(1 << 20);
    let server = tokio::spawn(super::handle_connection(server_io, peer(), None, TestApp(None)));
    client.write_all(super::PREFACE).await.unwrap();
    client.write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0]).await.unwrap();
    client.write_all(frames).await.unwrap();
    let mut out = Vec::new();
    let closed = tokio::time::timeout(
        crate::server::http1_async::READ_TIMEOUT + std::time::Duration::from_secs(5),
        client.read_to_end(&mut out),
    ).await.is_ok();
    if closed {
        server.await.unwrap().unwrap();
    }
    (out, closed)
}

fn goaway_in(out: &[u8]) -> Option<u32> {
    let mut pos = 0;
    while out.len() - pos >= 9 {
        let len = (out[pos] as usize) << 16 | (out[pos + 1] as usize) << 8 | out[pos + 2] as usize;
        if out[pos + 3] == super::frame::GOAWAY {
            let p = &out[pos + 9..pos + 9 + len];
            return Some(u32::from_be_bytes([p[4], p[5], p[6], p[7]]));
        }
        pos += 9 + len;
    }
    None
}

#[tokio::test(start_paused = true)]
async fn an_idle_connection_is_closed() {
    let (out, closed) = silent_after(&[]).await;
    assert!(closed, "the connection stayed open");
    assert_eq!(Some(super::code::NO_ERROR), goaway_in(&out));
}

#[tokio::test(start_paused = true)]
async fn a_half_sent_request_body_is_closed() {
    // POST with no END_STREAM and no DATA: the server waits for the client
    // only as long as an idle connection.
    let headers = [0x83, 0x87, 0x84]; // :method POST, :scheme https, :path /
    let (out, closed) = silent_after(&raw_frame(super::frame::HEADERS, super::frame::END_HEADERS, 1, &headers)).await;
    assert!(closed, "the connection stayed open");
    assert_eq!(Some(super::code::NO_ERROR), goaway_in(&out));
}

#[tokio::test(start_paused = true)]
async fn a_half_sent_header_block_is_closed() {
    // HEADERS without END_HEADERS, and no CONTINUATION.
    let (out, closed) = silent_after(&raw_frame(super::frame::HEADERS, 0, 1, &[0x82])).await;
    assert!(closed, "the connection stayed open");
    assert_eq!(Some(super::code::NO_ERROR), goaway_in(&out));
}

#[tokio::test]
async fn a_wrong_preface_is_refused() {
    let (mut client, server_io) = tokio::io::duplex(4096);
    let server = tokio::spawn(super::handle_connection(server_io, peer(), None, TestApp(None)));
    client.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
    let mut out = Vec::new();
    client.read_to_end(&mut out).await.unwrap();
    server.await.unwrap().unwrap();
    // Our SETTINGS, then GOAWAY(PROTOCOL_ERROR).
    let goaway = out.windows(5).position(|w| w[3] == super::frame::GOAWAY && w[4] == 0).is_some();
    assert!(goaway, "{out:02x?}");
}

#[tokio::test]
async fn mcp_sse_stream_over_http2() {
    // #12: GET /mcp's SSE stream (a stream_pipe body) is delivered over
    // HTTP/2 as it's produced, not buffered until the end.
    let mcp = crate::mcp::McpServer::new("t", "1");
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    tokio::spawn(super::handle_connection(server_io, peer(), None, mcp.clone()));
    let (send, connection) = h2::client::handshake(client_io).await.unwrap();
    tokio::spawn(connection);
    let request = http::Request::get("https://x/mcp").header("accept", "text/event-stream").body(()).unwrap();
    let (response, _) = send.ready().await.unwrap().send_request(request, true).unwrap();
    let (parts, mut body) = response.await.unwrap().into_parts();
    assert_eq!(200, parts.status.as_u16());
    assert_eq!("text/event-stream", parts.headers["content-type"]);
    mcp.notify("notifications/test", Some(r#"{"n":1}"#));
    let chunk = tokio::time::timeout(std::time::Duration::from_secs(5), body.data()).await
        .expect("no SSE event within 5 s").unwrap().unwrap();
    let text = String::from_utf8_lossy(&chunk);
    assert!(text.contains("data: ") && text.contains("notifications/test"), "{text}");
}

// ── #31: streamed request bodies over HTTP/2 ─────────────────────────────────

/// Streams every POST body; answers with the byte count, or sends the read
/// error to `errors`.
#[derive(Clone)]
struct StreamingApp {
    errors: std::sync::mpsc::Sender<String>,
}

impl Application for StreamingApp {
    fn execute(&self, request: &Request, _connection: &ConnectionInfo) -> Result<Response, String> {
        use std::io::Read;
        let mut body = &request.body;
        let mut total = 0usize;
        let mut buf = [0u8; 4096];
        loop {
            match body.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => total += n,
                Err(e) => {
                    let _ = self.errors.send(e.to_string());
                    return Ok(Response::text(STATUS_CODE_REASON_PHRASE.n400_bad_request, &e.to_string()));
                }
            }
        }
        Ok(Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &format!("streamed {total} is_stream={}", request.body.is_stream())))
    }

    fn streams_body(&self, request: &Request, _connection: &ConnectionInfo) -> bool {
        request.method == "POST"
    }
}

async fn streaming_client() -> (h2::client::SendRequest<Bytes>, std::sync::mpsc::Receiver<String>) {
    let (errors, rx) = std::sync::mpsc::channel();
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    tokio::spawn(super::handle_connection(server_io, peer(), None, StreamingApp { errors }));
    let (send, connection) = h2::client::handshake(client_io).await.unwrap();
    tokio::spawn(connection);
    (send, rx)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_body_bigger_than_the_window_streams_to_the_handler() {
    // 1 MiB through a 64 KiB stream window: it only completes if reading
    // credits the window back.
    let (send, _errors) = streaming_client().await;
    let request = http::Request::post("https://x/upload").body(()).unwrap();
    let (response, mut body) = send.ready().await.unwrap().send_request(request, false).unwrap();
    for _ in 0..16 {
        body.send_data(Bytes::from(vec![b'q'; 64 * 1024]), false).unwrap();
    }
    body.send_data(Bytes::new(), true).unwrap();
    let (parts, out) = tokio::time::timeout(std::time::Duration::from_secs(10), read(response)).await.expect("stalled");
    assert_eq!(200, parts.status.as_u16());
    assert_eq!(format!("streamed {} is_stream=true", 1024 * 1024).as_bytes(), &out[..]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reset_mid_body_is_an_error_for_the_handler() {
    let (send, errors) = streaming_client().await;
    let request = http::Request::post("https://x/upload").body(()).unwrap();
    let (_response, mut body) = send.ready().await.unwrap().send_request(request, false).unwrap();
    body.send_data(Bytes::from_static(b"partial"), false).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    body.send_reset(h2::Reason::CANCEL);
    let error = tokio::task::spawn_blocking(move || errors.recv_timeout(std::time::Duration::from_secs(5))).await.unwrap()
        .expect("the handler never saw an error");
    assert!(error.contains("reset"), "{error}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_content_length_is_an_error_for_the_handler() {
    let (send, errors) = streaming_client().await;
    let request = http::Request::post("https://x/upload").header("content-length", "100").body(()).unwrap();
    let (_response, mut body) = send.ready().await.unwrap().send_request(request, false).unwrap();
    body.send_data(Bytes::from_static(b"short"), true).unwrap();
    let error = tokio::task::spawn_blocking(move || errors.recv_timeout(std::time::Duration::from_secs(5))).await.unwrap()
        .expect("the handler never saw an error");
    assert!(error.contains("content-length"), "{error}");
}

#[tokio::test]
async fn trailers_follow_the_body_in_their_own_header_block() {
    let send = client(TestApp(None)).await;
    for (path, body, status) in [("/trailers", b"body".to_vec(), "0"), ("/only-trailers", Vec::new(), "5")] {
        let request = http::Request::get(format!("https://x{}", path)).body(()).unwrap();
        let (response, _) = send.clone().ready().await.unwrap().send_request(request, true).unwrap();
        let (_, mut stream) = response.await.unwrap().into_parts();
        let mut got = Vec::new();
        while let Some(chunk) = stream.data().await {
            got.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(body, got, "{path}");
        let trailers = stream.trailers().await.unwrap().expect("trailers");
        assert_eq!(status, trailers["grpc-status"], "{path}");
    }
}

#[tokio::test]
async fn the_authority_is_the_requests_host() {
    let mut send = client(TestApp(None)).await;
    let request = http::Request::get("https://shop.example:8443/p").body(()).unwrap();
    let (response, _) = send.clone().ready().await.unwrap().send_request(request, true).unwrap();
    let (parts, _) = read(response).await;
    assert_eq!("shop.example:8443", parts.headers["x-seen-host"], "RFC 9113 §8.3.1: `:authority` stands for Host");
    let request = http::Request::get("https://shop.example/p").header("host", "other.example").body(()).unwrap();
    let (response, _) = send.ready().await.unwrap().send_request(request, true).unwrap();
    let (parts, _) = read(response).await;
    assert_eq!("other.example", parts.headers["x-seen-host"], "a host field is kept");
}
