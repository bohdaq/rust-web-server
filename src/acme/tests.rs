use super::crypto::{self, AccountKey};

static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ── base64url ─────────────────────────────────────────────────────────────────

#[test]
fn base64url_empty() {
    assert_eq!(crypto::base64url(b""), "");
}

#[test]
fn base64url_known_value() {
    assert_eq!(crypto::base64url(b"Man"), "TWFu");
}

#[test]
fn base64url_no_padding() {
    let enc = crypto::base64url(b"foo");
    assert!(!enc.contains('='));
}

#[test]
fn base64url_no_plus_or_slash() {
    for b in 0u8..=255u8 {
        let enc = crypto::base64url(&[b]);
        assert!(!enc.contains('+'), "byte {} produced '+'", b);
        assert!(!enc.contains('/'), "byte {} produced '/'", b);
    }
}

// ── sha256 ────────────────────────────────────────────────────────────────────

#[test]
fn sha256_empty_string() {
    // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    let hash = crypto::sha256(b"");
    let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    assert_eq!(hex, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn sha256_abc() {
    // Verified: echo -n "abc" | openssl dgst -sha256
    let hash = crypto::sha256(b"abc");
    let expected = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea,
        0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
        0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c,
        0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
    ];
    assert_eq!(hash, expected);
}

// ── ec_point_xy ───────────────────────────────────────────────────────────────

#[test]
fn ec_point_xy_valid_uncompressed_point() {
    let mut raw = [0u8; 65];
    raw[0] = 0x04;
    raw[1..33].fill(0xAA);
    raw[33..65].fill(0xBB);
    let (x, y) = crypto::ec_point_xy(&raw).unwrap();
    assert_eq!(x, [0xAAu8; 32]);
    assert_eq!(y, [0xBBu8; 32]);
}

#[test]
fn ec_point_xy_rejects_wrong_prefix() {
    let raw = [0x02u8; 65];
    assert!(crypto::ec_point_xy(&raw).is_err());
}

#[test]
fn ec_point_xy_rejects_wrong_length() {
    let raw = [0x04u8; 33];
    assert!(crypto::ec_point_xy(&raw).is_err());
}

// ── ec_jwk_json ───────────────────────────────────────────────────────────────

#[test]
fn ec_jwk_json_contains_required_keys() {
    let x = [0u8; 32];
    let y = [0u8; 32];
    let jwk = crypto::ec_jwk_json(&x, &y);
    assert!(jwk.contains("\"kty\":\"EC\""));
    assert!(jwk.contains("\"crv\":\"P-256\""));
    assert!(jwk.contains("\"x\":"));
    assert!(jwk.contains("\"y\":"));
}

#[test]
fn ec_jwk_json_keys_sorted_alphabetically() {
    // RFC 7638 requires alphabetically ordered keys for thumbprint
    let x = [0u8; 32];
    let y = [0u8; 32];
    let jwk = crypto::ec_jwk_json(&x, &y);
    let crv_pos = jwk.find("\"crv\"").unwrap();
    let kty_pos = jwk.find("\"kty\"").unwrap();
    let x_pos = jwk.find("\"x\"").unwrap();
    let y_pos = jwk.find("\"y\"").unwrap();
    assert!(crv_pos < kty_pos, "crv must precede kty");
    assert!(kty_pos < x_pos, "kty must precede x");
    assert!(x_pos < y_pos, "x must precede y");
}

// ── AccountKey ────────────────────────────────────────────────────────────────

#[test]
fn account_key_generate_roundtrip() {
    let (key, der) = AccountKey::generate().unwrap();
    assert!(!der.is_empty());
    // Public key is 65 bytes (uncompressed P-256 point)
    assert_eq!(key.public_key_raw().len(), 65);
    assert_eq!(key.public_key_raw()[0], 0x04);
}

#[test]
fn account_key_from_pkcs8_roundtrip() {
    let (_, der) = AccountKey::generate().unwrap();
    let key2 = AccountKey::from_pkcs8(&der).unwrap();
    assert_eq!(key2.public_key_raw().len(), 65);
}

#[test]
fn account_key_sign_produces_64_bytes() {
    let (key, _) = AccountKey::generate().unwrap();
    let sig = key.sign(b"hello world").unwrap();
    assert_eq!(sig.len(), 64);
}

#[test]
fn account_key_sign_is_deterministic_in_length() {
    let (key, _) = AccountKey::generate().unwrap();
    let s1 = key.sign(b"test message").unwrap();
    let s2 = key.sign(b"test message").unwrap();
    assert_eq!(s1.len(), 64);
    assert_eq!(s2.len(), 64);
}

// ── key_thumbprint ────────────────────────────────────────────────────────────

#[test]
fn key_thumbprint_produces_base64url_string() {
    let (key, _) = AccountKey::generate().unwrap();
    let thumb = crypto::key_thumbprint(&key).unwrap();
    assert!(!thumb.is_empty());
    assert!(!thumb.contains('+'));
    assert!(!thumb.contains('/'));
    assert!(!thumb.contains('='));
}

#[test]
fn key_thumbprint_is_deterministic_for_same_key() {
    let (key, der) = AccountKey::generate().unwrap();
    let t1 = crypto::key_thumbprint(&key).unwrap();
    let key2 = AccountKey::from_pkcs8(&der).unwrap();
    let t2 = crypto::key_thumbprint(&key2).unwrap();
    assert_eq!(t1, t2);
}

#[test]
fn key_thumbprint_differs_for_different_keys() {
    let (k1, _) = AccountKey::generate().unwrap();
    let (k2, _) = AccountKey::generate().unwrap();
    let t1 = crypto::key_thumbprint(&k1).unwrap();
    let t2 = crypto::key_thumbprint(&k2).unwrap();
    assert_ne!(t1, t2);
}

// ── build_jws ────────────────────────────────────────────────────────────────

#[test]
fn build_jws_without_kid_uses_jwk() {
    let (key, _) = AccountKey::generate().unwrap();
    let jws = crypto::build_jws(&key, "nonce123", "https://example.com/new-acct", None, Some("{}")).unwrap();
    assert!(jws.contains("\"protected\""));
    assert!(jws.contains("\"payload\""));
    assert!(jws.contains("\"signature\""));
    let prot_b64 = extract_json_str_field(&jws, "protected").unwrap();
    let decoded = base64url_decode(&prot_b64);
    let prot_str = std::str::from_utf8(&decoded).unwrap();
    assert!(prot_str.contains("\"jwk\""), "JWS without account URL must use jwk");
    assert!(!prot_str.contains("\"kid\""));
}

#[test]
fn build_jws_with_kid_uses_kid() {
    let (key, _) = AccountKey::generate().unwrap();
    let jws = crypto::build_jws(
        &key,
        "nonce456",
        "https://example.com/orders",
        Some("https://example.com/acct/123"),
        Some("{}"),
    ).unwrap();
    let prot_b64 = extract_json_str_field(&jws, "protected").unwrap();
    let decoded = base64url_decode(&prot_b64);
    let prot_str = std::str::from_utf8(&decoded).unwrap();
    assert!(prot_str.contains("\"kid\""), "JWS with account URL must use kid");
    assert!(!prot_str.contains("\"jwk\""));
}

#[test]
fn build_jws_post_as_get_has_empty_payload() {
    let (key, _) = AccountKey::generate().unwrap();
    let jws = crypto::build_jws(
        &key, "n", "https://example.com/authz/1",
        Some("https://example.com/acct/1"),
        None, // POST-as-GET
    ).unwrap();
    let payload_b64 = extract_json_str_field(&jws, "payload").unwrap();
    assert_eq!(payload_b64, "", "POST-as-GET must have empty payload");
}

// ── cert_days_until_expiry ────────────────────────────────────────────────────

#[test]
fn cert_days_until_expiry_missing_file_returns_none() {
    assert!(crypto::cert_days_until_expiry("/nonexistent/cert.pem").is_none());
}

// ── json helpers ─────────────────────────────────────────────────────────────

#[test]
fn json_str_extracts_string_field() {
    let json = r#"{"status":"pending","token":"abc123"}"#;
    assert_eq!(super::json_str(json, "status").as_deref(), Some("pending"));
    assert_eq!(super::json_str(json, "token").as_deref(), Some("abc123"));
    assert_eq!(super::json_str(json, "missing"), None);
}

#[test]
fn json_array_strings_extracts_urls() {
    let json = r#"{"authorizations":["https://example.com/authz/1","https://example.com/authz/2"]}"#;
    let arr = super::json_array_strings(json, "authorizations");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0], "https://example.com/authz/1");
    assert_eq!(arr[1], "https://example.com/authz/2");
}

