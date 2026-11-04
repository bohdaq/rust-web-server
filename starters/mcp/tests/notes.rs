//! The tools, through `TestClient`: no socket, no server.

use {{crate_name}}::{server, Store};
use rust_web_server::mcp::McpServer;
use rust_web_server::test_client::TestClient;
use serde_json::{json, Value};

const TOKEN: &str = "test-token";

/// `tools/call`: the JSON-RPC answer.
fn call(client: &TestClient<McpServer>, tool: &str, arguments: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": tool, "arguments": arguments}});
    let response = client
        .post("/mcp")
        .header("Authorization", &format!("Bearer {TOKEN}"))
        .header("Accept", "application/json, text/event-stream")
        .body_json(&body.to_string())
        .send();
    assert_eq!(200, response.status(), "{}", response.body_text());
    serde_json::from_slice(response.body_bytes()).unwrap()
}

/// A tool's structured result.
fn result(answer: &Value) -> &Value {
    &answer["result"]["structuredContent"]
}

#[test]
fn notes_are_added_and_found() {
    let client = TestClient::new(server(Store::default(), TOKEN));
    let added = call(&client, "add_note", json!({"text": "Buy oat milk"}));
    assert_eq!(result(&added)["id"], 1, "{added}");
    let found = call(&client, "search_notes", json!({"query": "milk"}));
    assert_eq!(result(&found)["notes"][0]["text"], "Buy oat milk", "{found}");
    let none = call(&client, "search_notes", json!({"query": "bread"}));
    assert_eq!(result(&none)["notes"], json!([]), "{none}");
}

#[test]
fn a_bad_argument_is_a_tool_error_not_a_crash() {
    let client = TestClient::new(server(Store::default(), TOKEN));
    let answer = call(&client, "add_note", json!({"txt": "typo"}));
    assert_eq!(answer["result"]["isError"], true, "{answer}");
}

#[test]
fn mcp_needs_the_token() {
    let client = TestClient::new(server(Store::default(), TOKEN));
    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let response = client.post("/mcp").body_json(&list.to_string()).send();
    assert_eq!(401, response.status());
}
