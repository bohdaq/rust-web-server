//! Translation against the conformance suite's own manifests
//! (`testdata/`, from Gateway API v1.6.3), with the conditions and reasons
//! its Go tests expect.

use super::*;
use k8s_openapi::ByteString;

const CONTROLLER: &str = super::super::CONTROLLER_NAME;
const INFRA: &str = "gateway-conformance-infra";

fn fixture(name: &str) -> Vec<serde_json::Value> {
    let path = format!("{}/src/gateway_api/testdata/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn class(name: &str, controller: &str) -> GatewayClass {
    serde_json::from_value(serde_json::json!({
        "apiVersion": "gateway.networking.k8s.io/v1", "kind": "GatewayClass",
        "metadata": {"name": name, "generation": 1},
        "spec": {"controllerName": controller}
    }))
    .unwrap()
}

/// The Service rws created for a Gateway: what translation wants, with a
/// load-balancer address, as the API server would show it.
fn gateway_service(s: &Snapshot, gateway: &Gateway, ip: &str) -> Service {
    let wanted = super::super::services::desired(s, &[gateway], &Default::default());
    let mut service = wanted.into_values().next().unwrap();
    service.status = Some(serde_json::from_value(serde_json::json!({"loadBalancer": {"ingress": [{"ip": ip}]}})).unwrap());
    service
}

/// base.json plus each named fixture, the GatewayClass `rws`, and a Service
/// with an address for every Gateway.
fn snapshot(fixtures: &[&str]) -> Snapshot {
    let mut s = Snapshot { classes: vec![class("rws", CONTROLLER)], ..Default::default() };
    for doc in std::iter::once("base").chain(fixtures.iter().copied()).flat_map(fixture) {
        match doc["kind"].as_str().unwrap() {
            "Gateway" => s.gateways.push(serde_json::from_value(doc).unwrap()),
            "HTTPRoute" => s.http_routes.push(serde_json::from_value(doc).unwrap()),
            "ReferenceGrant" => s.reference_grants.push(serde_json::from_value(doc).unwrap()),
            "Service" => s.services.push(serde_json::from_value(doc).unwrap()),
            "Namespace" => s.namespaces.push(serde_json::from_value(doc).unwrap()),
            "Secret" => s.secrets.push(serde_json::from_value(doc).unwrap()),
            other => panic!("unexpected kind {other}"),
        }
    }
    let gateways = s.gateways.clone();
    for (i, gateway) in gateways.iter().enumerate() {
        let service = gateway_service(&s, gateway, &format!("10.0.0.{}", i + 1));
        s.services.push(service);
    }
    s
}

fn now() -> Time {
    Time(k8s_openapi::jiff::Timestamp::from_second(1_800_000_000).unwrap())
}

fn run(s: &Snapshot) -> (GatewayConfig, Desired) {
    translate(s, &Settings::default(), &now())
}

fn find<'a>(conditions: &'a [Condition], kind: &str) -> &'a Condition {
    conditions.iter().find(|c| c.type_ == kind).unwrap_or_else(|| panic!("no {kind} in {conditions:?}"))
}

fn route_parent<'a>(desired: &'a Desired, name: &str) -> &'a RouteParentStatus {
    &desired.http_routes[&(INFRA.to_string(), name.to_string())][0]
}

fn listener<'a>(desired: &'a Desired, gateway: &str, name: &str) -> &'a ListenerStatus {
    desired.gateways[&(INFRA.to_string(), gateway.to_string())].listeners.iter().find(|l| l.name == name).unwrap()
}

#[test]
fn only_our_gateway_classes_are_accepted_with_their_features() {
    let mut s = snapshot(&[]);
    s.classes.push(class("someone-else", "example.com/other"));
    let (_, desired) = run(&s);
    assert!(!desired.classes.contains_key("someone-else"));
    let ours = &desired.classes["rws"];
    let accepted = find(&ours.conditions, "Accepted");
    assert_eq!(("True", "Accepted", Some(1)), (accepted.status.as_str(), accepted.reason.as_str(), accepted.observed_generation));
    assert!(ours.supported_features.iter().any(|f| f.name == "HTTPRoute"));
}

