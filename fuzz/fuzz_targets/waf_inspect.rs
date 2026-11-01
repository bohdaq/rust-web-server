//! The WAF's decoding and detections on arbitrary input (#77). Must never
//! panic or loop.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::waf::fuzz::inspect(data);
});
