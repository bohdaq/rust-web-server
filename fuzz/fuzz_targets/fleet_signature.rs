//! A fleet signature file against a body (#80): never panics, never verifies.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::fleet_signature(data);
});
