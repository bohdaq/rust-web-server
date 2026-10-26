//! `Accept-Encoding` negotiation (#7): any header value. Must never panic,
//! and must never pick a coding the value refuses.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::accept_encoding(data);
});
