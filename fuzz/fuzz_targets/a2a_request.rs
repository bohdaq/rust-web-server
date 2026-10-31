//! A2A JSON-RPC requests (#65): any body. Must never panic, and always
//! answers with JSON.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::a2a_request(data);
});
