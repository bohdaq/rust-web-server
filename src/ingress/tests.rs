//! Unit tests for `IngressRule` and `parse_ingress_list`.

use super::{IngressRule, PathType, parse_ingress_list};

// ── parse_ingress_list ────────────────────────────────────────────────────────

#[test]
fn parse_empty_list() {
    let rules = parse_ingress_list(r#"{"items":[]}"#, None);
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
    let rules = parse_ingress_list(json, None);
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
    let rules = parse_ingress_list(json, None);
    assert_eq!(2, rules.len(), "expected two rules, got {:?}", rules);
    assert_eq!("frontend", rules[0].service_name);
    assert_eq!(3000, rules[0].service_port);
    assert_eq!("backend", rules[1].service_name);
    assert_eq!(8080, rules[1].service_port);
    assert_eq!("prod", rules[0].namespace, "namespace should be read from metadata, not defaulted");
    assert_eq!("prod", rules[1].namespace);
}

#[test]
fn parse_namespace_from_realistic_item_shape_with_metadata_before_spec() {
    // Regression test: a real Kubernetes API response always puts
    // `metadata` (and the `namespace` field within it) *before* `spec` in
    // an item's JSON encoding. An earlier version of this parser searched
    // for `namespace` *after* the `spec` marker it had just split on,
    // which never actually contains it — so every real API response
    // silently fell back to the "default" placeholder regardless of the
    // Ingress's real namespace, unless that namespace genuinely was
    // "default" (indistinguishable from the bug by coincidence).
    let json = r#"{
        "items": [{
            "metadata": {"name": "my-ingress", "namespace": "billing"},
            "spec": {
                "rules": [{
                    "host": "billing.example.com",
                    "http": {
                        "paths": [{"path": "/", "backend": {"service": {"name": "billing-svc", "port": {"number": 8080}}}}]
                    }
                }]
            }
        }]
    }"#;
    let rules = parse_ingress_list(json, None);
    assert_eq!(1, rules.len());
    assert_eq!("billing", rules[0].namespace, "namespace must be read correctly, not defaulted to 'default'");
}

#[test]
fn parse_multiple_items_each_get_their_own_namespace() {
    let json = r#"{
        "items": [
            {"metadata": {"namespace": "team-a"}, "spec": {"rules": [{"host":"a.example.com","http":{"paths":[{"path":"/","backend":{"service":{"name":"svc-a","port":{"number":80}}}}]}}]}},
            {"metadata": {"namespace": "team-b"}, "spec": {"rules": [{"host":"b.example.com","http":{"paths":[{"path":"/","backend":{"service":{"name":"svc-b","port":{"number":80}}}}]}}]}}
        ]
    }"#;
    let rules = parse_ingress_list(json, None);
    assert_eq!(2, rules.len());
    assert_eq!("team-a", rules[0].namespace);
    assert_eq!("svc-a", rules[0].service_name);
    assert_eq!("team-b", rules[1].namespace);
    assert_eq!("svc-b", rules[1].service_name);
}

#[test]
fn parse_path_type_exact() {
    let json = r#"{
        "items": [{
            "metadata": {"namespace": "default"},
            "spec": {
                "rules": [{
                    "host": "example.com",
                    "http": {"paths": [{"path": "/exact", "pathType": "Exact", "backend": {"service": {"name": "svc", "port": {"number": 80}}}}]}
                }]
            }
        }]
    }"#;
    let rules = parse_ingress_list(json, None);
    assert_eq!(1, rules.len());
    assert_eq!(PathType::Exact, rules[0].path_type);
}

#[test]
fn parse_path_type_defaults_to_prefix_when_absent() {
    let json = r#"{
        "items": [{
            "metadata": {"namespace": "default"},
            "spec": {
                "rules": [{
                    "host": "example.com",
                    "http": {"paths": [{"path": "/", "backend": {"service": {"name": "svc", "port": {"number": 80}}}}]}
                }]
            }
        }]
    }"#;
    let rules = parse_ingress_list(json, None);
    assert_eq!(1, rules.len());
    assert_eq!(PathType::Prefix, rules[0].path_type);
}

// ── ingressClassName filtering ──────────────────────────────────────────────

