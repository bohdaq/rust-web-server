use super::json_rpc;
use super::{extract_arg, json_escape, McpContent, McpServer, PromptArgDef, PromptMessage};
use crate::app::App;
use crate::core::New;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::test_client::TestClient;

// ── json_rpc::extract_str ─────────────────────────────────────────────────────

#[test]
fn extract_str_simple() {
    let j = r#"{"method":"tools/list","id":1}"#;
    assert_eq!(json_rpc::extract_str(j, "method").as_deref(), Some("tools/list"));
}

#[test]
fn extract_str_escaped_quotes() {
    let j = r#"{"text":"say \"hi\""}"#;
    assert_eq!(json_rpc::extract_str(j, "text").as_deref(), Some(r#"say "hi""#));
}

#[test]
fn extract_str_missing_key() {
    assert!(json_rpc::extract_str(r#"{"foo":"bar"}"#, "baz").is_none());
}

#[test]
fn extract_str_does_not_match_substrings() {
    // "clientId" must not match when looking for "id"
    let j = r#"{"clientId":"xyz","method":"ping"}"#;
    assert_eq!(json_rpc::extract_str(j, "method").as_deref(), Some("ping"));
    // "id" absent → None
    assert!(json_rpc::extract_str(j, "id").is_none());
}

// ── json_rpc::extract_raw ─────────────────────────────────────────────────────

#[test]
fn extract_raw_object() {
    let j = r#"{"params":{"name":"echo","arguments":{"text":"hi"}}}"#;
    let params = json_rpc::extract_raw(j, "params").unwrap();
    assert!(params.starts_with('{'));
    assert!(params.ends_with('}'));
    assert!(params.contains("\"name\""));
}

#[test]
fn extract_raw_number() {
    let j = r#"{"id":42,"method":"ping"}"#;
    assert_eq!(json_rpc::extract_raw(j, "id").as_deref(), Some("42"));
}

#[test]
fn extract_raw_nested_objects() {
    let j = r#"{"params":{"a":{"b":{"c":1}}}}"#;
    let params = json_rpc::extract_raw(j, "params").unwrap();
    assert_eq!(params, r#"{"a":{"b":{"c":1}}}"#);
}

#[test]
fn extract_raw_string_value() {
    let j = r#"{"method":"tools/list"}"#;
    assert_eq!(json_rpc::extract_raw(j, "method").as_deref(), Some(r#""tools/list""#));
}

// ── json_rpc::extract_id ─────────────────────────────────────────────────────

#[test]
fn extract_id_number() {
    let j = r#"{"jsonrpc":"2.0","method":"ping","id":7}"#;
    assert_eq!(json_rpc::extract_id(j).as_deref(), Some("7"));
}

#[test]
fn extract_id_string() {
    let j = r#"{"jsonrpc":"2.0","method":"ping","id":"req-1"}"#;
    assert_eq!(json_rpc::extract_id(j).as_deref(), Some("\"req-1\""));
}

#[test]
fn extract_id_absent_is_none() {
    // Notification — no id
    let j = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    assert!(json_rpc::extract_id(j).is_none());
}

#[test]
fn extract_id_null_returns_null_string() {
    let j = r#"{"jsonrpc":"2.0","method":"ping","id":null}"#;
    assert_eq!(json_rpc::extract_id(j).as_deref(), Some("null"));
}

// ── json_escape ───────────────────────────────────────────────────────────────

#[test]
fn json_escape_quotes_and_backslash() {
    assert_eq!(json_escape("say \"hi\" \\o/"), r#"say \"hi\" \\o/"#);
}

#[test]
fn json_escape_newlines() {
    assert_eq!(json_escape("line1\nline2"), r"line1\nline2");
}

#[test]
fn json_escape_plain_text_unchanged() {
    assert_eq!(json_escape("hello world"), "hello world");
}

// ── extract_arg ───────────────────────────────────────────────────────────────

#[test]
fn extract_arg_present() {
    assert_eq!(
        extract_arg(r#"{"text":"hello","count":"3"}"#, "text").as_deref(),
        Some("hello")
    );
}

#[test]
fn extract_arg_absent_returns_none() {
    assert!(extract_arg(r#"{"text":"hi"}"#, "missing").is_none());
}

// ── McpServer::handle_request ─────────────────────────────────────────────────

fn make_server() -> McpServer {
    McpServer::new("test-srv", "0.1")
        .tool(
            "echo",
            "Echo text back",
            r#"{"type":"object","properties":{"text":{"type":"string"}}}"#,
            |args| {
                let text = extract_arg(args, "text").unwrap_or_else(|| "(empty)".to_string());
                Ok(McpContent::text(text))
            },
        )
        .tool(
            "fail",
            "Always errors",
            r#"{"type":"object"}"#,
            |_| Err("something went wrong".to_string()),
        )
        .resource(
            "docs://{topic}",
            "Documentation",
            "Return docs for a topic",
            |uri| Ok(McpContent::text(format!("docs for {uri}"))),
        )
        .prompt(
            "summarize",
            "Summarize text",
            |args| {
                let text = extract_arg(args, "text").unwrap_or_default();
                Ok(vec![PromptMessage::user(format!("Please summarize: {text}"))])
            },
        )
        .prompt_with_args(
            "translate",
            "Translate to another language",
            vec![
                PromptArgDef::required("text", "Text to translate"),
                PromptArgDef::optional("lang", "Target language"),
            ],
            |args| {
                let text = extract_arg(args, "text").unwrap_or_default();
                let lang = extract_arg(args, "lang").unwrap_or_else(|| "Spanish".to_string());
                Ok(vec![PromptMessage::user(format!("Translate to {lang}: {text}"))])
            },
        )
}

fn body_of(resp: &crate::response::Response) -> String {
    resp.content_range_list
        .first()
        .map(|cr| String::from_utf8_lossy(&cr.body).into_owned())
        .unwrap_or_default()
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn initialize_returns_protocol_version() {
    let srv = make_server();
    let resp = srv.handle_request(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
    );
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains("\"protocolVersion\""), "missing protocolVersion: {body}");
    assert!(body.contains("2024-11-05"), "wrong version: {body}");
    assert!(body.contains("\"serverInfo\""), "missing serverInfo: {body}");
    assert!(body.contains("test-srv"), "missing server name: {body}");
}

// ── notifications ─────────────────────────────────────────────────────────────

#[test]
fn initialized_notification_returns_202() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    assert_eq!(resp.status_code, 202);
    assert!(resp.content_range_list.is_empty());
}

// ── ping ──────────────────────────────────────────────────────────────────────

#[test]
fn ping_returns_empty_result() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"ping","id":2}"#);
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains("\"result\":{}"), "expected empty result: {body}");
}

// ── tools/list ────────────────────────────────────────────────────────────────

#[test]
fn tools_list_contains_registered_tools() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":3}"#);
    let body = body_of(&resp);
    assert!(body.contains("\"echo\""), "echo missing: {body}");
    assert!(body.contains("\"fail\""), "fail missing: {body}");
    assert!(body.contains("\"inputSchema\""), "no inputSchema: {body}");
}

#[test]
fn tools_list_empty_when_no_tools() {
    let srv = McpServer::new("bare", "1.0");
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    let body = body_of(&resp);
    assert!(body.contains("\"tools\":[]"), "expected empty tools array: {body}");
}

// ── tools/call ────────────────────────────────────────────────────────────────

#[test]
fn tools_call_echo_returns_content() {
    let srv = make_server();
    let req = r#"{"jsonrpc":"2.0","method":"tools/call","id":4,"params":{"name":"echo","arguments":{"text":"hello MCP"}}}"#;
    let resp = srv.handle_request(req);
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains("hello MCP"), "echo result missing: {body}");
    assert!(body.contains("\"isError\":false"), "should not be error: {body}");
}

#[test]
fn tools_call_error_returns_is_error_true() {
    let srv = make_server();
    let req = r#"{"jsonrpc":"2.0","method":"tools/call","id":5,"params":{"name":"fail","arguments":{}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains("\"isError\":true"), "should be error: {body}");
    assert!(body.contains("something went wrong"), "error text missing: {body}");
}

#[test]
fn tools_call_unknown_tool_returns_error() {
    let srv = make_server();
    let req = r#"{"jsonrpc":"2.0","method":"tools/call","id":6,"params":{"name":"no_such_tool","arguments":{}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains("\"error\""), "should be JSON-RPC error: {body}");
    assert!(body.contains("no_such_tool") || body.contains("Unknown tool"), "should mention tool: {body}");
}

// ── resources/list ────────────────────────────────────────────────────────────

#[test]
fn resources_list_contains_registered_resource() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"resources/list","id":7}"#);
    let body = body_of(&resp);
    assert!(body.contains("docs://"), "uri template missing: {body}");
    assert!(body.contains("Documentation"), "name missing: {body}");
}

// ── resources/read ────────────────────────────────────────────────────────────

#[test]
fn resources_read_returns_content() {
    let srv = make_server();
    let req = r#"{"jsonrpc":"2.0","method":"resources/read","id":8,"params":{"uri":"docs://mcp-intro"}}"#;
    let resp = srv.handle_request(req);
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains("docs://mcp-intro"), "uri in response: {body}");
    assert!(body.contains("docs for"), "content text: {body}");
}

#[test]
fn resources_read_unknown_uri_returns_error() {
    let srv = make_server();
    let req = r#"{"jsonrpc":"2.0","method":"resources/read","id":9,"params":{"uri":"unknown://xyz"}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains("\"error\""), "should be error: {body}");
}

// ── prompts/list ──────────────────────────────────────────────────────────────

#[test]
fn prompts_list_contains_registered_prompts() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"prompts/list","id":10}"#);
    let body = body_of(&resp);
    assert!(body.contains("\"summarize\""), "summarize missing: {body}");
    assert!(body.contains("\"translate\""), "translate missing: {body}");
}

