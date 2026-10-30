//! What a WebAssembly plugin returns, as rws reads it (#40): request and
//! response directives. Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::wasm_plugin_output(data);
});
