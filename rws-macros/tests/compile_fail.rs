//! Compile-time route checks (18.28.0, #86): each `tests/ui/*.rs` must fail
//! with the message in its `.stderr`. Regenerate with `TRYBUILD=overwrite`.

#[test]
fn route_macros_reject_mismatched_path_parameters() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
