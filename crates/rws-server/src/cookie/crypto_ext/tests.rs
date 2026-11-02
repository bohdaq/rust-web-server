use crate::cookie::{decrypt_cookie, encrypted_cookie, signed_cookie, verify_signed_cookie, SetCookie};

// ── signed_cookie / verify_signed_cookie ─────────────────────────────────────

#[test]
fn signed_cookie_roundtrip() {
    let signed = signed_cookie("plan=pro", b"secret");
    assert_eq!(Some("plan=pro".to_string()), verify_signed_cookie(&signed, b"secret"));
}

#[test]
fn signed_cookie_value_stays_readable() {
    // Signing is tamper-evident, not confidential — the plain value must
    // still be visible in the output (unlike encrypted_cookie).
    let signed = signed_cookie("plan=pro", b"secret");
    assert!(signed.starts_with("plan=pro."));
}

#[test]
fn verify_signed_cookie_rejects_wrong_secret() {
    let signed = signed_cookie("plan=pro", b"secret");
    assert_eq!(None, verify_signed_cookie(&signed, b"wrong-secret"));
}

#[test]
fn verify_signed_cookie_rejects_tampered_value() {
    let signed = signed_cookie("plan=free", b"secret");
    let (_, sig) = signed.split_once('.').unwrap();
    let tampered = format!("plan=pro.{}", sig);
    assert_eq!(None, verify_signed_cookie(&tampered, b"secret"));
}

#[test]
fn verify_signed_cookie_rejects_tampered_signature() {
    let mut signed = signed_cookie("plan=pro", b"secret");
    signed.push('0'); // corrupt the trailing hex signature byte
    assert_eq!(None, verify_signed_cookie(&signed, b"secret"));
}

#[test]
fn verify_signed_cookie_rejects_missing_separator() {
    assert_eq!(None, verify_signed_cookie("no-signature-here", b"secret"));
}

#[test]
fn verify_signed_cookie_rejects_malformed_hex_signature() {
    assert_eq!(None, verify_signed_cookie("value.not-hex!!", b"secret"));
}

#[test]
fn signed_cookie_handles_value_containing_dots() {
    // The signature is fixed-length hex and never contains '.', so the last
    // '.' always separates it correctly regardless of dots in the value.
    let signed = signed_cookie("a.b.c", b"secret");
    assert_eq!(Some("a.b.c".to_string()), verify_signed_cookie(&signed, b"secret"));
}

#[test]
fn signed_cookie_composes_with_set_cookie_builder() {
    let signed = signed_cookie("user=42", b"secret");
    let header_value = SetCookie::new("prefs", &signed).http_only().path("/").build();
    assert!(header_value.starts_with("prefs=user=42."));
    assert!(header_value.contains("HttpOnly"));
}

// ── encrypted_cookie / decrypt_cookie ────────────────────────────────────────

#[test]
fn encrypted_cookie_roundtrip() {
    let encrypted = encrypted_cookie("session-token-abc123", b"a-32-byte-or-any-length-key");
    assert_eq!(Some("session-token-abc123".to_string()), decrypt_cookie(&encrypted, b"a-32-byte-or-any-length-key"));
}

#[test]
fn encrypted_cookie_hides_the_plaintext() {
    let encrypted = encrypted_cookie("super-secret-session-data", b"key");
    assert!(!encrypted.contains("super-secret-session-data"));
}

#[test]
fn encrypted_cookie_uses_a_fresh_nonce_each_call() {
    // Same plaintext + key must not produce the same ciphertext twice —
    // otherwise an observer could correlate repeated cookie issuances.
    let a = encrypted_cookie("same-value", b"key");
    let b = encrypted_cookie("same-value", b"key");
    assert_ne!(a, b);
}

#[test]
fn decrypt_cookie_rejects_wrong_key() {
    let encrypted = encrypted_cookie("session-token", b"key-one");
    assert_eq!(None, decrypt_cookie(&encrypted, b"key-two"));
}

#[test]
fn decrypt_cookie_rejects_tampered_ciphertext() {
    let encrypted = encrypted_cookie("session-token", b"key");
    let mut tampered = encrypted.clone();
    let last = tampered.pop().unwrap();
    tampered.push(if last == '0' { '1' } else { '0' });
    assert_eq!(None, decrypt_cookie(&tampered, b"key"));
}

#[test]
fn decrypt_cookie_rejects_tampered_nonce() {
    let encrypted = encrypted_cookie("session-token", b"key");
    let (nonce_hex, ct_hex) = encrypted.split_once('.').unwrap();
    let mut corrupted_nonce = nonce_hex.to_string();
    let last = corrupted_nonce.pop().unwrap();
    corrupted_nonce.push(if last == '0' { '1' } else { '0' });
    let tampered = format!("{}.{}", corrupted_nonce, ct_hex);
    assert_eq!(None, decrypt_cookie(&tampered, b"key"));
}

#[test]
fn decrypt_cookie_rejects_missing_separator() {
    assert_eq!(None, decrypt_cookie("not-a-valid-encrypted-cookie", b"key"));
}

#[test]
fn decrypt_cookie_rejects_malformed_hex() {
    assert_eq!(None, decrypt_cookie("nothex.alsonothex", b"key"));
}

#[test]
fn decrypt_cookie_rejects_wrong_length_nonce() {
    // A syntactically valid hex.hex pair, but the first segment isn't a
    // 12-byte nonce.
    assert_eq!(None, decrypt_cookie("aabb.aabbccdd", b"key"));
}

#[test]
fn encrypted_cookie_accepts_any_length_key() {
    let short = encrypted_cookie("value", b"k");
    let long = encrypted_cookie("value", b"a much longer passphrase used as a key");
    assert_eq!(Some("value".to_string()), decrypt_cookie(&short, b"k"));
    assert_eq!(Some("value".to_string()), decrypt_cookie(&long, b"a much longer passphrase used as a key"));
}

#[test]
fn encrypted_cookie_composes_with_set_cookie_builder() {
    let encrypted = encrypted_cookie("session=xyz", b"key");
    let header_value = SetCookie::new("sess", &encrypted).http_only().secure().build();
    assert!(header_value.starts_with(&format!("sess={}", encrypted)));
    assert!(header_value.contains("HttpOnly"));
    assert!(header_value.contains("Secure"));
}
