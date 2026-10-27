use super::crypto::{self, AccountKey};


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

// ── certificate info and renewal (#1) ───────────────────────────────────────

#[test]
fn read_cert_info_missing_file_returns_none() {
    assert!(crypto::read_cert_info("/nonexistent/cert.pem").is_none());
}

/// A leaf signed by a CA whose key identifier and the leaf's serial are the
/// ones in RFC 9773 §4.1's example.
fn rfc_9773_example_leaf() -> Vec<u8> {
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params.key_identifier_method = rcgen::KeyIdMethod::PreSpecified(vec![
        0x69, 0x88, 0x5B, 0x6B, 0x87, 0x46, 0x40, 0x41, 0xE1, 0xB3,
        0x7B, 0x84, 0x7B, 0xA0, 0xAE, 0x2C, 0xDE, 0x01, 0xC8, 0xD4,
    ]);
    let ca_key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    let issuer = rcgen::Issuer::new(ca_params, ca_key);

    let mut params = rcgen::CertificateParams::new(vec!["example.com".to_string()]).unwrap();
    params.serial_number = Some(rcgen::SerialNumber::from_slice(&[0x00, 0x87, 0x65, 0x43, 0x21]));
    params.not_before = rcgen::date_time_ymd(2026, 1, 1);
    params.not_after = rcgen::date_time_ymd(2026, 4, 1);
    params.use_authority_key_identifier_extension = true;
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    params.signed_by(&key, &issuer).unwrap().der().to_vec()
}

#[test]
fn cert_id_matches_the_rfc_9773_example() {
    let info = crypto::cert_info_from_der(&rfc_9773_example_leaf()).unwrap();
    assert_eq!(Some("aYhba4dGQEHhs3uEe6CuLN4ByNQ.AIdlQyE".to_string()), info.cert_id);
    assert_eq!(super::parse_rfc3339("2026-01-01T00:00:00Z"), Some(info.not_before));
    assert_eq!(super::parse_rfc3339("2026-04-01T00:00:00Z"), Some(info.not_after));
}

#[test]
fn a_certificate_without_aki_has_no_cert_id() {
    let mut params = rcgen::CertificateParams::new(vec!["example.com".to_string()]).unwrap();
    params.not_before = rcgen::date_time_ymd(2026, 1, 1);
    params.not_after = rcgen::date_time_ymd(2026, 1, 7);
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    let der = params.self_signed(&key).unwrap().der().to_vec();
    let info = crypto::cert_info_from_der(&der).unwrap();
    assert_eq!(None, info.cert_id);
    assert_eq!(6 * 86_400, info.not_after - info.not_before);
}

#[test]
fn malformed_certificates_are_none_not_a_panic() {
    let good = rfc_9773_example_leaf();
    for len in 0..good.len() {
        let _ = crypto::cert_info_from_der(&good[..len]);
    }
    let mut flipped = good.clone();
    for i in 0..flipped.len() {
        flipped[i] ^= 0xff;
        let _ = crypto::cert_info_from_der(&flipped);
        flipped[i] ^= 0xff;
    }
    // Length-of-length bytes that would overflow.
    assert!(crypto::cert_info_from_der(&[0x30, 0x89, 1, 1, 1, 1, 1, 1, 1, 1, 1]).is_none());
    assert!(crypto::cert_info_from_der(&[0x30, 0x84, 0xff, 0xff, 0xff, 0xff]).is_none());
}

fn info(not_before: i64, days: i64) -> crypto::CertInfo {
    crypto::CertInfo { not_before, not_after: not_before + days * 86_400, cert_id: Some("a.b".to_string()) }
}

#[test]
fn renewal_falls_back_to_two_thirds_of_the_lifetime() {
    use super::renewal_time;
    assert_eq!(60 * 86_400, renewal_time(&info(0, 90), None, None));
    assert_eq!(30 * 86_400, renewal_time(&info(0, 45), None, None));
    assert_eq!(4 * 86_400, renewal_time(&info(0, 6), None, None));
    // An explicit renew_before_days replaces the fallback.
    assert_eq!(60 * 86_400, renewal_time(&info(0, 90), None, Some(30)));
}

