//! In-process testing: [`TestClient`] dispatches requests through an
//! [`Application`] without opening a socket, and the helpers beside it cover
//! the rest of a test:
//!
//! - [`TestClient::websocket`] drives a WebSocket route's upgrade handler over
//!   an in-memory duplex ([`TestWebSocket`]);
//! - [`TestResponse::sse`] reads a `text/event-stream` response, streamed or
//!   buffered, and parses its events ([`TestSse`], [`SseFrame`]);
//! - [`assert_openapi_snapshot`] compares the generated OpenAPI document with a
//!   committed file (`RWS_UPDATE_SNAPSHOTS=1` rewrites it);
//! - `TestDb` (`model-*` features) gives each test a database that leaves
//!   nothing behind, and `factory` builds `#[derive(Factory)]` models with
//!   default values.
//!
//! Everything here is for tests: nothing on the request path uses it. It is
//! compiled into every build (like `TestClient` always was) so that an
//! application's own tests, in another crate, reach it without a feature of
//! their own.

#[cfg(test)]
mod tests;
#[cfg(test)]
mod helper_tests;
pub mod sse;
pub mod snapshot;
#[cfg(not(target_arch = "wasm32"))]
pub mod websocket;
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub mod db;
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub mod factory;

pub use snapshot::{assert_json_snapshot, assert_openapi_snapshot, OpenApiSource};
pub use sse::{SseFrame, SseParser, TestSse};
#[cfg(not(target_arch = "wasm32"))]
pub use websocket::{TestWebSocket, TestWebSocketRequest};
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub use db::{TestDb, TestDbBuilder};

use crate::application::Application;
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

/// An in-process HTTP test client that dispatches requests directly through an
/// [`Application`] without opening a TCP socket.
///
/// Use this in unit and integration tests to exercise controllers and routing
/// without starting the server.
///
/// # Example
///
/// ```rust,no_run
/// # extern crate rws_server as rust_web_server;
/// use rust_web_server::app::App;
/// use rust_web_server::core::New;
/// use rust_web_server::test_client::TestClient;
///
/// let client = TestClient::new(App::new());
///
/// let res = client.get("/healthz").send();
/// assert_eq!(200, res.status());
///
/// let res = client.post("/echo")
///     .header("Content-Type", "text/plain")
///     .body_text("hello")
///     .send();
/// assert_eq!(200, res.status());
/// ```
pub struct TestClient<A: Application> {
    app: A,
    connection: ConnectionInfo,
}

impl<A: Application> TestClient<A> {
    /// Create a test client wrapping `app`. Requests are dispatched on a
    /// synthetic `127.0.0.1:12345 → 127.0.0.1:7878` connection.
    pub fn new(app: A) -> Self {
        TestClient {
            app,
            connection: ConnectionInfo {
                client: Address { ip: "127.0.0.1".to_string(), port: 12345 },
                server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
                request_size: 16000,
                sni_hostname: None,
                tls: false,
            },
        }
    }

    /// Build a `GET` request to `path`.
    pub fn get(&self, path: &str) -> TestRequest<'_, A> {
        TestRequest::new(METHOD.get.to_string(), path, self)
    }

    /// Build a `POST` request to `path`.
    pub fn post(&self, path: &str) -> TestRequest<'_, A> {
        TestRequest::new(METHOD.post.to_string(), path, self)
    }

    /// Build a `PUT` request to `path`.
    pub fn put(&self, path: &str) -> TestRequest<'_, A> {
        TestRequest::new(METHOD.put.to_string(), path, self)
    }

    /// Build a `PATCH` request to `path`.
    pub fn patch(&self, path: &str) -> TestRequest<'_, A> {
        TestRequest::new(METHOD.patch.to_string(), path, self)
    }

    /// Build a `DELETE` request to `path`.
    pub fn delete(&self, path: &str) -> TestRequest<'_, A> {
        TestRequest::new(METHOD.delete.to_string(), path, self)
    }

    /// Build an `OPTIONS` request to `path`.
    pub fn options(&self, path: &str) -> TestRequest<'_, A> {
        TestRequest::new(METHOD.options.to_string(), path, self)
    }

    /// Build a WebSocket upgrade request to `path`; call
    /// [`connect`](TestWebSocketRequest::connect) to run the route's upgrade
    /// handler over an in-memory connection (no socket).
    ///
    /// ```rust,no_run
    /// # extern crate rws_server as rust_web_server;
    /// use rust_web_server::app::App;
    /// use rust_web_server::test_client::TestClient;
    /// use rust_web_server::websocket::{Message, WebSocket};
    ///
    /// let app = App::with_state(()).get("/echo", |request, _params, _conn, _state| {
    ///     WebSocket::upgrade(request, |mut ws| {
    ///         while let Ok(Some(Message::Text(text))) = ws.recv() {
    ///             let _ = ws.send_text(&text);
    ///         }
    ///     })
    /// });
    /// let client = TestClient::new(app);
    /// let mut ws = client.websocket("/echo").connect().expect("a 101");
    /// ws.send_text("hi");
    /// assert_eq!("hi", ws.recv_text());
    /// assert_eq!(Some(1000), ws.close(1000, "bye"));
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn websocket(&self, path: &str) -> TestWebSocketRequest<'_, A> {
        TestWebSocketRequest::new(TestRequest::new(METHOD.get.to_string(), path, self))
    }

    /// The application under test.
    pub fn app(&self) -> &A {
        &self.app
    }
}