// ── AcmeConfig::from_env ──────────────────────────────────────────────────────

#[test]
fn acme_config_from_env_returns_none_without_domains() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("RWS_CONFIG_ACME_DOMAINS");
    assert!(super::AcmeConfig::from_env().is_none());
}

#[test]
fn acme_config_from_env_parses_domains() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("RWS_CONFIG_ACME_DOMAINS", "example.com,www.example.com");
    std::env::set_var("RWS_CONFIG_ACME_EMAIL", "admin@example.com");
    let result = super::AcmeConfig::from_env();
    std::env::remove_var("RWS_CONFIG_ACME_DOMAINS");
    std::env::remove_var("RWS_CONFIG_ACME_EMAIL");
    let cfg = result.unwrap();
    assert_eq!(cfg.domains, vec!["example.com", "www.example.com"]);
    assert_eq!(cfg.email, "admin@example.com");
}

#[test]
fn acme_config_staging_flag() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("RWS_CONFIG_ACME_DOMAINS", "example.com");
    std::env::set_var("RWS_CONFIG_ACME_STAGING", "true");
    let result = super::AcmeConfig::from_env();
    std::env::remove_var("RWS_CONFIG_ACME_DOMAINS");
    std::env::remove_var("RWS_CONFIG_ACME_STAGING");
    let cfg = result.unwrap();
    assert!(cfg.directory_url.contains("staging"), "staging flag must use staging URL");
}

// ── test helpers ──────────────────────────────────────────────────────────────

fn extract_json_str_field(json: &str, field: &str) -> Option<String> {
    let key = format!("\"{}\":\"", field);
    let start = json.find(&key)? + key.len();
    let end = json[start..].find('"')?;
    Some(json[start..start + end].to_string())
}

fn base64url_decode(s: &str) -> Vec<u8> {
    crypto::base64_decode_std(s).unwrap_or_default()
}
