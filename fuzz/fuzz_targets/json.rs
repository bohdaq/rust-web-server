//! The crate's hand-written JSON parser, fed request bodies.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::json::object::JSON;
use rust_web_server::json::property::JSONProperty;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = JSON::parse_as_properties(text.to_string());
        let _ = JSONProperty::parse(text);
    }
});
