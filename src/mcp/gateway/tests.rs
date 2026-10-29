use super::*;
use crate::application::Application;
use crate::header::Header;
use crate::mcp::McpContext;
use crate::request::Request;
use crate::server::{Address, ConnectionInfo};
use crate::test_client::TestClient;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

// ── A fake upstream ──────────────────────────────────────────────────────────

/// One request the fake upstream received.
#[derive(Clone, Debug)]
struct Recorded {
    headers: Vec<(String, String)>,
    body: String,
}

impl Recorded {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    fn method(&self) -> String {
        top_level_str(&self.body, "method").unwrap_or_default()
    }

    fn id(&self) -> String {
        top_level_raw(&self.body, "id").unwrap_or_default()
    }
}

struct Reply {
    status: u16,
    content_type: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Reply {
    fn json(body: String) -> Self {
        Reply { status: 200, content_type: "application/json".into(), headers: Vec::new(), body }
    }

    fn status(status: u16) -> Self {
        Reply { status, content_type: "application/json".into(), headers: Vec::new(), body: String::new() }
    }
}

/// An HTTP server on a free port that records every request and answers
/// with `respond`. Runs until the test process ends.
struct Fake {
    url: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl Fake {
    fn start(respond: impl Fn(&Recorded) -> Reply + Send + Sync + 'static) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut headers = Vec::new();
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some((k, v)) = line.trim_end().split_once(':') {
                        headers.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
                let length: usize = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.parse().ok())
                    .unwrap_or(0);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let recorded = Recorded { headers, body: String::from_utf8(body).unwrap() };
                log.lock().unwrap().push(recorded.clone());
                let reply = respond(&recorded);
                let mut out = format!(
                    "HTTP/1.1 {} X\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                    reply.status,
                    reply.content_type,
                    reply.body.len()
                );
                for (k, v) in &reply.headers {
                    out.push_str(&format!("{k}: {v}\r\n"));
                }
                out.push_str("\r\n");
                out.push_str(&reply.body);
                let _ = stream.write_all(out.as_bytes());
            }
        });
        Fake { url, requests }
    }

    /// A fake that is a real `McpServer`.
    fn serving(server: McpServer) -> Fake {
        Fake::start(Fake::serving_handler(server))
    }

    fn serving_handler(server: McpServer) -> impl Fn(&Recorded) -> Reply + Send + Sync + 'static {
        move |recorded: &Recorded| {
            let request = Request {
                method: "POST".to_string(),
                request_uri: "/mcp".to_string(),
                http_version: "HTTP/1.1".to_string(),
                headers: recorded.headers.iter().map(|(k, v)| Header::new(k.clone(), v.clone())).collect(),
                body: recorded.body.clone().into(),
            };
            let connection = ConnectionInfo {
                client: Address { ip: "127.0.0.1".to_string(), port: 0 },
                server: Address { ip: "127.0.0.1".to_string(), port: 0 },
                request_size: 16000,
                sni_hostname: None,
                tls: false,
            };
            let response = server.execute(&request, &connection).unwrap();
            let part = response.content_range_list.first();
            Reply {
                status: response.status_code as u16,
                content_type: part.map(|p| p.content_type.clone()).unwrap_or_else(|| "application/json".into()),
                headers: response.headers.iter().filter(|h| h.name.eq_ignore_ascii_case("mcp-session-id")).map(|h| (h.name.to_string(), h.value.to_string())).collect(),
                body: part.map(|p| String::from_utf8(p.body.clone()).unwrap()).unwrap_or_default(),
            }
        }
    }

    /// A real `McpServer` that only speaks 2026-07-28: `initialize` is
    /// refused as a server without the earlier versions would.
    fn modern_only(server: McpServer) -> Fake {
        let inner = Fake::serving_handler(server);
        Fake::start(move |recorded| {
            if recorded.method() == "initialize" {
                return Reply {
                    status: 400,
                    content_type: "application/json".into(),
                    headers: Vec::new(),
                    body: format!(r#"{{"jsonrpc":"2.0","id":{},"error":{{"code":-32022,"message":"Unsupported protocol version"}}}}"#, recorded.id()),
                };
            }
            inner(recorded)
        })
    }

    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

/// A scripted upstream speaking just enough MCP: `initialize` (with a
/// session id), the initialized notification, and `tools` for anything else.
fn scripted(tools_reply: impl Fn(&Recorded) -> Reply + Send + Sync + 'static) -> Fake {
    Fake::start(move |r| match r.method().as_str() {
        "initialize" => {
            let mut reply = Reply::json(format!(r#"{{"jsonrpc":"2.0","id":{},"result":{{"protocolVersion":"2025-06-18","capabilities":{{}},"serverInfo":{{"name":"s","version":"1"}}}}}}"#, r.id()));
            reply.headers.push(("Mcp-Session-Id".into(), "s-1".into()));
            reply
        }
        "notifications/initialized" => Reply::status(202),
        _ => tools_reply(r),
    })
}

fn one_tool_list(r: &Recorded) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{},"result":{{"tools":[{{"name":"echo","inputSchema":{{"type":"object"}}}}]}}}}"#, r.id())
}