/// A pending test request. Chain builder methods then call [`TestRequest::send`].
pub struct TestRequest<'a, A: Application> {
    method: String,
    path: String,
    headers: Vec<Header>,
    body: Vec<u8>,
    client: &'a TestClient<A>,
}

impl<'a, A: Application> TestRequest<'a, A> {
    fn new(method: String, path: &str, client: &'a TestClient<A>) -> Self {
        TestRequest {
            method,
            path: path.to_string(),
            headers: vec![],
            body: Default::default(),
            client,
        }
    }

    /// Add a request header.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push(Header { name: name.to_string().into(), value: value.to_string().into() });
        self
    }

    /// Set the request body to raw bytes.
    pub fn body_bytes(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    /// Set the request body to a UTF-8 string.
    pub fn body_text(mut self, text: &str) -> Self {
        self.body = text.as_bytes().to_vec();
        self
    }

    /// Set the request body to a JSON string, with
    /// `Content-Type: application/json`.
    pub fn body_json(mut self, json: &str) -> Self {
        self.headers.push(Header { name: Header::_CONTENT_TYPE.into(), value: "application/json".into() });
        self.body = json.as_bytes().to_vec();
        self
    }

    /// Dispatch the request and return the response.
    pub fn send(self) -> TestResponse {
        TestResponse::from_response(self.dispatch())
    }

    /// Whether a header named `name` (case-insensitive) was added.
    #[cfg(not(target_arch = "wasm32"))]
    fn has_header(&self, name: &str) -> bool {
        self.headers.iter().any(|h| h.name.eq_ignore_ascii_case(name))
    }

    fn into_request(self) -> (Request, &'a TestClient<A>) {
        let request = Request {
            method: self.method,
            request_uri: self.path,
            http_version: VERSION.http_1_1.to_string(),
            headers: self.headers,
            body: self.body.into(),
        };
        (request, self.client)
    }

    /// Run the request through the application; an `Err` becomes a `500`.
    fn dispatch(self) -> Response {
        let (request, client) = self.into_request();
        client.app.execute(&request, &client.connection).unwrap_or_else(error_response)
    }
}

#[cfg(feature = "http2")]
impl<'a, S: Send + Sync + 'static> TestRequest<'a, crate::async_state::AsyncAppWithState<S>> {
    /// Dispatch the request to an `App::with_async_state` app and await
    /// the handler, as the async servers do (on this runtime when it's
    /// multi-threaded, else on rws's `rws-async` runtime). Use it from
    /// `#[tokio::test]`: [`send`](Self::send) blocks the test's thread while
    /// the handler runs, so on a single-threaded test runtime anything made
    /// on it — a PostgreSQL or MySQL `DbPool`'s connections — can't make
    /// progress meanwhile.
    pub async fn send_async(self) -> TestResponse {
        let (request, client) = self.into_request();
        let result = client.app.respond(&request, &client.connection).await;
        TestResponse::from_response(result.unwrap_or_else(error_response))
    }
}

/// The `500` a handler's `Err(message)` becomes.
fn error_response(message: String) -> Response {
    let dummy = Request {
        method: "GET".to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };
    let header_list = Header::get_header_list(&dummy);
    let cr = Range::get_content_range(message.into_bytes(), MimeType::TEXT_PLAIN.to_string());
    Response::get_response(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, Some(header_list), Some(vec![cr]))
}

/// The result of a dispatched test request.
pub struct TestResponse {
    status: i16,
    reason: String,
    headers: Vec<Header>,
    body: Vec<u8>,
    stream: StreamSlot,
}

/// A streamed body's reader, reachable only through `&mut` — so
/// `TestResponse` stays `Sync` and unwind-safe, as it was before it kept one.
struct StreamSlot(Option<Box<dyn std::io::Read + Send>>);

