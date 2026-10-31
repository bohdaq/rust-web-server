//! i18n catalog files (#33): any text. Must never panic, in parsing or in
//! translating what parsed.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rust_web_server::fuzz_api::i18n_catalog(data);
});
