//! `rws.config.toml` → proxy route parser.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::proxy_config::ProxyConfig;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = ProxyConfig::from_str(text);
    }
});
