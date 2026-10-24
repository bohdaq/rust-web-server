//! IdP-supplied XML → the SAML XML parser.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(xml) = std::str::from_utf8(data) {
        rust_web_server::fuzz_api::saml_xml_parse(xml);
    }
});
