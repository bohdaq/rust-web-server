//! Unit tests for the SSO module.
//!
//! All tests operate on pure in-memory logic — no network calls are made,
//! except the `exchange_code` tests and the
//! `jwks_tests`/`discovery_tests`/`oidc_auth_tests`/`config_tests` modules
//! below, which use an in-process loopback `TcpListener` as a fake
//! token/JWKS/discovery/userinfo endpoint (same pattern as
//! `http_client::tests`) rather than a real IdP.
//!
//! `oidc_auth_tests` drives the `OidcAuth` middleware directly via its
//! `Middleware::handle` method (not through a `TestClient`-wrapped `App`),
//! matching the convention already established in `auth::tests` for other
//! middleware — this avoids needing a router just to reach a `Middleware`
//! that isn't tied to any particular route.
//!
//! `config_tests` covers `OidcConfig::from_env`, which reads/writes
//! process-wide `RWS_OIDC_*` environment variables — every test that does
//! so holds `crate::test_env::lock()` for its full duration, the same rule
//! `CLAUDE.md` documents for `RWS_CONFIG_*` vars, since both are shared
//! mutable process state under `cargo test`'s parallelism.
//!
//! `server_tests` (behind the `sso-server` feature) drives `AuthServer` —
//! the Phase 6 OAuth 2.0 Authorization Server — the same way
//! `oidc_auth_tests` drives `OidcAuth`: directly via `Middleware::handle`,
//! no fake network servers needed since `AuthServer` is entirely
//! self-contained (no outbound HTTP calls of its own).

use super::{
    client::{url_encode, OidcClient},
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
    // RWS_OIDC_* vars are process-wide state shared with every other
    // from_env test in `config_tests` below — hold the lock for the whole
    // test, same rule CLAUDE.md documents for RWS_CONFIG_* vars.
    let _g = crate::test_env::lock();
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

// ── exchange_code tests (loopback fake token endpoint) ──────────────────────────

fn start_fake_token_server(handler: impl Fn(String) -> String + Send + 'static) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let resp = handler(req);
            stream.write_all(resp.as_bytes()).ok();
        }
    });
    format!("http://127.0.0.1:{}", addr.port())
}

fn config_with_token_endpoint(token_endpoint: &str, jwks_uri: &str) -> OidcConfig {
    OidcConfig {
        provider: OidcProvider {
            issuer: "https://idp.example.com".into(),
            authorization_endpoint: "https://idp.example.com/authorize".into(),
            token_endpoint: token_endpoint.into(),
            jwks_uri: jwks_uri.into(),
            userinfo_endpoint: None,
            end_session_endpoint: None,
        },
        client_id: "my-client-id".into(),
        client_secret: "my-client-secret".into(),
        redirect_uri: "https://app.example.com/cb".into(),
        scopes: vec!["openid".into()],
        post_login_redirect: "/".into(),
    }
}

