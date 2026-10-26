//! ACME renewal input from the CA (#1): certificate DER, renewalInfo
//! windows and directory profiles. Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::acme_renewal(data);
});
