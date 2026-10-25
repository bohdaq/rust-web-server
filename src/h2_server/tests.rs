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
            r.content_range_list[0].body = request.body.clone();
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
