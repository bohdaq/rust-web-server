//! Unit tests for `CanaryLayer`.

use super::{CanaryLayer, WeightedBackend};

#[test]
fn rotation_len_matches_sum_of_weights() {
    let layer = CanaryLayer::new(vec![
        WeightedBackend::new("a:8080", 3),
        WeightedBackend::new("b:8080", 1),
    ]);
    assert_eq!(4, layer.rotation.len());
}

#[test]
fn zero_weight_skipped() {
    let layer = CanaryLayer::new(vec![
        WeightedBackend::new("a:8080", 2),
        WeightedBackend::new("b:8080", 0),
        WeightedBackend::new("c:8080", 1),
    ]);
    // Only a (×2) and c (×1) should appear
    assert_eq!(3, layer.rotation.len());
    let hosts: Vec<&str> = layer.rotation.iter().map(|(h, _, _)| h.as_str()).collect();
    assert!(!hosts.contains(&"b"), "zero-weight backend should not appear in rotation");
}

#[test]
fn single_backend_rotation_len_equals_weight() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("backend:9000", 5)]);
    assert_eq!(5, layer.rotation.len());
    for (host, port, tls) in &layer.rotation {
        assert_eq!("backend", host);
        assert_eq!(9000, *port);
        assert!(!tls);
    }
}

#[test]
fn empty_backends_produce_empty_rotation() {
    let layer = CanaryLayer::new(vec![]);
    assert_eq!(0, layer.rotation.len());
}

#[test]
fn url_parsing_strips_http_prefix() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("http://myhost:1234", 1)]);
    assert_eq!(1, layer.rotation.len());
    assert_eq!("myhost", layer.rotation[0].0);
    assert_eq!(1234, layer.rotation[0].1);
    assert!(!layer.rotation[0].2);
}

#[test]
fn url_parsing_defaults_to_port_80() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("myhost", 1)]);
    assert_eq!(1, layer.rotation.len());
    assert_eq!("myhost", layer.rotation[0].0);
    assert_eq!(80, layer.rotation[0].1);
    assert!(!layer.rotation[0].2);
}

#[test]
fn weighted_distribution_is_proportional() {
    // Validate the rotation vector directly rather than making real TCP calls.
    let layer = CanaryLayer::new(vec![
        WeightedBackend::new("stable:8080", 3),
        WeightedBackend::new("canary:8080", 1),
    ]);
    let stable_count = layer.rotation.iter().filter(|(h, _, _)| h == "stable").count();
    let canary_count = layer.rotation.iter().filter(|(h, _, _)| h == "canary").count();
    assert_eq!(3, stable_count, "stable backend should appear 3 times");
    assert_eq!(1, canary_count, "canary backend should appear 1 time");
}

// ── TLS backend detection ────────────────────────────────────────────────────

#[test]
fn https_scheme_sets_tls_and_default_port_443() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("https://secure-backend", 1)]);
    assert_eq!(("secure-backend".to_string(), 443, true), layer.rotation[0]);
}

#[test]
fn https_scheme_with_explicit_port_sets_tls() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("https://secure-backend:8443", 1)]);
    assert_eq!(("secure-backend".to_string(), 8443, true), layer.rotation[0]);
}

#[test]
fn h2s_scheme_sets_tls_and_default_port_443() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("h2s://backend", 1)]);
    assert_eq!(("backend".to_string(), 443, true), layer.rotation[0]);
}

#[test]
fn grpcs_scheme_sets_tls_and_default_port_443() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("grpcs://backend", 1)]);
    assert_eq!(("backend".to_string(), 443, true), layer.rotation[0]);
}

#[test]
fn h2_scheme_is_plain_not_tls() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("h2://backend:3000", 1)]);
    assert_eq!(("backend".to_string(), 3000, false), layer.rotation[0]);
}

#[test]
fn grpc_scheme_is_plain_not_tls() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("grpc://backend:3000", 1)]);
    assert_eq!(("backend".to_string(), 3000, false), layer.rotation[0]);
}

#[test]
fn mixed_tls_and_plain_backends_in_same_rotation() {
    let layer = CanaryLayer::new(vec![
        WeightedBackend::new("https://secure:8443", 1),
        WeightedBackend::new("http://plain:8080", 1),
    ]);
    assert_eq!(2, layer.rotation.len());
    assert!(layer.rotation.iter().any(|(h, p, tls)| h == "secure" && *p == 8443 && *tls));
    assert!(layer.rotation.iter().any(|(h, p, tls)| h == "plain" && *p == 8080 && !*tls));
}
