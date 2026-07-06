use super::json_rpc;
use super::{
    decode_cursor, encode_cursor, extract_arg, json_escape, LogLevel, McpContent, McpContext,
    McpServer, PromptArgDef, PromptMessage, SamplingRequest, ToolAnnotations, PROTOCOL_VERSION,
};
use crate::app::App;
use crate::core::New;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::test_client::TestClient;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

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

// ── pagination cursor encode/decode ───────────────────────────────────────────

#[test]
fn cursor_round_trips_through_encode_and_decode() {
    for offset in [0usize, 1, 50, 12345, usize::MAX] {
        let cursor = encode_cursor(offset);
        assert_eq!(decode_cursor(&cursor), Some(offset), "round trip failed for {offset}");
    }
}

#[test]
fn decode_cursor_rejects_garbage() {
    assert!(decode_cursor("not valid base64 at all!!").is_none());
    assert!(decode_cursor("").is_none());
}

// ── LogLevel ───────────────────────────────────────────────────────────────────

#[test]
fn log_level_parse_round_trips_with_as_str() {
    let levels = [
        LogLevel::Debug, LogLevel::Info, LogLevel::Notice, LogLevel::Warning,
        LogLevel::Error, LogLevel::Critical, LogLevel::Alert, LogLevel::Emergency,
    ];
    for level in levels {
        assert_eq!(LogLevel::parse(level.as_str()), Some(level));
    }
}

#[test]
fn log_level_parse_rejects_unknown_strings() {
    assert!(LogLevel::parse("verbose").is_none());
    assert!(LogLevel::parse("").is_none());
    assert!(LogLevel::parse("INFO").is_none()); // case-sensitive, per spec's lowercase names
}

