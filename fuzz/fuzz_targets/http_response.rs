//! Raw bytes from a proxied backend → `Response::parse`.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::response::Response;

fuzz_target!(|data: &[u8]| {
    let _ = Response::parse(data);
});
