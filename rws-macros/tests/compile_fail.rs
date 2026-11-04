//! Compile-time checks — routes (18.28.0, #86) and derives: each `tests/ui/*.rs` must fail
//! with the message in its `.stderr`. Regenerate with `TRYBUILD=overwrite`.

#[test]
fn route_macros_reject_mismatched_path_parameters() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
