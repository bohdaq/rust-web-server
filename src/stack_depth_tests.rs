//! Parsers fed untrusted input must not recurse per line / part / nesting
//! level: a stack overflow aborts the whole process (it can't be caught like
//! a panic). Each case runs on a thread with a deliberately small stack, so a
//! regression shows up as this test binary aborting.

const STACK: usize = 256 * 1024;
fn on_small_stack<F: FnOnce() + Send + 'static>(f: F) {
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn request_with_thousands_of_header_lines() {
    on_small_stack(|| {
        let mut raw = b"GET / HTTP/1.1\r\nHost: a\r\n".to_vec();
        raw.extend(b"a:b\r\n".repeat(20_000));
        raw.extend(b"\r\n");
        let request = crate::request::Request::parse(&raw).unwrap();
        assert!(request.headers.len() > 20_000);
    });
}

#[test]
fn response_with_thousands_of_header_lines() {
    on_small_stack(|| {
        let mut raw = b"HTTP/1.1 200 OK\r\n".to_vec();
        raw.extend(b"X-A: b\r\n".repeat(20_000));
        raw.extend(b"\r\nbody");
        assert!(crate::response::Response::parse(&raw).is_ok());
    });
}

#[test]
fn multipart_form_with_thousands_of_parts() {
    on_small_stack(|| {
        let part = b"--XY\r\nContent-Disposition: form-data; name=\"f\"\r\n\r\nv\r\n";
        let mut body = part.repeat(5_000);
        body.extend(b"--XY--\r\n");
        let _ = crate::body::multipart_form_data::FormMultipartData::parse(&body, "XY".to_string());
    });
}

#[test]
fn multipart_byteranges_response_with_thousands_of_parts() {
    on_small_stack(|| {
        let mut raw = b"HTTP/1.1 206 Partial Content\r\nContent-Type: multipart/byteranges; boundary=XX\r\n\r\n".to_vec();
        raw.extend(b"--XX\r\nContent-Type: text/plain\r\nContent-Range: bytes 0-0/10\r\n\r\na\r\n".repeat(5_000));
        raw.extend(b"--XX--\r\n");
        let _ = crate::response::Response::parse(&raw);
    });
}

#[test]
fn deeply_nested_json() {
    on_small_stack(|| {
        let nested = format!("{}{}", "{\"a\":".repeat(50_000), "}".repeat(50_000));
        let _ = crate::json::object::JSON::parse_as_properties(nested.clone());
        let _ = McpServerProbe::run(&format!("[{}]", "[".repeat(50_000)));
    });
}

struct McpServerProbe;
impl McpServerProbe {
    fn run(body: &str) {
        let _ = crate::mcp::McpServer::new("t", "1").handle_request(body);
    }
}

#[cfg(feature = "sso-saml")]
#[test]
fn deeply_nested_saml_xml() {
    on_small_stack(|| {
        let xml = format!("{}{}", "<a>".repeat(100_000), "</a>".repeat(100_000));
        assert!(crate::sso::saml::xml_parse_for_tests(&xml).is_err());
    });
}
