//! An `Available-Dictionary` header value, and dictionary-compressed
//! (`dcz` / `dcb`) bodies through both decoders (#38). Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::available_dictionary(data);
});
