//! A disk-cache entry file as rws reads it back (#39): the header line and
//! key, then the stored response. Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::cache_entry(data);
});
