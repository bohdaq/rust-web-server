//! MCP JSON-RPC request bodies (single and batch) → `McpServer::handle_request`.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::mcp::{McpContent, McpServer};

fuzz_target!(|data: &[u8]| {
    if let Ok(body) = std::str::from_utf8(data) {
        let server = McpServer::new("fuzz", "0")
            .tool("echo", "Echo", r#"{"type":"object"}"#, |args| Ok(McpContent::text(args)))
            .page_size(2);
        let _ = server.handle_request(body);
    }
});
