use super::json_rpc;
use super::{extract_arg, json_escape, McpContent, McpServer, PromptArgDef, PromptMessage, ToolAnnotations, PROTOCOL_VERSION};
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

// ── json_rpc::split_array_elements ────────────────────────────────────────────

#[test]
fn split_array_elements_simple() {
    let elems = json_rpc::split_array_elements(r#"[{"a":1},{"b":2}]"#);
    assert_eq!(elems, vec![r#"{"a":1}"#, r#"{"b":2}"#]);
}

#[test]
fn split_array_elements_ignores_commas_inside_nested_objects_and_strings() {
    let elems = json_rpc::split_array_elements(
        r#"[{"method":"tools/call","params":{"a":1,"b":2}},{"text":"a, b, c"}]"#,
    );
    assert_eq!(elems.len(), 2, "expected exactly 2 top-level elements: {elems:?}");
    assert!(elems[0].contains(r#""a":1,"b":2"#));
    assert!(elems[1].contains("a, b, c"));
}

#[test]
fn split_array_elements_empty_array_returns_empty_vec() {
    assert!(json_rpc::split_array_elements("[]").is_empty());
}

#[test]
fn split_array_elements_single_element() {
    let elems = json_rpc::split_array_elements(r#"[{"jsonrpc":"2.0","method":"ping","id":1}]"#);
    assert_eq!(elems, vec![r#"{"jsonrpc":"2.0","method":"ping","id":1}"#]);
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

#[test]
fn initialize_negotiates_down_to_server_version_for_a_newer_client() {
    // Client asks for a version newer than this server implements — the
    // server must not just echo it back; it can only ever speak the version
    // it actually implements, which is the lower of the two here.
    let srv = make_server();
    let resp = srv.handle_request(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-06-18","clientInfo":{"name":"test","version":"1"}}}"#,
    );
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(
        body.contains(&format!("\"protocolVersion\":\"{PROTOCOL_VERSION}\"")),
        "should negotiate down to the server's own version: {body}"
    );
}

#[test]
fn initialize_honors_an_older_client_version() {
    // Client asks for a version older than the server's — "lower of the two"
    // means the server confirms it'll speak the client's (older) version,
    // rather than insisting on its own newer one.
    let srv = make_server();
    let resp = srv.handle_request(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2023-01-01","clientInfo":{"name":"test","version":"1"}}}"#,
    );
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains("\"protocolVersion\":\"2023-01-01\""), "should honor the older client version: {body}");
}

#[test]
fn initialize_matching_client_version_is_echoed() {
    let srv = make_server();
    let resp = srv.handle_request(&format!(
        r#"{{"jsonrpc":"2.0","method":"initialize","id":1,"params":{{"protocolVersion":"{PROTOCOL_VERSION}"}}}}"#
    ));
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains(&format!("\"protocolVersion\":\"{PROTOCOL_VERSION}\"")));
}

#[test]
fn initialize_without_protocol_version_defaults_to_server_version() {
    let srv = make_server();
    let resp = srv.handle_request(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"clientInfo":{"name":"test","version":"1"}}}"#,
    );
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains(&format!("\"protocolVersion\":\"{PROTOCOL_VERSION}\"")));
}

#[test]
fn initialize_without_params_at_all_does_not_error() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#);
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains(&format!("\"protocolVersion\":\"{PROTOCOL_VERSION}\"")));
}

// ── McpContext / sessions ───────────────────────────────────────────────────────

fn make_context_server() -> McpServer {
    McpServer::new("test-srv", "0.1").tool_with_context(
        "whoami",
        "Report the caller's client info",
        r#"{"type":"object"}"#,
        |ctx, _args| {
            Ok(McpContent::text(format!(
                "name={} version={} session={}",
                ctx.client_name.as_deref().unwrap_or("?"),
                ctx.client_version.as_deref().unwrap_or("?"),
                ctx.session_id.as_deref().unwrap_or("?"),
            )))
        },
    )
}

#[test]
fn initialize_returns_mcp_session_id_header() {
    let srv = make_context_server();
    let resp = srv.handle_request(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"clientInfo":{"name":"test","version":"1"}}}"#,
    );
    assert_eq!(resp.status_code, 200);
    let session_header = resp.headers.iter().find(|h| h.name.eq_ignore_ascii_case("Mcp-Session-Id"));
    assert!(session_header.is_some(), "expected an Mcp-Session-Id response header");
    assert!(!session_header.unwrap().value.is_empty());
}

