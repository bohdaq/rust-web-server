//! URLs → `URL::parse` (any input) and `URL::parse_request_target` (what the
//! static-file controller and `Request::get_path` use on every request).
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::url::URL;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = URL::parse(text);
        let _ = URL::parse_request_target(text);
        let _ = URL::parse_request_target(&format!("/{}", text));
    }
});
