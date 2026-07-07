//! Unit tests for the SSO module.
//!
//! All tests operate on pure in-memory logic — no network calls are made,
//! except the `exchange_code` tests and the `jwks_tests`/`discovery_tests`
//! modules below, which use an in-process loopback `TcpListener` as a fake
//! token/JWKS/discovery endpoint (same pattern as `http_client::tests`)
//! rather than a real IdP.

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

    fn jwks_server(jwk_json: &str) -> String {
        jwks_server_sequence(vec![format!(r#"{{"keys":[{jwk_json}]}}"#)])
    }

    fn build_jwt(header: &str, payload: &str, sign: impl FnOnce(&[u8]) -> Vec<u8>) -> String {
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

    fn rsa_jwk_json(kid: &str) -> String {
        use rsa::traits::PublicKeyParts;
        let pub_key = rsa_key().to_public_key();
        let n = base64url_encode(&pub_key.n().to_bytes_be());
        let e = base64url_encode(&pub_key.e().to_bytes_be());
        format!(r#"{{"kty":"RSA","kid":"{kid}","n":"{n}","e":"{e}"}}"#)
    }

    fn sign_rs256(message: &[u8]) -> Vec<u8> {
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
