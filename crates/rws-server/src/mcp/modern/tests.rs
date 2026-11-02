use super::*;
use crate::application::Application;
use crate::mcp::{McpContent, PromptReply};

const V: &str = "2026-07-28";

fn server() -> McpServer {
    McpServer::new("modern-test", "1.0")
        .tool("echo", "Echoes", r#"{"type":"object"}"#, |args| Ok(McpContent::text(args.to_string())))
        .tool_with_context("ask", "Asks your name", r#"{"type":"object"}"#, |ctx, _| {
            Ok(match (ctx.elicitation_content("name"), ctx.request_state()) {
                (Some(content), Some("s1")) => McpContent::text(format!("hello {content}")),
                _ => McpContent::input_required(
                    InputRequired::new()
                        .elicit("name", "Name?", r#"{"type":"object","properties":{"n":{"type":"string"}}}"#)
                        .state("s1"),
                ),
            })
        })
        .tool_with_context("needs_sampling", "Samples", r#"{"type":"object"}"#, |_ctx, _| {
            let request = SamplingRequest { messages: vec![PromptMessage::user("hi")], max_tokens: 5, system_prompt: None };
            Ok(McpContent::input_required(InputRequired::new().sample("q", &request)))
        })
        .tool(
            "weather",
            "Weather by region",
            r#"{"type":"object","properties":{"region":{"type":"string","x-mcp-header":"Region"},"opts":{"type":"object","properties":{"days":{"type":"integer","x-mcp-header":"Days"}}}}}"#,
            |args| Ok(McpContent::text(args.to_string())),
        )
        .resource("test://a", "a", "A", |_| Ok(McpContent::text("A")))
        .prompt_with_context("p", "Asks first", vec![], |ctx, _| {
            Ok(match ctx.input_response("c") {
                Some(_) => vec![PromptMessage::user("done")].into(),
                None => PromptReply::InputRequired(InputRequired::new().list_roots("c")),
            })
        })
}

