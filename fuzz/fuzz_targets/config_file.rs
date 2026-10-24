//! `rws.config.toml` → settings loader. Must reject bad input, never panic.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::entry_point::config_file::read_config_file;

fuzz_target!(|data: &[u8]| {
    let _ = read_config_file(std::io::Cursor::new(data), String::new());
});