#[test]
fn prompts_list_includes_argument_definitions() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"prompts/list","id":11}"#);
    let body = body_of(&resp);
    assert!(body.contains("\"text\""), "arg name missing: {body}");
    assert!(body.contains("\"required\":true"), "required flag missing: {body}");
}

// ── prompts/get ───────────────────────────────────────────────────────────────

#[test]
fn prompts_get_returns_messages() {
    let srv = make_server();
    let req = r#"{"jsonrpc":"2.0","method":"prompts/get","id":12,"params":{"name":"summarize","arguments":{"text":"the quick brown fox"}}}"#;
    let resp = srv.handle_request(req);
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains("\"messages\""), "messages array missing: {body}");
    assert!(body.contains("quick brown fox"), "text argument missing: {body}");
    assert!(body.contains("\"role\":\"user\""), "role missing: {body}");
}

#[test]
fn prompts_get_unknown_prompt_returns_error() {
    let srv = make_server();
    let req = r#"{"jsonrpc":"2.0","method":"prompts/get","id":13,"params":{"name":"no_such_prompt"}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains("\"error\""), "should be error: {body}");
}

// ── unknown method ────────────────────────────────────────────────────────────

#[test]
fn unknown_method_returns_method_not_found() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"foo/bar","id":99}"#);
    let body = body_of(&resp);
    assert!(body.contains("-32601"), "expected METHOD_NOT_FOUND code: {body}");
}

