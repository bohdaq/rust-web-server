//! The dev MCP endpoint answers every tool it lists — the check CI runs
//! for each starter: `initialize`, `tools/list`, then `tools/call` on each
//! tool, none answering with an error.

use rust_web_server::application::Application;
use rust_web_server::dev_mcp::{DevMcpExt, RwsEnv};
use rust_web_server::test_client::TestClient;
use serde_json::{json, Value};
use {{crate_name}}::{app, dev_tools, Store};

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
    client_for(Store::memory(), false)
}

fn client_for(store: Store, read_only: bool) -> TestClient<impl Application> {
    let mut tools = dev_tools(&store, "migrations").env(RwsEnv::Dev).token(TOKEN);
    if read_only {
        tools = tools.read_only();
    }
    let served = app(store).dev_mcp_with(tools);
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

/// `tools/list`, then `tools/call` on every tool listed; their names.
fn answer_every_tool<A: Application>(client: &TestClient<A>) -> Vec<String> {
    let session = start(client);
    let (status, list, _) = rpc(client, Some(&session), json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert_eq!(200, status, "{list}");
    let names: Vec<String> = list["result"]["tools"]
        .as_array()
        .expect("a tool list")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap_or_default().to_string())
        .collect();
    // The generators write files: `generators_plan_without_writing` calls
    // them with `dry_run`.
    for (i, name) in names.iter().enumerate().filter(|(_, n)| !n.starts_with("gen_")) {
        call(client, &session, 10 + i, name);
    }
    names
}

#[test]
fn every_dev_mcp_tool_answers() {
    let names = answer_every_tool(&client());
    for expected in ["routes", "config", "layout", "models", "recent_requests", "recent_errors"] {
        assert!(names.iter().any(|n| n == expected), "{expected} missing from {names:?}");
    }
}

/// With PostgreSQL (CI runs one) the database tools too; read-only, so
/// the run doesn't roll back the schema under the other tests.
#[tokio::test(flavor = "multi_thread")]
async fn every_dev_mcp_tool_answers_with_postgres() {
    if std::env::var("PGHOST").is_err() {
        return;
    }
    let store = Store::connect("migrations").await.expect("connect");
    let names = tokio::task::spawn_blocking(move || answer_every_tool(&client_for(store, true))).await.unwrap();
    for expected in ["models", "db_schema", "migrations"] {
        assert!(names.iter().any(|n| n == expected), "{expected} missing from {names:?}");
    }
}

#[test]
fn routes_models_and_layout_describe_this_project() {
    let client = client();
    let session = start(&client);
    let routes = call(&client, &session, 2, "routes");
    let patterns: Vec<&str> = routes["routes"].as_array().unwrap().iter().filter_map(|r| r["pattern"].as_str()).collect();
    assert!(patterns.contains(&"/api/notes/:id") && patterns.contains(&"/healthz"), "{routes}");

    let models = call(&client, &session, 3, "models");
    assert_eq!(models["models"][0]["table"], "notes", "{models}");
    assert_eq!(models["models"][0]["primary_key"], "id", "{models}");

    let layout = call(&client, &session, 4, "layout");
    assert_eq!(layout["manifest"], "rws.app.toml", "{layout}");
    assert_eq!(layout["error"], Value::Null, "{layout}");
    let exists = |role: &str| layout["paths"].as_array().unwrap().iter().any(|p| p["role"] == role && p["exists"] == true);
    for role in ["main", "lib", "routes", "models", "migrations", "tests"] {
        assert!(exists(role), "{role}: {layout}");
    }
}

#[test]
fn off_in_production_without_a_token() {
    let tools = dev_tools(&Store::memory(), "migrations").env(RwsEnv::Prod).token_env("STARTER_TEST_TOKEN_NEVER_SET");
    let served = app(Store::memory()).dev_mcp_with(tools);
    assert!(!served.is_mounted());
    assert_ne!(200, TestClient::new(served).post(PATH).body_json("{}").send().status());
}

/// The generators (`rws gen` as tools) with `dry_run`: a plan, no files.
#[test]
fn generators_plan_without_writing() {
    let client = client();
    let session = start(&client);
    let plan = |id: usize, name: &str, arguments: Value| {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": arguments}});
        let (status, answer, _) = rpc(&client, Some(&session), request);
        assert_eq!(200, status, "{name}: {answer}");
        let text = answer["result"]["content"][0]["text"].as_str().unwrap_or("null");
        (answer["result"]["isError"] == json!(true), serde_json::from_str::<Value>(text).unwrap_or(Value::Null))
    };
    let (failed, migration) = plan(2, "gen_migration", json!({"name": "probe_generators", "dry_run": true}));
    assert!(!failed && migration["status"] == "planned", "{migration}");
    let (failed, route) = plan(3, "gen_route", json!({"resource": "probes", "fields": ["title:String"], "dry_run": true}));
    assert!(!failed && route["status"] == "planned", "{route}");
    let (failed, bad) = plan(4, "gen_model", json!({"name": "Probe", "fields": ["title:u8"], "dry_run": true}));
    assert!(failed && bad["status"] == "usage", "{bad}");
    let dir = env!("CARGO_MANIFEST_DIR");
    assert!(!std::path::Path::new(&format!("{dir}/src/routes/probes.rs")).exists(), "a dry run writes nothing");
}