#[test]
fn a_simple_route_attaches_and_resolves() {
    let (config, desired) = run(&snapshot(&["httproute-simple-same-namespace"]));
    let parent = route_parent(&desired, "gateway-conformance-infra-test");
    assert_eq!(CONTROLLER, parent.controller_name);
    assert_eq!("True", find(&parent.conditions, "Accepted").status);
    assert_eq!("True", find(&parent.conditions, "ResolvedRefs").status);
    let gateway = &desired.gateways[&(INFRA.to_string(), "same-namespace".to_string())];
    assert_eq!("True", find(&gateway.conditions, "Programmed").status);
    assert!(gateway.addresses[0].value.starts_with("10.0.0."));
    assert_eq!(1, listener(&desired, "same-namespace", "http").attached_routes);
    let served = config.listeners.iter().find(|l| l.gateway.1 == "same-namespace").unwrap();
    assert_eq!(vec![(INFRA.to_string(), "gateway-conformance-infra-test".to_string())], served.http_routes.iter().map(|r| r.key.clone()).collect::<Vec<_>>());
    assert!((10000..=10999).contains(&served.container_port));
}

#[test]
fn a_cross_namespace_backend_needs_a_reference_grant() {
    let (_, desired) = run(&snapshot(&["httproute-invalid-cross-namespace-backend-ref"]));
    let refs = find(&route_parent(&desired, "invalid-cross-namespace-backend-ref").conditions, "ResolvedRefs");
    assert_eq!(("False", "RefNotPermitted"), (refs.status.as_str(), refs.reason.as_str()));
    let (_, desired) = run(&snapshot(&["httproute-reference-grant"]));
    let refs = find(&route_parent(&desired, "reference-grant").conditions, "ResolvedRefs");
    assert_eq!("True", refs.status, "{refs:?}");
}

#[test]
fn an_unknown_backend_kind_is_invalid_kind() {
    let (_, desired) = run(&snapshot(&["httproute-invalid-backendref-unknown-kind"]));
    let route = &desired.http_routes.values().next().unwrap()[0];
    let refs = find(&route.conditions, "ResolvedRefs");
    assert_eq!(("False", "InvalidKind"), (refs.status.as_str(), refs.reason.as_str()));
}

#[test]
fn invalid_route_kinds_are_reported_on_the_listener() {
    let (_, desired) = run(&snapshot(&["gateway-invalid-route-kind"]));
    let only_invalid = listener(&desired, "gateway-only-invalid-route-kind", "http");
    assert!(only_invalid.supported_kinds.is_empty());
    let refs = find(&only_invalid.conditions, "ResolvedRefs");
    assert_eq!(("False", "InvalidRouteKinds"), (refs.status.as_str(), refs.reason.as_str()));
    let mixed = listener(&desired, "gateway-supported-and-invalid-route-kind", "http");
    assert_eq!(vec!["HTTPRoute"], mixed.supported_kinds.iter().map(|k| k.kind.as_str()).collect::<Vec<_>>());
    assert_eq!("InvalidRouteKinds", find(&mixed.conditions, "ResolvedRefs").reason);
}

