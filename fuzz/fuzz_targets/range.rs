//! `Range` request header values against resources of fuzzed size.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::range::Range;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    let length = u64::from_le_bytes(data[..8].try_into().unwrap());
    if let Ok(spec) = std::str::from_utf8(&data[8..]) {
        if let Ok(range) = Range::parse_range_in_content_range(length, spec) {
            assert!(range.start <= range.end && range.end < length, "{spec:?} on {length} bytes -> {}-{}", range.start, range.end);
        }
        let _ = Range::_parse_content_range_header_value(spec.to_string());
    }
});
