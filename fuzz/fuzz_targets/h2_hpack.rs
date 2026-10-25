//! HPACK header-block decoder (HTTP/2 requests). Must reject bad input, never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::h2_server::fuzz::hpack_decode(data);
});