#[test]
fn exchange_code_sends_form_encoded_body_and_parses_response() {
    let base = start_fake_token_server(|req| {
        let has_ct = req.contains("Content-Type: application/x-www-form-urlencoded");
        let has_grant = req.contains("grant_type=authorization_code");
        let has_code = req.contains("code=auth-code-123");
        let has_client_id = req.contains("client_id=my-client-id");
        let has_secret = req.contains("client_secret=my-client-secret");
        let has_verifier = req.contains("code_verifier=verifier-abc");
        let ok = has_ct && has_grant && has_code && has_client_id && has_secret && has_verifier;
        let status = if ok { "200" } else { "400" };
        let body = r#"{"access_token":"tok","token_type":"Bearer","expires_in":3600,"id_token":"jwt"}"#;
        format!("HTTP/1.1 {status} OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
    });
    let config = config_with_token_endpoint(&base, "https://idp.example.com/jwks");
    let client = OidcClient::new(config);
    let result = client.exchange_code("auth-code-123", "verifier-abc").unwrap();
    assert_eq!(result.access_token, "tok");
    assert_eq!(result.token_type, "Bearer");
    assert_eq!(result.expires_in, Some(3600));
    assert_eq!(result.id_token, Some("jwt".to_string()));
}

#[test]
fn exchange_code_omits_code_verifier_when_provider_has_no_jwks_uri() {
    // GitHub-style OAuth-only providers have no jwks_uri and don't support PKCE.
    let base = start_fake_token_server(|req| {
        let has_verifier = req.contains("code_verifier");
        let status = if has_verifier { "400" } else { "200" };
        let body = r#"{"access_token":"tok","token_type":"Bearer"}"#;
        format!("HTTP/1.1 {status} OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
    });
    let config = config_with_token_endpoint(&base, "");
    let client = OidcClient::new(config);
    let result = client.exchange_code("auth-code-123", "verifier-abc").unwrap();
    assert_eq!(result.access_token, "tok");
}

#[test]
fn exchange_code_returns_error_on_non_success_status() {
    let base = start_fake_token_server(|_req| {
        let body = r#"{"error":"invalid_grant"}"#;
        format!("HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
    });
    let config = config_with_token_endpoint(&base, "https://idp.example.com/jwks");
    let client = OidcClient::new(config);
    let result = client.exchange_code("bad-code", "verifier-abc");
    match result {
        Err(e) => assert!(e.0.contains("400"), "expected 400 in error, got: {}", e.0),
        Ok(_) => panic!("expected an error for a 400 response"),
    }
}

// ── JWKS fetch + RS256/ES256 JWT verification (loopback fake JWKS endpoint) ─────

mod jwks_tests {
    use super::super::jwks::VerifyOptions;
    use super::super::pkce::base64url_encode;
    use super::super::JwksCache;
    use std::sync::OnceLock;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unix_now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    /// Serve `bodies[i]` to the i-th accepted connection, repeating the last
    /// entry for any connection beyond the list (JwksCache may fetch twice:
    /// once on lazy-load, once more on a failed-verification retry).
    fn jwks_server_sequence(bodies: Vec<String>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let mut i = 0usize;
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = vec![0u8; 4096];
                let _ = stream.read(&mut buf);
                let body = bodies.get(i).or_else(|| bodies.last()).cloned().unwrap_or_default();
                i += 1;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        format!("http://127.0.0.1:{}", addr.port())
    }

    pub(super) fn jwks_server(jwk_json: &str) -> String {
        jwks_server_sequence(vec![format!(r#"{{"keys":[{jwk_json}]}}"#)])
    }

    pub(super) fn build_jwt(header: &str, payload: &str, sign: impl FnOnce(&[u8]) -> Vec<u8>) -> String {
        let h = base64url_encode(header.as_bytes());
        let p = base64url_encode(payload.as_bytes());
        let message = format!("{h}.{p}");
        let sig = sign(message.as_bytes());
        let s = base64url_encode(&sig);
        format!("{message}.{s}")
    }

    // ── RSA (RS256) ──────────────────────────────────────────────────────────

    /// RSA key generation is slow (~100ms+); share one 2048-bit key across
    /// every RS256 test rather than generating a fresh one each time.
    fn rsa_key() -> &'static rsa::RsaPrivateKey {
        static KEY: OnceLock<rsa::RsaPrivateKey> = OnceLock::new();
        KEY.get_or_init(|| rsa::RsaPrivateKey::new(&mut rand_core::OsRng, 2048).unwrap())
    }

    pub(super) fn rsa_jwk_json(kid: &str) -> String {
        use rsa::traits::PublicKeyParts;
        let pub_key = rsa_key().to_public_key();
        let n = base64url_encode(&pub_key.n().to_bytes_be());
        let e = base64url_encode(&pub_key.e().to_bytes_be());
        format!(r#"{{"kty":"RSA","kid":"{kid}","n":"{n}","e":"{e}"}}"#)
    }

    pub(super) fn sign_rs256(message: &[u8]) -> Vec<u8> {
        use rsa::pkcs1v15::SigningKey;
        use rsa::signature::{SignatureEncoding, Signer};
        let signing_key = SigningKey::<sha2::Sha256>::new(rsa_key().clone());
        signing_key.sign(message).to_bytes().to_vec()
    }

    #[test]
    fn verify_jwt_rs256_success() {
        let claims = format!(
            r#"{{"sub":"user1","iss":"https://idp.example.com","aud":"client1","exp":{},"iat":{}}}"#,
            unix_now() + 3600,
            unix_now()
        );
        let header = r#"{"alg":"RS256","kid":"rsa-key-1"}"#;
        let token = build_jwt(header, &claims, sign_rs256);

        let jwks_url = jwks_server(&rsa_jwk_json("rsa-key-1"));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        let claims = cache.verify_jwt(&token, &opts).unwrap();
        assert_eq!(claims.sub, "user1");
        assert_eq!(claims.iss, "https://idp.example.com");
    }

    #[test]
    fn verify_jwt_rs256_tampered_signature_fails() {
        let claims = format!(
            r#"{{"sub":"user1","iss":"https://idp.example.com","aud":"client1","exp":{},"iat":{}}}"#,
            unix_now() + 3600,
            unix_now()
        );
        let header = r#"{"alg":"RS256","kid":"rsa-key-1"}"#;
        let mut token = build_jwt(header, &claims, sign_rs256);
        // Flip the first character of the signature segment.
        let sig_start = token.rfind('.').unwrap() + 1;
        let flipped = if token.as_bytes()[sig_start] == b'A' { 'B' } else { 'A' };
        token.replace_range(sig_start..sig_start + 1, &flipped.to_string());

        let jwks_url = jwks_server(&rsa_jwk_json("rsa-key-1"));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        assert!(cache.verify_jwt(&token, &opts).is_err());
    }

    // ── EC (ES256) ───────────────────────────────────────────────────────────

    fn ec_jwk_json(kid: &str, verifying_key: &p256::ecdsa::VerifyingKey) -> String {
        let point = verifying_key.to_encoded_point(false);
        let x = base64url_encode(point.x().unwrap());
        let y = base64url_encode(point.y().unwrap());
        format!(r#"{{"kty":"EC","kid":"{kid}","crv":"P-256","x":"{x}","y":"{y}"}}"#)
    }

    fn sign_es256(signing_key: &p256::ecdsa::SigningKey, message: &[u8]) -> Vec<u8> {
        use p256::ecdsa::signature::Signer;
        let sig: p256::ecdsa::Signature = signing_key.sign(message);
        sig.to_bytes().to_vec()
    }

    #[test]
    fn verify_jwt_es256_success() {
        let signing_key = p256::ecdsa::SigningKey::random(&mut rand_core::OsRng);
        let claims = format!(
            r#"{{"sub":"user2","iss":"https://idp.example.com","aud":"client1","exp":{},"iat":{}}}"#,
            unix_now() + 3600,
            unix_now()
        );
        let header = r#"{"alg":"ES256","kid":"ec-key-1"}"#;
        let token = build_jwt(header, &claims, |msg| sign_es256(&signing_key, msg));

        let jwks_url = jwks_server(&ec_jwk_json("ec-key-1", signing_key.verifying_key()));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        let claims = cache.verify_jwt(&token, &opts).unwrap();
        assert_eq!(claims.sub, "user2");
    }

    // ── claim validation ─────────────────────────────────────────────────────

    #[test]
    fn verify_jwt_expired_token_fails() {
        let claims = format!(
            r#"{{"sub":"user1","iss":"https://idp.example.com","aud":"client1","exp":{},"iat":{}}}"#,
            unix_now() - 3600,
            unix_now() - 7200
        );
        let header = r#"{"alg":"RS256","kid":"rsa-key-1"}"#;
        let token = build_jwt(header, &claims, sign_rs256);

        let jwks_url = jwks_server(&rsa_jwk_json("rsa-key-1"));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        let err = cache.verify_jwt(&token, &opts).unwrap_err();
        assert!(err.0.contains("expired"), "expected expiry error, got: {}", err.0);
    }

    #[test]
    fn verify_jwt_iat_in_future_fails() {
        let claims = format!(
            r#"{{"sub":"user1","iss":"https://idp.example.com","aud":"client1","exp":{},"iat":{}}}"#,
            unix_now() + 7200,
            unix_now() + 3600
        );
        let header = r#"{"alg":"RS256","kid":"rsa-key-1"}"#;
        let token = build_jwt(header, &claims, sign_rs256);

        let jwks_url = jwks_server(&rsa_jwk_json("rsa-key-1"));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        let err = cache.verify_jwt(&token, &opts).unwrap_err();
        assert!(err.0.contains("future"), "expected issued-in-future error, got: {}", err.0);
    }

    #[test]
    fn verify_jwt_leeway_permits_small_clock_skew() {
        // Expired 5 seconds ago, but leeway_secs=30 should still accept it.
        let claims = format!(
            r#"{{"sub":"user1","iss":"https://idp.example.com","aud":"client1","exp":{},"iat":{}}}"#,
            unix_now() - 5,
            unix_now() - 100
        );
        let header = r#"{"alg":"RS256","kid":"rsa-key-1"}"#;
        let token = build_jwt(header, &claims, sign_rs256);

        let jwks_url = jwks_server(&rsa_jwk_json("rsa-key-1"));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 30,
        };
        assert!(cache.verify_jwt(&token, &opts).is_ok());
    }

    #[test]
    fn verify_jwt_wrong_issuer_fails() {
        let claims = format!(
            r#"{{"sub":"user1","iss":"https://evil.example.com","aud":"client1","exp":{},"iat":{}}}"#,
            unix_now() + 3600,
            unix_now()
        );
        let header = r#"{"alg":"RS256","kid":"rsa-key-1"}"#;
        let token = build_jwt(header, &claims, sign_rs256);

        let jwks_url = jwks_server(&rsa_jwk_json("rsa-key-1"));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        let err = cache.verify_jwt(&token, &opts).unwrap_err();
        assert!(err.0.contains("issuer"), "expected issuer error, got: {}", err.0);
    }

    #[test]
    fn verify_jwt_wrong_audience_fails() {
        let claims = format!(
            r#"{{"sub":"user1","iss":"https://idp.example.com","aud":"someone-else","exp":{},"iat":{}}}"#,
            unix_now() + 3600,
            unix_now()
        );
        let header = r#"{"alg":"RS256","kid":"rsa-key-1"}"#;
        let token = build_jwt(header, &claims, sign_rs256);

        let jwks_url = jwks_server(&rsa_jwk_json("rsa-key-1"));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        let err = cache.verify_jwt(&token, &opts).unwrap_err();
        assert!(err.0.contains("audience"), "expected audience error, got: {}", err.0);
    }

    #[test]
    fn verify_jwt_aud_array_form_matches_one_of_multiple() {
        let claims = format!(
            r#"{{"sub":"user1","iss":"https://idp.example.com","aud":["other-client","client1"],"exp":{},"iat":{}}}"#,
            unix_now() + 3600,
            unix_now()
        );
        let header = r#"{"alg":"RS256","kid":"rsa-key-1"}"#;
        let token = build_jwt(header, &claims, sign_rs256);

        let jwks_url = jwks_server(&rsa_jwk_json("rsa-key-1"));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        let claims = cache.verify_jwt(&token, &opts).unwrap();
        assert_eq!(claims.aud, vec!["other-client".to_string(), "client1".to_string()]);
    }

    #[test]
    fn verify_jwt_unsupported_alg_fails() {
        let claims = format!(
            r#"{{"sub":"user1","iss":"https://idp.example.com","aud":"client1","exp":{},"iat":{}}}"#,
            unix_now() + 3600,
            unix_now()
        );
        // alg "HS256" is not implemented by try_verify's RS256/ES256 match arms.
        let header = r#"{"alg":"HS256","kid":"rsa-key-1"}"#;
        let token = build_jwt(header, &claims, sign_rs256);

        let jwks_url = jwks_server(&rsa_jwk_json("rsa-key-1"));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        assert!(cache.verify_jwt(&token, &opts).is_err());
    }

    #[test]
    fn verify_jwt_malformed_token_wrong_part_count_fails() {
        let jwks_url = jwks_server(&rsa_jwk_json("rsa-key-1"));
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        let err = cache.verify_jwt("only.two-parts", &opts).unwrap_err();
        assert!(err.0.contains("3 parts"), "expected part-count error, got: {}", err.0);
    }

    // ── key rotation ─────────────────────────────────────────────────────────

    #[test]
    fn verify_jwt_refetches_and_succeeds_after_kid_miss() {
        // Lazy-load populates the cache with an unrelated key (simulating a
        // cache from before the IdP rotated its signing key). The incoming
        // token's kid isn't among those cached keys, so the first
        // try_verify finds no candidates and fails; JwksCache must refetch
        // and retry before giving up.
        let old_signing_key = p256::ecdsa::SigningKey::random(&mut rand_core::OsRng);
        let old_jwk = ec_jwk_json("old-key", old_signing_key.verifying_key());

        let claims = format!(
            r#"{{"sub":"user1","iss":"https://idp.example.com","aud":"client1","exp":{},"iat":{}}}"#,
            unix_now() + 3600,
            unix_now()
        );
        let header = r#"{"alg":"RS256","kid":"rsa-key-new"}"#;
        let token = build_jwt(header, &claims, sign_rs256);

        let jwks_url = jwks_server_sequence(vec![
            format!(r#"{{"keys":[{old_jwk}]}}"#),
            format!(r#"{{"keys":[{}]}}"#, rsa_jwk_json("rsa-key-new")),
        ]);
        let cache = JwksCache::new(&jwks_url);
        let opts = VerifyOptions {
            audience: "client1",
            issuer: "https://idp.example.com",
            leeway_secs: 0,
        };
        let claims = cache.verify_jwt(&token, &opts).unwrap();
        assert_eq!(claims.sub, "user1");
    }
}

// ── OIDC discovery (loopback fake `.well-known/openid-configuration`) ──────────

mod discovery_tests {
    use super::super::discovery::OidcProvider;
    use super::start_fake_token_server;

    #[test]
    fn discover_parses_all_fields() {
        let base = start_fake_token_server(|req| {
            let path_ok = req.starts_with("GET /.well-known/openid-configuration ");
            let body = r#"{
                "issuer": "https://idp.example.com",
                "authorization_endpoint": "https://idp.example.com/authorize",
                "token_endpoint": "https://idp.example.com/token",
                "jwks_uri": "https://idp.example.com/jwks",
                "userinfo_endpoint": "https://idp.example.com/userinfo",
                "end_session_endpoint": "https://idp.example.com/logout"
            }"#;
            let status = if path_ok { "200" } else { "404" };
            format!("HTTP/1.1 {status} OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
        });
        let provider = OidcProvider::discover(&base).unwrap();
        assert_eq!(provider.issuer, "https://idp.example.com");
        assert_eq!(provider.authorization_endpoint, "https://idp.example.com/authorize");
        assert_eq!(provider.token_endpoint, "https://idp.example.com/token");
        assert_eq!(provider.jwks_uri, "https://idp.example.com/jwks");
        assert_eq!(provider.userinfo_endpoint, Some("https://idp.example.com/userinfo".to_string()));
        assert_eq!(provider.end_session_endpoint, Some("https://idp.example.com/logout".to_string()));
    }

    #[test]
    fn discover_optional_fields_absent_become_none() {
        let base = start_fake_token_server(|_req| {
            let body = r#"{
                "issuer": "https://idp.example.com",
                "authorization_endpoint": "https://idp.example.com/authorize",
                "token_endpoint": "https://idp.example.com/token",
                "jwks_uri": "https://idp.example.com/jwks"
            }"#;
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
        });
        let provider = OidcProvider::discover(&base).unwrap();
        assert_eq!(provider.userinfo_endpoint, None);
        assert_eq!(provider.end_session_endpoint, None);
    }

    #[test]
    fn discover_strips_trailing_slash_from_issuer_before_building_url() {
        let base = start_fake_token_server(|req| {
            // A trailing slash on the issuer must not produce a double
            // slash before .well-known.
            let ok = req.starts_with("GET /.well-known/openid-configuration ")
                && !req.starts_with("GET //.well-known");
            let body = r#"{
                "issuer": "https://idp.example.com",
                "authorization_endpoint": "https://idp.example.com/authorize",
                "token_endpoint": "https://idp.example.com/token",
                "jwks_uri": "https://idp.example.com/jwks"
            }"#;
            let status = if ok { "200" } else { "400" };
            format!("HTTP/1.1 {status} OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
        });
        let issuer_with_slash = format!("{base}/");
        let provider = OidcProvider::discover(&issuer_with_slash).unwrap();
        assert_eq!(provider.token_endpoint, "https://idp.example.com/token");
    }

    #[test]
    fn discover_missing_required_field_fails() {
        let base = start_fake_token_server(|_req| {
            // No token_endpoint.
            let body = r#"{
                "issuer": "https://idp.example.com",
                "authorization_endpoint": "https://idp.example.com/authorize",
                "jwks_uri": "https://idp.example.com/jwks"
            }"#;
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
        });
        let err = OidcProvider::discover(&base).unwrap_err();
        assert!(err.0.contains("token_endpoint"), "expected token_endpoint error, got: {}", err.0);
    }

    #[test]
    fn discover_non_success_status_fails() {
        let base = start_fake_token_server(|_req| {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string()
        });
        let err = OidcProvider::discover(&base).unwrap_err();
        assert!(err.0.contains("404"), "expected 404 in error, got: {}", err.0);
    }

    #[test]
    fn discover_missing_issuer_defaults_to_empty_string() {
        // `issuer` is the one field parse_discovery_json treats as optional
        // with a default rather than a hard error.
        let base = start_fake_token_server(|_req| {
            let body = r#"{
                "authorization_endpoint": "https://idp.example.com/authorize",
                "token_endpoint": "https://idp.example.com/token",
                "jwks_uri": "https://idp.example.com/jwks"
            }"#;
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
        });
        let provider = OidcProvider::discover(&base).unwrap();
        assert_eq!(provider.issuer, "");
    }

    #[test]
    fn discover_connection_failure_fails() {
        // Nothing is listening on this port.
        let err = OidcProvider::discover("http://127.0.0.1:1").unwrap_err();
        assert!(err.0.contains("discovery fetch failed"), "expected fetch-failed error, got: {}", err.0);
    }
}

// ── OidcAuth middleware — authorization-code + PKCE flow ────────────────────────

mod oidc_auth_tests {
    use super::super::{OidcAuth, OidcConfig};
    use super::super::discovery::OidcProvider;
    use super::{config_with_token_endpoint, jwks_tests, start_fake_token_server};
    use crate::application::Application;
    use crate::core::New;
    use crate::header::Header;
    use crate::http::VERSION;
    use crate::middleware::Middleware;
    use crate::request::{Request, METHOD};
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
    use crate::server::{Address, ConnectionInfo};
    use crate::session::SessionStore;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Mirrors the private `SESSION_COOKIE` constant in `oidc_auth.rs` — there
    // is exactly one session cookie this middleware ever sets, so hardcoding
    // its name here is simpler than round-tripping it through a login call
    // for every test that needs a pre-populated session.
    const SID_COOKIE: &str = "_rws_sid";

    fn unix_now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    fn conn() -> ConnectionInfo {
        ConnectionInfo {
            client: Address { ip: "127.0.0.1".to_string(), port: 0 },
            server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
            request_size: 16000,
            sni_hostname: None,
        }
    }

    fn get(uri: &str) -> Request {
        Request {
            method: METHOD.get.to_string(),
            request_uri: uri.to_string(),
            http_version: VERSION.http_1_1.to_string(),
            headers: vec![],
            body: vec![],
        }
    }

    fn with_cookie(mut req: Request, name: &str, value: &str) -> Request {
        req.headers.push(Header { name: "Cookie".to_string(), value: format!("{name}={value}") });
        req
    }

    fn header(response: &Response, name: &str) -> Option<String> {
        response.headers.iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.clone())
    }

    /// An `Application` that always returns 200 OK.
    struct OkApp;
    impl Application for OkApp {
        fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
            let mut r = Response::new();
            r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
            r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
            Ok(r)
        }
    }

    /// An `Application` that records the request it received, for asserting
    /// on headers injected by the middleware before the request reaches it.
    struct CapturingApp {
        captured: Mutex<Option<Request>>,
    }
    impl CapturingApp {
        fn new() -> Self {
            CapturingApp { captured: Mutex::new(None) }
        }
    }
    impl Application for CapturingApp {
        fn execute(&self, req: &Request, _: &ConnectionInfo) -> Result<Response, String> {
            *self.captured.lock().unwrap() = Some(req.clone());
            let mut r = Response::new();
            r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
            r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
            Ok(r)
        }
    }

    fn google_config() -> OidcConfig {
        OidcConfig::google("client1", "secret1", "https://app.example.com/cb")
    }

    // ── unauthenticated access ───────────────────────────────────────────────

    #[test]
    fn unauthenticated_request_redirects_to_login() {
        let sessions = Arc::new(SessionStore::new(3600));
        let oidc = OidcAuth::new(google_config(), sessions);
        let resp = oidc.handle(&get("/dashboard"), &conn(), &OkApp).unwrap();
        assert_eq!(302, resp.status_code);
        let location = header(&resp, "Location").unwrap();
        assert!(location.starts_with("/auth/login?return_to="));
    }

    #[test]
    fn excluded_path_bypasses_auth_without_session() {
        let sessions = Arc::new(SessionStore::new(3600));
        let oidc = OidcAuth::new(google_config(), sessions).exclude("/healthz");
        let resp = oidc.handle(&get("/healthz"), &conn(), &OkApp).unwrap();
        assert_eq!(200, resp.status_code);
    }

    // ── /auth/login ──────────────────────────────────────────────────────────

    #[test]
    fn login_generates_pkce_state_nonce_and_redirects_to_provider() {
        let sessions = Arc::new(SessionStore::new(3600));
        let oidc = OidcAuth::new(google_config(), sessions.clone());
        let resp = oidc.handle(&get("/auth/login"), &conn(), &OkApp).unwrap();
        assert_eq!(302, resp.status_code);
        let location = header(&resp, "Location").unwrap();
        assert!(location.starts_with("https://accounts.google.com/o/oauth2/v2/auth"));
        assert!(location.contains("code_challenge="), "PKCE challenge should be present for a JWKS-capable provider");
        assert!(location.contains("state="));

        let (cookie_name, sid) = extract_cookie(&resp);
        assert_eq!(cookie_name, SID_COOKIE);
        let session = sessions.load(&sid).expect("session should have been created");
        assert!(!session.get("_oidc_state").unwrap().is_empty());
        assert!(!session.get("_oidc_nonce").unwrap().is_empty());
        assert!(!session.get("_oidc_pkce").unwrap().is_empty());
    }

    #[test]
    fn login_two_calls_generate_different_state_and_nonce() {
        let sessions = Arc::new(SessionStore::new(3600));
        let oidc = OidcAuth::new(google_config(), sessions.clone());
        let (_, sid1) = extract_cookie(&oidc.handle(&get("/auth/login"), &conn(), &OkApp).unwrap());
        let (_, sid2) = extract_cookie(&oidc.handle(&get("/auth/login"), &conn(), &OkApp).unwrap());
        let s1 = sessions.load(&sid1).unwrap();
        let s2 = sessions.load(&sid2).unwrap();
        assert_ne!(s1.get("_oidc_state"), s2.get("_oidc_state"));
        assert_ne!(s1.get("_oidc_nonce"), s2.get("_oidc_nonce"));
    }

    #[test]
    fn login_stores_return_to_from_query_param() {
        let sessions = Arc::new(SessionStore::new(3600));
        let oidc = OidcAuth::new(google_config(), sessions.clone());
        let resp = oidc.handle(&get("/auth/login?return_to=%2Fdashboard"), &conn(), &OkApp).unwrap();
        let (_, sid) = extract_cookie(&resp);
        let session = sessions.load(&sid).unwrap();
        assert_eq!(session.get("_oidc_return_to"), Some("/dashboard"));
    }

    #[test]
    fn login_defaults_return_to_when_absent() {
        let sessions = Arc::new(SessionStore::new(3600));
        let config = google_config().post_login_redirect("/home");
        let oidc = OidcAuth::new(config, sessions.clone());
        let resp = oidc.handle(&get("/auth/login"), &conn(), &OkApp).unwrap();
        let (_, sid) = extract_cookie(&resp);
        let session = sessions.load(&sid).unwrap();
        assert_eq!(session.get("_oidc_return_to"), Some("/home"));
    }

    fn extract_cookie(response: &Response) -> (String, String) {
        let set_cookie = header(response, "Set-Cookie").expect("expected a Set-Cookie header");
        let first = set_cookie.split(';').next().unwrap();
        let mut parts = first.splitn(2, '=');
        (parts.next().unwrap().to_string(), parts.next().unwrap_or("").to_string())
    }

    // ── /auth/callback ───────────────────────────────────────────────────────

    #[test]
    fn callback_without_cookie_returns_forbidden() {
        let sessions = Arc::new(SessionStore::new(3600));
        let oidc = OidcAuth::new(google_config(), sessions);
        let resp = oidc.handle(&get("/auth/callback?code=abc&state=xyz"), &conn(), &OkApp).unwrap();
        assert_eq!(403, resp.status_code);
    }

    #[test]
    fn callback_with_unknown_session_returns_forbidden() {
        let sessions = Arc::new(SessionStore::new(3600));
        let oidc = OidcAuth::new(google_config(), sessions);
        let req = with_cookie(get("/auth/callback?code=abc&state=xyz"), SID_COOKIE, "does-not-exist");
        let resp = oidc.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(403, resp.status_code);
    }

    #[test]
    fn callback_state_mismatch_returns_forbidden() {
        let sessions = Arc::new(SessionStore::new(3600));
        let mut pre = sessions.create();
        pre.set("_oidc_state", "expected-state");
        sessions.save(&pre);

        let oidc = OidcAuth::new(google_config(), sessions);
        let req = with_cookie(get("/auth/callback?code=abc&state=wrong-state"), SID_COOKIE, &pre.id);
        let resp = oidc.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(403, resp.status_code);
    }

    #[test]
    fn callback_provider_error_surfaces_as_error_response() {
        let sessions = Arc::new(SessionStore::new(3600));
        let mut pre = sessions.create();
        pre.set("_oidc_state", "state1");
        sessions.save(&pre);

        let oidc = OidcAuth::new(google_config(), sessions);
        let req = with_cookie(get("/auth/callback?state=state1&error=access_denied"), SID_COOKIE, &pre.id);
        let resp = oidc.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(500, resp.status_code);
    }

    fn signed_id_token(nonce: &str, aud: &str, iss: &str) -> String {
        let claims = format!(
            r#"{{"sub":"user1","iss":"{iss}","aud":"{aud}","exp":{},"iat":{},"nonce":"{nonce}"}}"#,
            unix_now() + 3600,
            unix_now()
        );
        let header = r#"{"alg":"RS256","kid":"rsa-key-1"}"#;
        jwks_tests::build_jwt(header, &claims, jwks_tests::sign_rs256)
    }

    #[test]
    fn callback_success_verifies_id_token_and_stores_claims() {
        let id_token = signed_id_token("nonce1", "my-client-id", "https://idp.example.com");
        let token_body = format!(
            r#"{{"access_token":"tok","token_type":"Bearer","id_token":"{id_token}"}}"#
        );
        let token_url = start_fake_token_server(move |_req| {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                token_body.len(),
                token_body
            )
        });
        let jwks_url = jwks_tests::jwks_server(&jwks_tests::rsa_jwk_json("rsa-key-1"));
        let config = config_with_token_endpoint(&token_url, &jwks_url);

        let sessions = Arc::new(SessionStore::new(3600));
        let mut pre = sessions.create();
        pre.set("_oidc_state", "state1");
        pre.set("_oidc_nonce", "nonce1");
        pre.set("_oidc_pkce", "verifier1");
        pre.set("_oidc_return_to", "/dashboard");
        sessions.save(&pre);

        let oidc = OidcAuth::new(config, sessions.clone());
        let req = with_cookie(get("/auth/callback?code=authcode&state=state1"), SID_COOKIE, &pre.id);
        let resp = oidc.handle(&req, &conn(), &OkApp).unwrap();

        assert_eq!(302, resp.status_code);
        assert_eq!(header(&resp, "Location"), Some("/dashboard".to_string()));

        let session = sessions.load(&pre.id).unwrap();
        assert!(session.get("_oidc_claims").unwrap().contains("user1"));
        assert!(session.get("_oidc_state").is_none());
        assert!(session.get("_oidc_nonce").is_none());
        assert!(session.get("_oidc_pkce").is_none());
        assert!(session.get("_oidc_return_to").is_none());
    }

    #[test]
    fn callback_nonce_mismatch_returns_forbidden() {
        let id_token = signed_id_token("wrong-nonce", "my-client-id", "https://idp.example.com");
        let token_body = format!(
            r#"{{"access_token":"tok","token_type":"Bearer","id_token":"{id_token}"}}"#
        );
        let token_url = start_fake_token_server(move |_req| {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                token_body.len(),
                token_body
            )
        });
        let jwks_url = jwks_tests::jwks_server(&jwks_tests::rsa_jwk_json("rsa-key-1"));
        let config = config_with_token_endpoint(&token_url, &jwks_url);

        let sessions = Arc::new(SessionStore::new(3600));
        let mut pre = sessions.create();
        pre.set("_oidc_state", "state1");
        pre.set("_oidc_nonce", "expected-nonce");
        pre.set("_oidc_pkce", "verifier1");
        sessions.save(&pre);

        let oidc = OidcAuth::new(config, sessions);
        let req = with_cookie(get("/auth/callback?code=authcode&state=state1"), SID_COOKIE, &pre.id);
        let resp = oidc.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(403, resp.status_code);
    }

    #[test]
    fn callback_falls_back_to_userinfo_when_provider_has_no_jwks() {
        // GitHub-style: no id_token, so OidcAuth falls back to fetch_user_info.
        let userinfo_url = start_fake_token_server(|_req| {
            let body = r#"{"sub":"user2","email":"user2@example.com","name":"User Two"}"#;
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
        });
        let token_body = r#"{"access_token":"tok","token_type":"Bearer"}"#.to_string();
        let token_url = start_fake_token_server(move |_req| {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                token_body.len(),
                token_body
            )
        });
        let config = OidcConfig {
            provider: OidcProvider {
                issuer: "https://idp.example.com".into(),
                authorization_endpoint: "https://idp.example.com/authorize".into(),
                token_endpoint: token_url,
                jwks_uri: String::new(),
                userinfo_endpoint: Some(userinfo_url),
                end_session_endpoint: None,
            },
            client_id: "my-client-id".into(),
            client_secret: "my-client-secret".into(),
            redirect_uri: "https://app.example.com/cb".into(),
            scopes: vec!["read:user".into()],
            post_login_redirect: "/".into(),
        };

        let sessions = Arc::new(SessionStore::new(3600));
        let mut pre = sessions.create();
        pre.set("_oidc_state", "state1");
        sessions.save(&pre);

        let oidc = OidcAuth::new(config, sessions.clone());
        let req = with_cookie(get("/auth/callback?code=authcode&state=state1"), SID_COOKIE, &pre.id);
        let resp = oidc.handle(&req, &conn(), &OkApp).unwrap();

        assert_eq!(302, resp.status_code);
        let session = sessions.load(&pre.id).unwrap();
        assert!(session.get("_oidc_claims").unwrap().contains("user2@example.com"));
    }

    // ── authenticated pass-through ───────────────────────────────────────────

    #[test]
    fn authenticated_request_injects_claims_header_and_passes_through() {
        let sessions = Arc::new(SessionStore::new(3600));
        let mut session = sessions.create();
        session.set("_oidc_claims", r#"{"sub":"user1","iss":"i","aud":[],"exp":0,"iat":0,"email":"user1@example.com"}"#);
        sessions.save(&session);

        let oidc = OidcAuth::new(google_config(), sessions);
        let req = with_cookie(get("/dashboard"), SID_COOKIE, &session.id);
        let inner = CapturingApp::new();
        let resp = oidc.handle(&req, &conn(), &inner).unwrap();

        assert_eq!(200, resp.status_code);
        let captured = inner.captured.lock().unwrap().clone().expect("inner app should have been called");
        let claims_header = captured.headers.iter()
            .find(|h| h.name.eq_ignore_ascii_case(super::super::oidc_auth::CLAIMS_HEADER))
            .expect("claims header should be injected");
        assert!(claims_header.value.contains("user1@example.com"));
    }

    // ── /auth/logout ─────────────────────────────────────────────────────────

    #[test]
    fn logout_destroys_session_and_redirects_home() {
        let sessions = Arc::new(SessionStore::new(3600));
        let mut session = sessions.create();
        session.set("_oidc_claims", "{}");
        sessions.save(&session);

        let oidc = OidcAuth::new(google_config(), sessions.clone());
        let req = with_cookie(get("/auth/logout"), SID_COOKIE, &session.id);
        let resp = oidc.handle(&req, &conn(), &OkApp).unwrap();

        assert_eq!(302, resp.status_code);
        assert_eq!(header(&resp, "Location"), Some("/".to_string()));
        assert!(sessions.load(&session.id).is_none(), "session should be destroyed");
    }

    #[test]
    fn logout_without_cookie_still_redirects_home() {
        let sessions = Arc::new(SessionStore::new(3600));
        let oidc = OidcAuth::new(google_config(), sessions);
        let resp = oidc.handle(&get("/auth/logout"), &conn(), &OkApp).unwrap();
        assert_eq!(302, resp.status_code);
        assert_eq!(header(&resp, "Location"), Some("/".to_string()));
    }

    // ── custom paths ─────────────────────────────────────────────────────────

    #[test]
    fn custom_login_path_overrides_default() {
        let sessions = Arc::new(SessionStore::new(3600));
        let oidc = OidcAuth::new(google_config(), sessions).login_path("/custom-login");

        // The default /auth/login is no longer special-cased, so with no
        // session it falls through to the "redirect to login" branch, which
        // now points at the *custom* login path.
        let resp = oidc.handle(&get("/auth/login"), &conn(), &OkApp).unwrap();
        assert_eq!(302, resp.status_code);
        assert!(header(&resp, "Location").unwrap().starts_with("/custom-login?return_to="));

        // The custom path itself triggers the real login handler.
        let resp2 = oidc.handle(&get("/custom-login"), &conn(), &OkApp).unwrap();
        assert_eq!(302, resp2.status_code);
        assert!(header(&resp2, "Location").unwrap().starts_with("https://accounts.google.com"));
    }

    // ── claims accessors ─────────────────────────────────────────────────────

    #[test]
    fn claims_accessors_read_injected_header() {
        let mut req = get("/dashboard");
        req.headers.push(Header {
            name: super::super::oidc_auth::CLAIMS_HEADER.to_string(),
            value: r#"{"sub":"user1","iss":"i","aud":[],"exp":0,"iat":0,"email":"user1@example.com"}"#.to_string(),
        });
        assert_eq!(OidcAuth::sub(&req), Some("user1".to_string()));
        assert_eq!(OidcAuth::email(&req), Some("user1@example.com".to_string()));
        assert!(OidcAuth::claims(&req).is_some());
    }

    #[test]
    fn claims_accessors_return_none_without_header() {
        let req = get("/dashboard");
        assert_eq!(OidcAuth::sub(&req), None);
        assert_eq!(OidcAuth::email(&req), None);
        assert!(OidcAuth::claims(&req).is_none());
    }
}

// ── Provider presets and OidcConfig::from_env (Phase 5) ─────────────────────────

mod config_tests {
    use super::super::config::OidcConfig;
    use super::start_fake_token_server;

    // ── OidcConfig-level presets (client_id/secret/redirect_uri plumbing,
    //    not just the provider endpoints already covered by the top-level
    //    discovery preset tests) ───────────────────────────────────────────

    #[test]
    fn oidc_config_microsoft_preset() {
        let c = OidcConfig::microsoft("contoso-tenant", "id1", "secret1", "https://app.example.com/cb");
        assert_eq!(c.client_id, "id1");
        assert_eq!(c.client_secret, "secret1");
        assert_eq!(c.redirect_uri, "https://app.example.com/cb");
        assert!(c.provider.issuer.contains("contoso-tenant"));
        assert!(c.scopes.contains(&"openid".to_string()));
    }

    #[test]
    fn oidc_config_okta_preset() {
        let c = OidcConfig::okta("dev-1.okta.com", "id1", "secret1", "https://app.example.com/cb");
        assert_eq!(c.client_id, "id1");
        assert!(c.provider.issuer.contains("dev-1.okta.com"));
        assert!(c.scopes.contains(&"profile".to_string()));
    }

    #[test]
    fn oidc_config_auth0_preset() {
        let c = OidcConfig::auth0("myapp.us.auth0.com", "id1", "secret1", "https://app.example.com/cb");
        assert_eq!(c.client_id, "id1");
        assert!(c.provider.authorization_endpoint.contains("myapp.us.auth0.com"));
        assert!(c.scopes.contains(&"email".to_string()));
    }

    #[test]
    fn oidc_config_keycloak_preset() {
        let c = OidcConfig::keycloak(
            "https://keycloak.example.com",
            "myrealm",
            "id1",
            "secret1",
            "https://app.example.com/cb",
        );
        assert_eq!(c.client_id, "id1");
        assert!(c.provider.issuer.contains("myrealm"));
    }

    // ── OidcConfig::discover ─────────────────────────────────────────────────

    #[test]
    fn oidc_config_discover_builds_full_config_from_a_live_document() {
        let base = start_fake_token_server(|_req| {
            let body = r#"{
                "issuer": "https://idp.example.com",
                "authorization_endpoint": "https://idp.example.com/authorize",
                "token_endpoint": "https://idp.example.com/token",
                "jwks_uri": "https://idp.example.com/jwks"
            }"#;
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
        });
        let config = OidcConfig::discover(&base, "id1", "secret1", "https://app.example.com/cb").unwrap();
        assert_eq!(config.client_id, "id1");
        assert_eq!(config.provider.token_endpoint, "https://idp.example.com/token");
        assert!(config.scopes.contains(&"openid".to_string()));
    }

    #[test]
    fn oidc_config_discover_propagates_provider_error() {
        let base = start_fake_token_server(|_req| {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string()
        });
        let result = OidcConfig::discover(&base, "id1", "secret1", "https://app.example.com/cb");
        assert!(result.is_err());
    }

    // ── OidcConfig::from_env ─────────────────────────────────────────────────
    //
    // RWS_OIDC_* vars are process-wide state. Every test below holds
    // `crate::test_env::lock()` for its full duration (same rule CLAUDE.md
    // documents for RWS_CONFIG_* vars) and clears every var it touched
    // before returning, matching the established cleanup convention used
    // throughout this codebase's other env-var tests.

    // `OidcConfig` doesn't derive `Debug` (no reason to for a real config
    // type), so `Result::unwrap_err()` isn't available directly.
    fn expect_from_env_err() -> super::super::SsoError {
        match OidcConfig::from_env() {
            Err(e) => e,
            Ok(_) => panic!("expected OidcConfig::from_env() to fail"),
        }
    }

    fn clear_oidc_env() {
        for key in [
            "RWS_OIDC_PROVIDER",
            "RWS_OIDC_CLIENT_ID",
            "RWS_OIDC_CLIENT_SECRET",
            "RWS_OIDC_REDIRECT_URI",
            "RWS_OIDC_ISSUER",
            "RWS_OIDC_TENANT_ID",
            "RWS_OIDC_SCOPES",
            "RWS_OIDC_POST_LOGIN_REDIRECT",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn from_env_google_success_with_default_scopes_and_redirect() {
        let _g = crate::test_env::lock();
        clear_oidc_env();
        std::env::set_var("RWS_OIDC_PROVIDER", "google");
        std::env::set_var("RWS_OIDC_CLIENT_ID", "id1");
        std::env::set_var("RWS_OIDC_CLIENT_SECRET", "secret1");
        std::env::set_var("RWS_OIDC_REDIRECT_URI", "https://app.example.com/cb");

        let config = OidcConfig::from_env().unwrap();
        assert_eq!(config.client_id, "id1");
        assert_eq!(config.client_secret, "secret1");
        assert_eq!(config.redirect_uri, "https://app.example.com/cb");
        assert_eq!(config.provider.issuer, "https://accounts.google.com");
        assert_eq!(config.scopes, vec!["openid", "email", "profile"]);
        assert_eq!(config.post_login_redirect, "/");

        clear_oidc_env();
    }

    #[test]
    fn from_env_microsoft_requires_tenant_id() {
        let _g = crate::test_env::lock();
        clear_oidc_env();
        std::env::set_var("RWS_OIDC_PROVIDER", "microsoft");
        std::env::set_var("RWS_OIDC_CLIENT_ID", "id1");
        std::env::set_var("RWS_OIDC_REDIRECT_URI", "https://app.example.com/cb");
        // RWS_OIDC_TENANT_ID deliberately not set.

        let err = expect_from_env_err();
        assert!(err.0.contains("RWS_OIDC_TENANT_ID"), "expected tenant error, got: {}", err.0);

        clear_oidc_env();
    }

    #[test]
    fn from_env_microsoft_success_with_tenant_id() {
        let _g = crate::test_env::lock();
        clear_oidc_env();
        std::env::set_var("RWS_OIDC_PROVIDER", "microsoft");
        std::env::set_var("RWS_OIDC_CLIENT_ID", "id1");
        std::env::set_var("RWS_OIDC_REDIRECT_URI", "https://app.example.com/cb");
        std::env::set_var("RWS_OIDC_TENANT_ID", "contoso-tenant");

        let config = OidcConfig::from_env().unwrap();
        assert!(config.provider.issuer.contains("contoso-tenant"));

        clear_oidc_env();
    }

    #[test]
    fn from_env_okta_requires_issuer() {
        let _g = crate::test_env::lock();
        clear_oidc_env();
        std::env::set_var("RWS_OIDC_PROVIDER", "okta");
        std::env::set_var("RWS_OIDC_CLIENT_ID", "id1");
        std::env::set_var("RWS_OIDC_REDIRECT_URI", "https://app.example.com/cb");
        // RWS_OIDC_ISSUER deliberately not set.

        let err = expect_from_env_err();
        assert!(err.0.contains("RWS_OIDC_ISSUER"), "expected issuer error, got: {}", err.0);

        clear_oidc_env();
    }

    #[test]
    fn from_env_custom_provider_discovers_via_issuer() {
        let base = start_fake_token_server(|_req| {
            let body = r#"{
                "issuer": "https://idp.example.com",
                "authorization_endpoint": "https://idp.example.com/authorize",
                "token_endpoint": "https://idp.example.com/token",
                "jwks_uri": "https://idp.example.com/jwks"
            }"#;
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
        });

        let _g = crate::test_env::lock();
        clear_oidc_env();
        std::env::set_var("RWS_OIDC_PROVIDER", "custom");
        std::env::set_var("RWS_OIDC_CLIENT_ID", "id1");
        std::env::set_var("RWS_OIDC_REDIRECT_URI", "https://app.example.com/cb");
        std::env::set_var("RWS_OIDC_ISSUER", &base);

        let config = OidcConfig::from_env().unwrap();
        assert_eq!(config.provider.token_endpoint, "https://idp.example.com/token");

        clear_oidc_env();
    }

    #[test]
    fn from_env_missing_client_id_fails() {
        let _g = crate::test_env::lock();
        clear_oidc_env();
        std::env::set_var("RWS_OIDC_PROVIDER", "google");
        std::env::set_var("RWS_OIDC_REDIRECT_URI", "https://app.example.com/cb");
        // RWS_OIDC_CLIENT_ID deliberately not set.

        let err = expect_from_env_err();
        assert!(err.0.contains("RWS_OIDC_CLIENT_ID"), "expected client-id error, got: {}", err.0);

        clear_oidc_env();
    }

    #[test]
    fn from_env_missing_redirect_uri_fails() {
        let _g = crate::test_env::lock();
        clear_oidc_env();
        std::env::set_var("RWS_OIDC_PROVIDER", "google");
        std::env::set_var("RWS_OIDC_CLIENT_ID", "id1");
        // RWS_OIDC_REDIRECT_URI deliberately not set.

        let err = expect_from_env_err();
        assert!(err.0.contains("RWS_OIDC_REDIRECT_URI"), "expected redirect-uri error, got: {}", err.0);

        clear_oidc_env();
    }

    #[test]
    fn from_env_client_secret_defaults_to_empty_when_absent() {
        let _g = crate::test_env::lock();
        clear_oidc_env();
        std::env::set_var("RWS_OIDC_PROVIDER", "google");
        std::env::set_var("RWS_OIDC_CLIENT_ID", "id1");
        std::env::set_var("RWS_OIDC_REDIRECT_URI", "https://app.example.com/cb");
        // RWS_OIDC_CLIENT_SECRET deliberately not set — public/PKCE-only client.

        let config = OidcConfig::from_env().unwrap();
        assert_eq!(config.client_secret, "");

        clear_oidc_env();
    }

    #[test]
    fn from_env_custom_scopes_are_space_split() {
        let _g = crate::test_env::lock();
        clear_oidc_env();
        std::env::set_var("RWS_OIDC_PROVIDER", "google");
        std::env::set_var("RWS_OIDC_CLIENT_ID", "id1");
        std::env::set_var("RWS_OIDC_REDIRECT_URI", "https://app.example.com/cb");
        std::env::set_var("RWS_OIDC_SCOPES", "openid custom_scope another_scope");

        let config = OidcConfig::from_env().unwrap();
        assert_eq!(config.scopes, vec!["openid", "custom_scope", "another_scope"]);

        clear_oidc_env();
    }

    #[test]
    fn from_env_custom_post_login_redirect() {
        let _g = crate::test_env::lock();
        clear_oidc_env();
        std::env::set_var("RWS_OIDC_PROVIDER", "google");
        std::env::set_var("RWS_OIDC_CLIENT_ID", "id1");
        std::env::set_var("RWS_OIDC_REDIRECT_URI", "https://app.example.com/cb");
        std::env::set_var("RWS_OIDC_POST_LOGIN_REDIRECT", "/dashboard");

        let config = OidcConfig::from_env().unwrap();
        assert_eq!(config.post_login_redirect, "/dashboard");

        clear_oidc_env();
    }
}

// ── AuthServer — OAuth 2.0 Authorization Server (Phase 6, `sso-server` feature) ─

#[cfg(feature = "sso-server")]
mod server_tests {
    use super::super::client_store::{ClientStore, GrantType, OAuthClient};
    use super::super::server::{AuthServer, AuthServerConfig};
    use crate::application::Application;
    use crate::core::New;
    use crate::header::Header;
    use crate::http::VERSION;
    use crate::middleware::Middleware;
    use crate::request::{Request, METHOD};
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
    use crate::server::{Address, ConnectionInfo};
    use crate::session::SessionStore;
    use std::sync::Arc;
    use std::time::Duration;

    const SID_COOKIE: &str = "_rws_authz_sid";
    const SECRET: &str = "test-signing-secret";

    fn conn() -> ConnectionInfo {
        ConnectionInfo {
            client: Address { ip: "127.0.0.1".to_string(), port: 0 },
            server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
            request_size: 16000,
            sni_hostname: None,
        }
    }

    fn get(uri: &str) -> Request {
        Request {
            method: METHOD.get.to_string(),
            request_uri: uri.to_string(),
            http_version: VERSION.http_1_1.to_string(),
            headers: vec![],
            body: vec![],
        }
    }

    fn post(uri: &str, body: &str) -> Request {
        Request {
            method: METHOD.post.to_string(),
            request_uri: uri.to_string(),
            http_version: VERSION.http_1_1.to_string(),
            headers: vec![],
            body: body.as_bytes().to_vec(),
        }
    }

    fn with_cookie(mut req: Request, name: &str, value: &str) -> Request {
        req.headers.push(Header { name: "Cookie".to_string(), value: format!("{name}={value}") });
        req
    }

    fn header(response: &Response, name: &str) -> Option<String> {
        response.headers.iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.clone())
    }

    struct OkApp;
    impl Application for OkApp {
        fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
            let mut r = Response::new();
            r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
            r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
            Ok(r)
        }
    }

    fn confidential_client() -> OAuthClient {
        OAuthClient {
            client_id: "backend-service".into(),
            client_secret: Some("backend-secret".into()),
            redirect_uris: vec![],
            grants: vec![GrantType::ClientCredentials],
            scopes: vec!["api:read".into(), "api:write".into()],
        }
    }

    fn spa_client() -> OAuthClient {
        OAuthClient {
            client_id: "spa-frontend".into(),
            client_secret: None,
            redirect_uris: vec!["https://spa.example.com/callback".into()],
            grants: vec![GrantType::AuthorizationCode],
            scopes: vec!["openid".into(), "email".into()],
        }
    }

    fn server_with(clients: ClientStore, sessions: Arc<SessionStore>) -> AuthServer {
        AuthServer::new(AuthServerConfig {
            issuer: "https://auth.example.com".into(),
            signing_secret: SECRET.into(),
            access_token_ttl: Duration::from_secs(3600),
            refresh_token_ttl: Duration::from_secs(86_400),
            clients,
            sessions,
        })
    }

    // ── ClientStore ──────────────────────────────────────────────────────────

    #[test]
    fn client_store_get_returns_registered_client() {
        let store = ClientStore::new().add(confidential_client());
        assert!(store.get("backend-service").is_some());
    }

    #[test]
    fn client_store_get_returns_none_for_unknown_client() {
        let store = ClientStore::new().add(confidential_client());
        assert!(store.get("does-not-exist").is_none());
    }

    // ── client_credentials grant ─────────────────────────────────────────────

    #[test]
    fn client_credentials_success_issues_access_token() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(confidential_client()), sessions);
        let req = post("/oauth/token", "grant_type=client_credentials&client_id=backend-service&client_secret=backend-secret");
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(200, resp.status_code);

        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert!(body.contains("\"token_type\":\"Bearer\""));
        assert!(!body.contains("refresh_token"), "client_credentials should not issue a refresh token");

        let token = extract_json_field(&body, "access_token");
        let claims = crate::auth::verify_jwt(&token, SECRET.as_bytes()).expect("token should verify");
        assert_eq!(claims.sub.as_deref(), Some("backend-service"));
        assert!(claims.raw.contains("api:read"));
    }

    #[test]
    fn client_credentials_unknown_client_returns_401() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new(), sessions);
        let req = post("/oauth/token", "grant_type=client_credentials&client_id=nope&client_secret=x");
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(401, resp.status_code);
    }

    #[test]
    fn client_credentials_wrong_secret_returns_401() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(confidential_client()), sessions);
        let req = post("/oauth/token", "grant_type=client_credentials&client_id=backend-service&client_secret=wrong");
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(401, resp.status_code);
    }

    #[test]
    fn client_credentials_wrong_grant_returns_400_for_authcode_only_client() {
        let sessions = Arc::new(SessionStore::new(3600));
        let mut spa = spa_client();
        spa.client_secret = Some("spa-secret".into()); // give it a secret just for this check
        let server = server_with(ClientStore::new().add(spa), sessions);
        let req = post("/oauth/token", "grant_type=client_credentials&client_id=spa-frontend&client_secret=spa-secret");
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(400, resp.status_code);
    }

    #[test]
    fn client_credentials_requested_scope_outside_allowed_returns_400() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(confidential_client()), sessions);
        let req = post("/oauth/token", "grant_type=client_credentials&client_id=backend-service&client_secret=backend-secret&scope=api:admin");
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(400, resp.status_code);
    }

    #[test]
    fn client_credentials_narrower_requested_scope_is_honored() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(confidential_client()), sessions);
        let req = post("/oauth/token", "grant_type=client_credentials&client_id=backend-service&client_secret=backend-secret&scope=api:read");
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        let token = extract_json_field(&body, "access_token");
        let claims = crate::auth::verify_jwt(&token, SECRET.as_bytes()).unwrap();
        assert!(claims.raw.contains("\"scope\":\"api:read\""));
        assert!(!claims.raw.contains("api:write"));
    }

    #[test]
    fn token_endpoint_unsupported_grant_type_returns_400() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new(), sessions);
        let req = post("/oauth/token", "grant_type=password&username=a&password=b");
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(400, resp.status_code);
    }

    // ── /oauth/authorize ─────────────────────────────────────────────────────

    #[test]
    fn authorize_without_session_redirects_to_login() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(spa_client()), sessions);
        let req = get("/oauth/authorize?response_type=code&client_id=spa-frontend&redirect_uri=https://spa.example.com/callback");
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(302, resp.status_code);
        assert!(header(&resp, "Location").unwrap().starts_with("/login?return_to="));
    }

    #[test]
    fn authorize_with_session_issues_code_and_redirects_to_client() {
        let sessions = Arc::new(SessionStore::new(3600));
        let mut session = sessions.create();
        session.set("user_id", "alice");
        sessions.save(&session);

        let server = server_with(ClientStore::new().add(spa_client()), sessions);
        let req = with_cookie(
            get("/oauth/authorize?response_type=code&client_id=spa-frontend&redirect_uri=https://spa.example.com/callback&state=xyz"),
            SID_COOKIE,
            &session.id,
        );
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(302, resp.status_code);
        let location = header(&resp, "Location").unwrap();
        assert!(location.starts_with("https://spa.example.com/callback?code="));
        assert!(location.contains("&state=xyz"));
    }

    #[test]
    fn authorize_unknown_client_returns_400() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new(), sessions);
        let req = get("/oauth/authorize?response_type=code&client_id=nope&redirect_uri=https://x.example.com/cb");
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(400, resp.status_code);
    }

    #[test]
    fn authorize_unregistered_redirect_uri_returns_400() {
        let sessions = Arc::new(SessionStore::new(3600));
        let mut session = sessions.create();
        session.set("user_id", "alice");
        sessions.save(&session);
        let server = server_with(ClientStore::new().add(spa_client()), sessions);
        let req = with_cookie(
            get("/oauth/authorize?response_type=code&client_id=spa-frontend&redirect_uri=https://evil.example.com/cb"),
            SID_COOKIE,
            &session.id,
        );
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(400, resp.status_code);
    }

    #[test]
    fn authorize_client_without_authcode_grant_returns_400() {
        let sessions = Arc::new(SessionStore::new(3600));
        let mut session = sessions.create();
        session.set("user_id", "alice");
        sessions.save(&session);
        let server = server_with(ClientStore::new().add(confidential_client()), sessions);
        let req = with_cookie(
            get("/oauth/authorize?response_type=code&client_id=backend-service&redirect_uri=https://x.example.com/cb"),
            SID_COOKIE,
            &session.id,
        );
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(400, resp.status_code);
    }

    #[test]
    fn authorize_unsupported_code_challenge_method_returns_400() {
        let sessions = Arc::new(SessionStore::new(3600));
        let mut session = sessions.create();
        session.set("user_id", "alice");
        sessions.save(&session);
        let server = server_with(ClientStore::new().add(spa_client()), sessions);
        let req = with_cookie(
            get("/oauth/authorize?response_type=code&client_id=spa-frontend&redirect_uri=https://spa.example.com/callback&code_challenge=abc&code_challenge_method=plain"),
            SID_COOKIE,
            &session.id,
        );
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(400, resp.status_code);
    }

    // ── authorization_code grant (full round trip) ───────────────────────────

    fn mint_code(server: &AuthServer, sessions: &Arc<SessionStore>, extra_query: &str) -> String {
        let mut session = sessions.create();
        session.set("user_id", "alice");
        sessions.save(&session);
        let req = with_cookie(
            get(&format!(
                "/oauth/authorize?response_type=code&client_id=spa-frontend&redirect_uri=https://spa.example.com/callback{extra_query}"
            )),
            SID_COOKIE,
            &session.id,
        );
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        let location = header(&resp, "Location").unwrap();
        let code_start = location.find("code=").unwrap() + "code=".len();
        let code_end = location[code_start..].find('&').map(|i| code_start + i).unwrap_or(location.len());
        location[code_start..code_end].to_string()
    }

    #[test]
    fn authorization_code_grant_success_round_trip() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(spa_client()), sessions.clone());
        let code = mint_code(&server, &sessions, "");

        let body = format!(
            "grant_type=authorization_code&code={code}&redirect_uri=https://spa.example.com/callback&client_id=spa-frontend"
        );
        let resp = server.handle(&post("/oauth/token", &body), &conn(), &OkApp).unwrap();
        assert_eq!(200, resp.status_code);

        let resp_body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert!(resp_body.contains("refresh_token"));
        assert!(resp_body.contains("id_token"));
        let access_token = extract_json_field(&resp_body, "access_token");
        let claims = crate::auth::verify_jwt(&access_token, SECRET.as_bytes()).unwrap();
        assert_eq!(claims.sub.as_deref(), Some("alice"));
    }

    #[test]
    fn authorization_code_grant_reuse_fails() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(spa_client()), sessions.clone());
        let code = mint_code(&server, &sessions, "");
        let body = format!(
            "grant_type=authorization_code&code={code}&redirect_uri=https://spa.example.com/callback&client_id=spa-frontend"
        );
        let first = server.handle(&post("/oauth/token", &body), &conn(), &OkApp).unwrap();
        assert_eq!(200, first.status_code);
        let second = server.handle(&post("/oauth/token", &body), &conn(), &OkApp).unwrap();
        assert_eq!(400, second.status_code, "a used code must not be exchangeable a second time");
    }

    #[test]
    fn authorization_code_grant_wrong_redirect_uri_fails() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(spa_client()), sessions.clone());
        let code = mint_code(&server, &sessions, "");
        let body = format!(
            "grant_type=authorization_code&code={code}&redirect_uri=https://wrong.example.com/cb&client_id=spa-frontend"
        );
        let resp = server.handle(&post("/oauth/token", &body), &conn(), &OkApp).unwrap();
        assert_eq!(400, resp.status_code);
    }

    #[test]
    fn authorization_code_grant_pkce_verifier_mismatch_fails() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(spa_client()), sessions.clone());
        let code = mint_code(&server, &sessions, "&code_challenge=abc123&code_challenge_method=S256");
        let body = format!(
            "grant_type=authorization_code&code={code}&redirect_uri=https://spa.example.com/callback&client_id=spa-frontend&code_verifier=wrong-verifier"
        );
        let resp = server.handle(&post("/oauth/token", &body), &conn(), &OkApp).unwrap();
        assert_eq!(400, resp.status_code);
    }

    #[test]
    fn authorization_code_grant_pkce_verifier_success() {
        use crate::sso::pkce::PkceVerifier;
        let verifier = PkceVerifier::new();
        let challenge = verifier.challenge();

        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(spa_client()), sessions.clone());
        let code = mint_code(
            &server,
            &sessions,
            &format!("&code_challenge={}&code_challenge_method=S256", challenge.as_str()),
        );
        let body = format!(
            "grant_type=authorization_code&code={code}&redirect_uri=https://spa.example.com/callback&client_id=spa-frontend&code_verifier={}",
            verifier.as_str()
        );
        let resp = server.handle(&post("/oauth/token", &body), &conn(), &OkApp).unwrap();
        assert_eq!(200, resp.status_code);
    }

    // ── refresh_token grant ──────────────────────────────────────────────────

    #[test]
    fn refresh_token_grant_success_issues_new_access_token() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(spa_client()), sessions.clone());
        let code = mint_code(&server, &sessions, "");
        let exchange_body = format!(
            "grant_type=authorization_code&code={code}&redirect_uri=https://spa.example.com/callback&client_id=spa-frontend"
        );
        let first = server.handle(&post("/oauth/token", &exchange_body), &conn(), &OkApp).unwrap();
        let first_body = String::from_utf8(first.content_range_list[0].body.clone()).unwrap();
        let refresh_token = extract_json_field(&first_body, "refresh_token");

        let refresh_body = format!("grant_type=refresh_token&refresh_token={refresh_token}");
        let resp = server.handle(&post("/oauth/token", &refresh_body), &conn(), &OkApp).unwrap();
        assert_eq!(200, resp.status_code);
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert!(!body.contains("refresh_token"), "refresh grant response should not include a new refresh token");
        let access_token = extract_json_field(&body, "access_token");
        let claims = crate::auth::verify_jwt(&access_token, SECRET.as_bytes()).unwrap();
        assert_eq!(claims.sub.as_deref(), Some("alice"));
    }

    #[test]
    fn refresh_token_grant_unknown_token_fails() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new().add(spa_client()), sessions);
        let req = post("/oauth/token", "grant_type=refresh_token&refresh_token=does-not-exist");
        let resp = server.handle(&req, &conn(), &OkApp).unwrap();
        assert_eq!(400, resp.status_code);
    }

    // ── discovery / jwks ─────────────────────────────────────────────────────

    #[test]
    fn discovery_document_has_expected_endpoints() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new(), sessions);
        let resp = server.handle(&get("/.well-known/openid-configuration"), &conn(), &OkApp).unwrap();
        assert_eq!(200, resp.status_code);
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert!(body.contains("\"issuer\":\"https://auth.example.com\""));
        assert!(body.contains("/oauth/authorize"));
        assert!(body.contains("/oauth/token"));
        assert!(body.contains("/.well-known/jwks.json"));
    }

    #[test]
    fn jwks_document_is_an_empty_key_set() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new(), sessions);
        let resp = server.handle(&get("/.well-known/jwks.json"), &conn(), &OkApp).unwrap();
        assert_eq!(200, resp.status_code);
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert_eq!(body, r#"{"keys":[]}"#);
    }

    // ── pass-through ─────────────────────────────────────────────────────────

    #[test]
    fn non_oauth_path_passes_through_to_next_app() {
        let sessions = Arc::new(SessionStore::new(3600));
        let server = server_with(ClientStore::new(), sessions);
        let resp = server.handle(&get("/dashboard"), &conn(), &OkApp).unwrap();
        assert_eq!(200, resp.status_code);
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Extract a top-level string field's value from a small JSON object —
    /// good enough for the flat, hand-built response bodies under test.
    fn extract_json_field(json: &str, key: &str) -> String {
        let needle = format!("\"{key}\":\"");
        let start = json.find(&needle).unwrap_or_else(|| panic!("field {key} not found in {json}")) + needle.len();
        let end = json[start..].find('"').unwrap();
        json[start..start + end].to_string()
    }
}
