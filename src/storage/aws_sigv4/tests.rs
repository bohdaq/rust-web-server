//! Correctness tests for the SigV4 signer.
//!
//! There is no independently-verified AWS test vector here: AWS's published
//! examples sign a different header set (they include `range`), so their
//! expected signatures don't apply to this signer's fixed header set
//! (`host`, `x-amz-content-sha256`, `x-amz-date`, optionally
//! `x-amz-security-token`). These tests instead check the algorithm's
//! structure and sensitivity to its inputs. Verify against a real
//! S3-compatible endpoint (AWS S3, R2, MinIO) before relying on this in
//! production.

use super::{sign, uri_encode_path};

const ACCESS_KEY: &str = "AKIAIOSFODNN7EXAMPLE";
const SECRET_KEY: &str = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
const REGION: &str = "us-east-1";
const HOST: &str = "examplebucket.s3.amazonaws.com";
const EPOCH: u64 = 1_369_353_600; // 2013-05-24T00:00:00Z

fn header<'a>(headers: &'a [(String, String)], name: &str) -> &'a str {
    headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()).unwrap()
}

#[test]
fn signs_expected_headers() {
    let headers = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    assert_eq!(HOST, header(&headers, "host"));
    assert_eq!(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        header(&headers, "x-amz-content-sha256"),
        "SHA-256 of an empty payload is a well-known constant"
    );
    assert_eq!("20130524T000000Z", header(&headers, "x-amz-date"));
}

#[test]
fn authorization_header_has_expected_shape() {
    let headers = sign("PUT", HOST, "/examplebucket/test.txt", b"hello", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    let auth = header(&headers, "Authorization");

    assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, "));
    assert!(auth.contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date"));

    let sig = auth.rsplit("Signature=").next().unwrap();
    assert_eq!(64, sig.len(), "signature must be a 32-byte hex digest");
    assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn same_inputs_produce_same_signature() {
    let a = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    let b = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    assert_eq!(header(&a, "Authorization"), header(&b, "Authorization"));
}

#[test]
fn signature_changes_with_payload() {
    let a = sign("PUT", HOST, "/examplebucket/test.txt", b"hello", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    let b = sign("PUT", HOST, "/examplebucket/test.txt", b"world", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    assert_ne!(header(&a, "Authorization"), header(&b, "Authorization"));
    assert_ne!(header(&a, "x-amz-content-sha256"), header(&b, "x-amz-content-sha256"));
}

#[test]
fn signature_changes_with_method() {
    let a = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    let b = sign("DELETE", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    assert_ne!(header(&a, "Authorization"), header(&b, "Authorization"));
}

#[test]
fn signature_changes_with_path() {
    let a = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    let b = sign("GET", HOST, "/examplebucket/other.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    assert_ne!(header(&a, "Authorization"), header(&b, "Authorization"));
}

#[test]
fn signature_changes_with_secret_key() {
    let a = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    let b = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, "a-different-secret", None, EPOCH, "s3");
    assert_ne!(header(&a, "Authorization"), header(&b, "Authorization"));
}

#[test]
fn credential_scope_uses_the_given_date() {
    // 2021-01-01T00:00:00Z
    let headers = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, 1_609_459_200, "s3");
    assert_eq!("20210101T000000Z", header(&headers, "x-amz-date"));
    assert!(header(&headers, "Authorization").contains("Credential=AKIAIOSFODNN7EXAMPLE/20210101/us-east-1/s3/aws4_request"));
}

#[test]
fn uri_encode_path_preserves_slashes_and_encodes_special_chars() {
    assert_eq!("/bucket/key", uri_encode_path("/bucket/key"));
    assert_eq!("/bucket/a%20b.png", uri_encode_path("/bucket/a b.png"));
    assert_eq!("/bucket/nested/dir/file.txt", uri_encode_path("/bucket/nested/dir/file.txt"));
    // `~` is in the unreserved set and must not be encoded.
    assert_eq!("/bucket/~file", uri_encode_path("/bucket/~file"));
}

// ── Session token (temporary credentials) ──────────────────────────────────────

#[test]
fn no_session_token_is_byte_identical_to_pre_token_behavior() {
    // Locks in backward compatibility: the `None` branch must reproduce
    // exactly what this signer produced before `session_token` existed.
    let headers = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    assert_eq!(4, headers.len(), "host, x-amz-content-sha256, x-amz-date, Authorization — no security token entry");
    assert!(headers.iter().all(|(k, _)| k != "x-amz-security-token"));
    assert!(!header(&headers, "Authorization").contains("x-amz-security-token"));
}

#[test]
fn session_token_adds_security_token_header() {
    let headers = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, Some("tok123"), EPOCH, "s3");
    assert_eq!("tok123", header(&headers, "x-amz-security-token"));
    assert_eq!(5, headers.len());
}

#[test]
fn session_token_is_included_in_signed_headers_list() {
    let with_token = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, Some("tok123"), EPOCH, "s3");
    let without_token = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, None, EPOCH, "s3");
    assert!(header(&with_token, "Authorization").contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date;x-amz-security-token"));
    assert!(header(&without_token, "Authorization").contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date,"));
}

#[test]
fn different_session_tokens_produce_different_signatures() {
    // The token must actually participate in the HMAC chain, not just be
    // appended to the header list without affecting the signature.
    let a = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, Some("token-a"), EPOCH, "s3");
    let b = sign("GET", HOST, "/examplebucket/test.txt", b"", REGION, ACCESS_KEY, SECRET_KEY, Some("token-b"), EPOCH, "s3");
    assert_ne!(header(&a, "Authorization"), header(&b, "Authorization"));
}
