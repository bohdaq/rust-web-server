//! An RFC 7512 pkcs11: URI and its masking in output (#79). Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::pkcs11_uri(data);
});
