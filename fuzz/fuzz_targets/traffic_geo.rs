//! A country table as `[traffic] geo_db` loads it (18.31.0). Must never
//! panic or allocate without bound.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::traffic_geo(data);
});