#[test]
fn attached_routes_are_counted_even_when_refs_are_unresolved() {
    let (_, desired) = run(&snapshot(&["gateway-with-attached-routes"]));
    assert_eq!(1, listener(&desired, "gateway-with-one-attached-route", "http").attached_routes);
    // http-route-not-accepted's hostname matches no listener: two, not three.
    assert_eq!(2, listener(&desired, "gateway-with-two-attached-routes", "http").attached_routes);
    let not_accepted = find(&route_parent(&desired, "http-route-not-accepted").conditions, "Accepted");
    assert_eq!(("False", "NoMatchingListenerHostname"), (not_accepted.status.as_str(), not_accepted.reason.as_str()));
    // A TLS listener without its Secret: not programmed, unresolved, but the route still counts.
    let tls = listener(&desired, "unresolved-gateway-with-one-attached-unresolved-route", "tls");
    assert_eq!(1, tls.attached_routes);
    assert_eq!("False", find(&tls.conditions, "Programmed").status);
    assert_eq!("False", find(&tls.conditions, "ResolvedRefs").status);
    assert_eq!("False", find(&route_parent(&desired, "http-route-4").conditions, "ResolvedRefs").status);
}

#[test]
fn routes_attach_to_the_listeners_their_hostnames_match() {
    let (config, desired) = run(&snapshot(&["httproute-listener-hostname-matching"]));
    for (route, accepted) in [("backend-v1", true), ("backend-v2", true), ("backend-v3", true)] {
        assert_eq!(if accepted { "True" } else { "False" }, find(&route_parent(&desired, route).conditions, "Accepted").status, "{route}");
    }
    let on = |name: &str| config.listeners.iter().find(|l| l.gateway.1 == "httproute-listener-hostname-matching" && l.name == name).unwrap().http_routes.len();
    assert_eq!((1, 1, 1, 1), (on("listener-1"), on("listener-2"), on("listener-3"), on("listener-4")));
}

#[test]
fn https_listeners_need_a_valid_tls_secret() {
    let s = snapshot(&[]);
    let (_, desired) = run(&s);
    let https = listener(&desired, "same-namespace-with-https-listener", "https");
    assert_eq!("InvalidCertificateRef", find(&https.conditions, "ResolvedRefs").reason, "the suite creates the Secret at run time");

    let mut s = s;
    let gateway = s.gateways.iter().find(|g| g.name_any() == "same-namespace-with-https-listener").unwrap();
    let secret_name = gateway.spec.listeners[0].tls.as_ref().unwrap().certificate_refs[0].name.clone();
    let cert = rcgen::generate_simple_self_signed(vec!["example.org".to_string()]).unwrap();
    let mut secret: Secret = serde_json::from_value(serde_json::json!({
        "apiVersion": "v1", "kind": "Secret", "type": "kubernetes.io/tls",
        "metadata": {"name": secret_name, "namespace": INFRA}
    }))
    .unwrap();
    secret.data = Some(BTreeMap::from([
        ("tls.crt".to_string(), ByteString(cert.cert.pem().into_bytes())),
        ("tls.key".to_string(), ByteString(cert.signing_key.serialize_pem().into_bytes())),
    ]));
    s.secrets.push(secret);
    let (_, desired) = run(&s);
    let https = listener(&desired, "same-namespace-with-https-listener", "https");
    assert_eq!("True", find(&https.conditions, "ResolvedRefs").status, "{:?}", https.conditions);
    assert_eq!("True", find(&https.conditions, "Programmed").status);
}

#[test]
fn a_gateway_without_an_address_isnt_programmed() {
    let mut s = snapshot(&[]);
    s.services.retain(|svc| !svc.labels().contains_key(GATEWAY_NAME_LABEL));
    let (_, desired) = run(&s);
    let programmed = find(&desired.gateways[&(INFRA.to_string(), "same-namespace".to_string())].conditions, "Programmed");
    assert_eq!(("False", "AddressNotAssigned"), (programmed.status.as_str(), programmed.reason.as_str()));
}

