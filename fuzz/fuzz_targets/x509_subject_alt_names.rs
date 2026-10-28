//! A certificate's subjectAltName extension (`core::x509::subject_alt_names`):
//! what a backend sends when a BackendTLSPolicy checks its names. Must never
//! panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::x509_subject_alt_names(data);
});
