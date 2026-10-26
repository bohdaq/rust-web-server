//! A minimal MCP gateway: one authenticated `/mcp` in front of two upstream
//! MCP servers, each caller seeing only the tools it is allowed.
//!
//! ```sh
//! cargo run --example mcp_gateway
//! # alice sees every tool, bob only the forecast:
//! curl -s localhost:7878/mcp -H 'Authorization: Bearer alice-token' -H 'Content-Type: application/json' \
//!   -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
//! curl -s localhost:7878/mcp -H 'Authorization: Bearer bob-token' -H 'Content-Type: application/json' \
//!   -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
//! ```
//!
//! The two upstreams run in this process on their own ports, to need no
//! setup; point `Upstream::new` at real servers instead. See
//! docs/src/content/docs/mcp/gateway.md.

use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use rust_web_server::mcp::gateway::{McpGateway, Upstream};
use rust_web_server::mcp::{McpContent, McpServer, ToolAnnotations};
use rust_web_server::server::Server;

/// The weather upstream's own credential: only the gateway knows it.
const WEATHER_SECRET: &str = "weather-upstream-secret";

/// A notes server (no auth of its own).
fn notes() -> McpServer {
    let notes = Arc::new(Mutex::new(Vec::<String>::new()));
    let add = Arc::clone(&notes);
    McpServer::new("notes", "1.0")
        .tool(
            "add",
            "Save a note",
            r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
            move |args| {
                let text = rust_web_server::mcp::extract_arg(args, "text").unwrap_or_default();
                add.lock().unwrap().push(text);
                Ok(McpContent::text("saved"))
            },
        )
        .tool_annotated(
            "list",
            "List saved notes",
            r#"{"type":"object"}"#,
            ToolAnnotations { read_only_hint: Some(true), ..Default::default() },
            move |_| Ok(McpContent::text(notes.lock().unwrap().join("\n"))),
        )
}

/// A weather server that wants its own bearer token.
fn weather() -> McpServer {
    McpServer::new("weather", "1.0").require_bearer(WEATHER_SECRET).tool_annotated(
        "forecast",
        "Tomorrow's forecast for a city",
        r#"{"type":"object","properties":{"city":{"type":"string"}},"required":["city"]}"#,
        ToolAnnotations { read_only_hint: Some(true), ..Default::default() },
        |args| {
            let city = rust_web_server::mcp::extract_arg(args, "city").unwrap_or_default();
            Ok(McpContent::structured(format!(r#"{{"city":"{city}","high_c":21,"sky":"clear"}}"#)))
        },
    )
}

/// The gateway in front of `notes_url` and `weather_url`.
pub fn gateway(notes_url: &str, weather_url: &str) -> Result<McpServer, String> {
    Ok(McpGateway::new("gateway", "1.0")
        .upstream(Upstream::new("notes", notes_url))
        .upstream(Upstream::new("weather", weather_url).header("Authorization", format!("Bearer {WEATHER_SECRET}")))
        .allow("alice", ["*"])
        .allow("bob", ["weather__*"])
        .build()?
        .require_bearer_tokens([("alice-token", "alice"), ("bob-token", "bob")]))
}

/// Serves `server` on a free local port; returns its MCP URL.
fn serve_upstream(server: McpServer) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    tokio::spawn(Server::run_plain(listener, server));
    url
}

#[tokio::main]
async fn main() {
    let notes_url = serve_upstream(notes());
    let weather_url = serve_upstream(weather());
    // build() talks to the upstreams with a blocking client.
    let built = tokio::task::spawn_blocking(move || gateway(&notes_url, &weather_url)).await.unwrap();
    let server = match built {
        Ok(server) => server,
        Err(error) => {
            eprintln!("mcp_gateway: {error}");
            std::process::exit(2);
        }
    };
    println!("MCP gateway on /mcp — tokens: alice-token (all tools), bob-token (weather only)");
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, server).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    /// The upstreams on a runtime of their own, for the test's lifetime.
    fn upstreams() -> (String, String) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                tx.send((serve_upstream(notes()), serve_upstream(weather()))).unwrap();
                std::future::pending::<()>().await;
            });
        });
        rx.recv().unwrap()
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

    #[test]
    fn each_caller_gets_its_tools_and_upstream_credentials_stay_inside() {
        let (notes_url, weather_url) = upstreams();
        let client = TestClient::new(gateway(&notes_url, &weather_url).unwrap());
        let list = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;

        let alice = rpc(&client, "alice-token", list);
        for tool in ["notes__add", "notes__list", "weather__forecast"] {
            assert!(alice.contains(tool), "{tool}: {alice}");
        }
        let bob = rpc(&client, "bob-token", list);
        assert!(bob.contains("weather__forecast") && !bob.contains("notes__"), "{bob}");

        // The weather upstream wants its own token; the gateway sends it.
        let forecast = rpc(&client, "bob-token", r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"weather__forecast","arguments":{"city":"Lviv"}}}"#);
        assert!(forecast.contains(r#""structuredContent":{"city":"Lviv","high_c":21,"sky":"clear"}"#), "{forecast}");
        let denied = rpc(&client, "bob-token", r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"notes__add","arguments":{"text":"x"}}}"#);
        assert!(denied.contains("Unknown tool"), "{denied}");

        rpc(&client, "alice-token", r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"notes__add","arguments":{"text":"buy milk"}}}"#);
        let notes = rpc(&client, "alice-token", r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"notes__list","arguments":{}}}"#);
        assert!(notes.contains("buy milk"), "{notes}");

        // Without the secret, the weather upstream turns callers away.
        let direct = rust_web_server::http_client::Client::new()
            .post(&weather_url)
            .header("Content-Type", "application/json")
            .body(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_vec())
            .send()
            .unwrap();
        assert_eq!(401, direct.status());
    }
}