#[test]
fn two_initialize_calls_get_different_session_ids() {
    let srv = make_context_server();
    let resp1 = srv.handle_request(r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}"#);
    let resp2 = srv.handle_request(r#"{"jsonrpc":"2.0","method":"initialize","id":2,"params":{}}"#);
    let id1 = resp1.headers.iter().find(|h| h.name.eq_ignore_ascii_case("Mcp-Session-Id")).unwrap().value.clone();
    let id2 = resp2.headers.iter().find(|h| h.name.eq_ignore_ascii_case("Mcp-Session-Id")).unwrap().value.clone();
    assert_ne!(id1, id2, "each initialize should mint its own session id");
}

#[test]
fn handle_request_without_context_gives_tool_with_context_an_empty_context() {
    // handle_request() (no explicit McpContext) is what all the other tests
    // in this file use — it must still work for a tool_with_context handler,
    // just with every field empty.
    let srv = make_context_server();
    let resp = srv.handle_request(
        r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"whoami","arguments":{}}}"#,
    );
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains("name=? version=? session=?"), "expected an empty context: {body}");
}

#[test]
fn tool_with_context_sees_client_info_recorded_at_initialize_via_execute() {
    // The real flow: initialize over HTTP (via execute()), read back the
    // Mcp-Session-Id the server minted, send it on a later tools/call, and
    // confirm the tool sees the clientInfo that was sent at initialize time.
    let client = TestClient::new(make_context_server());

    let init_resp = client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"claude-code","version":"1.2.3"}}}"#)
        .send();
    assert_eq!(init_resp.status(), 200);
    let session_id = init_resp
        .header("Mcp-Session-Id")
        .expect("initialize should return a session id")
        .to_string();

    let call_resp = client
        .post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"whoami","arguments":{}}}"#)
        .send();
    assert_eq!(call_resp.status(), 200);
    let body = call_resp.body_text();
    assert!(body.contains("name=claude-code"), "missing client name: {body}");
    assert!(body.contains("version=1.2.3"), "missing client version: {body}");
    assert!(body.contains(&format!("session={session_id}")), "missing session id: {body}");
}

#[test]
fn tool_with_context_sees_empty_client_info_for_unrecognized_session_id() {
    let client = TestClient::new(make_context_server());
    let resp = client
        .post("/mcp")
        .header("Mcp-Session-Id", "not-a-real-session")
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"whoami","arguments":{}}}"#)
        .send();
    assert_eq!(resp.status(), 200);
    let body = resp.body_text();
    assert!(body.contains("name=? version=?"), "unknown session should have no stored clientInfo: {body}");
    assert!(body.contains("session=not-a-real-session"), "session_id should still echo the header sent: {body}");
}

#[test]
fn plain_tool_still_works_unaffected_by_context_plumbing() {
    // Regression guard: .tool() (not .tool_with_context()) must keep
    // ignoring context entirely and behave exactly as before.
    let srv = make_server();
    let resp = srv.handle_request(
        r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"echo","arguments":{"text":"hi"}}}"#,
    );
    assert_eq!(resp.status_code, 200);
    assert!(body_of(&resp).contains("hi"));
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

#[test]
fn tools_list_plain_tool_has_no_annotations_key() {
    // Regression guard: a tool registered via plain .tool() must not gain an
    // "annotations" key just because some other tool on the same server has one.
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    let body = body_of(&resp);
    assert!(!body.contains("\"annotations\""), "unexpected annotations key: {body}");
}

#[test]
fn tools_list_annotated_tool_serializes_camel_case_hints() {
    let srv = McpServer::new("test-srv", "0.1").tool_annotated(
        "delete_file",
        "Delete a file",
        r#"{"type":"object"}"#,
        ToolAnnotations {
            destructive_hint: Some(true),
            read_only_hint: Some(false),
            idempotent_hint: Some(true),
            open_world_hint: None,
        },
        |_args| Ok(McpContent::text("deleted")),
    );
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    let body = body_of(&resp);
    assert!(body.contains("\"annotations\":{"), "missing annotations block: {body}");
    assert!(body.contains("\"destructiveHint\":true"), "missing destructiveHint: {body}");
    assert!(body.contains("\"readOnlyHint\":false"), "missing readOnlyHint: {body}");
    assert!(body.contains("\"idempotentHint\":true"), "missing idempotentHint: {body}");
    assert!(!body.contains("\"openWorldHint\""), "openWorldHint should be omitted when None: {body}");
}

