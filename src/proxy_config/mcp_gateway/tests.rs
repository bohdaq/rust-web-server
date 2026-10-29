use super::*;
use crate::config_check::{check_str, check_str_offline};
use crate::mcp::McpContent;
use crate::proxy_config::{ActionConfig, ProxyConfig};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

const EXAMPLE: &str = r#"
[[route]]
  name = "gw"
  [route.match]
  path = "/mcp"
  [route.action]
  type = "mcp"
  [route.action.mcp]
  name = "my-gateway"
  refresh_secs = 0
  audit_log = true
  allowed_origins = ["https://app.example.com"]

  [[route.action.mcp.upstream]]
  prefix = "docs"
  url = "http://127.0.0.1:9/mcp"
  timeout_ms = 2000
  headers_env = { Authorization = "RWS_TEST_GW_UPSTREAM_AUTH" }

  [[route.action.mcp.caller]]
  identity = "alice"
  token_env = "RWS_TEST_GW_ALICE"

  [[route.action.mcp.allow]]
  identity = "alice"
  tools = ["docs__*"]

  [[route.action.mcp.limit]]
  tools = "docs__search"
  max_calls = 5
  window_secs = 60
"#;

fn gateway_of(toml: &str) -> McpGatewayConfig {
    match &ProxyConfig::from_str(toml).routes[0].action {
        ActionConfig::Mcp(g) => (**g).clone(),
        other => panic!("not an mcp action: {other:?}"),
    }
}

#[test]
fn the_whole_gateway_parses() {
    let g = gateway_of(EXAMPLE);
    assert_eq!(g.name, "my-gateway");
    assert_eq!((g.refresh_secs, g.audit_log), (0, true));
    assert_eq!(g.allowed_origins, ["https://app.example.com"]);
    assert_eq!(g.upstreams.len(), 1);
    assert_eq!((g.upstreams[0].prefix.as_str(), g.upstreams[0].timeout_ms), ("docs", 2000));
    assert_eq!(g.upstreams[0].headers_env, [("Authorization".to_string(), "RWS_TEST_GW_UPSTREAM_AUTH".to_string())]);
    assert_eq!(g.callers, [CallerConfig { identity: "alice".into(), token_env: "RWS_TEST_GW_ALICE".into() }]);
    assert_eq!(g.allow, [("alice".to_string(), vec!["docs__*".to_string()])]);
    assert_eq!(g.limits, [LimitConfig { tools: "docs__search".into(), max_calls: 5, window_secs: 60 }]);
}

#[test]
fn the_example_is_valid_offline_and_unset_secrets_fail_online() {
    let offline = check_str_offline(EXAMPLE);
    assert!(offline.errors.is_empty(), "{:?}", offline.errors);
    let online = check_str(EXAMPLE);
    let joined = online.errors.join("\n");
    assert!(joined.contains("RWS_TEST_GW_UPSTREAM_AUTH") && joined.contains("RWS_TEST_GW_ALICE"), "{joined}");
}

