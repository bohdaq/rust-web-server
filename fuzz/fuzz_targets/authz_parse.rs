//! Authorization text (#32): a tuple (`doc:42#editor@user:alice`), a
//! subject, and a relation definition. Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::authz_parse(data);
});
