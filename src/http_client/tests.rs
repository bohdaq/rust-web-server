//! Tests for the outbound HTTP client.
//!
//! All tests spin up an in-process `TcpListener` bound to a random ephemeral
//! port and act as a fake HTTP server — no real network access required.

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use crate::http_client::{Client, HttpClientError};

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Spawn a fake HTTP server that handles **one** connection.
    ///
    /// `handler` receives the raw request text and returns the raw response
    /// bytes that should be sent back.  Returns the base URL of the server.
    fn start_fake_server(handler: impl Fn(String) -> String + Send + 'static) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = vec![0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let resp = handler(req);
                stream.write_all(resp.as_bytes()).ok();
            }
        });
        format!("http://127.0.0.1:{}", addr.port())
    }

    // ── URL parsing ───────────────────────────────────────────────────────────

    #[test]
    fn parse_url_http() {
        // Access private ParsedUrl via the public API behaviour — we parse a
        // constructed URL by making a request to a fake server on the port it
        // resolves to. Alternatively, expose ParsedUrl through a test helper.
        // Here we test via the public `send()` path: a 200 response proves
        // the URL was parsed correctly.
        let base = start_fake_server(|_req| {
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nbody".to_string()
        });
        // Build a URL with path + query
        let url = format!("{}/path?q=1", base);
        let resp = Client::new().get(&url).send().unwrap();
        assert_eq!(200, resp.status());
        assert_eq!(b"body", resp.bytes());
    }

    #[test]
    fn parse_url_https_custom_port() {
        // We test URL parsing directly via the private ParsedUrl struct
        // using a module-level re-export trick only available in the same crate.
        // The cleanest cross-boundary test is to verify `HttpClientError` is
        // returned for bad URLs and success for well-formed ones.
        // For HTTPS we cannot spin up a TLS server easily, so we just verify
        // parsing by attempting a connection that fails at the TLS layer — the
        // error must NOT say "unsupported or missing URL scheme".
        let result = Client::new()
            .get("https://127.0.0.1:19999/v1")
            .timeout_ms(200)
            .send();
        match result {
            Err(HttpClientError(msg)) => {
                assert!(
                    !msg.contains("missing URL scheme"),
                    "URL was not parsed (got: {msg})"
                );
            }
            Ok(_) => { /* unexpected success is also fine for this parse-only test */ }
        }
    }

    #[test]
    fn parse_url_missing_scheme_returns_error() {
        let result = Client::new().get("example.com/path").timeout_ms(100).send();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.0.contains("missing URL scheme") || err.0.contains("unsupported"),
            "expected scheme error, got: {}",
            err.0
        );
    }

    // ── Plain HTTP requests ───────────────────────────────────────────────────

    #[test]
    fn get_plain_http() {
        let base = start_fake_server(|_req| {
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nbody".to_string()
        });
        let resp = Client::new().get(&base).send().unwrap();
        assert_eq!(200, resp.status());
        assert_eq!("body", resp.text().unwrap());
    }

    #[test]
    fn post_with_body() {
        // The fake server echoes "received" so we just check the request was
        // sent with the right Content-Length (by inspecting its echo).
        let base = start_fake_server(|req| {
            // req should contain "Content-Length: 5"
            let has_len = req.contains("Content-Length: 5");
            let has_body = req.contains("hello");
            let status = if has_len && has_body { "200" } else { "400" };
            format!("HTTP/1.1 {status} OK\r\nContent-Length: 2\r\n\r\nok")
        });
        let resp = Client::new()
            .post(&base)
            .body(b"hello".to_vec())
            .send()
            .unwrap();
        assert_eq!(200, resp.status());
    }

    #[test]
    fn response_with_chunked_encoding() {
        // Server sends chunked body: "Hello, " + "World!"
        let base = start_fake_server(|_req| {
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n7\r\nHello, \r\n6\r\nWorld!\r\n0\r\n\r\n".to_string()
        });
        let resp = Client::new().get(&base).send().unwrap();
        assert_eq!(200, resp.status());
        assert_eq!("Hello, World!", resp.text().unwrap());
    }

    // ── Response helpers ──────────────────────────────────────────────────────

    #[test]
    fn is_success_true_for_200_range() {
        let base = start_fake_server(|_| {
            "HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n".to_string()
        });
        let resp = Client::new().get(&base).send().unwrap();
        assert!(resp.is_success());
    }

    #[test]
    fn is_success_false_for_404() {
        let base = start_fake_server(|_| {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string()
        });
        let resp = Client::new().get(&base).send().unwrap();
        assert!(!resp.is_success());
    }

    #[test]
    fn header_lookup_case_insensitive() {
        let base = start_fake_server(|_| {
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}".to_string()
        });
        let resp = Client::new().get(&base).send().unwrap();
        assert_eq!(Some("application/json"), resp.header("content-type"));
        assert_eq!(Some("application/json"), resp.header("Content-Type"));
        assert_eq!(Some("application/json"), resp.header("CONTENT-TYPE"));
    }

    // ── Redirect following ────────────────────────────────────────────────────

    #[test]
    fn redirect_followed() {
        // First connection: 301 → /new
        // Second connection: 200 OK
        // We need two servers because the client opens a new TCP connection for
        // each hop.

        // Start the final destination server first so we know its port
        let dest = start_fake_server(|_| {
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ndone".to_string()
        });

        // Start the redirect server that sends Location pointing at dest
        let dest_url = dest.clone();
        let redir = start_fake_server(move |_| {
            format!(
                "HTTP/1.1 301 Moved Permanently\r\nLocation: {dest_url}\r\nContent-Length: 0\r\n\r\n"
            )
        });

        let resp = Client::new().get(&redir).send().unwrap();
        assert_eq!(200, resp.status());
        assert_eq!("done", resp.text().unwrap());
    }

    #[test]
    fn max_redirects_exceeded_returns_last_response() {
        // Build a chain of 3 servers each 301-ing to the next, with the last
        // one also returning 301 (no final 200). With max_redirects=2 the
        // client should stop after 2 follows and return the last 301 response.

        // Server C — always returns 301 back to itself (loop), but client
        // should stop before reaching it a third time.
        let server_c = start_fake_server(|_| {
            "HTTP/1.1 301 Moved\r\nLocation: http://127.0.0.1:1\r\nContent-Length: 0\r\n\r\n"
                .to_string()
        });

        let c_url = server_c.clone();
        let server_b = start_fake_server(move |_| {
            format!(
                "HTTP/1.1 301 Moved\r\nLocation: {c_url}\r\nContent-Length: 0\r\n\r\n"
            )
        });

        let b_url = server_b.clone();
        let server_a = start_fake_server(move |_| {
            format!(
                "HTTP/1.1 301 Moved\r\nLocation: {b_url}\r\nContent-Length: 0\r\n\r\n"
            )
        });

        // max_redirects=2 means we follow at most 2 redirects (A→B, B→C)
        // then return C's response (301) without an error
        let resp = Client::new()
            .max_redirects(2)
            .get(&server_a)
            .send()
            .unwrap();
        // The client must return the last response it received, even if it's a redirect
        assert!(resp.is_redirect());
    }

    // ── Custom headers ────────────────────────────────────────────────────────

    #[test]
    fn custom_header_sent() {
        let base = start_fake_server(|req| {
            // Echo whether the header was present
            let found = req.contains("X-Custom: value");
            let body = if found { "yes" } else { "no" };
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
        });
        let resp = Client::new()
            .get(&base)
            .header("X-Custom", "value")
            .send()
            .unwrap();
        assert_eq!("yes", resp.text().unwrap());
    }

    // ── Timeout ───────────────────────────────────────────────────────────────

    #[test]
    fn timeout_returns_error() {
        // Accept the connection but never write anything back — triggers a
        // read timeout on the client.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((_stream, _)) = listener.accept() {
                // hold the connection open without sending anything
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
        let url = format!("http://127.0.0.1:{}", addr.port());
        let result = Client::new().timeout_ms(150).get(&url).send();
        assert!(
            result.is_err(),
            "expected timeout error but got a response"
        );
    }

    // ── DELETE method ─────────────────────────────────────────────────────────

    #[test]
    fn delete_method_sent_correctly() {
        let base = start_fake_server(|req| {
            let is_delete = req.starts_with("DELETE ");
            let body = if is_delete { "deleted" } else { "wrong" };
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
        });
        let resp = Client::new().delete(&base).send().unwrap();
        assert_eq!(200, resp.status());
        assert_eq!("deleted", resp.text().unwrap());
    }

    // ── form encoding ─────────────────────────────────────────────────────────

    #[test]
    fn form_sets_content_type_and_urlencoded_body() {
        let base = start_fake_server(|req| {
            let has_ct = req.contains("Content-Type: application/x-www-form-urlencoded");
            let has_body = req.contains("grant_type=authorization_code&code=abc%20123");
            let status = if has_ct && has_body { "200" } else { "400" };
            format!("HTTP/1.1 {status} OK\r\nContent-Length: 2\r\n\r\nok")
        });
        let resp = Client::new()
            .post(&base)
            .form(&[("grant_type", "authorization_code"), ("code", "abc 123")])
            .send()
            .unwrap();
        assert_eq!(200, resp.status());
    }

    #[test]
    fn form_encodes_reserved_characters_in_keys_and_values() {
        let base = start_fake_server(|req| {
            let has_body = req.contains("redirect_uri=https%3A%2F%2Fexample.com%2Fcb%3Fx%3D1");
            let status = if has_body { "200" } else { "400" };
            format!("HTTP/1.1 {status} OK\r\nContent-Length: 2\r\n\r\nok")
        });
        let resp = Client::new()
            .post(&base)
            .form(&[("redirect_uri", "https://example.com/cb?x=1")])
            .send()
            .unwrap();
        assert_eq!(200, resp.status());
    }

    #[test]
    fn form_empty_pairs_sends_content_type_with_empty_body() {
        // An empty body never gets a Content-Length header (pre-existing
        // build_request_bytes behavior, unrelated to .form()) — just confirm
        // the Content-Type header is still set and nothing sent for the body.
        let base = start_fake_server(|req| {
            let has_ct = req.contains("Content-Type: application/x-www-form-urlencoded");
            let ends_with_headers = req.ends_with("\r\n\r\n");
            let status = if has_ct && ends_with_headers { "200" } else { "400" };
            format!("HTTP/1.1 {status} OK\r\nContent-Length: 2\r\n\r\nok")
        });
        let resp = Client::new().post(&base).form(&[]).send().unwrap();
        assert_eq!(200, resp.status());
    }
}
