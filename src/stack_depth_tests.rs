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

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn llm_usage_with_deep_nesting_and_thousands_of_events() {
    on_small_stack(|| {
        let mut parser = crate::llm::UsageParser::new("text/event-stream");
        let deep = format!("data: {{\"usage\":{}{}}}\n", "[".repeat(100_000), "]".repeat(100_000));
        parser.feed(deep.as_bytes());
        for _ in 0..20_000 {
            parser.feed(b"data: {\"choices\":[{\"delta\":{}}],\"usage\":null}\n\n");
        }
        parser.feed(b"data: {\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2}}\n");
        assert_eq!(parser.finish().map(|u| u.total()), Some(3));
        let mut json = crate::llm::UsageParser::new("application/json");
        json.feed(format!("{{\"usage\":{}{}}}", "{\"a\":".repeat(50_000), "}".repeat(50_000)).as_bytes());
        let _ = json.finish();
    });
}

#[test]
#[cfg(feature = "wasm-plugins")]
fn wasm_plugin_output_with_hundreds_of_thousands_of_lines() {
    on_small_stack(|| {
        let output = "set x-a: 1\n".repeat(200_000);
        assert!(crate::wasm_plugin::parse_request_output(output.as_bytes()).is_ok());
        let respond = format!("respond 200\n{}\nbody", "x-h: v\n".repeat(200_000));
        assert!(crate::wasm_plugin::parse_request_output(respond.as_bytes()).is_ok());
        assert!(crate::wasm_plugin::parse_response_output("status 200\n".repeat(200_000).as_bytes()).is_ok());
    });
}

#[test]
fn authz_check_through_a_long_cycle_of_groups() {
    on_small_stack(|| {
        use crate::authz::{Authorizer, MemoryStore, Schema};
        let schema = Schema::new().relation("group", "member", "user | group#member").unwrap();
        let authz = Authorizer::new(schema, MemoryStore::new()).unwrap();
        for i in 0..5000 {
            authz.write_str(&format!("group:g{i}#member@group:g{}#member", (i + 1) % 5000)).unwrap();
        }
        let _ = authz.check("user:x", "member", "group:g0");
        let long = "user | ".repeat(500) + "user";
        let _ = crate::authz::rebac::parse_expression(&long);
    });
}

#[test]
fn i18n_catalog_and_accept_language_of_any_length() {
    on_small_stack(|| {
        let text = "k = \"{a}{{b}}\"\n".repeat(50_000);
        let catalogs = crate::i18n::Catalogs::new("en").add("en", &text).unwrap();
        let message = "{x}".repeat(50_000);
        let catalogs = catalogs.add("uk", &format!("m = \"{message}\"")).unwrap();
        assert_eq!(150_000, catalogs.for_language("uk").t("m").len());
        let header = "en;q=0.5,".repeat(50_000);
        assert_eq!(crate::i18n::MAX_LANGUAGE_RANGES, crate::i18n::AcceptLanguage::parse(&header).0.len());
    });
}

#[test]
fn json_lite_refuses_deep_nesting() {
    on_small_stack(|| {
        let deep = "[".repeat(100_000) + &"]".repeat(100_000);
        assert!(crate::service_discovery::json_lite::parse(&deep).unwrap_err().contains("nested deeper"));
        let ok = "[".repeat(100) + &"]".repeat(100);
        assert!(crate::service_discovery::json_lite::parse(&ok).is_ok());
    });
}