fn two_class_fixture() -> String {
    r#"{
        "items": [
            {"metadata": {"namespace": "default"}, "spec": {"ingressClassName": "rws", "rules": [{"host":"rws.example.com","http":{"paths":[{"path":"/","backend":{"service":{"name":"rws-svc","port":{"number":80}}}}]}}]}},
            {"metadata": {"namespace": "default"}, "spec": {"ingressClassName": "nginx", "rules": [{"host":"nginx.example.com","http":{"paths":[{"path":"/","backend":{"service":{"name":"nginx-svc","port":{"number":80}}}}]}}]}}
        ]
    }"#.to_string()
}

#[test]
fn ingress_class_filter_none_accepts_every_class() {
    let rules = parse_ingress_list(&two_class_fixture(), None);
    assert_eq!(2, rules.len(), "no filter should accept both classes");
}

#[test]
fn ingress_class_filter_selects_matching_class_only() {
    let rules = parse_ingress_list(&two_class_fixture(), Some("rws"));
    assert_eq!(1, rules.len());
    assert_eq!("rws-svc", rules[0].service_name);
}

#[test]
fn ingress_class_filter_excludes_items_with_no_class_name() {
    let json = r#"{
        "items": [{
            "metadata": {"namespace": "default"},
            "spec": {"rules": [{"host":"h","http":{"paths":[{"path":"/","backend":{"service":{"name":"svc","port":{"number":80}}}}]}}]}
        }]
    }"#;
    let rules = parse_ingress_list(json, Some("rws"));
    assert!(rules.is_empty(), "an Ingress with no ingressClassName should never match a Some(..) filter");
}

// ── IngressRule::matches ──────────────────────────────────────────────────────

fn make_rule(host: &str, path: &str) -> IngressRule {
    IngressRule {
        host: host.to_string(),
        path: path.to_string(),
        path_type: PathType::Prefix,
        service_name: "svc".to_string(),
        service_port: 80,
        namespace: "default".to_string(),
    }
}

fn make_rule_typed(host: &str, path: &str, path_type: PathType) -> IngressRule {
    IngressRule { path_type, ..make_rule(host, path) }
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
        path_type: PathType::Prefix,
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
        path_type: PathType::Prefix,
        service_name: "api".to_string(),
        service_port: 3000,
        namespace: "default".to_string(),
    };
    assert_eq!("api.default.svc.cluster.local:3000", rule.upstream_addr());
}

// ── pathType semantics ────────────────────────────────────────────────────────

#[test]
fn prefix_match_respects_path_element_boundaries() {
    let rule = make_rule_typed("", "/foo", PathType::Prefix);
    assert!(rule.matches("h", "/foo"), "exact segment should match");
    assert!(rule.matches("h", "/foo/"), "trailing slash should match");
    assert!(rule.matches("h", "/foo/bar"), "sub-path should match");
    assert!(!rule.matches("h", "/foobar"), "/foobar must NOT match prefix /foo (element-wise, not raw byte prefix)");
}

#[test]
fn exact_match_requires_full_equality() {
    let rule = make_rule_typed("", "/foo", PathType::Exact);
    assert!(rule.matches("h", "/foo"), "identical path should match");
    assert!(!rule.matches("h", "/foo/"), "trailing slash must NOT match Exact");
    assert!(!rule.matches("h", "/foo/bar"), "sub-path must NOT match Exact");
    assert!(!rule.matches("h", "/foobar"), "unrelated longer path must NOT match Exact");
}

#[test]
fn exact_match_ignores_query_string() {
    let rule = make_rule_typed("", "/foo", PathType::Exact);
    assert!(rule.matches("h", "/foo?x=1"), "query string should be ignored for Exact matching");
}

#[test]
fn prefix_match_ignores_query_string() {
    let rule = make_rule_typed("", "/foo", PathType::Prefix);
    assert!(rule.matches("h", "/foo/bar?x=1"), "query string should be ignored for Prefix matching");
}

#[test]
fn implementation_specific_falls_back_to_prefix_semantics() {
    let rule = make_rule_typed("", "/foo", PathType::ImplementationSpecific);
    assert!(rule.matches("h", "/foo/bar"));
    assert!(!rule.matches("h", "/foobar"));
}

// ── watch stream chunked-line reader ────────────────────────────────────────

