use super::*;
use k8s_openapi::api::core::v1::Service;

fn service(namespace: &str, name: &str, ports: serde_json::Value) -> Service {
    serde_json::from_value(serde_json::json!({"apiVersion": "v1", "kind": "Service",
        "metadata": {"name": name, "namespace": namespace}, "spec": {"ports": ports}})).unwrap()
}

fn slice(namespace: &str, service: &str, ports: serde_json::Value, endpoints: serde_json::Value) -> EndpointSlice {
    serde_json::from_value(serde_json::json!({"apiVersion": "discovery.k8s.io/v1", "kind": "EndpointSlice",
        "metadata": {"name": format!("{service}-x"), "namespace": namespace, "labels": {"kubernetes.io/service-name": service}},
        "addressType": "IPv4", "ports": ports, "endpoints": endpoints})).unwrap()
}

fn reference(name: &str, port: Option<i32>) -> BackendObjectReference {
    BackendObjectReference { name: name.to_string(), port, ..Default::default() }
}

#[test]
fn backends_resolve_to_ready_endpoints_through_named_ports() {
    let snapshot = Snapshot {
        services: vec![service("ns", "web", serde_json::json!([{"name": "http", "port": 80, "targetPort": 8080, "appProtocol": "kubernetes.io/h2c"}, {"name": "admin", "port": 9000}]))],
        endpoint_slices: vec![slice("ns", "web", serde_json::json!([{"name": "http", "port": 8080}, {"name": "admin", "port": 9000}]), serde_json::json!([
            {"addresses": ["10.1.0.2"], "conditions": {"ready": true}},
            {"addresses": ["10.1.0.3"], "conditions": {"ready": false}},
            {"addresses": ["fd00::4"]}
        ]))],
        ..Default::default()
    };
    let (endpoints, app_protocol) = resolve(&reference("web", Some(80)), "ns", "HTTPRoute", &snapshot);
    assert_eq!(Ok(vec!["10.1.0.2:8080".to_string(), "[fd00::4]:8080".to_string()]), endpoints, "not-ready left out, IPv6 bracketed");
    assert_eq!(Some("kubernetes.io/h2c".to_string()), app_protocol);
    assert_eq!(Ok(vec!["10.1.0.2:9000".to_string(), "[fd00::4]:9000".to_string()]), resolve(&reference("web", Some(9000)), "ns", "HTTPRoute", &snapshot).0);
}

#[test]
fn unresolvable_backends_are_errors() {
    let snapshot = Snapshot { services: vec![service("ns", "web", serde_json::json!([{"port": 80}]))], ..Default::default() };
    assert!(resolve(&reference("web", None), "ns", "HTTPRoute", &snapshot).0.is_err(), "a port is required");
    assert!(resolve(&reference("web", Some(81)), "ns", "HTTPRoute", &snapshot).0.is_err());
    assert!(resolve(&reference("nope", Some(80)), "ns", "HTTPRoute", &snapshot).0.is_err());
    let other_ns = BackendObjectReference { namespace: Some("elsewhere".to_string()), ..reference("web", Some(80)) };
    assert!(resolve(&other_ns, "ns", "HTTPRoute", &snapshot).0.is_err(), "no ReferenceGrant");
    assert_eq!(Ok(Vec::<String>::new()), resolve(&reference("web", Some(80)), "ns", "HTTPRoute", &snapshot).0, "no endpoints yet: resolvable, empty");
}

#[test]
fn a_rule_without_matches_matches_every_path() {
    let route: crate::gateway_api::resources::HttpRoute = serde_json::from_value(serde_json::json!({
        "apiVersion": "gateway.networking.k8s.io/v1", "kind": "HTTPRoute",
        "metadata": {"name": "r", "namespace": "ns", "creationTimestamp": "2026-10-01T00:00:00Z"},
        "spec": {"rules": [{"backendRefs": [{"name": "web", "port": 80, "weight": 0}]}]}
    })).unwrap();
    let compiled = compile(&route, &Snapshot::default());
    assert_eq!(Some("/".to_string()), compiled.rules[0].matches[0].path.as_ref().unwrap().value);
    assert_eq!(0, compiled.rules[0].backends[0].weight);
    assert!(compiled.created.starts_with("2026-10-01"), "{}", compiled.created);
}
