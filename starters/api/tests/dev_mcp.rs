//! The dev MCP endpoint answers every tool it lists — the check CI runs
//! for each starter: `initialize`, `tools/list`, then `tools/call` on each
//! tool, none answering with an error.

use rust_web_server::application::Application;
use rust_web_server::dev_mcp::{DevMcpExt, RwsEnv};
use rust_web_server::test_client::TestClient;
use serde_json::{json, Value};

/// Where `dev_tools()` mounts the endpoint.
const PATH: &str = "/mcp";
const TOKEN: &str = "dev-mcp-test-token";

/// One JSON-RPC request; the answer and the session id it carried.
fn rpc<A: Application>(client: &TestClient<A>, session: Option<&str>, body: Value) -> (i16, Value, Option<String>) {
    let mut request = client
        .post(PATH)
        .header("Authorization", &format!("Bearer {TOKEN}"))
        .header("Accept", "application/json, text/event-stream")
        .body_json(&body.to_string());
    if let Some(id) = session {
        request = request.header("Mcp-Session-Id", id).header("MCP-Protocol-Version", "2025-11-25");
    }
    let response = request.send();
    let answer = serde_json::from_slice(response.body_bytes()).unwrap_or(Value::Null);
    (response.status(), answer, response.header("mcp-session-id").map(str::to_string))
}

fn client() -> TestClient<impl Application> {
    let served = {{crate_name}}::app().dev_mcp_with({{crate_name}}::dev_tools().env(RwsEnv::Dev).token(TOKEN));
    assert!(served.is_mounted());
    TestClient::new(served)
}

/// `initialize` and `notifications/initialized`; the session id.
fn start<A: Application>(client: &TestClient<A>) -> String {
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "starter-test", "version": "1"}
    }});
    let (status, answer, session) = rpc(client, None, init);
    assert_eq!(200, status, "{answer}");
    assert!(answer["result"]["serverInfo"]["name"].is_string(), "{answer}");
    let session = session.expect("initialize answers with a session id");
    let (status, _, _) = rpc(client, Some(&session), json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    assert_eq!(202, status);
    session
}

/// `tools/call` with no arguments: the tool's JSON answer.
fn call<A: Application>(client: &TestClient<A>, session: &str, id: usize, name: &str) -> Value {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": {}}});
    let (status, answer, _) = rpc(client, Some(session), request);
    assert_eq!(200, status, "{name}: {answer}");
    assert!(answer.get("error").is_none(), "{name}: a JSON-RPC error: {answer}");
    assert_ne!(answer["result"]["isError"], json!(true), "{name}: a tool error: {answer}");
    let text = answer["result"]["content"][0]["text"].as_str().unwrap_or("null");
    serde_json::from_str(text).unwrap_or(Value::String(text.to_string()))
}

#[test]
fn every_dev_mcp_tool_answers() {
    let client = client();
    let session = start(&client);
    let (status, list, _) = rpc(&client, Some(&session), json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert_eq!(200, status, "{list}");
    let names: Vec<String> = list["result"]["tools"]
        .as_array()
        .expect("a tool list")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap_or_default().to_string())
        .collect();
    for expected in ["routes", "config", "layout", "recent_requests", "recent_errors"] {
        assert!(names.iter().any(|n| n == expected), "{expected} missing from {names:?}");
    }
    for (i, name) in names.iter().enumerate() {
        call(&client, &session, 10 + i, name);
    }
}

#[test]
fn routes_and_layout_describe_this_project() {
    let client = client();
    let session = start(&client);
    let routes = call(&client, &session, 2, "routes");
    let patterns: Vec<&str> = routes["routes"].as_array().unwrap().iter().filter_map(|r| r["pattern"].as_str()).collect();
    assert!(patterns.contains(&"/todos/:id"), "{routes}");

    let layout = call(&client, &session, 3, "layout");
    assert_eq!(layout["manifest"], "rws.app.toml", "{layout}");
    assert_eq!(layout["error"], Value::Null, "{layout}");
    let exists = |role: &str| layout["paths"].as_array().unwrap().iter().any(|p| p["role"] == role && p["exists"] == true);
    for role in ["main", "lib", "routes", "tests"] {
        assert!(exists(role), "{role}: {layout}");
    }
}

#[test]
fn off_in_production_without_a_token() {
    let tools = {{crate_name}}::dev_tools().env(RwsEnv::Prod).token_env("STARTER_TEST_TOKEN_NEVER_SET");
    let served = {{crate_name}}::app().dev_mcp_with(tools);
    assert!(!served.is_mounted());
    assert_ne!(200, TestClient::new(served).post(PATH).body_json("{}").send().status());
}