mod watch_tests {
    use super::super::watch::read_chunked_lines;
    use std::io::Cursor;

    /// Wrap `body` (already-chunk-encoded) with a minimal 200 response
    /// header, matching what a real `Transfer-Encoding: chunked` watch
    /// response looks like on the wire.
    fn chunked_response(body: &str) -> Vec<u8> {
        let mut out = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
        out.extend_from_slice(body.as_bytes());
        out
    }

    fn chunk(data: &str) -> String {
        format!("{:x}\r\n{data}\r\n", data.len())
    }

    #[test]
    fn reads_multiple_lines_across_multiple_chunks() {
        let body = format!("{}{}{}", chunk("{\"type\":\"ADDED\"}\n"), chunk("{\"type\":\"MODIFIED\"}\n"), "0\r\n\r\n");
        let resp = chunked_response(&body);
        let mut seen = Vec::new();
        read_chunked_lines(Cursor::new(resp), |line| seen.push(line.to_string())).unwrap();
        assert_eq!(vec!["{\"type\":\"ADDED\"}", "{\"type\":\"MODIFIED\"}"], seen);
    }

    #[test]
    fn a_single_line_split_across_two_chunks_is_reassembled() {
        let body = format!("{}{}{}", chunk("{\"type\":\"ADD"), chunk("ED\"}\n"), "0\r\n\r\n");
        let resp = chunked_response(&body);
        let mut seen = Vec::new();
        read_chunked_lines(Cursor::new(resp), |line| seen.push(line.to_string())).unwrap();
        assert_eq!(vec!["{\"type\":\"ADDED\"}"], seen);
    }

    #[test]
    fn non_2xx_status_is_an_error() {
        let resp = b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n".to_vec();
        let result = read_chunked_lines(Cursor::new(resp), |_| {});
        assert!(result.is_err());
    }

    #[test]
    fn empty_lines_are_skipped() {
        let body = format!("{}{}", chunk("\n{\"type\":\"ADDED\"}\n\n"), "0\r\n\r\n");
        let resp = chunked_response(&body);
        let mut seen = Vec::new();
        read_chunked_lines(Cursor::new(resp), |line| seen.push(line.to_string())).unwrap();
        assert_eq!(vec!["{\"type\":\"ADDED\"}"], seen);
    }

    #[test]
    fn clean_eof_before_any_data_is_not_an_error() {
        // A connection that closes right after headers with no chunks at
        // all (e.g. the API server closing an idle watch) should be a
        // normal "stream ended, go reconnect" outcome, not a hard error.
        let resp = chunked_response("");
        let mut seen: Vec<String> = Vec::new();
        let result = read_chunked_lines(Cursor::new(resp), |line| seen.push(line.to_string()));
        assert!(result.is_ok());
        assert!(seen.is_empty());
    }
}

// ── in-cluster TLS client (http-client or http2 feature) ────────────────────

#[cfg(any(feature = "http-client", feature = "http2"))]
mod tls_tests {
    use super::super::tls;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;

