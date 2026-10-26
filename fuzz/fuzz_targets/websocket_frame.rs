//! A WebSocket frame from a client (#8): any bytes. Must never panic or
//! hold more payload than the limit.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::websocket_frame(data);
});
