//! A DNS response read for SRV records (#8). Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::dns_srv_response(data);
});