    // A real, openssl-generated self-signed CA and a server certificate it
    // signed for "localhost" (SAN: DNS:localhost, IP:127.0.0.1) — used to
    // drive an actual TLS handshake against a local listener, not just
    // parse-and-hope. Generated once offline; not produced at build time
    // (no new dependency for cert generation).
    const CA_CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\nMIIDFTCCAf2gAwIBAgIUVmuRv7eMznPcuDiCQXfhcwEfz7cwDQYJKoZIhvcNAQEL\nBQAwGjEYMBYGA1UEAwwPdGVzdC1jbHVzdGVyLWNhMB4XDTI2MDcwNzE3MzkyNloX\nDTM2MDcwNDE3MzkyNlowGjEYMBYGA1UEAwwPdGVzdC1jbHVzdGVyLWNhMIIBIjAN\nBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAor2GnXVkjOP9icer4DKRqXKhxMWD\n7CJtzxfWp1BmBo6NSD8lX3QBtekYQDNmXvoXH5KsJrUjJXAkSPFNXfuxkIUkcXOm\neClVtKjACb/PKW1y45DTOpAJvxrT6nMBVIoTgeU/e6Vi1fMdnDGsudVoaOnQ8MFB\nq737PXx+YO/6ej0oIsoxr9IC8B86ukN59QpbpH+sZkEaty+2M9gC2hpwrb2/TIcX\nuhYZ6mIRiCU4d6q0XiLGm39FEEQRN/H9ytswEesJwQtFmI23IbGv2GWKkXFnFweZ\n5/J7v5y0iGFzaxV1c05q3mI4DKg/Ib3XVhx56uE7SKNrMsuWy0oktRAdcQIDAQAB\no1MwUTAdBgNVHQ4EFgQU8pQv1C6kFK28S6U0h/IJjAKScncwHwYDVR0jBBgwFoAU\n8pQv1C6kFK28S6U0h/IJjAKScncwDwYDVR0TAQH/BAUwAwEB/zANBgkqhkiG9w0B\nAQsFAAOCAQEAJQDBTRlJ11+Pgdkw6P6b4vX+aPdUE0tmQhXEYTJVBi1KTf6p9uIu\nd7zDiUJinf1lPk6bKczhLwLW0QFIPM6nniflbiUK902NY4e3H1yOFm/MrnxjESmw\nQoGOHtunGt8a77e1P3nGoDceMNRAYXOwVu0OJFzMwLwiblIUIcNHPDi0KvG+7jtC\npMPp3+gieJUSBALbuhk+CBcI+rALHzdIIYUIePCq617QucbIouZOaWyInVVT5yfv\nLBPdw3Z/xaXX3GuRCQvk+jOGMUr3VyadeiT7guR1vvQxRJqBY1kBNFRi9ScT5bSt\nNWfZZEuetYTJS8yOno9nDBxYyfKMe4uE/w==\n-----END CERTIFICATE-----\n";

    const SERVER_CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\nMIIDGjCCAgKgAwIBAgIUc2S4xWv1bjjGybqxVSytNihz14owDQYJKoZIhvcNAQEL\nBQAwGjEYMBYGA1UEAwwPdGVzdC1jbHVzdGVyLWNhMB4XDTI2MDcwNzE3MzkyN1oX\nDTM2MDcwNDE3MzkyN1owFDESMBAGA1UEAwwJbG9jYWxob3N0MIIBIjANBgkqhkiG\n9w0BAQEFAAOCAQ8AMIIBCgKCAQEAwvk+9qz8DEfG/vl58yraXuMCdFTHUx/VeZgG\n20ktwxla1+ZxenxOpA2dETQDnc3MicNz31tKDYZMolsyW5kr6yDaKvvWsiEgjMMu\nWOmPYCoxnfZF0aBYBDKOrQWIPlJLHjl/36r6QbIKMwpQWT0NjzaZ7QpqJBJoOhKb\nSTQZq4xo7gArzeUDYCidbAfSz9TWNH2G9XB27/Dl+Xy9N4ZSmV39y0vTchl4CdrH\nlnRTQS6pIB/CqrR7QJf15IBw7vrokHDa0niSIm0p3T4sZ2zXcIt8UXHHhRoxeFtX\nsutFJVyK8P7oO9ZZmwHcLv7BvxYc/YV2SlNfONFigISexhco/QIDAQABo14wXDAa\nBgNVHREEEzARgglsb2NhbGhvc3SHBH8AAAEwHQYDVR0OBBYEFD0JB2W2xqZl7p6x\nQ8ju7DUMzBypMB8GA1UdIwQYMBaAFPKUL9QupBStvEulNIfyCYwCknJ3MA0GCSqG\nSIb3DQEBCwUAA4IBAQAGACCaXCQNIK0qRLb9kVRr/p47fdhSfm+6wR9ANJ2poYws\n4JpO3UOvcqUhOY/23DGMfDvvntXwiRBRshJRTPm6WTnPPU3/msAYC9uN99Q3wOzJ\nQkhNNu8cLiac+tu4pjXjBqSjPsYUVL5zVWtECDIkJecJ2jbC1/dBeGHKhis7MJ18\nvWZGxoadSsjaqMzYpH+U6mWOWC4XnLwzgVW58ExNTCDx3ftsoxqiinNhTTP3vE+Y\npVcWCuEy6ZREmhPA2BrY3Z2XOFTFJvug5cDl4Cee3/w6pJSnwk78jYrv8/8yudCn\ny4Af37UG9v4k29l1C1sB8cxO5EU++rzvAjnxVbEj\n-----END CERTIFICATE-----\n";