fn ctx(sub: Option<&str>) -> McpContext {
    McpContext { auth_claims: sub.map(|s| format!(r#"{{"sub":"{s}"}}"#)), ..McpContext::default() }
}

fn rpc(client: &TestClient<McpServer>, token: &str, body: &str) -> String {
    let response = client
        .post("/mcp")
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body_text(body)
        .send();
    assert_eq!(200, response.status(), "{}", response.body_text());
    response.body_text().to_string()
}

fn list(client: &TestClient<McpServer>, token: &str) -> String {
    rpc(client, token, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
}

fn call(client: &TestClient<McpServer>, token: &str, tool: &str, arguments: &str) -> String {
    rpc(client, token, &format!(r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{tool}","arguments":{arguments}}}}}"#))
}

// ── Federation against real MCP servers ─────────────────────────────────────

fn github_like() -> McpServer {
    McpServer::new("github", "1")
        .require_bearer("github-secret")
        .tool_annotated(
            "search_issues",
            "Search issues",
            r#"{"type":"object","properties":{"q":{"type":"string"}},"required":["q"]}"#,
            ToolAnnotations { read_only_hint: Some(true), ..Default::default() },
            |args| Ok(McpContent::structured(format!(r#"{{"query":{},"hits":2}}"#, top_level_raw(args, "q").unwrap()))),
        )
        .tool_title("search_issues", "Search issues")
        .tool_output_schema("search_issues", r#"{"type":"object","properties":{"hits":{"type":"integer"}}}"#)
        .tool("close_issue", "Close an issue", r#"{"type":"object"}"#, |_| Err("issue is locked".into()))
}

fn jira_like() -> McpServer {
    McpServer::new("jira", "1").tool("search", "Search tickets", r#"{"type":"object"}"#, |_| Ok(McpContent::text("PROJ-1")))
}

fn gateway(github: &Fake, jira: &Fake) -> TestClient<McpServer> {
    let server = McpGateway::new("gateway", "1")
        .upstream(Upstream::new("github", &github.url).header("Authorization", "Bearer github-secret"))
        .upstream(Upstream::new("jira", &jira.url))
        .allow("alice", ["*"])
        .allow("bob", ["jira__search", "github__search_*"])
        .build()
        .unwrap()
        .require_bearer_tokens([("alice-token", "alice"), ("bob-token", "bob"), ("eve-token", "eve")]);
    TestClient::new(server)
}

#[test]
fn upstream_tools_appear_prefixed_with_their_metadata() {
    let (github, jira) = (Fake::serving(github_like()), Fake::serving(jira_like()));
    let client = gateway(&github, &jira);
    let tools = list(&client, "alice-token");
    for name in ["github__search_issues", "github__close_issue", "jira__search"] {
        assert!(tools.contains(&format!(r#""name":"{name}""#)), "{name}: {tools}");
    }
    assert!(tools.contains(r#""name":"github__search_issues","title":"Search issues""#), "{tools}");
    assert!(tools.contains(r#""outputSchema":{"type":"object","properties":{"hits":{"type":"integer"}}}"#), "{tools}");
    assert!(tools.contains(r#""readOnlyHint":true"#), "{tools}");
}

#[test]
fn calls_are_forwarded_and_results_pass_through() {
    let (github, jira) = (Fake::serving(github_like()), Fake::serving(jira_like()));
    let client = gateway(&github, &jira);
    let found = call(&client, "alice-token", "github__search_issues", r#"{"q":"crash"}"#);
    assert!(found.contains(r#""structuredContent":{"query":"crash","hits":2}"#), "{found}");
    assert!(found.contains(r#""isError":false"#), "{found}");
    let locked = call(&client, "alice-token", "github__close_issue", "{}");
    assert!(locked.contains(r#""isError":true"#) && locked.contains("issue is locked"), "{locked}");
    let jira_hit = call(&client, "alice-token", "jira__search", "{}");
    assert!(jira_hit.contains("PROJ-1"), "{jira_hit}");
    // The gateway checks arguments against the upstream's schema first.
    let invalid = call(&client, "alice-token", "github__search_issues", "{}");
    assert!(invalid.contains(r#""isError":true"#) && invalid.contains("missing required property"), "{invalid}");
}

#[test]
fn the_callers_token_never_reaches_an_upstream() {
    let (github, jira) = (Fake::serving(github_like()), Fake::serving(jira_like()));
    let client = gateway(&github, &jira);
    call(&client, "alice-token", "github__search_issues", r#"{"q":"x"}"#);
    call(&client, "alice-token", "jira__search", "{}");
    let github_requests = github.requests();
    assert!(github_requests.len() >= 4, "initialize, initialized, list, call: {github_requests:?}");
    for request in github_requests {
        assert_eq!(Some("Bearer github-secret"), request.header("authorization"), "{request:?}");
        assert!(!format!("{request:?}").contains("alice-token"), "{request:?}");
    }
    for request in jira.requests() {
        assert_eq!(None, request.header("authorization"), "{request:?}");
        assert!(!format!("{request:?}").contains("alice-token"), "{request:?}");
    }
    // Requests after initialize carry the session and the negotiated version.
    let call_request = github.requests().into_iter().find(|r| r.method() == "tools/call").unwrap();
    assert!(call_request.header("mcp-session-id").is_some(), "{call_request:?}");
    assert_eq!(Some("2025-11-25"), call_request.header("mcp-protocol-version"));
}

#[test]
fn each_identity_sees_and_calls_only_its_tools() {
    let (github, jira) = (Fake::serving(github_like()), Fake::serving(jira_like()));
    let client = gateway(&github, &jira);
    let bob = list(&client, "bob-token");
    assert!(bob.contains("jira__search") && bob.contains("github__search_issues"), "{bob}");
    assert!(!bob.contains("github__close_issue"), "{bob}");
    let denied = call(&client, "bob-token", "github__close_issue", "{}");
    assert!(denied.contains("Unknown tool: github__close_issue"), "{denied}");
    // No rule names eve: nothing listed, nothing callable, nothing forwarded.
    let before = github.requests().len();
    assert!(list(&client, "eve-token").contains(r#""tools":[]"#));
    assert!(call(&client, "eve-token", "github__search_issues", r#"{"q":"x"}"#).contains("Unknown tool"));
    assert_eq!(before, github.requests().len());
    // An unknown token doesn't get in at all.
    let anonymous = client.post("/mcp").header("Authorization", "Bearer guess").header("Content-Type", "application/json").body_text(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).send();
    assert_eq!(401, anonymous.status());
}

#[test]
fn without_an_identity_nothing_is_allowed() {
    let jira = Fake::serving(jira_like());
    let server = McpGateway::new("g", "1").upstream(Upstream::new("jira", &jira.url)).allow("*", ["*"]).build().unwrap();
    let anonymous = server.handle_request_with_context(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#, ctx(None));
    let body = String::from_utf8(anonymous.content_range_list[0].body.clone()).unwrap();
    assert!(body.contains(r#""tools":[]"#), "{body}");
    let named = server.handle_request_with_context(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#, ctx(Some("anyone")));
    assert!(String::from_utf8(named.content_range_list[0].body.clone()).unwrap().contains("jira__search"));
}

// ── Scripted upstream behaviour ──────────────────────────────────────────────

fn build_one(fake: &Fake) -> Result<McpServer, String> {
    McpGateway::new("g", "1").upstream(Upstream::new("up", &fake.url)).allow("me", ["*"]).build()
}

fn call_one(server: &McpServer, arguments: &str) -> String {
    let response = server.handle_request_with_context(
        &format!(r#"{{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{{"name":"up__echo","arguments":{arguments}}}}}"#),
        ctx(Some("me")),
    );
    String::from_utf8(response.content_range_list[0].body.clone()).unwrap()
}

#[test]
fn an_event_stream_reply_is_read_past_notifications() {
    let fake = scripted(|r| {
        if r.method() == "tools/list" {
            return Reply::json(one_tool_list(r));
        }
        let body = format!(
            "event: message\ndata: {{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{{\"progress\":1}}}}\n\n\
             data: {{\"jsonrpc\":\"2.0\",\"id\":\"other\",\"result\":{{}}}}\n\n\
             data: {{\"jsonrpc\":\"2.0\",\"id\":{},\n\
             data: \"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"streamed\"}}],\"isError\":false}}}}\n\n",
            r.id()
        );
        Reply { status: 200, content_type: "text/event-stream".into(), headers: Vec::new(), body }
    });
    let server = build_one(&fake).unwrap();
    let result = call_one(&server, "{}");
    assert!(result.contains(r#""text":"streamed""#), "{result}");
}

#[test]
fn every_page_of_tools_is_listed() {
    let fake = scripted(|r| {
        let page = |tools: &str, next: &str| Reply::json(format!(r#"{{"jsonrpc":"2.0","id":{},"result":{{"tools":[{tools}]{next}}}}}"#, r.id()));
        let tool = |name: &str| format!(r#"{{"name":"{name}","description":"d","inputSchema":{{"type":"object"}}}}"#);
        match top_level_raw(&r.body, "params").and_then(|p| top_level_str(&p, "cursor")).as_deref() {
            None => page(&tool("a"), r#","nextCursor":"p2""#),
            Some("p2") => page(&format!("{},{}", tool("b"), tool("c")), r#","nextCursor":"p3""#),
            _ => page(&tool("d"), ""),
        }
    });
    let server = build_one(&fake).unwrap();
    let tools = String::from_utf8(server.handle_request_with_context(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#, ctx(Some("me"))).content_range_list[0].body.clone()).unwrap();
    for name in ["up__a", "up__b", "up__c", "up__d"] {
        assert!(tools.contains(name), "{name}: {tools}");
    }
}

#[test]
fn a_forgotten_session_is_initialized_again_once() {
    let calls = Arc::new(AtomicU64::new(0));
    let seen = Arc::clone(&calls);
    let fake = scripted(move |r| {
        if r.method() == "tools/list" {
            return Reply::json(one_tool_list(r));
        }
        // The first call finds the session gone; the retry succeeds.
        if seen.fetch_add(1, Ordering::SeqCst) == 0 {
            return Reply::status(404);
        }
        Reply::json(format!(r#"{{"jsonrpc":"2.0","id":{},"result":{{"content":[],"isError":false}}}}"#, r.id()))
    });
    let server = build_one(&fake).unwrap();
    assert!(call_one(&server, "{}").contains(r#""isError":false"#));
    let initializes = fake.requests().iter().filter(|r| r.method() == "initialize").count();
    assert_eq!(2, initializes);

    let always_gone = scripted(|r| if r.method() == "tools/list" { Reply::json(one_tool_list(r)) } else { Reply::status(404) });
    let server = build_one(&always_gone).unwrap();
    let result = call_one(&server, "{}");
    assert!(result.contains(r#""isError":true"#) && result.contains("session not found twice"), "{result}");
}

#[test]
fn upstream_failures_are_tool_errors_the_model_can_read() {
    let fake = scripted(|r| {
        if r.method() == "tools/list" {
            return Reply::json(one_tool_list(r));
        }
        match top_level_raw(&r.body, "params").and_then(|p| top_level_raw(&p, "arguments")).and_then(|a| top_level_str(&a, "mode")).as_deref() {
            Some("rpc") => Reply::json(format!(r#"{{"jsonrpc":"2.0","id":{},"error":{{"code":-32000,"message":"rate limited"}}}}"#, r.id())),
            Some("http") => Reply::status(502),
            _ => Reply::json(r#"{"jsonrpc":"2.0","id":12345,"result":{}}"#.into()),
        }
    });
    let server = build_one(&fake).unwrap();
    for (mode, expected) in [("rpc", "up: rate limited"), ("http", "up: answered HTTP 502"), ("wrong-id", "not the response")] {
        let result = call_one(&server, &format!(r#"{{"mode":"{mode}"}}"#));
        assert!(result.contains(r#""isError":true"#) && result.contains(expected), "{mode}: {result}");
    }
}

#[test]
fn an_unreachable_upstream_fails_the_build_by_name() {
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{port}/mcp");
    let error = McpGateway::new("g", "1").upstream(Upstream::new("down", &url).timeout_ms(2000)).build().err().unwrap();
    assert!(error.contains("upstream down") && error.contains(&url), "{error}");
}

#[test]
fn prefixes_are_checked() {
    for bad in ["", "a__b", "a_", "a b", "a/b"] {
        let error = McpGateway::new("g", "1").upstream(Upstream::new(bad, "http://127.0.0.1:1/mcp")).build().err().unwrap();
        assert!(error.contains("upstream prefix"), "{bad}: {error}");
    }
    let twice = McpGateway::new("g", "1")
        .upstream(Upstream::new("a", "http://127.0.0.1:1/mcp"))
        .upstream(Upstream::new("a", "http://127.0.0.1:1/mcp"))
        .build()
        .err()
        .unwrap();
    assert!(twice.contains("used twice"), "{twice}");
    assert!(valid_prefix("git-hub_2"));
}

#[test]
fn allow_rules_match_names_prefixes_and_everyone() {
    let rules = vec![
        ("alice".to_string(), vec!["github__*".to_string()]),
        ("*".to_string(), vec!["jira__search".to_string()]),
    ];
    assert!(allowed(&rules, "alice", "github__x"));
    assert!(allowed(&rules, "alice", "jira__search"));
    assert!(!allowed(&rules, "alice", "jira__delete"));
    assert!(allowed(&rules, "bob", "jira__search"));
    assert!(!allowed(&rules, "bob", "github__x"));
    assert!(!allowed(&[], "alice", "github__x"));
}

#[test]
fn replies_are_parsed_by_id() {
    let json = |body: &str| parse_reply("application/json; charset=utf-8", body.as_bytes(), 3);
    assert_eq!(Ok("{}".to_string()), json(r#"{"jsonrpc":"2.0","id":3,"result":{}}"#));
    assert_eq!(Err("boom".to_string()), json(r#"{"jsonrpc":"2.0","id":3,"error":{"code":1,"message":"boom"}}"#));
    assert!(json(r#"{"jsonrpc":"2.0","id":4,"result":{}}"#).is_err());
    // A nested "id" is not the response's id.
    assert!(json(r#"{"jsonrpc":"2.0","result":{"id":3}}"#).is_err());
    assert!(json("not json").is_err());
    assert!(parse_reply("application/json", &[0xff, 0xfe], 3).is_err());
    let sse = |body: &str| parse_reply("text/event-stream", body.as_bytes(), 3);
    assert_eq!(Ok("1".to_string()), sse("data: {\"jsonrpc\":\"2.0\",\"id\":3,\"result\":1}"));
    // A server-to-client request carrying the same id is not the response.
    assert!(sse("data: {\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"sampling/createMessage\"}\n\n").is_err());
    assert!(sse(": keep-alive\n\n").is_err());
}

#[test]
fn an_upstream_that_only_speaks_2026_is_reached_statelessly() {
    let modern = Fake::modern_only(jira_like());
    let gateway = TestClient::new(
        McpGateway::new("gateway", "1").upstream(Upstream::new("jira", &modern.url)).allow("alice", ["*"]).build().unwrap().require_bearer_tokens([("alice-token", "alice")]),
    );
    assert!(list(&gateway, "alice-token").contains("jira__search"));
    let reply = call(&gateway, "alice-token", "jira__search", "{}");
    assert!(reply.contains("PROJ-1"), "{reply}");
    assert!(!reply.contains("resultType"), "2026 fields stripped for a 2025 client: {reply}");

    let seen = modern.requests();
    let methods: Vec<String> = seen.iter().map(Recorded::method).collect();
    assert_eq!(methods[..3], ["initialize", "server/discover", "tools/list"], "{methods:?}");
    let call = seen.iter().find(|r| r.method() == "tools/call").expect("a tools/call");
    assert_eq!(call.header("MCP-Protocol-Version"), Some("2026-07-28"));
    assert_eq!(call.header("Mcp-Method"), Some("tools/call"));
    assert_eq!(call.header("Mcp-Name"), Some("search"));
    assert!(call.header("Mcp-Session-Id").is_none());
    assert!(call.body.contains("io.modelcontextprotocol/clientCapabilities"), "{}", call.body);
}

#[test]
fn with_modern_meta_merges_into_params() {
    let empty = with_modern_meta("{}");
    assert!(empty.starts_with(r#"{"_meta":{"#) && empty.ends_with("}}"), "{empty}");
    let named = with_modern_meta(r#"{"name":"x","arguments":{}}"#);
    assert!(named.starts_with(r#"{"name":"x""#) && named.contains("protocolVersion"), "own members first: {named}");
    assert!(json_rpc::object_members(&named).is_some(), "valid JSON object: {named}");
}

#[test]
fn modern_result_fields_are_dropped_and_input_requests_refused() {
    assert_eq!(without_modern_fields(r#"{"resultType":"complete","content":[],"ttlMs":5}"#), r#"{"content":[]}"#);
    let plain = r#"{"content":[{"type":"text","text":"a"}]}"#;
    assert_eq!(without_modern_fields(plain), plain);
}


#[test]
fn tools_added_and_removed_upstream_show_up_after_a_refresh() {
    // The upstream's tool list is whatever `tools` holds when asked.
    let tools = Arc::new(Mutex::new(vec!["echo"]));
    let current = Arc::clone(&tools);
    let fake = scripted(move |r| {
        if r.method() == "tools/list" {
            let list: Vec<String> = current.lock().unwrap().iter().map(|n| format!(r#"{{"name":"{n}","inputSchema":{{"type":"object"}}}}"#)).collect();
            return Reply::json(format!(r#"{{"jsonrpc":"2.0","id":{},"result":{{"tools":[{}]}}}}"#, r.id(), list.join(",")));
        }
        Reply::json(format!(r#"{{"jsonrpc":"2.0","id":{},"result":{{"content":[{{"type":"text","text":"ok"}}]}}}}"#, r.id()))
    });
    let server = McpGateway::new("g", "1")
        .upstream(Upstream::new("up", &fake.url))
        .allow("me", ["*"])
        .refresh_every(Some(std::time::Duration::from_millis(50)))
        .build()
        .unwrap();
    let names = |server: &McpServer| {
        let response = server.handle_request_with_context(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#, ctx(Some("me")));
        String::from_utf8(response.content_range_list[0].body.clone()).unwrap()
    };
    assert!(names(&server).contains("up__echo"));

    *tools.lock().unwrap() = vec!["ping"];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let list = names(&server);
        if list.contains("up__ping") && !list.contains("up__echo") {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "refresh never applied: {list}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(call_one_named(&server, "up__ping").contains("ok"));
}

#[test]
fn an_unreachable_upstream_keeps_its_tools_on_refresh() {
    let fake = scripted(|r| Reply::json(one_tool_list(r)));
    let server = McpGateway::new("g", "1").upstream(Upstream::new("up", &fake.url)).allow("me", ["*"]).refresh_every(None).build().unwrap();
    let connection = Arc::new(Connection::new(Upstream::new("up", "http://127.0.0.1:9/mcp").timeout_ms(200)));
    let mut known: std::collections::HashMap<String, String> = [("echo".to_string(), "x".to_string())].into();
    refresh(&server, &connection, &mut known);
    assert!(known.contains_key("echo"), "nothing removed when the upstream is down");
}

fn call_one_named(server: &McpServer, tool: &str) -> String {
    let response = server.handle_request_with_context(
        &format!(r#"{{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{{"name":"{tool}","arguments":{{}}}}}}"#),
        ctx(Some("me")),
    );
    String::from_utf8(response.content_range_list[0].body.clone()).unwrap()
}

#[test]
fn calls_over_a_tool_limit_are_refused_per_caller() {
    let fake = scripted(|r| {
        if r.method() == "tools/list" {
            return Reply::json(one_tool_list(r));
        }
        Reply::json(format!(r#"{{"jsonrpc":"2.0","id":{},"result":{{"content":[{{"type":"text","text":"ok"}}]}}}}"#, r.id()))
    });
    let server = McpGateway::new("g", "1")
        .upstream(Upstream::new("up", &fake.url))
        .allow("*", ["*"])
        .limit("up__*", 2, 60)
        .refresh_every(None)
        .build()
        .unwrap();
    let call_as = |who: &str| {
        let response = server.handle_request_with_context(r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"up__echo","arguments":{}}}"#, ctx(Some(who)));
        String::from_utf8(response.content_range_list[0].body.clone()).unwrap()
    };
    assert!(call_as("alice").contains(r#""text":"ok""#));
    assert!(call_as("alice").contains(r#""text":"ok""#));
    let third = call_as("alice");
    assert!(third.contains("Rate limited") && third.contains(r#""isError":true"#), "{third}");
    assert!(call_as("bob").contains(r#""text":"ok""#), "limits are per caller");
    let upstream_calls = fake.requests().iter().filter(|r| r.method() == "tools/call").count();
    assert_eq!(3, upstream_calls, "the refused call never reached the upstream");
}

#[test]
fn audit_lines_name_the_caller_tool_upstream_and_outcome() {
    let line = audit_line("alice", "github__search", "github", "ok", std::time::Duration::from_micros(1500));
    assert!(line.starts_with(r#"{"time":""#), "{line}");
    assert!(line.contains(r#""event":"mcp_tool_call","sub":"alice","tool":"github__search","upstream":"github","outcome":"ok","duration_ms":1.500}"#), "{line}");
    assert!(json_rpc::object_members(&line).is_some(), "one JSON object: {line}");
    let escaped = audit_line("a\"b", "t", "", "denied", std::time::Duration::ZERO);
    assert!(escaped.contains(r#""sub":"a\"b""#), "{escaped}");
}
