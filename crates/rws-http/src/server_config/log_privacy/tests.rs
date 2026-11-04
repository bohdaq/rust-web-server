use super::*;

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

fn written(privacy: &LogPrivacy, address: &str) -> String {
    privacy.ip_string(ip(address))
}

#[test]
fn the_default_is_off_and_writes_addresses_as_they_are() {
    let privacy = LogPrivacy::default();
    assert!(privacy.is_off());
    assert_eq!("203.0.113.77", written(&privacy, "203.0.113.77"));
    assert_eq!("2001:db8:1:2::5", written(&privacy, "2001:db8:1:2::5"));
    assert_eq!("/a?token=x", privacy.redacted_target("/a?token=x"));
}

#[test]
fn truncate_keeps_the_ipv4_24_and_the_ipv6_48() {
    let privacy = LogPrivacy { anonymize_ip: AnonymizeIp::Truncate, ..LogPrivacy::default() };
    assert!(!privacy.is_off());
    assert_eq!("203.0.113.0", written(&privacy, "203.0.113.77"));
    assert_eq!("2001:db8:1::", written(&privacy, "2001:db8:1:2:3:4:5:6"));
    assert_eq!("::", written(&privacy, "::2"));
    // An IPv4-mapped address is the IPv4 address it maps.
    assert_eq!("192.0.2.0", written(&privacy, "::ffff:192.0.2.9"));
}

#[test]
fn hash_is_a_keyed_stable_pseudonym() {
    let privacy = LogPrivacy { anonymize_ip: AnonymizeIp::Hash, ip_hash_key: b"k1".as_slice().into(), ..LogPrivacy::default() };
    let a = written(&privacy, "203.0.113.77");
    assert_eq!(16, a.len());
    assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(a, written(&privacy, "203.0.113.77"), "the same client, the same pseudonym");
    assert_eq!(a, written(&privacy, "::ffff:203.0.113.77"), "mapped addresses hash as IPv4");
    assert_ne!(a, written(&privacy, "203.0.113.78"));
    let other_key = LogPrivacy { ip_hash_key: b"k2".as_slice().into(), ..privacy.clone() };
    assert_ne!(a, written(&other_key, "203.0.113.77"), "another key, another pseudonym");
    // The first 8 bytes of HMAC-SHA256("k1", "203.0.113.77").
    let mac = crate::core::sha256::hmac_sha256(b"k1", b"203.0.113.77");
    let hex: String = mac[..8].iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, a);
}

#[test]
fn hash_without_a_key_truncates() {
    let privacy = LogPrivacy { anonymize_ip: AnonymizeIp::Hash, ..LogPrivacy::default() };
    assert_eq!("203.0.113.0", written(&privacy, "203.0.113.77"));
}

#[test]
fn parse_accepts_the_documented_values_only() {
    assert_eq!(Some(AnonymizeIp::Off), AnonymizeIp::parse(""));
    assert_eq!(Some(AnonymizeIp::Off), AnonymizeIp::parse("off"));
    assert_eq!(Some(AnonymizeIp::Truncate), AnonymizeIp::parse(" Truncate "));
    assert_eq!(Some(AnonymizeIp::Hash), AnonymizeIp::parse("hash"));
    assert_eq!(None, AnonymizeIp::parse("mask"));
}

#[test]
fn query_values_of_listed_parameters_are_redacted() {
    let privacy = LogPrivacy { redact_query: vec!["token".into(), "api_key".into()], ..LogPrivacy::default() };
    assert_eq!("/a?token=REDACTED&x=1&API_KEY=REDACTED", privacy.redacted_target("/a?token=s3cr3t&x=1&API_KEY=k"));
    assert_eq!("/a?x=1#token=frag", privacy.redacted_target("/a?x=1#token=frag"), "fragments aren't parameters");
    assert_eq!("/a?token&tokens=1", privacy.redacted_target("/a?token&tokens=1"), "no value, or another name: as is");
    assert_eq!("/no-query", privacy.redacted_target("/no-query"));
    assert_eq!("https://ref.example/p?token=REDACTED", privacy.redacted_target("https://ref.example/p?token=t"));
    assert_eq!("/a?", privacy.redacted_target("/a?"));
    assert_eq!("/a?token=REDACTED&token=REDACTED", privacy.redacted_target("/a?token=1&token=2"));
}

#[test]
fn credential_headers_are_always_redacted_and_the_list_adds_more() {
    let privacy = LogPrivacy::default();
    for name in ["Authorization", "proxy-authorization", "COOKIE", "Set-Cookie"] {
        assert!(privacy.redacts_header(name), "{name}");
    }
    assert!(!privacy.redacts_header("user-agent"));
    let privacy = LogPrivacy { redact_headers: vec!["x-api-key".into()], ..LogPrivacy::default() };
    assert!(privacy.redacts_header("X-Api-Key"));
    assert!(!privacy.is_off());
}

#[test]
fn debug_never_shows_the_key() {
    let privacy = LogPrivacy { anonymize_ip: AnonymizeIp::Hash, ip_hash_key: b"very-secret".as_slice().into(), ..LogPrivacy::default() };
    let shown = format!("{privacy:?}");
    assert!(!shown.contains("very-secret"), "{shown}");
    assert!(shown.contains("(set)"));
}

#[test]
fn from_env_reads_the_settings() {
    let _g = crate::test_env::lock();
    let keys = [RWS_CONFIG_ACCESS_LOG_ANONYMIZE_IP, RWS_CONFIG_ACCESS_LOG_IP_HASH_SECRET, RWS_CONFIG_LOG_REDACT_HEADERS, RWS_CONFIG_LOG_REDACT_QUERY];
    let saved: Vec<Option<String>> = keys.iter().map(|k| std::env::var(k).ok()).collect();
    std::env::set_var(RWS_CONFIG_ACCESS_LOG_ANONYMIZE_IP, "hash");
    std::env::set_var(RWS_CONFIG_ACCESS_LOG_IP_HASH_SECRET, " key ");
    std::env::set_var(RWS_CONFIG_LOG_REDACT_HEADERS, "X-Api-Key, ,x-other");
    std::env::set_var(RWS_CONFIG_LOG_REDACT_QUERY, "token");
    let privacy = LogPrivacy::from_env();
    assert_eq!(AnonymizeIp::Hash, privacy.anonymize_ip);
    assert_eq!(b"key", &*privacy.ip_hash_key);
    assert_eq!(vec!["x-api-key".to_string(), "x-other".to_string()], privacy.redact_headers);
    assert_eq!(vec!["token".to_string()], privacy.redact_query);
    assert!(maybe_active());
    // An unknown mode fails towards privacy.
    std::env::set_var(RWS_CONFIG_ACCESS_LOG_ANONYMIZE_IP, "mask");
    assert_eq!(AnonymizeIp::Truncate, LogPrivacy::from_env().anonymize_ip);
    for k in keys {
        std::env::remove_var(k);
    }
    assert!(LogPrivacy::from_env().is_off());
    assert!(!maybe_active());
    for (k, v) in keys.iter().zip(saved) {
        if let Some(v) = v {
            std::env::set_var(k, v);
        }
    }
    crate::server_config::invalidate();
}
