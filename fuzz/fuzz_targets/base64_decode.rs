//! The shared base64 decoder: never panics; decoded bytes round-trip.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::base64_decode(data);
});
