//! `rws gen`'s readers of project files (lib.rs, mod.rs, Cargo.toml, route
//! modules, tests) and its argument parsers. Must never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        rust_web_server::generator::fuzz(text);
    }
});
