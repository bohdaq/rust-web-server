//! OpenAPI documents to MCP tools (#66): any text. Must never panic, and
//! every tool it makes is well formed.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::openapi_to_mcp(data);
});
