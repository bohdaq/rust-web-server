//! Unit tests for `IngressRule` and `parse_ingress_list`.

use super::{IngressRule, parse_ingress_list};

// ── parse_ingress_list ────────────────────────────────────────────────────────

#[test]
fn parse_empty_list() {
    let rules = parse_ingress_list(r#"{"items":[]}"#);
    assert!(rules.is_empty(), "empty items array should produce no rules");
}

#[test]
fn parse_single_rule() {
    let json = r#"{
        "items": [{
            "metadata": {"namespace": "default"},
            "spec": {
                "rules": [{
                    "host": "example.com",
                    "http": {
                        "paths": [{
                            "path": "/api",
                            "backend": {
                                "service": {
                                    "name": "my-svc",
                                    "port": {"number": 8080}
                                }
                            }
                        }]
                    }
                }]
            }
        }]
    }"#;
    let rules = parse_ingress_list(json);
    assert_eq!(1, rules.len(), "expected one rule, got {:?}", rules);
    let r = &rules[0];
    assert_eq!("example.com", r.host);
    assert_eq!("/api", r.path);
    assert_eq!("my-svc", r.service_name);
    assert_eq!(8080, r.service_port);
    assert_eq!("default", r.namespace);
}

#[test]
fn parse_multiple_paths() {
    let json = r#"{
        "items": [{
            "metadata": {"namespace": "prod"},
            "spec": {
                "rules": [{
                    "host": "myapp.example.com",
                    "http": {
                        "paths": [
                            {"path": "/web","backend":{"service":{"name":"frontend","port":{"number":3000}}}},
                            {"path": "/api","backend":{"service":{"name":"backend","port":{"number":8080}}}}
                        ]
                    }
                }]
            }
        }]
    }"#;
    let rules = parse_ingress_list(json);
    assert_eq!(2, rules.len(), "expected two rules, got {:?}", rules);
    assert_eq!("frontend", rules[0].service_name);
    assert_eq!(3000, rules[0].service_port);
    assert_eq!("backend", rules[1].service_name);
    assert_eq!(8080, rules[1].service_port);
}

// ── IngressRule::matches ──────────────────────────────────────────────────────

fn make_rule(host: &str, path: &str) -> IngressRule {
    IngressRule {
        host: host.to_string(),
        path: path.to_string(),
        service_name: "svc".to_string(),
        service_port: 80,
        namespace: "default".to_string(),
    }
}

#[test]
fn rule_matches_host_and_path() {
    let rule = make_rule("example.com", "/api");
    assert!(rule.matches("example.com", "/api/foo"), "should match prefix /api");
    assert!(rule.matches("example.com", "/api"), "exact path should match");
}

#[test]
fn rule_no_match_wrong_host() {
    let rule = make_rule("example.com", "/api");
    assert!(!rule.matches("other.com", "/api/foo"), "wrong host should not match");
}

#[test]
fn rule_no_match_wrong_path() {
    let rule = make_rule("example.com", "/api");
    assert!(!rule.matches("example.com", "/other"), "wrong path prefix should not match");
}

#[test]
fn rule_empty_host_matches_any_host() {
    let rule = make_rule("", "/api");
    assert!(rule.matches("anything.example.com", "/api/v1"), "empty host should match any");
    assert!(rule.matches("", "/api"), "empty host should match empty host too");
}

#[test]
fn rule_root_path_matches_everything() {
    let rule = make_rule("", "/");
    assert!(rule.matches("host.com", "/some/path"), "/ should match everything");
    assert!(rule.matches("host.com", "/"), "/ should match /");
}

#[test]
fn rule_host_matching_is_case_insensitive() {
    let rule = make_rule("Example.COM", "/");
    assert!(rule.matches("example.com", "/"), "host match should be case-insensitive");
    assert!(rule.matches("EXAMPLE.COM", "/"), "host match should be case-insensitive (upper)");
}

// ── IngressRule::upstream_addr ────────────────────────────────────────────────

#[test]
fn upstream_addr_format() {
    let rule = IngressRule {
        host: "example.com".to_string(),
        path: "/api".to_string(),
        service_name: "my-svc".to_string(),
        service_port: 8080,
        namespace: "production".to_string(),
    };
    assert_eq!(
        "my-svc.production.svc.cluster.local:8080",
        rule.upstream_addr()
    );
}

#[test]
fn upstream_addr_default_namespace() {
    let rule = IngressRule {
        host: String::new(),
        path: "/".to_string(),
        service_name: "api".to_string(),
        service_port: 3000,
        namespace: "default".to_string(),
    };
    assert_eq!("api.default.svc.cluster.local:3000", rule.upstream_addr());
}
