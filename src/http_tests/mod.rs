//! Tests of rws-http modules (`cors`, `range`, `server_config`) that run them
//! through the server, the app or a config reload: those live in this crate,
//! so the tests do too (spec/WORKSPACE_SPLIT.md).

mod cors;
mod fixtures;
mod range;
mod server_config;

/// `extract::RequestId` reads the header `RequestIdLayer` sets; rws-http
/// can't name the layer, so it keeps its own copy of the name.
#[test]
fn request_id_extractor_reads_the_layers_header() {
    assert_eq!(crate::extract::REQUEST_ID_HEADER, crate::request_id::DEFAULT_HEADER);
}
