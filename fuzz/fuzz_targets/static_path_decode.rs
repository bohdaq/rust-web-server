//! Static-file request-path decoding (#55). Must never panic, and a path
//! it accepts never leaves the web root through a decoded `..`.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::static_path_decode(data);
});