// SAFETY: a shared `&StreamSlot` gives no access to the reader (only whether
// there is one); the reader itself is `Send`, and is used only through
// `&mut StreamSlot`, i.e. by one thread at a time.
unsafe impl Sync for StreamSlot {}
impl std::panic::UnwindSafe for StreamSlot {}
impl std::panic::RefUnwindSafe for StreamSlot {}

impl std::fmt::Debug for TestResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestResponse")
            .field("status", &self.status)
            .field("reason", &self.reason)
            .field("headers", &self.headers.iter().map(|h| format!("{}: {}", h.name, h.value)).collect::<Vec<_>>())
            .field("body", &String::from_utf8_lossy(&self.body))
            .field("streamed", &self.is_streamed())
            .finish()
    }
}

impl TestResponse {
    fn from_response(mut r: Response) -> Self {
        let body: Vec<u8> = r.content_range_list.iter()
            .flat_map(|cr| cr.body.iter().copied())
            .collect();

        // Mirror Response::generate_response, which only adds these headers
        // at HTTP/1.1 write time — TestClient bypasses that path entirely.
        if r.content_range_list.len() == 1 {
            let content_range = r.content_range_list.get(0).unwrap();
            r.headers.push(Header {
                name: Header::_CONTENT_TYPE.into(),
                value: content_range.content_type.to_string().into(),
            });
            if let Some(value) = crate::response::content_range_header_value(r.status_code, content_range) {
                r.headers.push(Header {
                    name: Header::_CONTENT_RANGE.into(),
                    value: value.into(),
                });
            }
            r.headers.push(Header {
                name: Header::_CONTENT_LENGTH.into(),
                value: content_range.body.len().to_string().into(),
            });
        }

        TestResponse {
            status: r.status_code,
            reason: r.reason_phrase,
            headers: r.headers,
            body,
            stream: StreamSlot(r.stream_pipe.take()),
        }
    }

    /// HTTP status code, e.g. `200`.
    pub fn status(&self) -> i16 {
        self.status
    }

    /// HTTP reason phrase, e.g. `"OK"`.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Return the value of the first header matching `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        let lower = name.to_lowercase();
        self.headers
            .iter()
            .find(|h| h.name.to_lowercase() == lower)
            .map(|h| h.value.as_ref())
    }

    /// All response headers.
    pub fn headers(&self) -> &[Header] {
        &self.headers
    }

    /// Raw response body bytes.
    pub fn body_bytes(&self) -> &[u8] {
        &self.body
    }

    /// Response body decoded as UTF-8. Panics if the body is not valid UTF-8.
    pub fn body_text(&self) -> &str {
        std::str::from_utf8(&self.body).expect("response body is not valid UTF-8")
    }

    /// `true` if the status code is 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// `true` if the body is streamed (`Response::stream_pipe`, as from
    /// [`Sse::stream`](crate::sse::Sse::stream)) rather than buffered. A
    /// streamed body isn't in [`body_bytes`](Self::body_bytes): read it with
    /// [`sse`](Self::sse) or [`take_stream`](Self::take_stream).
    pub fn is_streamed(&self) -> bool {
        self.stream.0.is_some()
    }

    /// The streamed body's reader, if there is one (taken: a second call
    /// returns `None`).
    pub fn take_stream(&mut self) -> Option<Box<dyn std::io::Read + Send>> {
        self.stream.0.take()
    }

    /// Read the body as Server-Sent Events: a streamed body as it arrives
    /// (on a thread of its own), a buffered one at once.
    ///
    /// ```rust,no_run
    /// # extern crate rws_server as rust_web_server;
    /// use std::time::Duration;
    /// use rust_web_server::app::App;
    /// use rust_web_server::sse::{Sse, SseEvent};
    /// use rust_web_server::test_client::TestClient;
    ///
    /// let app = App::with_state(()).get("/events", |_req, _params, _conn, _state| {
    ///     let (events, response) = Sse::stream();
    ///     std::thread::spawn(move || {
    ///         let _ = events.send(SseEvent::data("1").event_type("tick").id("1"));
    ///     });
    ///     response
    /// });
    /// let client = TestClient::new(app);
    /// let events = client.get("/events").send().sse_events(1, Duration::from_secs(5));
    /// assert_eq!("tick", events[0].event);
    /// assert_eq!("1", events[0].data);
    /// ```
    pub fn sse(mut self) -> TestSse {
        match self.take_stream() {
            Some(reader) => TestSse::from_reader(reader),
            None => TestSse::from_bytes(&self.body),
        }
    }

    /// Up to `n` events from [`sse`](Self::sse), waiting at most `timeout` in
    /// all (fewer when the stream ends or the time runs out).
    pub fn sse_events(self, n: usize, timeout: std::time::Duration) -> Vec<SseFrame> {
        self.sse().events(n, timeout)
    }
}