#[test]
fn log_level_orders_from_debug_to_emergency() {
    assert!(LogLevel::Debug < LogLevel::Info);
    assert!(LogLevel::Info < LogLevel::Notice);
    assert!(LogLevel::Notice < LogLevel::Warning);
    assert!(LogLevel::Warning < LogLevel::Error);
    assert!(LogLevel::Error < LogLevel::Critical);
    assert!(LogLevel::Critical < LogLevel::Alert);
    assert!(LogLevel::Alert < LogLevel::Emergency);
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

// ── pagination ────────────────────────────────────────────────────────────────

fn make_paged_tools_server() -> McpServer {
    McpServer::new("test-srv", "0.1")
        .tool("t1", "Tool 1", "{}", |_| Ok(McpContent::text("1")))
        .tool("t2", "Tool 2", "{}", |_| Ok(McpContent::text("2")))
        .tool("t3", "Tool 3", "{}", |_| Ok(McpContent::text("3")))
        .page_size(2)
}

#[test]
fn tools_list_first_page_returns_page_size_items_and_next_cursor() {
    let srv = make_paged_tools_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    let body = body_of(&resp);
    assert!(body.contains("\"t1\""), "missing t1: {body}");
    assert!(body.contains("\"t2\""), "missing t2: {body}");
    assert!(!body.contains("\"t3\""), "t3 should not be on the first page: {body}");
    assert!(body.contains("\"nextCursor\""), "expected nextCursor on a partial page: {body}");
}

#[test]
fn tools_list_second_page_via_cursor_returns_remainder_with_no_next_cursor() {
    let srv = make_paged_tools_server();
    let first = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    let first_body = body_of(&first);
    let cursor = first_body
        .split("\"nextCursor\":\"").nth(1).unwrap()
        .split('"').next().unwrap()
        .to_string();

    let req = format!(r#"{{"jsonrpc":"2.0","method":"tools/list","id":2,"params":{{"cursor":"{cursor}"}}}}"#);
    let resp = srv.handle_request(&req);
    let body = body_of(&resp);
    assert!(body.contains("\"t3\""), "missing t3 on the second page: {body}");
    assert!(!body.contains("\"t1\""), "t1 should not repeat on the second page: {body}");
    assert!(!body.contains("\"nextCursor\""), "expected no nextCursor on the last page: {body}");
}

#[test]
fn tools_list_invalid_cursor_returns_invalid_params_error() {
    let srv = make_paged_tools_server();
    let req = r#"{"jsonrpc":"2.0","method":"tools/list","id":1,"params":{"cursor":"not valid base64!!"}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains("\"error\""), "expected a JSON-RPC error for an invalid cursor: {body}");
    assert!(body.contains("-32602"), "expected INVALID_PARAMS code: {body}");
}

#[test]
fn tools_list_cursor_past_the_end_returns_an_empty_page() {
    let srv = make_paged_tools_server();
    let far_cursor = super::encode_cursor(100); // well past the 3 registered tools
    let req = format!(r#"{{"jsonrpc":"2.0","method":"tools/list","id":1,"params":{{"cursor":"{far_cursor}"}}}}"#);
    let resp = srv.handle_request(&req);
    let body = body_of(&resp);
    assert!(body.contains("\"tools\":[]"), "expected an empty page past the end: {body}");
    assert!(!body.contains("\"nextCursor\""), "should not offer a nextCursor past the end: {body}");
}

#[test]
fn tools_list_without_page_size_is_unpaginated_and_has_no_next_cursor() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    let body = body_of(&resp);
    assert!(!body.contains("\"nextCursor\""), "should not paginate without page_size: {body}");
}

#[test]
fn resources_list_paginates_when_page_size_set() {
    let srv = McpServer::new("test-srv", "0.1")
        .resource("docs://a", "A", "Doc A", |uri| Ok(McpContent::text(format!("a:{uri}"))))
        .resource("docs://b", "B", "Doc B", |uri| Ok(McpContent::text(format!("b:{uri}"))))
        .page_size(1);
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"resources/list","id":1}"#);
    let body = body_of(&resp);
    assert!(body.contains("\"nextCursor\""), "expected nextCursor on a partial resources page: {body}");
}

#[test]
fn prompts_list_paginates_when_page_size_set() {
    let srv = McpServer::new("test-srv", "0.1")
        .prompt("p1", "Prompt 1", |_| Ok(vec![PromptMessage::user("hi")]))
        .prompt("p2", "Prompt 2", |_| Ok(vec![PromptMessage::user("hi")]))
        .page_size(1);
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"prompts/list","id":1}"#);
    let body = body_of(&resp);
    assert!(body.contains("\"nextCursor\""), "expected nextCursor on a partial prompts page: {body}");
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
fn application_opens_sse_stream_for_get_on_mcp_path() {
    let srv = make_server();
    let client = TestClient::new(srv);
    let resp = client.get("/mcp").send();
    assert_eq!(resp.status(), 200);
    let content_type = resp.headers().iter().find(|h| h.name.eq_ignore_ascii_case("content-type"));
    assert_eq!(content_type.map(|h| h.value.as_str()), Some("text/event-stream"));
}

#[test]
fn application_returns_405_for_delete_on_mcp_path() {
    let srv = make_server();
    let client = TestClient::new(srv);
    let resp = client.delete("/mcp").send();
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
fn auth_protects_the_get_sse_endpoint_too() {
    let client = TestClient::new(make_protected_server());
    let resp = client.get("/mcp").send();
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

// ── SSE streaming (GET /mcp) ───────────────────────────────────────────────────
//
// `TestClient` dispatches through `Application::execute` but never drives
// `Response::stream_pipe` (that's `Server::pipe_stream`'s job, which only runs
// in the real HTTP/1.1 accept loop) — so these tests call the private
// `start_sse_stream`/`notify` methods directly and read from the returned
// `stream_pipe` reader in-process to exercise the actual channel plumbing.

use std::io::Read as _;

fn get_request() -> crate::request::Request {
    get_request_with_session(None)
}

fn get_request_with_session(session_id: Option<&str>) -> crate::request::Request {
    let headers = match session_id {
        Some(sid) => vec![crate::header::Header { name: "Mcp-Session-Id".to_string(), value: sid.to_string() }],
        None => vec![],
    };
    crate::request::Request {
        method: "GET".to_string(),
        request_uri: "/mcp".to_string(),
        http_version: crate::http::VERSION.http_1_1.to_string(),
        headers,
        body: vec![],
    }
}

#[test]
fn start_sse_stream_returns_event_stream_headers_and_a_reader() {
    let srv = make_server();
    let resp = srv.start_sse_stream(&get_request());
    assert_eq!(resp.status_code, 200);
    let content_type = resp.headers.iter().find(|h| h.name == "Content-Type").map(|h| h.value.as_str());
    assert_eq!(content_type, Some("text/event-stream"));
    assert!(resp.stream_pipe.is_some());
}

#[test]
fn notify_delivers_a_frame_with_method_and_params_to_a_connected_client() {
    let srv = make_server();
    let mut resp = srv.start_sse_stream(&get_request());
    let mut reader = resp.stream_pipe.take().unwrap();

    srv.notify("notifications/message", Some(r#"{"level":"info","data":"hi"}"#));

    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]).into_owned();
    assert!(text.starts_with("data: "), "expected an SSE data frame: {text}");
    assert!(text.contains(r#""method":"notifications/message""#), "missing method: {text}");
    assert!(text.contains(r#""params":{"level":"info","data":"hi"}"#), "missing params: {text}");
    assert!(text.ends_with("\n\n"), "expected a trailing blank line: {text:?}");
}

#[test]
fn notify_without_params_omits_the_params_field() {
    let srv = make_server();
    let mut resp = srv.start_sse_stream(&get_request());
    let mut reader = resp.stream_pipe.take().unwrap();

    srv.notify("ping", None);

    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(!text.contains("\"params\""), "did not expect a params field: {text}");
}

#[test]
fn notify_reaches_every_connected_client() {
    let srv = make_server();
    let mut resp1 = srv.start_sse_stream(&get_request());
    let mut resp2 = srv.start_sse_stream(&get_request());
    let mut r1 = resp1.stream_pipe.take().unwrap();
    let mut r2 = resp2.stream_pipe.take().unwrap();

    srv.notify("ping", None);

    let mut buf = [0u8; 4096];
    assert!(r1.read(&mut buf).unwrap() > 0, "client 1 got nothing");
    assert!(r2.read(&mut buf).unwrap() > 0, "client 2 got nothing");
}

#[test]
fn notify_drops_a_client_whose_buffer_fills_up() {
    let srv = make_server();
    let resp = srv.start_sse_stream(&get_request());
    // Keep the reader (and thus the receiving end) alive but never read from
    // it, so its bounded channel fills up rather than reporting disconnected.
    let _reader = resp.stream_pipe;

    for _ in 0..=super::SSE_CHANNEL_CAPACITY {
        srv.notify("ping", None);
    }

    assert_eq!(srv.sse_clients.lock().unwrap().len(), 0, "overflowed client should have been dropped");
}

#[test]
fn disconnected_client_is_pruned_on_the_next_notify() {
    let srv = make_server();
    let resp = srv.start_sse_stream(&get_request());
    assert_eq!(srv.sse_clients.lock().unwrap().len(), 1);

    drop(resp); // drops stream_pipe -> drops the Receiver -> sender becomes disconnected
    srv.notify("ping", None); // sweeps dead senders

    assert_eq!(srv.sse_clients.lock().unwrap().len(), 0);
}

// ── logging/setLevel and notifications/message ────────────────────────────────

#[test]
fn initialize_omits_logging_capability_by_default() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}"#);
    let body = body_of(&resp);
    assert!(!body.contains("\"logging\""), "logging capability should be absent by default: {body}");
}

#[test]
fn initialize_advertises_logging_capability_when_enabled() {
    let srv = McpServer::new("test-srv", "0.1").logging_enabled();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}"#);
    let body = body_of(&resp);
    assert!(body.contains(r#""logging":{}"#), "expected an advertised logging capability: {body}");
}

#[test]
fn set_log_level_with_valid_level_succeeds() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"logging/setLevel","id":1,"params":{"level":"warning"}}"#);
    let body = body_of(&resp);
    assert!(body.contains("\"result\""), "expected a successful result: {body}");
    assert!(!body.contains("\"error\""), "did not expect an error: {body}");
}

#[test]
fn set_log_level_missing_level_returns_invalid_params() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"logging/setLevel","id":1,"params":{}}"#);
    let body = body_of(&resp);
    assert!(body.contains("-32602"), "expected INVALID_PARAMS: {body}");
}

#[test]
fn set_log_level_unknown_level_returns_invalid_params() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"logging/setLevel","id":1,"params":{"level":"verbose"}}"#);
    let body = body_of(&resp);
    assert!(body.contains("-32602"), "expected INVALID_PARAMS: {body}");
}