// ── invalid request ───────────────────────────────────────────────────────────

#[test]
fn missing_method_field_returns_invalid_request() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","id":1}"#);
    let body = body_of(&resp);
    assert!(body.contains("-32600"), "expected INVALID_REQUEST code: {body}");
}

// ── Application impl (via TestClient) ─────────────────────────────────────────

#[test]
fn application_dispatches_post_to_mcp_endpoint() {
    let srv = make_server();
    let client = TestClient::new(srv);
    let resp = client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#)
        .send();
    assert_eq!(resp.status(), 200);
    let body = resp.body_text().to_string();
    assert!(body.contains("\"tools\""), "tools key: {body}");
    assert!(body.contains("\"echo\""), "echo tool: {body}");
}

#[test]
fn application_returns_405_for_get_on_mcp_path() {
    let srv = make_server();
    let client = TestClient::new(srv);
    let resp = client.get("/mcp").send();
    assert_eq!(resp.status(), 405);
}

#[test]
fn application_falls_through_for_non_mcp_path() {
    let srv = make_server();
    let client = TestClient::new(srv);
    let resp = client.get("/healthz").send();
    assert_eq!(resp.status(), 200); // handled by built-in App
}

#[test]
fn mcp_path_override_with_at() {
    let srv = McpServer::new("srv", "1.0")
        .at("/api/v1/mcp")
        .tool("t", "T", "{}", |_| Ok(McpContent::text("ok")));
    let client = TestClient::new(srv);

    // Old path → falls through to App (404 for missing file)
    let resp = client.get("/mcp").send();
    assert_ne!(resp.status(), 200); // not the MCP endpoint

    // New path → handled
    let resp2 = client
        .post("/api/v1/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#)
        .send();
    assert_eq!(resp2.status(), 200);
}

// ── wrap() — compose with existing Application ────────────────────────────────

#[test]
fn wrap_forwards_non_mcp_to_existing_app() {
    // An AppWithState-style app with a custom route.
    let existing = App::with_state(())
        .get("/api/hello", |_req, _params, _conn, _state| {
            let mut r = Response::new();
            r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
            r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
            r
        });

    let srv = McpServer::new("srv", "1.0")
        .tool("ping", "Ping", "{}", |_| Ok(McpContent::text("pong")))
        .wrap(existing);

    let client = TestClient::new(srv);

    // MCP endpoint still works.
    let mcp_resp = client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#)
        .send();
    assert_eq!(mcp_resp.status(), 200);
    assert!(mcp_resp.body_text().contains("ping"), "MCP tool missing");

    // Custom route from existing app also works.
    let api_resp = client.get("/api/hello").send();
    assert_eq!(api_resp.status(), 200, "custom route unreachable");
}

#[test]
fn wrap_mcp_takes_priority_over_wrapped_app() {
    // Even if the wrapped app has a /mcp route, McpServer handles it.
    let competing = App::with_state(())
        .get("/mcp", |_req, _params, _conn, _state| {
            let mut r = Response::new();
            r.status_code = *STATUS_CODE_REASON_PHRASE.n418_im_a_teapot.status_code;
            r.reason_phrase = STATUS_CODE_REASON_PHRASE.n418_im_a_teapot.reason_phrase.to_string();
            r
        });

    let srv = McpServer::new("srv", "1.0").wrap(competing);
    let client = TestClient::new(srv);

    // POST /mcp → MCP server (200), not the wrapped app's 418.
    let resp = client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
        .send();
    assert_eq!(resp.status(), 200);
}

// ── require_bearer auth ───────────────────────────────────────────────────────

fn make_protected_server() -> McpServer {
    McpServer::new("srv", "1.0")
        .require_bearer("secret-token")
        .tool("ping", "Ping", "{}", |_| Ok(McpContent::text("pong")))
}

#[test]
fn auth_correct_token_succeeds() {
    let client = TestClient::new(make_protected_server());
    let resp = client
        .post("/mcp")
        .header("Authorization", "Bearer secret-token")
        .body_text(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
        .send();
    assert_eq!(resp.status(), 200);
}

#[test]
fn auth_missing_token_returns_401() {
    let client = TestClient::new(make_protected_server());
    let resp = client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
        .send();
    assert_eq!(resp.status(), 401);
}

#[test]
fn auth_wrong_token_returns_401() {
    let client = TestClient::new(make_protected_server());
    let resp = client
        .post("/mcp")
        .header("Authorization", "Bearer wrong-token")
        .body_text(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
        .send();
    assert_eq!(resp.status(), 401);
}

#[test]
fn auth_options_preflight_also_requires_token() {
    let client = TestClient::new(make_protected_server());
    let resp = client.options("/mcp").send();
    assert_eq!(resp.status(), 401);
}

#[test]
fn auth_non_mcp_path_not_affected() {
    // Auth only guards /mcp — other paths go to the fallback App unchanged.
    let client = TestClient::new(make_protected_server());
    let resp = client.get("/healthz").send();
    assert_eq!(resp.status(), 200);
}

#[test]
fn auth_www_authenticate_header_present_on_401() {
    let client = TestClient::new(make_protected_server());
    let resp = client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
        .send();
    assert_eq!(resp.status(), 401);
    let has_www_auth = resp.headers().iter()
        .any(|h| h.name.eq_ignore_ascii_case("www-authenticate"));
    assert!(has_www_auth, "WWW-Authenticate header missing on 401");
}

#[test]
fn no_auth_configured_allows_all() {
    // Without require_bearer, any request is accepted.
    let srv = McpServer::new("open", "1.0").tool("ping", "Ping", "{}", |_| Ok(McpContent::text("pong")));
    let client = TestClient::new(srv);
    let resp = client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
        .send();
    assert_eq!(resp.status(), 200);
}
