//! A User-Agent and path through the traffic classifier (18.15.0). Must
//! never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::traffic_classify(data);
});
