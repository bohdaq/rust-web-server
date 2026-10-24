//! Raw client bytes → HTTP/1.1 request framing (head detection, body framing,
//! chunked request body decoding).
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::http1_framing(data);
});
