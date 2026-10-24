//! Bearer tokens → HS256 `verify_jwt`.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(token) = std::str::from_utf8(data) {
        let _ = rust_web_server::auth::verify_jwt(token, b"fuzz-secret");
    }
});