#[test]
fn mistakes_are_reported() {
    let bad = EXAMPLE
        .replace(r#"path = "/mcp""#, r#"path = "/mcp/*""#)
        .replace(r#"prefix = "docs""#, r#"prefix = "do__cs""#)
        .replace(r#"url = "http://127.0.0.1:9/mcp""#, r#"url = "ftp://x""#)
        .replace("timeout_ms = 2000", "timeout = 2000");
    let report = check_str_offline(&bad);
    let joined = report.errors.join("\n");
    for expected in ["exact [route.match] path", "prefix \"do__cs\"", "must start with http", "timeout: unknown key"] {
        assert!(joined.contains(expected), "missing {expected:?} in:\n{joined}");
    }
    let no_callers = EXAMPLE.replace("[[route.action.mcp.caller]]\n  identity = \"alice\"\n  token_env = \"RWS_TEST_GW_ALICE\"", "");
    assert!(check_str_offline(&no_callers).errors.iter().any(|e| e.contains("nobody can call")));
}

/// A real McpServer on a TCP port, for the end-to-end test.
fn serve(server: McpServer) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = Vec::new();
            let mut line = String::new();
            let mut length = 0usize;
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    head.extend_from_slice(b"\r\n");
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap_or(0);
                }
                head.extend_from_slice(line.as_bytes());
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            head.extend_from_slice(&body);
            let Ok(request) = Request::parse(&head) else { continue };
            let response = server.execute(&request, &ConnectionInfo::default()).unwrap();
            let part = response.content_range_list.first();
            let body = part.map(|p| p.body.clone()).unwrap_or_default();
            let mut out = format!(
                "HTTP/1.1 {} X\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                response.status_code,
                part.map(|p| p.content_type.clone()).unwrap_or_else(|| "application/json".into()),
                body.len()
            );
            for h in response.headers.iter().filter(|h| h.name.eq_ignore_ascii_case("mcp-session-id")) {
                out.push_str(&format!("{}: {}\r\n", h.name, h.value));
            }
            out.push_str("\r\n");
            let _ = stream.write_all(out.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    url
}

#[test]
fn a_config_file_gateway_federates_and_enforces_its_rules() {
    let _g = crate::test_env::lock();
    let upstream = serve(
        McpServer::new("docs", "1")
            .tool("search", "Search", r#"{"type":"object"}"#, |_| Ok(McpContent::text("hit")))
            .tool("delete", "Delete", r#"{"type":"object"}"#, |_| Ok(McpContent::text("gone"))),
    );
    std::env::set_var("RWS_TEST_GW_E2E_ALICE", "alice-token");
    std::env::set_var("RWS_TEST_GW_E2E_BOB", "bob-token");
    let toml = format!(
        r#"
[[route]]
  [route.match]
  path = "/mcp"
  [route.action]
  type = "mcp"
  [route.action.mcp]
  refresh_secs = 0
  [[route.action.mcp.upstream]]
  prefix = "docs"
  url = "{upstream}"
  [[route.action.mcp.caller]]
  identity = "alice"
  token_env = "RWS_TEST_GW_E2E_ALICE"
  [[route.action.mcp.caller]]
  identity = "bob"
  token_env = "RWS_TEST_GW_E2E_BOB"
  [[route.action.mcp.allow]]
  identity = "alice"
  tools = ["docs__*"]
  [[route.action.mcp.allow]]
  identity = "bob"
  tools = ["docs__search"]
  [[route.action.mcp.limit]]
  tools = "docs__search"
  max_calls = 1
  window_secs = 60
"#
    );
    assert!(check_str(&toml).errors.is_empty(), "{:?}", check_str(&toml).errors);
    let (app, _handles) = crate::proxy_config::builder::build(ProxyConfig::from_str(&toml));
    let post = |token: &str, body: &str| {
        let raw = format!(
            "POST /mcp HTTP/1.1\r\nHost: gw\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nMCP-Protocol-Version: 2026-07-28\r\nMcp-Method: {}\r\n{}Authorization: Bearer {token}\r\nContent-Length: {}\r\n\r\n{body}",
            crate::mcp::json_rpc::extract_str(body, "method").unwrap(),
            crate::mcp::json_rpc::extract_raw(body, "params").and_then(|p| crate::mcp::json_rpc::extract_str(&p, "name")).map(|n| format!("Mcp-Name: {n}\r\n")).unwrap_or_default(),
            body.len()
        );
        let response = app.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap();
        (response.status_code, response.content_range_list.first().map(|p| String::from_utf8(p.body.clone()).unwrap()).unwrap_or_default())
    };
    let meta = r#""_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"t","version":"1"},"io.modelcontextprotocol/clientCapabilities":{}}"#;
    let list = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{{{meta}}}}}"#);
    let call = |tool: &str| format!(r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{tool}","arguments":{{}},{meta}}}}}"#);

    // The gateway builds in the background: wait for it.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while post("alice-token", &list).0 == 503 {
        assert!(std::time::Instant::now() < deadline, "the gateway never came up");
        std::thread::sleep(Duration::from_millis(50));
    }
    let (_, alice_tools) = post("alice-token", &list);
    assert!(alice_tools.contains("docs__search") && alice_tools.contains("docs__delete"), "{alice_tools}");
    let (_, bob_tools) = post("bob-token", &list);
    assert!(bob_tools.contains("docs__search") && !bob_tools.contains("docs__delete"), "{bob_tools}");
    assert!(post("alice-token", &call("docs__delete")).1.contains("gone"));
    assert!(post("bob-token", &call("docs__search")).1.contains("hit"));
    assert!(post("bob-token", &call("docs__search")).1.contains("Rate limited"), "bob's second search is over the limit");
    assert_eq!(post("eve-token", &list).0, 401, "unknown callers are refused");
    std::env::remove_var("RWS_TEST_GW_E2E_ALICE");
    std::env::remove_var("RWS_TEST_GW_E2E_BOB");
}