    const SERVER_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIIEvwIBADANBgkqhkiG9w0BAQEFAASCBKkwggSlAgEAAoIBAQDC+T72rPwMR8b+\n+XnzKtpe4wJ0VMdTH9V5mAbbSS3DGVrX5nF6fE6kDZ0RNAOdzcyJw3PfW0oNhkyi\nWzJbmSvrINoq+9ayISCMwy5Y6Y9gKjGd9kXRoFgEMo6tBYg+UkseOX/fqvpBsgoz\nClBZPQ2PNpntCmokEmg6EptJNBmrjGjuACvN5QNgKJ1sB9LP1NY0fYb1cHbv8OX5\nfL03hlKZXf3LS9NyGXgJ2seWdFNBLqkgH8KqtHtAl/XkgHDu+uiQcNrSeJIibSnd\nPixnbNdwi3xRcceFGjF4W1ey60UlXIrw/ug71lmbAdwu/sG/Fhz9hXZKU1840WKA\nhJ7GFyj9AgMBAAECggEABy5CI7hH6XU0nqwLfL7a9LGsTnfXhiKJrIUX2OL/Z3cQ\nlxuqmZXobjb+I5BkgwmoUHk9VEA6yvgOQdCABfWZ3iVYmPVWI+X/xToPA+vzgYQs\nTaKwpuwehxEMRie1AS6WC6fseLgFqCkUImsX0yGM1UsagitnBgBo4Z1RJhy1cVGV\nufoZrek6R8UYDjeC7SFBHBdj9rz5GYIdlryuVIcQr4S/5sH2OOh1/LsZuwRZeYHo\nKBT8uxe2fgw2a5YLyJvezN9t4YRBZ3zSrBU5LcR8qSdGayeqLS9rLpZ8t1Oteo0y\nKN5RYBbYTCyqOcFPiPWiL93E8+tJfneN4CuuKmwIdQKBgQDkHPPWclka5dh812rv\nmy2PvMFHshJ/ZZdjL6HANCAg51qVdIrP6u9qSsIVe0vxD6BVIc87bqlfq4iLjSO8\n2f407jalDT++V1BS8RlpRjZhXj18nsGj++gZ2tzDyhcvYXcaUCQ+vpG+Tw48jsHH\nblgnqUEej1H5Jz+2nDe8sRxb3wKBgQDazyhMut8Tihyrhmk4RUk+MYob5TCo97Z7\nc1Cj3PZqsfeXDwjkHKPlPtt75UT35gjClSfkB+tnViAotbVeNTI4T146DdM6RvhE\nLVACD61Y8UFaH7YvQQuWgDiLTYLKeMSFO6ew1c6Wqckn8VG8PXTCb9AC17kXQTiv\nEUhbg0yWowKBgQC09driJjhVxDynXOTyS7IrMtxJmhReiCM+hgzVQwSx1ZbgtWFh\na8ieE8w/6l3mUDUrE/Un+rPWt2dM/Zx6Np0ZNFiZOxd0UiPgiG9WOmLtfytb7z1C\nb5ZC3IMBtxIJflJTx3vZYqiPxntOwxkqsniwje6g5aVr+BztKqR0xjPvFQKBgQDB\nk0GI2E3gIHCKwoe1s34/ml4fnZx172guQO9XeHU8ISP0LOXlwPyyI/DS5Bsm4Qhg\n9Mnsr6Dvs78RpOfGZ3N9Y6Ht5Cs5xG2BC0FcAXiPVihFzgZEOdxBkj/z5WfPLhZV\n9Fe/VvfETILcZl60FP6FwZuZ2DU0QIwgPT7xTvBj1wKBgQDRoxfjjOsh8A5Y9JbU\nEDDYJTTjk/bSNdjTXv6cQ/dUUAUplQVqJ3anh3uu8QM1669cgranhaPrCM7eZwXl\naQdYhKxo6+tYe46lWtodtNCV1DEIpl+BkNAKhcub7yNdptVXr9vetMNupyMzLmr6\no75rknulaXwZoBl9Q2s/StEqWg==\n-----END PRIVATE KEY-----\n";

