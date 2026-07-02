//! Unit tests for the SSO module.
//!
//! All tests operate on pure in-memory logic — no network calls are made.

use super::{
    client::url_encode,
    config::OidcConfig,
    discovery::OidcProvider,
    jwks::json_str,
    pkce::{base64url_decode, base64url_encode, PkceVerifier},
};

// ── PKCE tests ────────────────────────────────────────────────────────────────

#[test]
fn pkce_verifier_is_base64url() {
    let v = PkceVerifier::new();
    let s = v.as_str();
    // Must be non-empty and contain only base64url characters
    assert!(!s.is_empty(), "verifier should not be empty");
    for ch in s.chars() {
        assert!(
            ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
            "unexpected char in verifier: {ch}"
        );
    }
}

#[test]
fn pkce_verifier_length_is_43() {
    // 32 bytes → 43 base64url chars (no padding)
    let v = PkceVerifier::new();
    assert_eq!(v.as_str().len(), 43, "verifier should be 43 chars for 32 bytes input");
}

#[test]
fn pkce_challenge_is_sha256_of_verifier() {
    use sha2::{Digest, Sha256};
    let v   = PkceVerifier::new();
    let chl = v.challenge();
    let expected_digest = Sha256::digest(v.as_str().as_bytes());
    let expected_b64 = base64url_encode(&expected_digest);
    assert_eq!(
        chl.as_str(),
        expected_b64,
        "challenge must be SHA-256(verifier) base64url-encoded"
    );
}

#[test]
fn pkce_two_verifiers_are_different() {
    let v1 = PkceVerifier::new();
    let v2 = PkceVerifier::new();
    // Extremely unlikely to collide with 32 random bytes
    assert_ne!(v1.as_str(), v2.as_str());
}

#[test]
fn base64url_roundtrip_all_bytes() {
    let input: Vec<u8> = (0u8..=255).collect();
    let encoded = base64url_encode(&input);
    let decoded = base64url_decode(&encoded).expect("decode should succeed");
    assert_eq!(decoded, input, "roundtrip should reproduce original bytes");
}

#[test]
fn base64url_encode_empty() {
    assert_eq!(base64url_encode(&[]), "");
}

#[test]
fn base64url_decode_empty() {
    let decoded = base64url_decode("").expect("empty string should decode to empty vec");
    assert!(decoded.is_empty());
}

#[test]
fn base64url_decode_padded() {
    // "dGVzdA==" is base64 for "test"
    let decoded = base64url_decode("dGVzdA==").expect("padded input should decode");
    assert_eq!(decoded, b"test");
}

#[test]
fn base64url_decode_url_safe_chars() {
    // base64url uses - and _ instead of + and /
    // Encode something that would produce + or / in standard base64
    // 0xFB = 11111011 → in base64: "+" or "-"
    // Let's verify both - and _ are accepted
    let data = [0xFBu8, 0xFFu8, 0xFEu8];
    let encoded = base64url_encode(&data);
    // encoded will use - and _
    let decoded = base64url_decode(&encoded).expect("url-safe chars should decode");
    assert_eq!(decoded, data);
}

#[test]
fn base64url_decode_standard_chars_accepted() {
    // Verify that + and / (standard base64) are also accepted as aliases
    // base64url_decode treats + == - and / == _
    // base64 of [0xFB, 0xFF, 0xFE] in standard form might have + or /
    // Test with a known value: "a+b/" → same bits as "a-b_"
    let with_standard = "a+b/";
    let with_url_safe = "a-b_";
    let d1 = base64url_decode(with_standard).expect("standard chars");
    let d2 = base64url_decode(with_url_safe).expect("url-safe chars");
    assert_eq!(d1, d2, "standard and url-safe aliases should produce same output");
}

#[test]
fn base64url_decode_invalid_char_returns_error() {
    let result = base64url_decode("abc!def");
    assert!(result.is_err(), "invalid char should return error");
}

// ── discovery / preset tests ──────────────────────────────────────────────────

#[test]
fn google_preset_has_correct_endpoints() {
    let p = OidcProvider::google();
    assert_eq!(p.issuer, "https://accounts.google.com");
    assert!(p.authorization_endpoint.contains("accounts.google.com"));
    assert!(p.token_endpoint.contains("googleapis.com"));
    assert!(!p.jwks_uri.is_empty(), "Google has a JWKS URI");
    assert!(p.userinfo_endpoint.is_some());
    assert!(p.end_session_endpoint.is_none());
}

#[test]
fn microsoft_preset_has_correct_endpoints() {
    let p = OidcProvider::microsoft("contoso.onmicrosoft.com");
    assert!(p.issuer.contains("contoso.onmicrosoft.com"));
    assert!(p.authorization_endpoint.contains("oauth2/v2.0/authorize"));
    assert!(p.token_endpoint.contains("oauth2/v2.0/token"));
    assert!(!p.jwks_uri.is_empty());
    assert!(p.end_session_endpoint.is_some());
}

