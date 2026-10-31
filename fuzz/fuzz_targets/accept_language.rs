//! `Accept-Language` parsing and negotiation (#33): any header value. Must
//! never panic, keep at most the capped number of ranges, and only pick an
//! offered language.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::accept_language(data);
});