    // A second, wholly unrelated self-signed CA — used to prove that
    // certificate trust is actually enforced, not merely parsed.
    const OTHER_CA_CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\nMIIDDzCCAfegAwIBAgIUeSAwS9EF7/A17yB7LppWO27S1/4wDQYJKoZIhvcNAQEL\nBQAwFzEVMBMGA1UEAwwMdW5yZWxhdGVkLWNhMB4XDTI2MDcwODA2MDkzNFoXDTM2\nMDcwNTA2MDkzNFowFzEVMBMGA1UEAwwMdW5yZWxhdGVkLWNhMIIBIjANBgkqhkiG\n9w0BAQEFAAOCAQ8AMIIBCgKCAQEA52/7+PB/V/eFw2jdJsOzngSoN+h8CrELPn/b\ndDoMPI+Gl5TmWdeCVt68J8JoFRXTfmApAAMoOJz3dz1juLwMwkF153J0jwsQen37\nMi1VM8iVxSPwarYBgjbyMMu94qXZVDYu2VUg5fvxfUn0r1tm9j4Hn/oDbIGX1uSa\n4ddanb1gfNf3cusw1TJoMOxL0y0NirrirjorazRXju6UGk2SIZTJIn3qEJaeo1ZQ\nxgYFrUp5LdBGimSrOFQRup+w0Yjf2tZmdtYQHCvBgqR0GWxVakaxnjztetu2Ahsa\n6Xw7LW88a3wbRu/bLyH7kowqEHoFt+j55qClvh4YlzcYVAd/QwIDAQABo1MwUTAd\nBgNVHQ4EFgQUVozHVFt61sSF7VBE12EC1AlXXEowHwYDVR0jBBgwFoAUVozHVFt6\n1sSF7VBE12EC1AlXXEowDwYDVR0TAQH/BAUwAwEB/zANBgkqhkiG9w0BAQsFAAOC\nAQEAlEg56lDEkr3yPyP9xH1432w3Jyru1Xcso2a5atd/6KiT2xm5XIoI/RcgrbEs\nVCC2q9R7930FMYe25kMPn6ThJUU+wC8087jWqhJRxe6+agBRe3egn3RPGqzi1mD7\nHKhv5tizlg3goil6nMtQRzOAVBHUfraGSWFcBJu1LpFYRHPxpQfYvOJbk04F2QvR\nBoOUf64JXxr3cREjyF2xydZzuInCW1Nk56FKXboHhGYM1aSJXYVzk3adn9jZ6qT7\nA/tc4GCPilLVGObk3EZWF5K3cIjmPSpfKu/IBllKSXujYphOGRtH6rZaVQxAUCVc\nRMBxrU/KwQi6q9Kj8OQ3mBDdqw==\n-----END CERTIFICATE-----\n";

    // ── parse_pem_certificates ───────────────────────────────────────────────

    #[test]
    fn parse_pem_certificates_finds_one_cert_in_ca_bundle() {
        let certs = tls::parse_pem_certificates(CA_CERT_PEM).unwrap();
        assert_eq!(1, certs.len());
    }

    #[test]
    fn parse_pem_certificates_finds_multiple_concatenated_certs() {
        let bundle = format!("{CA_CERT_PEM}{OTHER_CA_CERT_PEM}");
        let certs = tls::parse_pem_certificates(&bundle).unwrap();
        assert_eq!(2, certs.len());
    }

    #[test]
    fn parse_pem_certificates_empty_input_yields_no_certs() {
        let certs = tls::parse_pem_certificates("").unwrap();
        assert!(certs.is_empty());
    }

    #[test]
    fn parse_pem_certificates_rejects_unterminated_block() {
        let broken = "-----BEGIN CERTIFICATE-----\nMIIDFTCC\n";
        assert!(tls::parse_pem_certificates(broken).is_err());
    }

    #[test]
    fn build_client_config_rejects_a_ca_bundle_with_no_certs() {
        assert!(tls::build_client_config("not a pem file").is_err());
    }

    // ── real TLS handshake against a local listener ─────────────────────────

