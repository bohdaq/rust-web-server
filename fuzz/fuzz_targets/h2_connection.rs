//! A whole HTTP/2 connection fed arbitrary frames after a valid preface:
//! frame parsing, stream states, flow control. Must never panic or hang.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::h2_server::fuzz::connection(data);
});
