//! FastCGI records, name-value pairs and CGI response headers from a
//! FastCGI server (#72). Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fastcgi::fuzz::records(data);
});
