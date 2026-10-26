//! An upstream MCP server's reply as the gateway reads it (#62): JSON or an
//! event stream, then the tools it lists. Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::mcp_gateway_reply(data);
});
