//! Form bodies: application/x-www-form-urlencoded and multipart/form-data.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rust_web_server::body::form_urlencoded::FormUrlEncoded;
use rust_web_server::body::multipart_form_data::FormMultipartData;

fuzz_target!(|data: &[u8]| {
    let _ = FormUrlEncoded::parse(data.to_vec());
    // Typed handlers' `Query<T>` / `Form<T>` deserializer (18.28.0).
    if let Ok(text) = std::str::from_utf8(data) {
        rust_web_server::handler::__private::fuzz_urlencoded(text);
    }
    // First line is the Content-Type header value, the rest the body.
    if let Some(split) = data.iter().position(|b| *b == b'\n') {
        if let Ok(content_type) = std::str::from_utf8(&data[..split]) {
            if let Ok(boundary) = FormMultipartData::extract_boundary(content_type) {
                let _ = FormMultipartData::parse(&data[split + 1..], boundary);
            }
        }
    }
});
