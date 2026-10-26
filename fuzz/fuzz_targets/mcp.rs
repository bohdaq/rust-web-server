//! MCP JSON-RPC request bodies (single and batch) → `McpServer::handle_request`.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::mcp::{McpContent, McpServer};

fuzz_target!(|data: &[u8]| {
    if let Ok(body) = std::str::from_utf8(data) {
        let server = McpServer::new("fuzz", "0")
            // A schema with required/typed properties: tools/call arguments
            // go through the argument validation (#3).
            .tool("echo", "Echo", r#"{"type":"object","properties":{"a":{"type":"number"},"t":{"type":["string","null"]}},"required":["a"]}"#, |args| Ok(McpContent::text(args)))
            .page_size(2);
        let _ = server.handle_request(body);
    }
});