#[test]
fn tools_list_annotated_tool_with_all_hints_none_emits_empty_object() {
    let srv = McpServer::new("test-srv", "0.1").tool_annotated(
        "noop",
        "Does nothing",
        r#"{"type":"object"}"#,
        ToolAnnotations::default(),
        |_args| Ok(McpContent::text("ok")),
    );
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    let body = body_of(&resp);
    assert!(body.contains("\"annotations\":{}"), "expected empty annotations object: {body}");
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

#[test]
fn tools_call_image_content_serializes_data_and_mime_type() {
    let srv = McpServer::new("test-srv", "0.1").tool(
        "screenshot",
        "Return a screenshot",
        r#"{"type":"object"}"#,
        |_args| Ok(McpContent::image("QUJD", "image/png")),
    );
    let req = r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"screenshot","arguments":{}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains(r#""type":"image""#), "missing image type: {body}");
    assert!(body.contains(r#""data":"QUJD""#), "missing base64 data: {body}");
    assert!(body.contains(r#""mimeType":"image/png""#), "missing mimeType: {body}");
    assert!(!body.contains("\"text\":"), "image content should not have a text field: {body}");
}

#[test]
fn tools_call_embedded_resource_serializes_uri_mime_type_and_text() {
    let srv = McpServer::new("test-srv", "0.1").tool(
        "fetch_doc",
        "Return an embedded doc",
        r#"{"type":"object"}"#,
        |_args| Ok(McpContent::embedded("docs://readme", "hello docs", "text/markdown")),
    );
    let req = r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"fetch_doc","arguments":{}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains(r#""type":"resource""#), "missing resource type: {body}");
    assert!(body.contains(r#""uri":"docs://readme""#), "missing uri: {body}");
    assert!(body.contains(r#""mimeType":"text/markdown""#), "missing mimeType: {body}");
    assert!(body.contains(r#""text":"hello docs""#), "missing embedded text: {body}");
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

// ── batch requests ────────────────────────────────────────────────────────────

#[test]
fn batch_dispatches_each_element_and_wraps_results_in_an_array() {
    let srv = make_server();
    let req = r#"[{"jsonrpc":"2.0","method":"tools/list","id":1},
                  {"jsonrpc":"2.0","method":"ping","id":2}]"#;
    let resp = srv.handle_request(req);
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.starts_with('['), "expected a JSON array response: {body}");
    assert!(body.contains(r#""id":1"#), "missing response for id 1: {body}");
    assert!(body.contains(r#""id":2"#), "missing response for id 2: {body}");
    assert!(body.contains("\"echo\""), "tools/list result missing from batch: {body}");
}

#[test]
fn batch_preserves_per_element_success_and_error_results() {
    let srv = make_server();
    let req = r#"[{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"echo","arguments":{"text":"hi"}}},
                  {"jsonrpc":"2.0","method":"no/such/method","id":2}]"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains("\"isError\":false"), "expected the echo call to succeed: {body}");
    assert!(body.contains("-32601"), "expected METHOD_NOT_FOUND for the second element: {body}");
}

#[test]
fn batch_omits_notifications_from_the_response_array() {
    let srv = make_server();
    // Only the first element has an id; the second is a notification.
    let req = r#"[{"jsonrpc":"2.0","method":"ping","id":1},
                  {"jsonrpc":"2.0","method":"notifications/initialized"}]"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains(r#""id":1"#), "expected a response entry for the ping: {body}");
    // Only one entry in the array — the notification contributed nothing.
    assert_eq!(body.matches("\"jsonrpc\"").count(), 1, "notification leaked an entry: {body}");
}

#[test]
fn batch_of_only_notifications_returns_202_with_no_body() {
    let srv = make_server();
    let req = r#"[{"jsonrpc":"2.0","method":"notifications/initialized"}]"#;
    let resp = srv.handle_request(req);
    assert_eq!(resp.status_code, 202);
    assert!(body_of(&resp).is_empty());
}

#[test]
fn empty_batch_array_returns_a_single_invalid_request_error() {
    let srv = make_server();
    let resp = srv.handle_request("[]");
    let body = body_of(&resp);
    assert!(!body.starts_with('['), "empty batch should not produce an array response: {body}");
    assert!(body.contains("-32600"), "expected INVALID_REQUEST code: {body}");
}

#[test]
fn batch_initialize_establishes_a_session_via_mcp_session_id_header() {
    let srv = make_server();
    let req = r#"[{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"batch-client","version":"1"}}},
                  {"jsonrpc":"2.0","method":"ping","id":2}]"#;
    let resp = srv.handle_request(req);
    let session_header = resp.headers.iter().find(|h| h.name == "Mcp-Session-Id");
    assert!(session_header.is_some(), "expected Mcp-Session-Id header on a batch containing initialize");
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