    fn pem_body_der(pem: &str, label: &str) -> Vec<u8> {
        let begin = format!("-----BEGIN {label}-----");
        let end = format!("-----END {label}-----");
        let start = pem.find(&begin).unwrap() + begin.len();
        let stop = pem.find(&end).unwrap();
        let b64: String = pem[start..stop].chars().filter(|c| !c.is_whitespace()).collect();
        // Reuse the same alphabet as tls.rs's own decoder via a tiny local copy
        // (that decoder is private — this is test-only, standard base64).
        let mut out = Vec::with_capacity(b64.len() * 3 / 4);
        let mut buf = 0u32;
        let mut bits = 0u32;
        for ch in b64.chars() {
            if ch == '=' {
                break;
            }
            let v: u32 = match ch {
                'A'..='Z' => ch as u32 - 'A' as u32,
                'a'..='z' => ch as u32 - 'a' as u32 + 26,
                '0'..='9' => ch as u32 - '0' as u32 + 52,
                '+' => 62,
                '/' => 63,
                _ => continue,
            };
            buf = (buf << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buf >> bits) as u8);
            }
        }
        out
    }

    /// Start a local rustls-backed TLS server presenting `SERVER_CERT_PEM`,
    /// handling exactly one connection: read a request (ignored) and write
    /// back a fixed HTTP/1.1 response. Returns the port it's listening on.
    fn start_test_tls_server(response: &'static str) -> u16 {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let cert = CertificateDer::from(pem_body_der(SERVER_CERT_PEM, "CERTIFICATE"));
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pem_body_der(SERVER_KEY_PEM, "PRIVATE KEY")));
        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap();
        let server_config = Arc::new(server_config);

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            if let Ok((tcp, _)) = listener.accept() {
                let conn = rustls::ServerConnection::new(server_config).unwrap();
                let mut stream = rustls::StreamOwned::new(conn, tcp);
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        port
    }

    #[test]
    fn https_get_succeeds_against_a_server_signed_by_the_trusted_ca() {
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
        let port = start_test_tls_server(response);

        let client_config = tls::build_client_config(CA_CERT_PEM).unwrap();
        let body = tls::https_get(
            "127.0.0.1",
            port,
            "localhost",
            client_config,
            "",
            "/apis/networking.k8s.io/v1/ingresses",
            std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!("ok", body);
    }

    #[test]
    fn https_get_sends_the_bearer_token() {
        // The server just echoes back whether it saw the Authorization
        // header, proving https_get actually sent it.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let cert = rustls::pki_types::CertificateDer::from(pem_body_der(SERVER_CERT_PEM, "CERTIFICATE"));
        let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(pem_body_der(SERVER_KEY_PEM, "PRIVATE KEY")));
        let server_config = Arc::new(rustls::ServerConfig::builder().with_no_client_auth().with_single_cert(vec![cert], key).unwrap());
        std::thread::spawn(move || {
            if let Ok((tcp, _)) = listener.accept() {
                let conn = rustls::ServerConnection::new(server_config).unwrap();
                let mut stream = rustls::StreamOwned::new(conn, tcp);
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let found = req.contains("Authorization: Bearer test-token-123");
                let body = if found { "yes" } else { "no" };
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(resp.as_bytes());
            }
        });

        let client_config = tls::build_client_config(CA_CERT_PEM).unwrap();
        let body = tls::https_get("127.0.0.1", port, "localhost", client_config, "test-token-123", "/", std::time::Duration::from_secs(5)).unwrap();
        assert_eq!("yes", body);
    }

    #[test]
    fn https_get_fails_when_server_cert_is_not_signed_by_the_trusted_ca() {
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
        let port = start_test_tls_server(response);

        // Trust a completely unrelated CA — the handshake must fail.
        let client_config = tls::build_client_config(OTHER_CA_CERT_PEM).unwrap();
        let result = tls::https_get(
            "127.0.0.1",
            port,
            "localhost",
            client_config,
            "",
            "/",
            std::time::Duration::from_secs(5),
        );
        assert!(result.is_err(), "handshake should fail: server cert is not signed by the trusted CA");
    }

    #[test]
    fn https_get_fails_on_connection_refused() {
        let client_config = tls::build_client_config(CA_CERT_PEM).unwrap();
        let result = tls::https_get("127.0.0.1", 1, "localhost", client_config, "", "/", std::time::Duration::from_secs(2));
        assert!(result.is_err());
    }
}