#[test]
fn log_delivers_a_notifications_message_frame_with_level_logger_and_data() {
    let srv = make_server();
    let mut resp = srv.start_sse_stream(&get_request());
    let mut reader = resp.stream_pipe.take().unwrap();

    srv.log(LogLevel::Error, Some("database"), r#"{"detail":"connection pool exhausted"}"#);

    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]).into_owned();
    assert!(text.contains(r#""method":"notifications/message""#), "wrong method: {text}");
    assert!(text.contains(r#""level":"error""#), "missing level: {text}");
    assert!(text.contains(r#""logger":"database""#), "missing logger: {text}");
    assert!(text.contains(r#""data":{"detail":"connection pool exhausted"}"#), "missing data: {text}");
}

#[test]
fn log_without_logger_omits_the_logger_field() {
    let srv = make_server();
    let mut resp = srv.start_sse_stream(&get_request());
    let mut reader = resp.stream_pipe.take().unwrap();

    srv.log(LogLevel::Info, None, r#""hello""#);

    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(!text.contains("\"logger\""), "did not expect a logger field: {text}");
}

#[test]
fn log_is_delivered_by_default_at_every_level() {
    // No logging/setLevel call yet — the default (LogLevel::Debug) filters nothing.
    let srv = make_server();
    let mut resp = srv.start_sse_stream(&get_request());
    let mut reader = resp.stream_pipe.take().unwrap();

    srv.log(LogLevel::Debug, None, r#""a debug message""#);

    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    assert!(n > 0, "expected the debug-level message to be delivered by default");
}

#[test]
fn log_below_the_set_level_is_filtered_out_and_never_queued() {
    let srv = make_server();
    srv.handle_request(r#"{"jsonrpc":"2.0","method":"logging/setLevel","id":1,"params":{"level":"warning"}}"#);

    let mut resp = srv.start_sse_stream(&get_request());
    let mut reader = resp.stream_pipe.take().unwrap();

    srv.log(LogLevel::Info, None, r#""should be filtered out""#);  // below "warning" — never queued
    srv.log(LogLevel::Error, None, r#""should be delivered""#);    // at/above "warning" — queued

    // If the filtered call had been queued, this first read would return it
    // instead of the allowed one.
    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(!text.contains("should be filtered out"), "filtered message leaked through: {text}");
    assert!(text.contains("should be delivered"), "expected the allowed message: {text}");
}

// ── dynamic registration + listChanged ────────────────────────────────────────

#[test]
fn initialize_advertises_list_changed_true_for_tools_resources_prompts() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}"#);
    let body = body_of(&resp);
    assert!(body.contains(r#""tools":{"listChanged":true}"#), "expected tools.listChanged: {body}");
    assert!(body.contains(r#""prompts":{"listChanged":true}"#), "expected prompts.listChanged: {body}");
    assert!(body.contains(r#""subscribe":true,"listChanged":true"#), "expected resources caps: {body}");
}

#[test]
fn register_tool_makes_it_immediately_callable() {
    let srv = make_server();
    srv.register_tool("late_tool", "Registered at runtime", "{}", |_| Ok(McpContent::text("hi from late_tool")));

    let list_resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    assert!(body_of(&list_resp).contains("\"late_tool\""), "new tool missing from tools/list");

    let call_resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"late_tool","arguments":{}}}"#);
    assert!(body_of(&call_resp).contains("hi from late_tool"), "new tool did not run");
}

#[test]
fn register_tool_pushes_tools_list_changed_notification() {
    let srv = make_server();
    let mut resp = srv.start_sse_stream(&get_request());
    let mut reader = resp.stream_pipe.take().unwrap();

    srv.register_tool("late_tool", "Registered at runtime", "{}", |_| Ok(McpContent::text("ok")));

    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(text.contains(r#""method":"notifications/tools/list_changed""#), "missing notification: {text}");
    assert!(!text.contains("\"params\""), "list_changed notifications carry no params: {text}");
}

#[test]
fn remove_tool_returns_true_and_removes_it() {
    let srv = make_server(); // registers "echo" and "fail"
    assert!(srv.remove_tool("echo"));

    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    let body = body_of(&resp);
    assert!(!body.contains("\"echo\""), "echo should have been removed: {body}");
    assert!(body.contains("\"fail\""), "fail should be unaffected: {body}");
}

#[test]
fn remove_tool_returns_false_when_not_found_and_sends_no_notification() {
    let srv = make_server();
    let mut resp = srv.start_sse_stream(&get_request());
    let mut reader = resp.stream_pipe.take().unwrap();

    assert!(!srv.remove_tool("does_not_exist"));

    // If the no-op removal had (incorrectly) pushed a notification, this
    // would be the first frame the reader sees instead of the marker below.
    srv.notify("marker", None);
    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(text.contains(r#""method":"marker""#), "expected only the marker notification: {text}");
}

#[test]
fn register_resource_makes_it_immediately_readable() {
    let srv = make_server();
    srv.register_resource("late://{id}", "Late Resource", "Registered at runtime", |uri| {
        Ok(McpContent::text(format!("late content for {uri}")))
    });

    let list_resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"resources/list","id":1}"#);
    assert!(body_of(&list_resp).contains("late://{id}"), "new resource missing from resources/list");

    let read_resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"resources/read","id":2,"params":{"uri":"late://42"}}"#);
    assert!(body_of(&read_resp).contains("late content for late://42"), "new resource did not run");
}

#[test]
fn remove_resource_by_uri_template_removes_it() {
    let srv = make_server(); // registers "docs://{topic}"
    assert!(srv.remove_resource("docs://{topic}"));
    assert!(!srv.remove_resource("docs://{topic}"), "second removal should find nothing");

    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"resources/list","id":1}"#);
    assert!(!body_of(&resp).contains("docs://{topic}"));
}

#[test]
fn register_prompt_makes_it_immediately_usable() {
    let srv = make_server();
    srv.register_prompt("late_prompt", "Registered at runtime", |_args| {
        Ok(vec![PromptMessage::user("hello from late_prompt")])
    });

    let list_resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"prompts/list","id":1}"#);
    assert!(body_of(&list_resp).contains("\"late_prompt\""), "new prompt missing from prompts/list");

    let get_resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"prompts/get","id":2,"params":{"name":"late_prompt","arguments":{}}}"#);
    assert!(body_of(&get_resp).contains("hello from late_prompt"), "new prompt did not run");
}

#[test]
fn remove_prompt_removes_it() {
    let srv = make_server(); // registers "summarize" and "translate"
    assert!(srv.remove_prompt("summarize"));

    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"prompts/list","id":1}"#);
    let body = body_of(&resp);
    assert!(!body.contains("\"summarize\""), "summarize should have been removed: {body}");
    assert!(body.contains("\"translate\""), "translate should be unaffected: {body}");
}

#[test]
fn dynamic_registration_is_visible_across_clones() {
    // Arc<RwLock<_>> storage means every clone shares the same live list —
    // this is the whole point of dynamic registration working from any
    // thread holding a clone of the server.
    let srv = McpServer::new("srv", "1.0");
    let clone_of_srv = srv.clone();

    clone_of_srv.register_tool("from_clone", "Registered via a clone", "{}", |_| Ok(McpContent::text("ok")));

    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#);
    assert!(body_of(&resp).contains("\"from_clone\""), "tool registered via a clone should be visible on the original");
}

// ── notifications/progress ────────────────────────────────────────────────────

fn make_progress_server() -> McpServer {
    McpServer::new("test-srv", "0.1").tool_with_context(
        "long_job",
        "Do something slow",
        r#"{"type":"object"}"#,
        |ctx, _args| {
            ctx.report_progress(0.0, Some(100.0), Some("starting"));
            ctx.report_progress(100.0, Some(100.0), Some("done"));
            Ok(McpContent::text("finished"))
        },
    )
}

#[test]
fn tools_call_with_progress_token_delivers_progress_notifications_over_sse() {
    let srv = make_progress_server();
    let mut sse_resp = srv.start_sse_stream(&get_request());
    let mut reader = sse_resp.stream_pipe.take().unwrap();

    let client = TestClient::new(srv.clone());
    let call_resp = client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"long_job","arguments":{},"_meta":{"progressToken":"abc123"}}}"#)
        .send();
    assert_eq!(call_resp.status(), 200);
    assert!(call_resp.body_text().contains("finished"));

    let mut buf = [0u8; 4096];
    let n1 = reader.read(&mut buf).unwrap();
    let first = String::from_utf8_lossy(&buf[..n1]).into_owned();
    assert!(first.contains(r#""method":"notifications/progress""#), "missing method: {first}");
    assert!(first.contains(r#""progressToken":"abc123""#), "missing token: {first}");
    assert!(first.contains(r#""progress":0"#), "missing first progress value: {first}");
    assert!(first.contains(r#""total":100"#), "missing total: {first}");
    assert!(first.contains(r#""message":"starting""#), "missing message: {first}");

    let n2 = reader.read(&mut buf).unwrap();
    let second = String::from_utf8_lossy(&buf[..n2]).into_owned();
    assert!(second.contains(r#""progress":100"#), "missing second progress value: {second}");
    assert!(second.contains(r#""message":"done""#), "missing second message: {second}");
}

#[test]
fn tools_call_without_progress_token_reports_nothing() {
    let srv = make_progress_server();
    let mut sse_resp = srv.start_sse_stream(&get_request());
    let mut reader = sse_resp.stream_pipe.take().unwrap();

    let client = TestClient::new(srv.clone());
    client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"long_job","arguments":{}}}"#)
        .send();

    // If report_progress had (incorrectly) queued anything without a
    // progressToken, this marker would not be the first frame read back.
    srv.notify("marker", None);
    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(text.contains(r#""method":"marker""#), "expected only the marker notification: {text}");
}

#[test]
fn report_progress_is_a_safe_no_op_without_a_live_server_context() {
    // handle_request() builds ctx via McpContext::default() (no sse_clients),
    // even though this request's params._meta.progressToken is present.
    let srv = make_progress_server();
    let resp = srv.handle_request(
        r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"long_job","arguments":{},"_meta":{"progressToken":"abc"}}}"#,
    );
    assert_eq!(resp.status_code, 200);
    let body = body_of(&resp);
    assert!(body.contains("finished"), "tool should still complete normally: {body}");
}

#[test]
fn progress_token_numeric_type_round_trips_unquoted() {
    let srv = make_progress_server();
    let mut sse_resp = srv.start_sse_stream(&get_request());
    let mut reader = sse_resp.stream_pipe.take().unwrap();

    let client = TestClient::new(srv.clone());
    client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"long_job","arguments":{},"_meta":{"progressToken":42}}}"#)
        .send();

    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(text.contains(r#""progressToken":42"#), "expected the numeric token unquoted: {text}");
}

#[test]
fn report_progress_omits_total_and_message_when_not_given() {
    let srv = McpServer::new("test-srv", "0.1").tool_with_context(
        "minimal_job",
        "Report bare progress",
        r#"{"type":"object"}"#,
        |ctx, _args| {
            ctx.report_progress(50.0, None, None);
            Ok(McpContent::text("ok"))
        },
    );
    let mut sse_resp = srv.start_sse_stream(&get_request());
    let mut reader = sse_resp.stream_pipe.take().unwrap();

    let client = TestClient::new(srv.clone());
    client
        .post("/mcp")
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"minimal_job","arguments":{},"_meta":{"progressToken":"t1"}}}"#)
        .send();

    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(text.contains(r#""progress":50"#), "missing progress: {text}");
    assert!(!text.contains("\"total\""), "did not expect a total field: {text}");
    assert!(!text.contains("\"message\""), "did not expect a message field: {text}");
}

// ── completion/complete ────────────────────────────────────────────────────────

fn make_completion_server() -> McpServer {
    McpServer::new("test-srv", "0.1").completion("tool", "deploy", |arg_name, partial| {
        match arg_name {
            "region" => Ok(vec!["us-east-1", "eu-west-1", "ap-southeast-1"]
                .into_iter()
                .filter(|r| r.starts_with(partial))
                .map(String::from)
                .collect()),
            _ => Ok(vec![]),
        }
    })
}

#[test]
fn completion_returns_matching_values_for_a_registered_tool_argument() {
    let srv = make_completion_server();
    let req = r#"{"jsonrpc":"2.0","method":"completion/complete","id":1,"params":{"ref":{"type":"ref/tool","name":"deploy"},"argument":{"name":"region","value":"us"}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains(r#""values":["us-east-1"]"#), "expected filtered values: {body}");
    assert!(body.contains(r#""hasMore":false"#), "expected hasMore false: {body}");
    assert!(body.contains(r#""total":1"#), "expected total 1: {body}");
}

#[test]
fn completion_argument_without_value_defaults_to_empty_partial() {
    let srv = make_completion_server();
    let req = r#"{"jsonrpc":"2.0","method":"completion/complete","id":1,"params":{"ref":{"type":"ref/tool","name":"deploy"},"argument":{"name":"region"}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains("us-east-1") && body.contains("eu-west-1") && body.contains("ap-southeast-1"), "expected all three regions with an empty partial: {body}");
    assert!(body.contains(r#""total":3"#));
}

#[test]
fn completion_for_unregistered_ref_returns_empty_values_not_an_error() {
    let srv = make_completion_server();
    let req = r#"{"jsonrpc":"2.0","method":"completion/complete","id":1,"params":{"ref":{"type":"ref/tool","name":"no_such_tool"},"argument":{"name":"region","value":"us"}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(!body.contains("\"error\""), "should not be a JSON-RPC error: {body}");
    assert!(body.contains(r#""values":[]"#), "expected empty values: {body}");
    assert!(body.contains(r#""total":0"#));
}

#[test]
fn completion_for_unregistered_argument_name_returns_empty_values() {
    let srv = make_completion_server();
    let req = r#"{"jsonrpc":"2.0","method":"completion/complete","id":1,"params":{"ref":{"type":"ref/tool","name":"deploy"},"argument":{"name":"unrelated_arg","value":""}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains(r#""values":[]"#), "expected empty values: {body}");
}

#[test]
fn completion_handler_error_returns_invalid_params() {
    let srv = McpServer::new("test-srv", "0.1").completion("tool", "broken", |_arg, _partial| {
        Err("completion source unavailable".to_string())
    });
    let req = r#"{"jsonrpc":"2.0","method":"completion/complete","id":1,"params":{"ref":{"type":"ref/tool","name":"broken"},"argument":{"name":"anything","value":""}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains("-32602"), "expected INVALID_PARAMS: {body}");
    assert!(body.contains("completion source unavailable"), "expected the handler's error message: {body}");
}

#[test]
fn completion_missing_ref_returns_invalid_params() {
    let srv = make_completion_server();
    let req = r#"{"jsonrpc":"2.0","method":"completion/complete","id":1,"params":{"argument":{"name":"region","value":"us"}}}"#;
    let resp = srv.handle_request(req);
    assert!(body_of(&resp).contains("-32602"));
}

#[test]
fn completion_missing_argument_returns_invalid_params() {
    let srv = make_completion_server();
    let req = r#"{"jsonrpc":"2.0","method":"completion/complete","id":1,"params":{"ref":{"type":"ref/tool","name":"deploy"}}}"#;
    let resp = srv.handle_request(req);
    assert!(body_of(&resp).contains("-32602"));
}

#[test]
fn completion_supports_prompt_ref_type() {
    let srv = McpServer::new("test-srv", "0.1").completion("prompt", "summarize", |arg_name, _partial| {
        match arg_name {
            "language" => Ok(vec!["english".to_string(), "spanish".to_string()]),
            _ => Ok(vec![]),
        }
    });
    let req = r#"{"jsonrpc":"2.0","method":"completion/complete","id":1,"params":{"ref":{"type":"ref/prompt","name":"summarize"},"argument":{"name":"language","value":""}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains("english") && body.contains("spanish"), "expected both languages: {body}");
}

#[test]
fn completion_truncates_to_100_values_and_reports_has_more() {
    let srv = McpServer::new("test-srv", "0.1").completion("tool", "big", |_arg, _partial| {
        Ok((0..150).map(|i| format!("value{i}")).collect())
    });
    let req = r#"{"jsonrpc":"2.0","method":"completion/complete","id":1,"params":{"ref":{"type":"ref/tool","name":"big"},"argument":{"name":"x","value":""}}}"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert!(body.contains(r#""hasMore":true"#), "expected hasMore true: {body}");
    assert!(body.contains(r#""total":150"#), "expected total 150: {body}");
    assert!(!body.contains("value100"), "expected only the first 100 values: {body}");
    assert!(body.contains("value99"), "expected value99 (the 100th) to be included: {body}");
}

#[test]
fn initialize_omits_completions_capability_by_default() {
    let srv = make_server(); // registers no completions
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}"#);
    let body = body_of(&resp);
    assert!(!body.contains("\"completions\""), "completions capability should be absent by default: {body}");
}

#[test]
fn initialize_advertises_completions_capability_once_one_is_registered() {
    let srv = make_completion_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}"#);
    let body = body_of(&resp);
    assert!(body.contains(r#""completions":{}"#), "expected an advertised completions capability: {body}");
}

// ── notifications/cancelled ────────────────────────────────────────────────────

#[test]
fn is_cancelled_defaults_to_false_without_a_cancellation_notification() {
    let srv = McpServer::new("test-srv", "0.1").tool_with_context("job", "Do work", "{}", |ctx, _args| {
        Ok(McpContent::text(if ctx.is_cancelled() { "cancelled" } else { "not cancelled" }))
    });
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"job","arguments":{}}}"#);
    assert!(body_of(&resp).contains("not cancelled"));
}

#[test]
fn tool_handler_observes_a_cancellation_sent_mid_call() {
    // A single-threaded test can't send notifications/cancelled concurrently
    // from another connection, so the handler simulates it: it holds a
    // clone of the server (sharing the same `cancellations` map) and sends
    // the cancellation to itself, targeting its own request id, then checks
    // whether is_cancelled() picked it up.
    let observed = Arc::new(AtomicBool::new(false));
    let observed_in_handler = observed.clone();

    let srv = McpServer::new("test-srv", "0.1");
    let srv_for_handler = srv.clone();
    let srv = srv.tool_with_context("cancellable_job", "Checks for cancellation", "{}", move |ctx, _args| {
        srv_for_handler.handle_request(
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":42}}"#,
        );
        observed_in_handler.store(ctx.is_cancelled(), Ordering::Relaxed);
        Ok(McpContent::text("done"))
    });

    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/call","id":42,"params":{"name":"cancellable_job","arguments":{}}}"#);
    assert_eq!(resp.status_code, 200);
    assert!(observed.load(Ordering::Relaxed), "handler should have observed is_cancelled() == true");
}

#[test]
fn cancellation_matches_a_string_request_id_too() {
    let observed = Arc::new(AtomicBool::new(false));
    let observed_in_handler = observed.clone();

    let srv = McpServer::new("test-srv", "0.1");
    let srv_for_handler = srv.clone();
    let srv = srv.tool_with_context("job", "Checks for cancellation", "{}", move |ctx, _args| {
        srv_for_handler.handle_request(
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"req-abc"}}"#,
        );
        observed_in_handler.store(ctx.is_cancelled(), Ordering::Relaxed);
        Ok(McpContent::text("done"))
    });

    srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/call","id":"req-abc","params":{"name":"job","arguments":{}}}"#);
    assert!(observed.load(Ordering::Relaxed), "a string requestId should match a string id the same way a number does");
}

#[test]
fn notifications_cancelled_for_an_unknown_request_id_is_a_silent_no_op() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":999}}"#);
    assert_eq!(resp.status_code, 202);
    assert!(body_of(&resp).is_empty());
}

#[test]
fn cancellation_entry_is_removed_after_the_call_completes() {
    let srv = McpServer::new("test-srv", "0.1").tool_with_context("job", "Do work", "{}", |_ctx, _args| {
        Ok(McpContent::text("done"))
    });
    srv.handle_request(r#"{"jsonrpc":"2.0","method":"tools/call","id":7,"params":{"name":"job","arguments":{}}}"#);
    assert!(srv.cancellations.lock().unwrap().is_empty(), "cancellation flag should be cleaned up once the call finishes");
}

#[test]
fn cancellation_notification_in_a_batch_produces_no_response_entry() {
    let srv = make_server();
    let req = r#"[{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}},
                  {"jsonrpc":"2.0","method":"ping","id":2}]"#;
    let resp = srv.handle_request(req);
    let body = body_of(&resp);
    assert_eq!(body.matches("\"jsonrpc\"").count(), 1, "only the ping should produce a response entry: {body}");
}

// ── resources/subscribe and resources/unsubscribe ─────────────────────────────

fn ctx_with_session(session_id: &str) -> McpContext {
    McpContext { session_id: Some(session_id.to_string()), ..Default::default() }
}

#[test]
fn subscribe_with_a_session_id_succeeds() {
    let srv = make_server();
    let resp = srv.handle_request_with_context(
        r#"{"jsonrpc":"2.0","method":"resources/subscribe","id":1,"params":{"uri":"config://main"}}"#,
        ctx_with_session("session-1"),
    );
    let body = body_of(&resp);
    assert!(body.contains("\"result\""), "expected success: {body}");
    assert!(!body.contains("\"error\""), "did not expect an error: {body}");
}

#[test]
fn subscribe_without_a_session_id_returns_invalid_params() {
    let srv = make_server();
    let resp = srv.handle_request(
        r#"{"jsonrpc":"2.0","method":"resources/subscribe","id":1,"params":{"uri":"config://main"}}"#,
    );
    assert!(body_of(&resp).contains("-32602"));
}

#[test]
fn subscribe_missing_uri_returns_invalid_params() {
    let srv = make_server();
    let resp = srv.handle_request_with_context(
        r#"{"jsonrpc":"2.0","method":"resources/subscribe","id":1,"params":{}}"#,
        ctx_with_session("session-1"),
    );
    assert!(body_of(&resp).contains("-32602"));
}

#[test]
fn unsubscribe_without_a_session_id_returns_invalid_params() {
    let srv = make_server();
    let resp = srv.handle_request(
        r#"{"jsonrpc":"2.0","method":"resources/unsubscribe","id":1,"params":{"uri":"config://main"}}"#,
    );
    assert!(body_of(&resp).contains("-32602"));
}

#[test]
fn notify_resource_updated_delivers_to_the_subscribed_session() {
    let srv = make_server();

    // Open the SSE connection first, tagged with this session id.
    let mut sse_resp = srv.start_sse_stream(&get_request_with_session(Some("session-1")));
    let mut reader = sse_resp.stream_pipe.take().unwrap();

    srv.handle_request_with_context(
        r#"{"jsonrpc":"2.0","method":"resources/subscribe","id":1,"params":{"uri":"config://main"}}"#,
        ctx_with_session("session-1"),
    );

    srv.notify_resource_updated("config://main");

    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(text.contains(r#""method":"notifications/resources/updated""#), "missing method: {text}");
    assert!(text.contains(r#""uri":"config://main""#), "missing uri: {text}");
}

#[test]
fn notify_resource_updated_does_not_reach_an_unsubscribed_session() {
    let srv = make_server();

    // session-1 subscribes; session-2 connects via SSE but never subscribes.
    let mut sub_resp = srv.start_sse_stream(&get_request_with_session(Some("session-1")));
    let mut sub_reader = sub_resp.stream_pipe.take().unwrap();
    let mut other_resp = srv.start_sse_stream(&get_request_with_session(Some("session-2")));
    let mut other_reader = other_resp.stream_pipe.take().unwrap();

    srv.handle_request_with_context(
        r#"{"jsonrpc":"2.0","method":"resources/subscribe","id":1,"params":{"uri":"config://main"}}"#,
        ctx_with_session("session-1"),
    );

    srv.notify_resource_updated("config://main");

    // The subscriber gets it...
    let mut buf = [0u8; 4096];
    let n = sub_reader.read(&mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("notifications/resources/updated"));

    // ...but session-2 (connected, not subscribed) doesn't. Prove it by
    // sending a marker broadcast next and confirming that's the first thing
    // session-2's reader sees.
    srv.notify("marker", None);
    let n2 = other_reader.read(&mut buf).unwrap();
    let text2 = String::from_utf8_lossy(&buf[..n2]);
    assert!(text2.contains(r#""method":"marker""#), "expected only the marker notification: {text2}");
}

#[test]
fn notify_resource_updated_for_an_unsubscribed_uri_is_a_no_op() {
    let srv = make_server();
    // Should not panic even though nobody has ever subscribed to anything.
    srv.notify_resource_updated("config://nobody-subscribed");
}

#[test]
fn unsubscribe_stops_further_notifications() {
    let srv = make_server();
    let mut sse_resp = srv.start_sse_stream(&get_request_with_session(Some("session-1")));
    let mut reader = sse_resp.stream_pipe.take().unwrap();

    srv.handle_request_with_context(
        r#"{"jsonrpc":"2.0","method":"resources/subscribe","id":1,"params":{"uri":"config://main"}}"#,
        ctx_with_session("session-1"),
    );
    srv.handle_request_with_context(
        r#"{"jsonrpc":"2.0","method":"resources/unsubscribe","id":2,"params":{"uri":"config://main"}}"#,
        ctx_with_session("session-1"),
    );

    srv.notify_resource_updated("config://main");

    // Nothing should have been queued for the now-unsubscribed session;
    // prove it the same way as other "nothing queued" tests: a marker sent
    // next should be the first (and only) thing read back.
    srv.notify("marker", None);
    let mut buf = [0u8; 4096];
    let n = reader.read(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(text.contains(r#""method":"marker""#), "expected only the marker notification: {text}");
}

#[test]
fn unsubscribe_removes_the_uri_entry_entirely_once_empty() {
    let srv = make_server();
    srv.handle_request_with_context(
        r#"{"jsonrpc":"2.0","method":"resources/subscribe","id":1,"params":{"uri":"config://main"}}"#,
        ctx_with_session("session-1"),
    );
    srv.handle_request_with_context(
        r#"{"jsonrpc":"2.0","method":"resources/unsubscribe","id":2,"params":{"uri":"config://main"}}"#,
        ctx_with_session("session-1"),
    );
    assert!(srv.subscriptions.lock().unwrap().is_empty(), "expected the URI entry to be pruned once its subscriber list is empty");
}

#[test]
fn initialize_advertises_resources_subscribe_true() {
    let srv = make_server();
    let resp = srv.handle_request(r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}"#);
    let body = body_of(&resp);
    assert!(body.contains(r#""resources":{"subscribe":true,"listChanged":true}"#), "expected resources.subscribe true: {body}");
}

// ── sampling/createMessage ─────────────────────────────────────────────────────

fn make_sampling_server() -> McpServer {
    McpServer::new("test-srv", "0.1").tool_with_context("ask", "Ask the client's model", "{}", |ctx, _args| {
        match ctx.sample(
            SamplingRequest {
                messages: vec![PromptMessage::user("2+2?")],
                max_tokens: 50,
                system_prompt: None,
            },
            Duration::from_millis(500),
        ) {
            Ok(response) => Ok(response.content),
            Err(e) => Ok(McpContent::text(e)),
        }
    })
}

#[test]
fn sample_fails_fast_without_sampling_capability_declared() {
    let client = TestClient::new(make_sampling_server());
    let init_resp = client.post("/mcp").body_text(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}"#, // no capabilities.sampling
    ).send();
    let session_id = init_resp.header("Mcp-Session-Id").unwrap().to_string();

    let call_resp = client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"ask","arguments":{}}}"#)
        .send();
    assert!(call_resp.body_text().contains("did not declare sampling support"), "unexpected body: {}", call_resp.body_text());
}

#[test]
fn sample_fails_without_a_session_id_even_with_sampling_declared() {
    let srv = make_sampling_server();
    let ctx = McpContext { sampling_supported: true, ..Default::default() };
    let resp = srv.handle_request_with_context(
        r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"ask","arguments":{}}}"#,
        ctx,
    );
    let body = body_of(&resp);
    assert!(body.contains("requires a session"), "expected a session-id error: {body}");
}

#[test]
fn sample_times_out_when_the_client_never_responds() {
    let client = TestClient::new(make_sampling_server());
    let init_resp = client.post("/mcp").body_text(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"capabilities":{"sampling":{}}}}"#,
    ).send();
    let session_id = init_resp.header("Mcp-Session-Id").unwrap().to_string();
    // Deliberately no GET /mcp SSE connection opened — nobody could ever respond.

    let call_resp = client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"ask","arguments":{}}}"#)
        .send();
    assert!(call_resp.body_text().contains("timed out"), "unexpected body: {}", call_resp.body_text());
}

#[test]
fn sample_full_round_trip_via_sse_and_post_response() {
    let srv_for_responder = make_sampling_server();
    let client = TestClient::new(srv_for_responder.clone());

    let init_resp = client.post("/mcp").body_text(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"capabilities":{"sampling":{}}}}"#,
    ).send();
    let session_id = init_resp.header("Mcp-Session-Id").unwrap().to_string();

    // Open the GET /mcp SSE stream for this session directly on the shared
    // server (TestClient never drives Response::stream_pipe).
    let mut sse_resp = srv_for_responder.start_sse_stream(&get_request_with_session(Some(&session_id)));
    let mut reader = sse_resp.stream_pipe.take().unwrap();

    // A separate thread plays the client: block reading the outbound
    // sampling/createMessage request off the SSE stream, extract its id,
    // and POST a response back with that same id.
    let responder_srv = srv_for_responder.clone();
    let responder = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let n = reader.read(&mut buf).unwrap();
        let text = String::from_utf8_lossy(&buf[..n]).into_owned();
        let json_line = text.trim().trim_start_matches("data:").trim();
        assert!(json_line.contains(r#""method":"sampling/createMessage""#), "unexpected SSE frame: {json_line}");
        let id = json_rpc::extract_id(json_line).expect("outbound sampling request should carry an id");
        let response_body = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"result":{{"role":"assistant","content":{{"type":"text","text":"4"}},"model":"test-model","stopReason":"endTurn"}}}}"#
        );
        responder_srv.handle_request(&response_body);
    });

    let call_resp = client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"ask","arguments":{}}}"#)
        .send();

    responder.join().unwrap();

    assert_eq!(call_resp.status(), 200);
    assert!(call_resp.body_text().contains('4'), "expected the sampled response text: {}", call_resp.body_text());
    assert!(srv_for_responder.pending_replies.lock().unwrap().is_empty(), "pending_replies entry should be cleaned up after delivery");
}

#[test]
fn sample_error_response_is_surfaced_to_the_caller() {
    let srv_for_responder = make_sampling_server();
    let client = TestClient::new(srv_for_responder.clone());

    let init_resp = client.post("/mcp").body_text(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"capabilities":{"sampling":{}}}}"#,
    ).send();
    let session_id = init_resp.header("Mcp-Session-Id").unwrap().to_string();

    let mut sse_resp = srv_for_responder.start_sse_stream(&get_request_with_session(Some(&session_id)));
    let mut reader = sse_resp.stream_pipe.take().unwrap();

    let responder_srv = srv_for_responder.clone();
    let responder = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let n = reader.read(&mut buf).unwrap();
        let text = String::from_utf8_lossy(&buf[..n]).into_owned();
        let json_line = text.trim().trim_start_matches("data:").trim();
        let id = json_rpc::extract_id(json_line).expect("outbound sampling request should carry an id");
        let response_body = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":-1,"message":"user declined the sampling request"}}}}"#
        );
        responder_srv.handle_request(&response_body);
    });

    let call_resp = client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"ask","arguments":{}}}"#)
        .send();

    responder.join().unwrap();

    assert!(call_resp.body_text().contains("user declined the sampling request"), "unexpected body: {}", call_resp.body_text());
}

// ── roots/list and notifications/roots/list_changed ───────────────────────────

fn make_roots_server() -> McpServer {
    McpServer::new("test-srv", "0.1").tool_with_context("get_roots", "Get workspace roots", "{}", |ctx, _args| {
        match ctx.list_roots(Duration::from_millis(500)) {
            Ok(roots) => Ok(McpContent::text(roots.iter().map(|r| r.uri.clone()).collect::<Vec<_>>().join(","))),
            Err(e) => Ok(McpContent::text(e)),
        }
    })
}

#[test]
fn list_roots_fails_fast_without_roots_capability_declared() {
    let client = TestClient::new(make_roots_server());
    let init_resp = client.post("/mcp").body_text(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}"#, // no capabilities.roots
    ).send();
    let session_id = init_resp.header("Mcp-Session-Id").unwrap().to_string();

    let call_resp = client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"get_roots","arguments":{}}}"#)
        .send();
    assert!(call_resp.body_text().contains("did not declare roots support"), "unexpected body: {}", call_resp.body_text());
}

#[test]
fn list_roots_fails_without_a_session_id_even_with_roots_declared() {
    let srv = make_roots_server();
    let ctx = McpContext { roots_supported: true, ..Default::default() };
    let resp = srv.handle_request_with_context(
        r#"{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"get_roots","arguments":{}}}"#,
        ctx,
    );
    let body = body_of(&resp);
    assert!(body.contains("requires a session"), "expected a session-id error: {body}");
}

#[test]
fn list_roots_times_out_when_the_client_never_responds() {
    let client = TestClient::new(make_roots_server());
    let init_resp = client.post("/mcp").body_text(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"capabilities":{"roots":{}}}}"#,
    ).send();
    let session_id = init_resp.header("Mcp-Session-Id").unwrap().to_string();
    // Deliberately no GET /mcp SSE connection opened — nobody could ever respond.

    let call_resp = client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"get_roots","arguments":{}}}"#)
        .send();
    assert!(call_resp.body_text().contains("timed out"), "unexpected body: {}", call_resp.body_text());
}

#[test]
fn list_roots_full_round_trip_and_caches_within_the_session() {
    let srv_for_responder = make_roots_server();
    let client = TestClient::new(srv_for_responder.clone());

    let init_resp = client.post("/mcp").body_text(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"capabilities":{"roots":{}}}}"#,
    ).send();
    let session_id = init_resp.header("Mcp-Session-Id").unwrap().to_string();

    let mut sse_resp = srv_for_responder.start_sse_stream(&get_request_with_session(Some(&session_id)));
    let mut reader = sse_resp.stream_pipe.take().unwrap();

    // The responder only needs to answer once — the second tools/call
    // below should be served from cache with no further SSE request.
    let responder_srv = srv_for_responder.clone();
    let responder = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let n = reader.read(&mut buf).unwrap();
        let text = String::from_utf8_lossy(&buf[..n]).into_owned();
        let json_line = text.trim().trim_start_matches("data:").trim();
        assert!(json_line.contains(r#""method":"roots/list""#), "unexpected SSE frame: {json_line}");
        let id = json_rpc::extract_id(json_line).expect("outbound roots/list request should carry an id");
        let response_body = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"result":{{"roots":[{{"uri":"file:///workspace","name":"Workspace"}}]}}}}"#
        );
        responder_srv.handle_request(&response_body);
    });

    let first_call = client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"get_roots","arguments":{}}}"#)
        .send();
    responder.join().unwrap();
    assert!(first_call.body_text().contains("file:///workspace"), "unexpected first call body: {}", first_call.body_text());

    let second_call = client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"name":"get_roots","arguments":{}}}"#)
        .send();
    assert!(second_call.body_text().contains("file:///workspace"), "expected the cached result: {}", second_call.body_text());
}

#[test]
fn roots_list_changed_notification_invalidates_the_cache() {
    let srv_for_responder = make_roots_server();
    let client = TestClient::new(srv_for_responder.clone());

    let init_resp = client.post("/mcp").body_text(
        r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"capabilities":{"roots":{}}}}"#,
    ).send();
    let session_id = init_resp.header("Mcp-Session-Id").unwrap().to_string();

    let mut sse_resp = srv_for_responder.start_sse_stream(&get_request_with_session(Some(&session_id)));
    let mut reader = sse_resp.stream_pipe.take().unwrap();

    // Answers exactly two roots/list requests — one for the first
    // tools/call, one for the second (post-invalidation) tools/call. If
    // invalidation didn't work, the second tools/call would be served from
    // cache and this thread would hang forever waiting on its second read.
    let responder_srv = srv_for_responder.clone();
    let responder = std::thread::spawn(move || {
        for _ in 0..2 {
            let mut buf = [0u8; 4096];
            let n = reader.read(&mut buf).unwrap();
            let text = String::from_utf8_lossy(&buf[..n]).into_owned();
            let json_line = text.trim().trim_start_matches("data:").trim();
            let id = json_rpc::extract_id(json_line).expect("expected a roots/list request");
            let response_body = format!(
                r#"{{"jsonrpc":"2.0","id":{id},"result":{{"roots":[{{"uri":"file:///workspace","name":"Workspace"}}]}}}}"#
            );
            responder_srv.handle_request(&response_body);
        }
    });

    let first_call = client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"get_roots","arguments":{}}}"#)
        .send();
    assert!(first_call.body_text().contains("file:///workspace"), "unexpected first call body: {}", first_call.body_text());

    client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"notifications/roots/list_changed"}"#)
        .send();

    let second_call = client.post("/mcp")
        .header("Mcp-Session-Id", &session_id)
        .body_text(r#"{"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"name":"get_roots","arguments":{}}}"#)
        .send();

    responder.join().unwrap();
    assert!(second_call.body_text().contains("file:///workspace"), "unexpected second call body: {}", second_call.body_text());
}

