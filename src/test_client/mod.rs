#[cfg(test)]
mod tests;

use crate::application::Application;
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};
use crate::symbol::SYMBOL;

/// An in-process HTTP test client that dispatches requests directly through an
/// [`Application`] without opening a TCP socket.
///
/// Use this in unit and integration tests to exercise controllers and routing
/// without starting the server.
///
/// # Example
///
/// ```rust,no_run
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
            body: vec![],
            client,
        }
    }

    /// Add a request header.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push(Header { name: name.to_string(), value: value.to_string() });
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

    /// Dispatch the request and return the response.
    pub fn send(self) -> TestResponse {
        let request = Request {
            method: self.method,
            request_uri: self.path,
            http_version: VERSION.http_1_1.to_string(),
            headers: self.headers,
            body: self.body,
        };

        let response = self.client.app.execute(&request, &self.client.connection)
            .unwrap_or_else(|msg| {
                let dummy = Request {
                    method: "GET".to_string(),
                    request_uri: "/".to_string(),
                    http_version: VERSION.http_1_1.to_string(),
                    headers: vec![],
                    body: vec![],
                };
                let header_list = Header::get_header_list(&dummy);
                let body = msg.into_bytes();
                let cr = Range::get_content_range(body, MimeType::TEXT_PLAIN.to_string());
                Response::get_response(
                    STATUS_CODE_REASON_PHRASE.n500_internal_server_error,
                    Some(header_list),
                    Some(vec![cr]),
                )
            });

        TestResponse::from_response(response)
    }
}

/// The result of a dispatched test request.
pub struct TestResponse {
    status: i16,
    reason: String,
    headers: Vec<Header>,
    body: Vec<u8>,
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
                name: Header::_CONTENT_TYPE.to_string(),
                value: content_range.content_type.to_string(),
            });
            let content_range_header_value = [
                Range::BYTES,
                SYMBOL.whitespace,
                &content_range.range.start.to_string(),
                SYMBOL.hyphen,
                &content_range.range.end.to_string(),
                SYMBOL.slash,
                &content_range.size,
            ].join("");
            r.headers.push(Header {
                name: Header::_CONTENT_RANGE.to_string(),
                value: content_range_header_value,
            });
            r.headers.push(Header {
                name: Header::_CONTENT_LENGTH.to_string(),
                value: content_range.body.len().to_string(),
            });
        }

        TestResponse {
            status: r.status_code,
            reason: r.reason_phrase,
            headers: r.headers,
            body,
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
            .map(|h| h.value.as_str())
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
}
