//! Correctness tests for the Azure Shared Key signer.
//!
//! The two `*_golden_vector` tests below are cross-checked against an
//! independent Python implementation of the same StringToSign/HMAC-SHA256
//! algorithm (not just this Rust code), so they catch a broken
//! canonicalization or key derivation, not just internal self-consistency.
//! The rest are structural/sensitivity tests, mirroring `aws_sigv4/tests.rs`
//! — Microsoft's own docs illustrate the StringToSign shape but never
//! publish the account key behind their worked examples, so there's no
//! official vector to test against directly.

use super::{rfc1123_date, sign};

// base64("keybodytestkeybodytestkeybodytest1234")
const ACCOUNT_KEY: &str = "a2V5Ym9keXRlc3RrZXlib2R5dGVzdGtleWJvZHl0ZXN0MTIzNA==";
const ACCOUNT: &str = "myaccount";
const PATH: &str = "/mycontainer/myblob.txt";
const X_MS_DATE: &str = "Fri, 26 Jun 2015 23:39:12 GMT";

fn headers(extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut h: Vec<(String, String)> =
        extra.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    h.push(("x-ms-date".to_string(), X_MS_DATE.to_string()));
    h.push(("x-ms-version".to_string(), "2021-08-06".to_string()));
    h
}

#[test]
fn get_matches_independently_computed_golden_vector() {
    let auth = sign("GET", ACCOUNT, PATH, "", 0, ACCOUNT_KEY, &headers(&[])).unwrap();
    assert_eq!(
        "SharedKey myaccount:ryqinqMoFzRyt2Tt9Z9noaZB2NgPG8sROs+A79DLaas=",
        auth
    );
}

#[test]
fn put_with_blob_type_header_matches_independently_computed_golden_vector() {
    let auth = sign(
        "PUT",
        ACCOUNT,
        PATH,
        "text/plain",
        11,
        ACCOUNT_KEY,
        &headers(&[("x-ms-blob-type", "BlockBlob")]),
    )
    .unwrap();
    assert_eq!(
        "SharedKey myaccount:WFnVZXcUO9luc46bjEnMk8Sr8cMb0Y4jFfYJpWDNKcs=",
        auth
    );
}

#[test]
fn signature_changes_with_method() {
    let a = sign("GET", ACCOUNT, PATH, "", 0, ACCOUNT_KEY, &headers(&[])).unwrap();
    let b = sign("DELETE", ACCOUNT, PATH, "", 0, ACCOUNT_KEY, &headers(&[])).unwrap();
    assert_ne!(a, b);
}

#[test]
fn signature_changes_with_path() {
    let a = sign("GET", ACCOUNT, "/mycontainer/a.txt", "", 0, ACCOUNT_KEY, &headers(&[])).unwrap();
    let b = sign("GET", ACCOUNT, "/mycontainer/b.txt", "", 0, ACCOUNT_KEY, &headers(&[])).unwrap();
    assert_ne!(a, b);
}

#[test]
fn signature_changes_with_account_key() {
    let a = sign("GET", ACCOUNT, PATH, "", 0, ACCOUNT_KEY, &headers(&[])).unwrap();
    // base64("different-key-different-key-1234567")
    let other_key = "ZGlmZmVyZW50LWtleS1kaWZmZXJlbnQta2V5LTEyMzQ1Njc=";
    let b = sign("GET", ACCOUNT, PATH, "", 0, other_key, &headers(&[])).unwrap();
    assert_ne!(a, b);
}

#[test]
fn signature_changes_with_content_length() {
    let a = sign("PUT", ACCOUNT, PATH, "text/plain", 5, ACCOUNT_KEY, &headers(&[("x-ms-blob-type", "BlockBlob")])).unwrap();
    let b = sign("PUT", ACCOUNT, PATH, "text/plain", 11, ACCOUNT_KEY, &headers(&[("x-ms-blob-type", "BlockBlob")])).unwrap();
    assert_ne!(a, b);
}

#[test]
fn signature_changes_with_x_ms_blob_type_presence() {
    // GET/DELETE never send x-ms-blob-type; PUT does — the header list
    // actually sent must exactly match what's canonicalized here.
    let without = sign("GET", ACCOUNT, PATH, "", 0, ACCOUNT_KEY, &headers(&[])).unwrap();
    let with = sign("GET", ACCOUNT, PATH, "", 0, ACCOUNT_KEY, &headers(&[("x-ms-blob-type", "BlockBlob")])).unwrap();
    assert_ne!(without, with);
}

#[test]
fn same_inputs_produce_same_signature() {
    let a = sign("GET", ACCOUNT, PATH, "", 0, ACCOUNT_KEY, &headers(&[])).unwrap();
    let b = sign("GET", ACCOUNT, PATH, "", 0, ACCOUNT_KEY, &headers(&[])).unwrap();
    assert_eq!(a, b);
}

#[test]
fn authorization_header_has_expected_shape() {
    let auth = sign("GET", ACCOUNT, PATH, "", 0, ACCOUNT_KEY, &headers(&[])).unwrap();
    assert!(auth.starts_with("SharedKey myaccount:"));
    let sig_b64 = auth.rsplit(':').next().unwrap();
    assert_eq!(44, sig_b64.len(), "base64 of a 32-byte HMAC-SHA256 digest is 44 chars with padding");
    assert!(sig_b64.ends_with('='));
}

#[test]
fn invalid_base64_account_key_is_a_clean_error() {
    let err = sign("GET", ACCOUNT, PATH, "", 0, "not valid base64 !!!", &headers(&[])).unwrap_err();
    assert!(err.to_string().contains("base64"));
}

#[test]
fn rfc1123_date_formats_known_timestamp() {
    // 2015-06-26T23:39:12Z was a Friday.
    assert_eq!("Fri, 26 Jun 2015 23:39:12 GMT", rfc1123_date(1_435_361_952));
}
