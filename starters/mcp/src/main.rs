//! An MCP server on rust-web-server: typed tools an AI agent can call (input
//! and output schemas from serde types), a resource, a bearer token, and
//! tests that need no socket. Start here; see AGENTS.md.
//!
//! ```sh
//! MCP_TOKEN=$(openssl rand -hex 32) cargo run
//! claude mcp add --transport http notes http://127.0.0.1:7878/mcp --header "Authorization: Bearer $MCP_TOKEN"
//! ```

use std::sync::{Arc, Mutex};

use rust_web_server::mcp::{McpContent, McpServer};
use rust_web_server::prelude::*;
use rust_web_server::JsonSchema;
use serde::{Deserialize, Serialize};

/// A note.
#[derive(Clone, Serialize, JsonSchema)]
pub struct Note {
    pub id: usize,
    pub text: String,
}

/// What to remember.
#[derive(Deserialize, JsonSchema)]
pub struct AddNote {
    /// The note's text.
    pub text: String,
}

/// What to look for.
#[derive(Deserialize, JsonSchema)]
pub struct Search {
    /// Words the note must contain (case-insensitive).
    pub query: String,
}

/// The matching notes.
#[derive(Serialize, JsonSchema)]
pub struct Notes {
    pub notes: Vec<Note>,
}

pub type Store = Arc<Mutex<Vec<Note>>>;

/// The server: every tool and resource, behind `token`.
pub fn server(store: Store, token: &str) -> McpServer {
    let (add, search, read) = (Arc::clone(&store), Arc::clone(&store), store);
    McpServer::new("notes", env!("CARGO_PKG_VERSION"))
        .require_bearer(token)
        .typed_tool("add_note", "Remember a note; returns it with its id.", move |_, args: AddNote| {
            let mut notes = add.lock().unwrap();
            let note = Note { id: notes.len() + 1, text: args.text };
            notes.push(note.clone());
            Ok(note)
        })
        .typed_tool("search_notes", "Find notes containing the query.", move |_, args: Search| {
            let query = args.query.to_lowercase();
            let notes = search.lock().unwrap().iter().filter(|n| n.text.to_lowercase().contains(&query)).cloned().collect();
            Ok(Notes { notes })
        })
        .resource("notes://{id}", "note", "One note by id.", move |uri| {
            let id: usize = uri.trim_start_matches("notes://").parse().map_err(|_| format!("not a note: {uri}"))?;
            let notes = read.lock().unwrap();
            let note = notes.iter().find(|n| n.id == id).ok_or_else(|| format!("no note {id}"))?;
            Ok(McpContent::text(note.text.clone()))
        })
}

#[tokio::main]
async fn main() {
    let token = std::env::var("MCP_TOKEN").unwrap_or_default();
    if token.trim().is_empty() {
        eprintln!("set MCP_TOKEN (e.g. MCP_TOKEN=$(openssl rand -hex 32)) — /mcp never runs without a token");
        std::process::exit(2);
    }
    let (listener, pool) = Server::setup().expect("server setup");
    Server::run_tls(listener, pool, server(Store::default(), token.trim())).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    const TOKEN: &str = "test-token";

    fn call(client: &TestClient<McpServer>, tool: &str, arguments: &str) -> String {
        let body = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{tool}","arguments":{arguments}}}}}"#);
        let response = client
            .post("/mcp")
            .header("Authorization", &format!("Bearer {TOKEN}"))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body_text(&body)
            .send();
        assert_eq!(200, response.status(), "{}", response.body_text());
        response.body_text().to_string()
    }

    #[test]
    fn notes_are_added_and_found() {
        let client = TestClient::new(server(Store::default(), TOKEN));
        let added = call(&client, "add_note", r#"{"text":"Buy oat milk"}"#);
        assert!(added.contains(r#"\"id\":1"#) || added.contains(r#""id":1"#), "{added}");
        let found = call(&client, "search_notes", r#"{"query":"milk"}"#);
        assert!(found.contains("Buy oat milk"), "{found}");
        let none = call(&client, "search_notes", r#"{"query":"bread"}"#);
        assert!(!none.contains("Buy oat milk"), "{none}");
    }

    #[test]
    fn a_bad_argument_is_a_tool_error_not_a_crash() {
        let client = TestClient::new(server(Store::default(), TOKEN));
        let answer = call(&client, "add_note", r#"{"txt":"typo"}"#);
        assert!(answer.contains(r#""isError":true"#), "{answer}");
    }

    #[test]
    fn mcp_needs_the_token() {
        let client = TestClient::new(server(Store::default(), TOKEN));
        let response = client.post("/mcp").header("Content-Type", "application/json").body_text(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).send();
        assert_eq!(401, response.status());
    }
}
