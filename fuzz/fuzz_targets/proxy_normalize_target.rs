//! Proxy route-matching path normalization (#50). Must never panic, and
//! normalizing an already normalized target changes nothing.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::proxy_normalize_target(data);
});