#[test]
fn transition_times_are_kept_while_the_status_holds() {
    let s = snapshot(&["httproute-simple-same-namespace"]);
    let (_, first) = run(&s);
    let mut s2 = s.clone();
    let route = s2.http_routes.iter_mut().find(|r| r.name_any() == "gateway-conformance-infra-test").unwrap();
    route.status = Some(RouteStatus { parents: first.http_routes[&(INFRA.to_string(), "gateway-conformance-infra-test".to_string())].clone() });
    route.metadata.generation = Some(2);
    let later = Time(k8s_openapi::jiff::Timestamp::from_second(1_900_000_000).unwrap());
    let (_, second) = translate(&s2, &Settings::default(), &later);
    let accepted = find(&second.http_routes[&(INFRA.to_string(), "gateway-conformance-infra-test".to_string())][0].conditions, "Accepted");
    assert_eq!(now(), accepted.last_transition_time, "unchanged status keeps its time");
    assert_eq!(Some(2), accepted.observed_generation, "but follows the generation");
}

#[test]
fn listeners_conflict_on_port_protocol_and_hostname() {
    let gateway: Gateway = serde_json::from_value(serde_json::json!({
        "apiVersion": "gateway.networking.k8s.io/v1", "kind": "Gateway",
        "metadata": {"name": "conflicts", "namespace": INFRA, "generation": 1},
        "spec": {"gatewayClassName": "rws", "listeners": [
            {"name": "a", "port": 8080, "protocol": "HTTP", "hostname": "x.example.com"},
            {"name": "b", "port": 8080, "protocol": "HTTP", "hostname": "x.example.com"},
            {"name": "c", "port": 8081, "protocol": "HTTP"},
            {"name": "d", "port": 8081, "protocol": "TCP"}
        ]}
    }))
    .unwrap();
    let mut s = snapshot(&[]);
    let service = gateway_service(&s, &gateway, "10.0.1.1");
    s.gateways.push(gateway);
    s.services.push(service);
    let (_, desired) = run(&s);
    assert_eq!("HostnameConflict", find(&listener(&desired, "conflicts", "a").conditions, "Conflicted").reason);
    assert_eq!("ProtocolConflict", find(&listener(&desired, "conflicts", "c").conditions, "Conflicted").reason);
}

#[test]
fn hostnames_intersect_like_the_spec_says() {
    let h = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(hostnames_intersect(None, &h(&["a.com"])));
    assert!(hostnames_intersect(Some("a.com"), &[]));
    assert!(hostnames_intersect(Some("*.bar.com"), &h(&["foo.bar.com"])));
    assert!(hostnames_intersect(Some("*.bar.com"), &h(&["a.b.bar.com"])), "a wildcard covers one or more labels");
    assert!(!hostnames_intersect(Some("*.bar.com"), &h(&["bar.com"])));
    assert!(hostnames_intersect(Some("foo.bar.com"), &h(&["*.bar.com"])));
    assert!(hostnames_intersect(Some("*.foo.bar.com"), &h(&["*.bar.com"])));
    assert!(!hostnames_intersect(Some("foo.com"), &h(&["bar.com", "*.baz.com"])));
}

#[test]
fn label_selectors_match_labels_and_expressions() {
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelectorRequirement;
    let labels = BTreeMap::from([("tier".to_string(), "backend".to_string())]);
    let selector = |labels: Option<BTreeMap<String, String>>, exprs: Vec<LabelSelectorRequirement>| LabelSelector {
        match_labels: labels,
        match_expressions: Some(exprs),
    };
    let req = |op: &str, values: &[&str]| LabelSelectorRequirement {
        key: "tier".to_string(),
        operator: op.to_string(),
        values: Some(values.iter().map(|v| v.to_string()).collect()),
    };
    assert!(selector_matches(&selector(Some(labels.clone()), vec![]), &labels));
    assert!(selector_matches(&selector(None, vec![req("In", &["backend", "x"])]), &labels));
    assert!(!selector_matches(&selector(None, vec![req("NotIn", &["backend"])]), &labels));
    assert!(selector_matches(&selector(None, vec![req("Exists", &[])]), &labels));
    assert!(!selector_matches(&selector(None, vec![req("DoesNotExist", &[])]), &labels));
}
