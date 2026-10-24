//! Request targets → `URL::parse_request_target` (what the static-file
//! controller and `Request::get_path` use on every request). `URL::parse`
//! on arbitrary absolute URLs is not fuzzed: it delegates to
//! url-build-parse, which panics on some malformed authorities — rws never
//! hands it client input except through parse_request_target.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::url::URL;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = URL::parse_request_target(text);
        let _ = URL::parse_request_target(&format!("/{}", text));
    }
});
