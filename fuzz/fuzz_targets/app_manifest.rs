//! `rws.app.toml` app manifest parser. Must reject bad input, never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::app_manifest::AppManifest;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = AppManifest::parse(text);
    }
});