#[test]
fn openapi_schemas_nested_and_referenced_deeply() {
    on_small_stack(|| {
        let nested = "{\"type\":\"object\",\"properties\":{\"a\":".repeat(60) + "{}" + &"}}".repeat(60);
        let spec = format!(r#"{{"openapi":"3.1.0","paths":{{"/x":{{"get":{{"operationId":"x","parameters":[{{"name":"p","in":"query","schema":{nested}}}]}}}}}}}}"#);
        let found = crate::mcp::openapi_tools::tools(&spec, &Default::default()).unwrap();
        assert_eq!(1, found.len());
        let chain: String = (0..200).map(|i| format!(r##""S{i}":{{"$ref":"#/components/schemas/S{}"}},"##, i + 1)).collect();
        let refs = format!(r##"{{"openapi":"3.1.0","paths":{{"/y":{{"get":{{"operationId":"y","parameters":[{{"name":"p","in":"query","schema":{{"$ref":"#/components/schemas/S0"}}}}]}}}}}},"components":{{"schemas":{{{chain}"S200":{{"type":"string"}}}}}}}}"##);
        assert_eq!(1, crate::mcp::openapi_tools::tools(&refs, &Default::default()).unwrap().len());
    });
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn fastcgi_stream_of_thousands_of_records() {
    on_small_stack(|| {
        let mut buf = Vec::new();
        for _ in 0..20_000 {
            crate::fastcgi::encode_record(&mut buf, crate::fastcgi::STDOUT, 1, b"x");
        }
        assert_eq!(crate::fastcgi::parse_records(&buf).unwrap().len(), 20_000);
        crate::fastcgi::fuzz::records(&"Status: 200\nX: y\n".repeat(5000).into_bytes());
    });
}

#[test]
fn waf_on_deeply_nested_and_long_values() {
    on_small_stack(|| {
        let nested = format!("{}{}", "${".repeat(5000), "}".repeat(5000));
        let json = format!("{}\"<script>\"{}", "[".repeat(20_000), "]".repeat(20_000));
        crate::waf::fuzz::inspect(nested.as_bytes());
        crate::waf::fuzz::inspect(json.as_bytes());
        crate::waf::fuzz::inspect("&#60;".repeat(20_000).as_bytes());
    });
}

#[test]
#[cfg(feature = "http2")]
fn fleet_signature_on_long_input() {
    on_small_stack(|| {
        let sig = format!("rws-fleet-sig v2 1 {}", "A".repeat(4_000));
        assert!(crate::fleet::verify(b"x", sig.as_bytes(), &[7u8; 32]).is_err());
    });
}

#[cfg(feature = "serde")]
#[test]
fn urlencoded_with_tens_of_thousands_of_pairs() {
    on_small_stack(|| {
        // Distinct keys, one repeated key, and pairs without values.
        let distinct: String = (0..50_000).map(|i| format!("k{i}=v&")).collect();
        let repeated = "tag=a&".repeat(50_000);
        let bare = "&=&x".repeat(50_000);
        for input in [distinct, repeated, bare] {
            crate::handler::__private::fuzz_urlencoded(&input);
        }
    });
}

#[test]
fn app_manifest_with_thousands_of_lines() {
    on_small_stack(|| {
        // Up to the size limit: comments, unknown tables and keys, brackets.
        let mut text = String::from("[paths]\n");
        while text.len() < crate::app_manifest::MAX_MANIFEST_SIZE - 64 {
            text.push_str("# c\n[x]\nk = \"[[v]]\"\n");
        }
        let manifest = crate::app_manifest::AppManifest::parse(&text).unwrap();
        assert!(manifest.warnings.len() > 1000);
    });
}

#[test]
fn test_client_sse_parser_on_hundreds_of_thousands_of_lines() {
    on_small_stack(|| {
        let mut parser = crate::test_client::SseParser::new();
        let events = parser.feed("data: x\n\n".repeat(200_000).as_bytes()).unwrap();
        assert_eq!(200_000, events.len());
        assert!(parser.feed(": ping\r\n".repeat(200_000).as_bytes()).unwrap().is_empty());
        let joined = parser.feed(format!("{}\n", "data: y\n".repeat(100_000)).as_bytes()).unwrap();
        assert_eq!(1, joined.len());
    });
}

#[test]
fn generator_scanners_on_deep_nesting_and_many_lines() {
    on_small_stack(|| {
        // `rws gen` reads lib.rs, mod.rs, Cargo.toml, routes and tests: loops
        // only, whatever the nesting.
        let mut text = String::from("pub fn app() -> AppWithState<");
        text.push_str(&"Vec<".repeat(50_000));
        text.push_str(&">".repeat(50_000));
        text.push_str("> {\n    routes::todos::register(");
        text.push_str(&"(".repeat(100_000));
        text.push_str(&")".repeat(100_000));
        text.push_str(")\n}\n");
        for _ in 0..20_000 {
            text.push_str("pub mod m;\n#[get(\"/x\")]\nrust-web-server = { features = [[[[ }\n");
        }
        crate::generator::fuzz(&text);
    });
}