fn meta(capabilities: &str) -> String {
    format!(r#""_meta":{{"io.modelcontextprotocol/protocolVersion":"{V}","io.modelcontextprotocol/clientCapabilities":{capabilities}}}"#)
}

fn connection() -> crate::server::ConnectionInfo {
    crate::server::ConnectionInfo {
        client: crate::server::Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: crate::server::Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 10_000,
        sni_hostname: None,
        tls: false,
    }
}

/// A modern POST to a fresh [`server`].
fn post(headers: &[(&str, &str)], body: &str) -> (u16, String) {
    post_to(&server(), headers, body)
}

/// A modern POST: `headers` beside Content-Type, `body` as given.
fn post_to(server: &McpServer, headers: &[(&str, &str)], body: &str) -> (u16, String) {
    let mut all = vec![
        crate::header::Header { name: "Content-Type".into(), value: "application/json".into() },
        crate::header::Header { name: "Accept".into(), value: "application/json, text/event-stream".into() },
    ];
    all.extend(headers.iter().map(|(n, v)| crate::header::Header { name: n.to_string().into(), value: v.to_string().into() }));
    let request = Request {
        method: "POST".to_string(),
        request_uri: "/mcp".to_string(),
        http_version: crate::http::VERSION.http_1_1.to_string(),
        headers: all,
        body: body.as_bytes().to_vec().into(),
    };
    let response = server.execute(&request, &connection()).unwrap();
    let body = response.content_range_list.first().map(|c| String::from_utf8_lossy(&c.body).into_owned()).unwrap_or_default();
    (response.status_code as u16, body)
}

/// A well-formed modern request for `method` with `params` (without `_meta`).
fn call(method: &str, name: Option<&str>, params: &str, capabilities: &str) -> (u16, String) {
    call_on(&server(), method, name, params, capabilities)
}

/// [`call`] on `server` (the same one across a round trip: its key signs
/// the state).
fn call_on(server: &McpServer, method: &str, name: Option<&str>, params: &str, capabilities: &str) -> (u16, String) {
    let mut headers = vec![("MCP-Protocol-Version", V), ("Mcp-Method", method)];
    if let Some(name) = name {
        headers.push(("Mcp-Name", name));
    }
    let comma = if params.is_empty() { "" } else { "," };
    post_to(server, &headers, &format!(r#"{{"jsonrpc":"2.0","id":7,"method":"{method}","params":{{{params}{comma}{}}}}}"#, meta(capabilities)))
}

#[test]
fn modern_requests_are_told_apart_from_session_ones() {
    let request = |header: Option<&str>| Request {
        method: "POST".to_string(),
        request_uri: "/mcp".to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: header.map(|v| vec![crate::header::Header { name: "MCP-Protocol-Version".into(), value: v.to_string().into() }]).unwrap_or_default(),
        body: Default::default(),
    };
    let with_meta = format!(r#"{{"method":"tools/list","params":{{{}}}}}"#, meta("{}"));
    assert!(is_modern(&request(Some(V)), "{}"));
    assert!(is_modern(&request(Some("v999")), "{}"));
    assert!(is_modern(&request(None), &with_meta));
    assert!(!is_modern(&request(Some("2025-11-25")), &with_meta), "a session's requests stay legacy");
    assert!(!is_modern(&request(None), r#"{"method":"initialize"}"#));
}

#[test]
fn discover_lists_every_version_and_decorates_the_result() {
    let (status, body) = call("server/discover", None, "", "{}");
    assert_eq!(200, status, "{body}");
    assert!(body.contains(r#""resultType":"complete""#), "{body}");
    assert!(body.contains(r#""supportedVersions":["2026-07-28","2025-11-25","2025-06-18","2025-03-26","2024-11-05"]"#), "{body}");
    assert!(body.contains(r#""io.modelcontextprotocol/serverInfo":{"name":"modern-test","version":"1.0"}"#), "{body}");
    assert!(body.contains(r#""ttlMs":0,"cacheScope":"private""#), "{body}");
}

#[test]
fn validation_errors_have_their_codes_and_statuses() {
    let body = |meta: &str| format!(r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{{{meta}}}}}"#);
    let headers = [("MCP-Protocol-Version", V), ("Mcp-Method", "tools/list")];
    // _meta missing, or without its required fields: -32602, 400.
    for params in ["", r#""_meta":{"io.modelcontextprotocol/clientCapabilities":{}}"#, &format!(r#""_meta":{{"io.modelcontextprotocol/protocolVersion":"{V}"}}"#)] {
        let (status, body) = post(&headers, &body(params));
        assert_eq!(400, status, "{body}");
        assert!(body.contains(r#""code":-32602"#), "{body}");
    }
    // Header and body disagree on the version, or on the method: -32020.
    let (status, reply) = post(&[("MCP-Protocol-Version", "2026-12-01"), ("Mcp-Method", "tools/list")], &body(&meta("{}")));
    assert_eq!((400, true), (status, reply.contains("-32020")), "{reply}");
    let (status, reply) = post(&[("MCP-Protocol-Version", V), ("Mcp-Method", "prompts/list")], &body(&meta("{}")));
    assert_eq!((400, true), (status, reply.contains("-32020")), "{reply}");
    // An unknown version: -32022 with what's supported and what was asked.
    let unknown = body(r#""_meta":{"io.modelcontextprotocol/protocolVersion":"v9","io.modelcontextprotocol/clientCapabilities":{}}"#);
    let (status, reply) = post(&[("MCP-Protocol-Version", "v9"), ("Mcp-Method", "tools/list")], &unknown);
    assert_eq!(400, status);
    assert!(reply.contains(r#""code":-32022"#) && reply.contains(r#""requested":"v9""#) && reply.contains(r#""supported":["2026-07-28""#), "{reply}");
}

#[test]
fn mcp_name_must_match_the_body_and_may_be_base64() {
    assert!(call("tools/call", None, r#""name":"echo","arguments":{}"#, "{}").1.contains("-32020"), "Mcp-Name required");
    assert!(call("tools/call", Some("other"), r#""name":"echo","arguments":{}"#, "{}").1.contains("-32020"));
    let (status, body) = call("tools/call", Some("=?base64?ZWNobw==?="), r#""name":"echo","arguments":{"x":1}"#, "{}");
    assert_eq!(200, status, "{body}");
    assert!(body.contains(r#""resultType":"complete""#) && body.contains(r#"{\"x\":1}"#), "{body}");
    assert!(!body.contains("ttlMs"), "tool results aren't cacheable: {body}");
}

#[test]
fn removed_and_unknown_methods_are_404() {
    for method in ["initialize", "ping", "logging/setLevel", "resources/subscribe", "nope/nothing"] {
        let (status, body) = call(method, None, "", "{}");
        assert_eq!(404, status, "{method}: {body}");
        assert!(body.contains(r#""code":-32601"#) && body.contains(r#""id":7"#), "{method}: {body}");
    }
}

#[test]
fn input_required_round_trip_with_a_signed_state() {
    let server = server();
    let (status, first) = call_on(&server, "tools/call", Some("ask"), r#""name":"ask","arguments":{}"#, r#"{"elicitation":{}}"#);
    assert_eq!(200, status, "{first}");
    assert!(first.contains(r#""resultType":"input_required""#) && first.contains(r#""method":"elicitation/create""#), "{first}");
    let state = json_rpc::extract_str(&first, "requestState").expect("a requestState");
    assert_ne!("s1", state, "the state is signed, not sent as is");
    let retry = |state: &str| {
        let params = format!(r#""name":"ask","arguments":{{}},"inputResponses":{{"name":{{"action":"accept","content":{{"n":"Ann"}}}}}},"requestState":"{state}""#);
        call_on(&server, "tools/call", Some("ask"), &params, r#"{"elicitation":{}}"#).1
    };
    let done = retry(&state);
    assert!(done.contains(r#""resultType":"complete""#) && done.contains(r#"hello {\"n\":\"Ann\"}"#), "{done}");
    // An altered state is refused, not passed to the handler.
    let mut tampered = state.clone();
    tampered.insert(3, 'x');
    let refused = retry(&tampered);
    assert!(refused.contains(r#""code":-32602"#) && refused.contains("requestState"), "{refused}");
}

#[test]
fn a_state_for_one_tool_is_refused_by_another() {
    let server = server();
    let state = server.sign_state("tools/call", "ask", "", "s1");
    assert_eq!(Some("s1".to_string()), server.verify_state(&state, "tools/call", "ask", ""));
    assert_eq!(None, server.verify_state(&state, "tools/call", "echo", ""));
    assert_eq!(None, server.verify_state(&state, "prompts/get", "ask", ""));
    assert_eq!(None, server.verify_state(&state, "tools/call", "ask", "someone-else"));
    assert_eq!(None, server.clone().request_state_key("another key").verify_state(&state, "tools/call", "ask", ""));
}

#[test]
fn input_for_an_undeclared_capability_is_refused_with_32021() {
    let (status, body) = call("tools/call", Some("needs_sampling"), r#""name":"needs_sampling","arguments":{}"#, "{}");
    assert_eq!(400, status, "{body}");
    assert!(body.contains(r#""code":-32021"#) && body.contains(r#""requiredCapabilities":{"sampling":{}}"#), "{body}");
}

#[test]
fn prompts_may_ask_for_input_too() {
    let (_, first) = call("prompts/get", Some("p"), r#""name":"p""#, r#"{"roots":{}}"#);
    assert!(first.contains(r#""resultType":"input_required""#) && first.contains(r#""method":"roots/list""#), "{first}");
    let (_, done) = call("prompts/get", Some("p"), r#""name":"p","inputResponses":{"c":{"roots":[]}}"#, r#"{"roots":{}}"#);
    assert!(done.contains(r#""messages":[{"role":"user""#), "{done}");
}

#[test]
fn malformed_input_responses_are_invalid_params() {
    let (_, body) = call("tools/call", Some("ask"), r#""name":"ask","arguments":{},"inputResponses":"nope""#, r#"{"elicitation":{}}"#);
    assert!(body.contains(r#""code":-32602"#), "{body}");
}

#[test]
fn a_missing_resource_names_its_uri() {
    let (_, body) = call("resources/read", Some("test://none"), r#""uri":"test://none""#, "{}");
    assert!(body.contains(r#""code":-32602"#) && body.contains(r#""data":{"uri":"test://none"}"#), "{body}");
    let (_, body) = call("resources/read", Some("test://a"), r#""uri":"test://a""#, "{}");
    assert!(body.contains(r#""ttlMs":0"#), "resources/read is cacheable: {body}");
}

#[test]
fn listen_acknowledges_then_delivers_only_what_was_asked() {
    use std::io::Read;
    let server = server();
    let mut stream = server.listen("5", r#"{"notifications":{"toolsListChanged":true,"resourceSubscriptions":["test://a"]}}"#, false);
    let mut read = || {
        let mut buf = [0u8; 2048];
        let n = stream.stream_pipe.as_mut().unwrap().read(&mut buf).unwrap();
        String::from_utf8_lossy(&buf[..n]).into_owned()
    };
    let ack = read();
    assert!(ack.contains("notifications/subscriptions/acknowledged") && ack.contains(r#""io.modelcontextprotocol/subscriptionId":5"#), "{ack}");
    assert!(ack.contains(r#""toolsListChanged":true"#) && ack.contains(r#""resourceSubscriptions":["test://a"]"#), "{ack}");
    server.register_prompt("q", "Q", |_| Ok(vec![])); // not asked for
    server.notify_resource_updated("test://other"); // not asked for
    server.register_tool("t", "T", "{}", |_| Ok(McpContent::text("t")));
    let tools = read();
    assert!(tools.contains("notifications/tools/list_changed") && tools.contains(r#"subscriptionId":5"#), "{tools}");
    server.notify_resource_updated("test://a");
    let updated = read();
    assert!(updated.contains("notifications/resources/updated") && updated.contains(r#""uri":"test://a""#), "{updated}");
}

#[test]
fn notifications_are_202_and_batches_400() {
    let (status, _) = post(&[("MCP-Protocol-Version", V), ("Mcp-Method", "notifications/x")], &format!(r#"{{"jsonrpc":"2.0","method":"notifications/x","params":{{{}}}}}"#, meta("{}")));
    assert_eq!(202, status);
    let (status, _) = post(&[("MCP-Protocol-Version", V)], "[]");
    assert_eq!(400, status);
}

#[test]
fn header_values_decode_the_base64_sentinel() {
    assert_eq!(Some("Hello, 世界".to_string()), header_value("=?base64?SGVsbG8sIOS4lueVjA==?="));
    assert_eq!(Some("plain".to_string()), header_value("plain"));
    assert_eq!(None, header_value("bad\u{1}value"));
    assert_eq!(None, header_value("=?base64?!!!?="));
}

#[test]
fn header_params_follow_only_properties() {
    let schema = r#"{"type":"object","properties":{"a":{"type":"string","x-mcp-header":"A"},"o":{"type":"object","properties":{"b":{"type":"integer","x-mcp-header":"B"}}},"l":{"type":"array","items":{"x-mcp-header":"C"}}}}"#;
    let mut params = header_params(schema);
    params.sort();
    assert_eq!(vec![(vec!["a".to_string()], "A".to_string()), (vec!["o".to_string(), "b".to_string()], "B".to_string())], params);
}

/// `tools/call weather` with `arguments` and extra headers.
fn weather(arguments: &str, headers: &[(&str, &str)]) -> (u16, String) {
    let mut all = vec![("MCP-Protocol-Version", V), ("Mcp-Method", "tools/call"), ("Mcp-Name", "weather")];
    all.extend_from_slice(headers);
    post(&all, &format!(r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"weather","arguments":{arguments},{}}}}}"#, meta("{}")))
}

#[test]
fn param_headers_must_match_the_arguments() {
    let ok = |arguments: &str, headers: &[(&str, &str)]| weather(arguments, headers).0 == 200;
    assert!(ok(r#"{"region":"us-west1"}"#, &[("Mcp-Param-Region", "us-west1")]));
    assert!(ok(r#"{"region":"Hello"}"#, &[("mcp-param-region", "=?base64?SGVsbG8=?=")]), "base64, header names case-insensitive");
    assert!(ok(r#"{"region":"x","opts":{"days":3}}"#, &[("Mcp-Param-Region", "x"), ("Mcp-Param-Days", "3.0")]), "integers compared as numbers");
    assert!(ok(r#"{"region":null}"#, &[]), "a null argument needs no header");
    assert!(ok("{}", &[]), "nor an absent one");
    for (arguments, headers) in [
        (r#"{"region":"us-west1"}"#, &[][..]),
        (r#"{"region":"us-west1"}"#, &[("Mcp-Param-Region", "eu-west1")][..]),
        (r#"{"region":"Hello"}"#, &[("Mcp-Param-Region", "=?base64?SGVsbG8?=")][..]),
        (r#"{"region":"Hello"}"#, &[("Mcp-Param-Region", "=?base64?SGV!bG8=?=")][..]),
        (r#"{"region":"x","opts":{"days":3}}"#, &[("Mcp-Param-Region", "x"), ("Mcp-Param-Days", "4")][..]),
    ] {
        let (status, body) = weather(arguments, headers);
        assert_eq!((400, true), (status, body.contains(r#""code":-32020"#)), "{arguments} {headers:?}: {body}");
    }
}