#[test]
fn the_ca_window_can_only_bring_renewal_earlier() {
    use super::renewal_time;
    let cert = info(0, 90);
    // A window before the fallback: a point inside it.
    let (start, end) = (10 * 86_400, 12 * 86_400);
    let at = renewal_time(&cert, Some((start, end)), None);
    assert!((start..=end).contains(&at), "{at}");
    // The same point every time for the same certificate.
    assert_eq!(at, renewal_time(&cert, Some((start, end)), None));
    // A window after the fallback doesn't push renewal later.
    assert_eq!(60 * 86_400, renewal_time(&cert, Some((70 * 86_400, 80 * 86_400)), None));
    // Nor does renew_before_days suppress an open window.
    assert!(renewal_time(&cert, Some((start, end)), Some(1)) <= end);
    // A window that has passed means now (or earlier).
    assert!(renewal_time(&cert, Some((-200, -100)), None) <= -100);
}

#[test]
fn suggested_windows_and_rfc_3339_times_parse() {
    use super::{parse_rfc3339, parse_suggested_window};
    assert_eq!(Some(0), parse_rfc3339("1970-01-01T00:00:00Z"));
    assert_eq!(Some(784_111_777), parse_rfc3339("1994-11-06T08:49:37Z"));
    assert_eq!(Some(784_111_777), parse_rfc3339("1994-11-06T08:49:37.123456Z"));
    assert_eq!(Some(784_111_777), parse_rfc3339("1994-11-06T10:49:37+02:00"));
    assert_eq!(Some(784_111_777), parse_rfc3339("1994-11-06T03:49:37-05:00"));
    for bad in ["", "1994-11-06", "1994-11-06T08:49:37", "1994-13-06T08:49:37Z", "1994-11-06T08:49:37.Z", "1994-11-06T08:49:37+0200", "199x-11-06T08:49:37Z"] {
        assert_eq!(None, parse_rfc3339(bad), "{bad:?}");
    }
    let body = r#"{"suggestedWindow":{"start":"2026-03-01T00:00:00Z","end":"2026-03-03T00:00:00Z"},"explanationURL":"https://x"}"#;
    let (start, end) = parse_suggested_window(body).unwrap();
    assert_eq!(2 * 86_400, end - start);
    assert_eq!(None, parse_suggested_window(r#"{"suggestedWindow":{"start":"2026-03-03T00:00:00Z","end":"2026-03-01T00:00:00Z"}}"#));
    assert_eq!(None, parse_suggested_window("not json"));
}

#[test]
fn directory_profiles_and_new_order_payload() {
    let directory = r#"{"newNonce":"n","meta":{"profiles":{"classic":"The usual","shortlived":{"description":"6 days, \"short\""},"tlsserver":"x"},"website":"w"}}"#;
    assert_eq!(vec!["classic", "shortlived", "tlsserver"], super::json_object_keys(directory, "profiles"));
    assert!(super::json_object_keys(r#"{"meta":{}}"#, "profiles").is_empty());
    assert!(super::json_object_keys("{}", "profiles").is_empty());

    let domains = vec!["a.example".to_string(), "b.example".to_string()];
    assert_eq!(r#"{"identifiers":[{"type":"dns","value":"a.example"},{"type":"dns","value":"b.example"}]}"#,
        super::new_order_payload(&domains, None, None));
    assert_eq!(r#"{"identifiers":[{"type":"dns","value":"a.example"},{"type":"dns","value":"b.example"}],"replaces":"x.y","profile":"shortlived"}"#,
        super::new_order_payload(&domains, Some("x.y"), Some("shortlived")));
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

#[test]
fn dns_persist_challenges_have_issuers_and_no_token() {
    let authz = r#"{"status":"pending","identifier":{"type":"dns","value":"example.com"},"challenges":[
        {"type":"http-01","url":"https://ca/chall/1","token":"tok","status":"pending"},
        {"type":"dns-persist-01","url":"https://ca/chall/2","status":"pending","accounturi":"https://ca/acct/7","issuer-domain-names":["ca.example","ca2.example"]}
    ],"wildcard":false}"#;
    let challenges = super::parse_challenges(authz);
    assert_eq!(2, challenges.len());
    assert_eq!("tok", challenges[0].token);
    assert!(challenges[0].issuer_domain_names.is_empty());
    assert_eq!("dns-persist-01", challenges[1].challenge_type);
    assert_eq!("", challenges[1].token);
    assert_eq!(vec!["ca.example", "ca2.example"], challenges[1].issuer_domain_names);
}

#[test]
fn json_objects_stay_inside_their_array() {
    let json = r#"{"result":[],"other":[{"id":"x"}]}"#;
    assert!(super::json_objects(json, "result").is_empty());
    let json = r#"{"result": [ {"id":"a","meta":{"n":1}} , {"id":"b"} ], "x":{"id":"c"}}"#;
    let ids: Vec<_> = super::json_objects(json, "result").iter().filter_map(|o| super::json_str(o, "id")).collect();
    assert_eq!(vec!["a", "b"], ids);
}

#[test]
fn dns_config_for_the_api_providers() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_ACME_DNS_PROVIDER", "cloudflare");
    std::env::remove_var("RWS_CONFIG_ACME_DNS_API_TOKEN");
    assert!(super::DnsConfig::from_env().unwrap_err().contains("RWS_CONFIG_ACME_DNS_API_TOKEN"));
    std::env::set_var("RWS_CONFIG_ACME_DNS_API_TOKEN", "secret-token");
    let cloudflare = super::DnsConfig::from_env();
    std::env::set_var("RWS_CONFIG_ACME_DNS_PROVIDER", "route53");
    std::env::remove_var("RWS_CONFIG_ACME_DNS_API_TOKEN");
    let route53 = super::DnsConfig::from_env();
    std::env::remove_var("RWS_CONFIG_ACME_DNS_PROVIDER");
    let cloudflare = cloudflare.unwrap();
    assert!(matches!(&cloudflare.provider, super::DnsProvider::Cloudflare { api, zone_id: None, .. } if api == super::dns_api::CLOUDFLARE_API));
    assert!(!format!("{cloudflare:?}").contains("secret-token"), "the token is never printed");
    // Without AWS credentials in the environment (CI has none).
    if std::env::var("AWS_ACCESS_KEY_ID").is_err() {
        assert!(route53.unwrap_err().contains("AWS_ACCESS_KEY_ID"));
    }
}

// ── AcmeConfig::from_env ──────────────────────────────────────────────────────

#[test]
fn acme_config_from_env_returns_none_without_domains() {
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_ACME_DOMAINS");
    assert!(super::AcmeConfig::from_env().is_none());
}

#[test]
fn acme_config_from_env_parses_domains() {
    let _g = crate::test_env::lock();
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
    let _g = crate::test_env::lock();
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

// ── generate_csr ──────────────────────────────────────────────────────────────

#[test]
fn generate_csr_lists_every_domain_and_returns_a_usable_key() {
    use rustls::pki_types::pem::PemObject;
    let domains = vec!["example.com".to_string(), "www.example.com".to_string()];
    let (key, csr) = super::generate_csr(&domains).unwrap();

    assert_eq!(0x30, csr[0], "CSR must be a DER SEQUENCE");
    for domain in &domains {
        assert!(
            csr.windows(domain.len()).any(|w| w == domain.as_bytes()),
            "CSR missing SAN {domain}"
        );
    }

    // The key is written to disk as PEM and later loaded by crate::tls.
    let der = rustls::pki_types::PrivateKeyDer::from_pem_slice(key.serialize_pem().as_bytes()).unwrap();
    assert!(rustls::crypto::aws_lc_rs::sign::any_supported_type(&der).is_ok());
}

#[test]
fn generate_csr_makes_a_fresh_key_each_time() {
    let domains = vec!["example.com".to_string()];
    let (a, _) = super::generate_csr(&domains).unwrap();
    let (b, _) = super::generate_csr(&domains).unwrap();
    assert_ne!(a.serialize_pem(), b.serialize_pem());
}

#[test]
fn generate_csr_rejects_non_ascii_domain() {
    assert!(super::generate_csr(&["bücher.example".to_string()]).is_err());
}

// ── from_env: empty values mean "not configured" ──────────────────────────────

const ACME_TEST_VARS: &[&str] = &[
    "RWS_CONFIG_ACME_DOMAINS", "RWS_CONFIG_ACME_EMAIL", "RWS_CONFIG_ACME_STAGING",
    "RWS_CONFIG_ACME_DIRECTORY", "RWS_CONFIG_ACME_CERT_PATH", "RWS_CONFIG_ACME_KEY_PATH",
    "RWS_CONFIG_ACME_CHALLENGE_PORT", "RWS_CONFIG_TLS_CERT_FILE", "RWS_CONFIG_TLS_KEY_FILE",
    "RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS",
];

/// Startup sets every unset RWS_CONFIG_* variable to its default, which is
/// an empty string for most of these — simulate that.
fn reset_acme_env_to_startup_defaults() {
    for var in ACME_TEST_VARS {
        std::env::set_var(var, "");
    }
}

#[test]
fn from_env_with_only_domains_set_uses_the_state_directory() {
    let _g = crate::test_env::lock();
    reset_acme_env_to_startup_defaults();
    std::env::set_var("RWS_CONFIG_ACME_DOMAINS", "example.com");
    let cfg = super::AcmeConfig::from_env().unwrap();
    reset_acme_env_to_startup_defaults();
    // Not the working directory — the web root (#45).
    assert_eq!(".rws/acme/cert.pem", cfg.cert_path);
    assert_eq!(".rws/acme/key.pem", cfg.key_path);
    assert_eq!(".rws/acme/account.key", cfg.account_key_path);
    assert_eq!(super::AcmeConfig::LETSENCRYPT, cfg.directory_url);
    assert_eq!(80, cfg.challenge_port);
}

#[test]
fn from_env_cert_path_prefers_acme_path_then_tls_path() {
    let _g = crate::test_env::lock();
    reset_acme_env_to_startup_defaults();
    std::env::set_var("RWS_CONFIG_ACME_DOMAINS", "example.com");
    std::env::set_var("RWS_CONFIG_TLS_CERT_FILE", "/etc/tls/cert.pem");
    std::env::set_var("RWS_CONFIG_TLS_KEY_FILE", "/etc/tls/key.pem");
    std::env::set_var("RWS_CONFIG_ACME_KEY_PATH", "/var/acme/key.pem");
    let cfg = super::AcmeConfig::from_env().unwrap();
    reset_acme_env_to_startup_defaults();
    assert_eq!("/etc/tls/cert.pem", cfg.cert_path);
    assert_eq!("/var/acme/key.pem", cfg.key_path);
}

#[test]
fn install_as_tls_certificate_fills_only_empty_tls_paths() {
    let _g = crate::test_env::lock();
    reset_acme_env_to_startup_defaults();
    std::env::set_var("RWS_CONFIG_ACME_DOMAINS", "example.com");
    std::env::set_var("RWS_CONFIG_TLS_KEY_FILE", "/explicit/key.pem");
    let cfg = super::AcmeConfig::from_env().unwrap();
    cfg.install_as_tls_certificate();
    let cert = std::env::var("RWS_CONFIG_TLS_CERT_FILE").unwrap();
    let key = std::env::var("RWS_CONFIG_TLS_KEY_FILE").unwrap();
    reset_acme_env_to_startup_defaults();
    assert_eq!(".rws/acme/cert.pem", cert);
    assert_eq!("/explicit/key.pem", key);
}

#[test]
fn renew_check_interval_defaults_to_twelve_hours_and_ignores_zero() {
    let _g = crate::test_env::lock();
    reset_acme_env_to_startup_defaults();
    assert_eq!(12 * 3600, super::renew_check_interval_secs());
    std::env::set_var("RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS", "0");
    assert_eq!(12 * 3600, super::renew_check_interval_secs());
    std::env::set_var("RWS_CONFIG_ACME_RENEW_CHECK_INTERVAL_SECS", "20");
    assert_eq!(20, super::renew_check_interval_secs());
    reset_acme_env_to_startup_defaults();
}

// ── HTTP-01 challenge store ───────────────────────────────────────────────────

#[test]
fn challenge_http_response_serves_only_pending_tokens() {
    super::challenges().lock().unwrap().insert("tok-123".to_string(), "tok-123.thumb".to_string());
    let hit = super::challenge_http_response("/.well-known/acme-challenge/tok-123").unwrap();
    let miss = super::challenge_http_response("/.well-known/acme-challenge/other");
    let not_challenge = super::challenge_http_response("/tok-123");
    super::challenges().lock().unwrap().remove("tok-123");
    let after_removal = super::challenge_http_response("/.well-known/acme-challenge/tok-123");

    assert!(hit.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(hit.contains("Content-Length: 13\r\n"));
    assert!(hit.ends_with("\r\n\r\ntok-123.thumb"));
    assert!(miss.is_none());
    assert!(not_challenge.is_none());
    assert!(after_removal.is_none());
}

// ── json_str ──────────────────────────────────────────────────────────────────

#[test]
fn json_str_unescapes_quotes_in_error_details() {
    let body = r#"{"type":"urn:ietf:params:acme:error:connection","detail":"Get \"http://a/b\": refused","status":400}"#;
    assert_eq!(Some(r#"Get "http://a/b": refused"#.to_string()), super::json_str(body, "detail"));
    assert_eq!(None, super::json_str(r#"{"detail":"unterminated"#, "detail"));
}

// ── ACME HTTP client ──────────────────────────────────────────────────────────

#[test]
fn host_header_includes_non_default_port() {
    assert_eq!("acme.example", super::client::host_header("acme.example", 443));
    assert_eq!("localhost:14000", super::client::host_header("localhost", 14000));
}

#[test]
fn user_agent_names_rws_and_version() {
    assert!(super::client::USER_AGENT.starts_with(&format!("rws-acme/{}", env!("CARGO_PKG_VERSION"))));
}

#[test]
fn add_extra_roots_trusts_a_private_ca() {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::new(vec!["private-ca.test".to_string()]).unwrap();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca = params.self_signed(&key).unwrap();
    let path = crate::test_env::TempPath::new("acme_ca.pem");
    std::fs::write(&*path, ca.pem()).unwrap();

    let mut store = tokio_rustls::rustls::RootCertStore::empty();
    super::client::add_extra_roots(&mut store, path.to_str().unwrap()).unwrap();
    assert_eq!(1, store.len());
}

#[test]
fn add_extra_roots_rejects_missing_or_empty_files() {
    let mut store = tokio_rustls::rustls::RootCertStore::empty();
    assert!(super::client::add_extra_roots(&mut store, "/nonexistent/ca.pem").is_err());
    let path = crate::test_env::TempPath::new("acme_empty_ca.pem");
    std::fs::write(&*path, "not a certificate\n").unwrap();
    let err = super::client::add_extra_roots(&mut store, path.to_str().unwrap()).unwrap_err();
    assert!(err.contains("no certificates found"), "{err}");
}


// ── #45: private keys out of the web root ────────────────────────────────────

#[cfg(unix)]
#[test]
fn private_keys_are_written_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = crate::test_env::TempPath::new("acme_state");
    let key = format!("{}/nested/key.pem", dir.as_str());
    super::write_private(&key, b"secret").unwrap();
    assert_eq!(0o600, std::fs::metadata(&key).unwrap().permissions().mode() & 0o777);
    assert_eq!(0o700, std::fs::metadata(format!("{}/nested", dir.as_str())).unwrap().permissions().mode() & 0o777);
    // An existing world-readable key is tightened on rewrite.
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
    super::write_private(&key, b"secret2").unwrap();
    assert_eq!(0o600, std::fs::metadata(&key).unwrap().permissions().mode() & 0o777);
    assert_eq!(b"secret2", &std::fs::read(&key).unwrap()[..]);
}

#[cfg(unix)]
#[test]
fn legacy_files_move_out_of_the_web_root() {
    use std::os::unix::fs::PermissionsExt;
    let dir = crate::test_env::TempPath::new("acme_legacy");
    std::fs::create_dir_all(&*dir).unwrap();
    let legacy = format!("{}/key.pem", dir.as_str());
    std::fs::write(&legacy, b"old key").unwrap();
    std::fs::set_permissions(&legacy, std::fs::Permissions::from_mode(0o644)).unwrap();
    let current = format!("{}/.rws/acme/key.pem", dir.as_str());
    super::move_private(&legacy, &current).unwrap();
    assert!(!std::path::Path::new(&legacy).exists());
    assert_eq!(b"old key", &std::fs::read(&current).unwrap()[..]);
    assert_eq!(0o600, std::fs::metadata(&current).unwrap().permissions().mode() & 0o777);
}
