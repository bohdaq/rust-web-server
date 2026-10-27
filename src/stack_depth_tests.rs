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
        // An upstream reply the MCP gateway reads (#62).
        let deep = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{\"tools\":[{}]}}}}", "[".repeat(50_000));
        let tools = r#"{"tools":["#.to_string() + &"[".repeat(50_000);
        let _ = crate::mcp::gateway::parse_reply("application/json", deep.as_bytes(), 1);
        let _ = crate::mcp::gateway::fuzzing::tools(&tools);
        let events = "data: {}\n\n".repeat(50_000);
        let _ = crate::mcp::gateway::parse_reply("text/event-stream", events.as_bytes(), 1);
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

#[cfg(feature = "http2")]
#[test]
fn hpack_block_with_thousands_of_fields() {
    on_small_stack(|| {
        // Length-prefixed blocks of indexed fields (":method GET", 0x82).
        let mut data = Vec::new();
        for _ in 0..2_000 {
            data.push(250);
            data.extend(std::iter::repeat(0x82).take(250));
        }
        crate::h2_server::fuzz::hpack_decode(&data);
    });
}

#[cfg(feature = "http2")]
#[test]
fn h2_connection_with_thousands_of_frames() {
    on_small_stack(|| {
        // PINGs, WINDOW_UPDATEs and a request split into many CONTINUATIONs.
        let mut data = Vec::new();
        for _ in 0..5_000 {
            data.extend([0, 0, 8, 6, 0, 0, 0, 0, 0]);
            data.extend(*b"12345678");
            data.extend([0, 0, 4, 8, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        }
        data.extend([0, 0, 1, 1, 1, 0, 0, 0, 1, 0x82]); // HEADERS, END_STREAM
        for i in 0..3_000 {
            let flags = if i == 2_999 { 4 } else { 0 };
            data.extend([0, 0, 1, 9, flags, 0, 0, 0, 1, if i % 2 == 0 { 0x87 } else { 0x84 }]);
        }
        crate::h2_server::fuzz::connection(&data);
    });
}

#[test]
fn dns_message_with_tens_of_thousands_of_records() {
    on_small_stack(|| {
        let mut raw = vec![0, 1, 0x80, 0, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0];
        for _ in 0..0xFFFF {
            // Each owner name a compression pointer back to... itself: the
            // per-name hop limit, not the stack, stops it.
            raw.extend_from_slice(&[0xC0, 12, 0, 16, 0, 1, 0, 0, 0, 0, 0, 0]);
        }
        assert!(crate::dns::parse(&raw).is_err());
        let mut ok = vec![0, 1, 0x80, 0, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0];
        for _ in 0..0xFFFF {
            ok.extend_from_slice(&[0, 0, 16, 0, 1, 0, 0, 0, 0, 0, 1, 0]);
        }
        assert_eq!(0xFFFF, crate::dns::parse(&ok).unwrap().answers.len());
    });
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn baggage_header_with_thousands_of_members() {
    on_small_stack(|| {
        let header = vec!["k=v;p"; 50_000].join(",");
        let baggage = crate::otel::Baggage::parse(&header);
        assert_eq!(1, baggage.len(), "one key, set again and again");
    });
}