#[test]
fn github_preset_has_empty_jwks_uri() {
    let p = OidcProvider::github();
    assert!(p.jwks_uri.is_empty(), "GitHub does not issue JWTs");
    assert!(p.userinfo_endpoint.as_deref() == Some("https://api.github.com/user"));
}

#[test]
fn keycloak_preset_has_correct_endpoints() {
    let p = OidcProvider::keycloak("https://keycloak.example.com", "myrealm");
    assert!(p.issuer.contains("myrealm"));
    assert!(p.authorization_endpoint.contains("openid-connect/auth"));
    assert!(p.token_endpoint.contains("openid-connect/token"));
    assert!(p.jwks_uri.contains("openid-connect/certs"));
    assert!(p.end_session_endpoint.is_some());
}

#[test]
fn okta_preset_has_correct_endpoints() {
    let p = OidcProvider::okta("dev-12345.okta.com");
    assert!(p.issuer.contains("oauth2/default"));
    assert!(p.token_endpoint.contains("v1/token"));
    assert!(!p.jwks_uri.is_empty());
}

#[test]
fn auth0_preset_has_correct_endpoints() {
    let p = OidcProvider::auth0("myapp.us.auth0.com");
    assert!(p.authorization_endpoint.contains("/authorize"));
    assert!(p.token_endpoint.contains("/oauth/token"));
    assert!(p.jwks_uri.contains(".well-known/jwks.json"));
}

// ── config tests ──────────────────────────────────────────────────────────────

#[test]
fn oidc_config_google_sets_correct_scopes() {
    let c = OidcConfig::google("id", "secret", "https://app.example.com/cb");
    assert!(c.scopes.contains(&"openid".to_string()));
    assert!(c.scopes.contains(&"email".to_string()));
    assert!(c.scopes.contains(&"profile".to_string()));
}

#[test]
fn oidc_config_github_sets_github_scopes() {
    let c = OidcConfig::github("id", "secret", "https://app.example.com/cb");
    assert!(c.scopes.contains(&"read:user".to_string()));
    assert!(c.scopes.contains(&"user:email".to_string()));
}

#[test]
fn oidc_config_post_login_redirect_default_is_slash() {
    let c = OidcConfig::google("id", "secret", "https://app.example.com/cb");
    assert_eq!(c.post_login_redirect, "/");
}

#[test]
fn oidc_config_post_login_redirect_builder() {
    let c = OidcConfig::google("id", "secret", "https://app.example.com/cb")
        .post_login_redirect("/dashboard");
    assert_eq!(c.post_login_redirect, "/dashboard");
}

#[test]
fn oidc_config_scopes_builder() {
    let c = OidcConfig::google("id", "secret", "https://app.example.com/cb")
        .scopes(["openid", "custom_scope"]);
    assert_eq!(c.scopes, vec!["openid", "custom_scope"]);
}

#[test]
fn oidc_config_from_env_fails_without_env_vars() {
    // Ensure env vars are not set (they shouldn't be in test env)
    std::env::remove_var("RWS_OIDC_PROVIDER");
    let result = OidcConfig::from_env();
    assert!(result.is_err(), "from_env should fail without RWS_OIDC_PROVIDER");
}

// ── url_encode tests ──────────────────────────────────────────────────────────

#[test]
fn url_encode_preserves_safe_chars() {
    let safe = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~";
    assert_eq!(url_encode(safe), safe);
}

#[test]
fn url_encode_encodes_space() {
    assert_eq!(url_encode(" "), "%20");
}

#[test]
fn url_encode_encodes_special_chars() {
    let input = "hello world&foo=bar+baz";
    let encoded = url_encode(input);
    assert!(encoded.contains("%20"), "space encoded");
    assert!(encoded.contains("%26"), "& encoded");
    assert!(encoded.contains("%3D"), "= encoded");
    assert!(encoded.contains("%2B"), "+ encoded");
    assert!(!encoded.contains(' '), "no raw spaces");
}

#[test]
fn url_encode_empty() {
    assert_eq!(url_encode(""), "");
}

// ── json_str tests ────────────────────────────────────────────────────────────

#[test]
fn json_str_extracts_string_field() {
    let json = r#"{"foo": "bar", "baz": "qux"}"#;
    assert_eq!(json_str(json, "foo"), Some("bar".to_string()));
    assert_eq!(json_str(json, "baz"), Some("qux".to_string()));
}

#[test]
fn json_str_returns_none_for_missing() {
    let json = r#"{"foo": "bar"}"#;
    assert_eq!(json_str(json, "missing"), None);
}

#[test]
fn json_str_handles_escaped_quote() {
    let json = r#"{"msg": "say \"hello\""}"#;
    assert_eq!(json_str(json, "msg"), Some(r#"say "hello""#.to_string()));
}

#[test]
fn json_str_handles_nested_json() {
    // The extractor should pick the first occurrence of the key
    let json = r#"{"outer": "val1", "inner": {"outer": "val2"}}"#;
    // Should find the first "outer"
    assert_eq!(json_str(json, "outer"), Some("val1".to_string()));
}
