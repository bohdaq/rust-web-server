//! Raw bytes from any client → `Request::parse`.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::request::Request;

fuzz_target!(|data: &[u8]| {
    let _ = Request::parse(data);
});
