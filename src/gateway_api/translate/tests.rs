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
            "GRPCRoute" => s.grpc_routes.push(serde_json::from_value(doc).unwrap()),
            "TLSRoute" => s.tls_routes.push(serde_json::from_value(doc).unwrap()),
            "TCPRoute" => s.tcp_routes.push(serde_json::from_value(doc).unwrap()),
            "UDPRoute" => s.udp_routes.push(serde_json::from_value(doc).unwrap()),
            "ListenerSet" => s.listener_sets.push(serde_json::from_value(doc).unwrap()),
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

#[test]
fn claimed_features_are_sorted_and_match_the_conformance_job() {
    let mut sorted = SUPPORTED_FEATURES.to_vec();
    sorted.sort();
    assert_eq!(sorted, SUPPORTED_FEATURES);
    let workflow = include_str!("../../../.github/workflows/gateway-api.yml");
    let line = workflow.lines().find_map(|l| l.trim().strip_prefix("FEATURES: ")).expect("FEATURES in the workflow");
    assert_eq!(SUPPORTED_FEATURES.join(","), line);
}

#[test]
fn mirror_targets_are_resolved_like_backends() {
    let mirrored = |service: &str| {
        let mut s = snapshot(&["httproute-simple-same-namespace"]);
        let filter = HttpRouteFilter {
            filter_type: "RequestMirror".to_string(),
            request_mirror: Some(HttpRequestMirrorFilter {
                backend_ref: BackendObjectReference { name: service.to_string(), port: Some(8080), ..Default::default() },
                percent: Some(35),
                fraction: None,
            }),
            ..Default::default()
        };
        s.http_routes[0].spec.rules[0].filters.push(filter);
        run(&s)
    };
    let (config, desired) = mirrored("infra-backend-v2");
    assert_eq!("True", find(&route_parent(&desired, "gateway-conformance-infra-test").conditions, "ResolvedRefs").status);
    let served = config.listeners.iter().find(|l| l.gateway.1 == "same-namespace").unwrap();
    assert_eq!(0.35, served.http_routes[0].rules[0].mirrors[0].share);
    let (_, desired) = mirrored("no-such-service");
    let refs = find(&route_parent(&desired, "gateway-conformance-infra-test").conditions, "ResolvedRefs");
    assert_eq!(("False", "BackendNotFound"), (refs.status.as_str(), refs.reason.as_str()));
}

#[test]
fn grpc_routes_attach_beside_http_routes_and_compile_methods_to_paths() {
    let (config, desired) = run(&snapshot(&["grpcroute-exact-method-matching"]));
    let parent = &desired.grpc_routes[&(INFRA.to_string(), "exact-matching".to_string())][0];
    assert_eq!("True", find(&parent.conditions, "Accepted").status);
    assert_eq!("True", find(&parent.conditions, "ResolvedRefs").status);
    assert!(desired.http_routes.is_empty());
    let http = listener(&desired, "same-namespace", "http");
    assert!(http.supported_kinds.iter().any(|k| k.kind == "GRPCRoute"));
    assert_eq!(1, http.attached_routes);
    let served = config.listeners.iter().find(|l| l.gateway.1 == "same-namespace").unwrap();
    let rule = &served.http_routes[0].rules[0];
    let path = rule.matches[0].path.as_ref().unwrap();
    assert_eq!((Some("Exact"), Some("/gateway_api_conformance.echo_basic.grpcecho.GrpcEcho/Echo")), (path.match_type.as_deref(), path.value.as_deref()));
    assert_eq!(Some("kubernetes.io/h2c"), rule.backends[0].app_protocol.as_deref(), "gRPC backends are h2c");
}

#[test]
fn grpc_routes_match_listener_hostnames() {
    let (_, desired) = run(&snapshot(&["grpcroute-listener-hostname-matching"]));
    for name in ["backend-v1", "backend-v2", "backend-v3"] {
        let parents = &desired.grpc_routes[&(INFRA.to_string(), name.to_string())];
        assert!(parents.iter().all(|p| find(&p.conditions, "Accepted").status == "True"), "{name}: {parents:?}");
    }
    assert_eq!(1, listener(&desired, "grpcroute-listener-hostname-matching", "listener-1").attached_routes);
}

/// A TLS Secret named `name` in the infra namespace, as the suite creates at run time.
fn tls_secret(name: &str) -> Secret {
    let cert = rcgen::generate_simple_self_signed(vec!["example.org".to_string()]).unwrap();
    let mut secret: Secret = serde_json::from_value(serde_json::json!({
        "apiVersion": "v1", "kind": "Secret", "type": "kubernetes.io/tls",
        "metadata": {"name": name, "namespace": INFRA}
    }))
    .unwrap();
    secret.data = Some(BTreeMap::from([
        ("tls.crt".to_string(), ByteString(cert.cert.pem().into_bytes())),
        ("tls.key".to_string(), ByteString(cert.signing_key.serialize_pem().into_bytes())),
    ]));
    secret
}

#[test]
fn tls_passthrough_routes_attach_by_hostname_without_certificates() {
    let (config, desired) = run(&snapshot(&["tlsroute-simple-same-namespace"]));
    let parent = &desired.tls_routes[&(INFRA.to_string(), "gateway-conformance-infra-test".to_string())][0];
    assert_eq!("True", find(&parent.conditions, "Accepted").status, "{parent:?}");
    assert_eq!("True", find(&parent.conditions, "ResolvedRefs").status);
    let https = listener(&desired, "gateway-tlsroute", "https");
    assert_eq!(1, https.attached_routes);
    assert_eq!("True", find(&https.conditions, "Programmed").status, "passthrough needs no certificate: {:?}", https.conditions);
    let served = config.listeners.iter().find(|l| l.gateway.1 == "gateway-tlsroute").unwrap();
    assert_eq!(("TLS", None), (served.protocol.as_str(), served.tls.as_ref().map(|_| ())));
    assert_eq!(vec!["abc.example.com".to_string()], served.http_routes[0].hostnames);
}

#[test]
fn tls_listeners_carry_only_tls_routes() {
    let (_, desired) = run(&snapshot(&["tlsroute-listener-passthrough-supported-kinds"]));
    let l = listener(&desired, "gateway-tlsroute-passthrough-supported-kind", "tls-passthrough");
    assert_eq!(vec!["TLSRoute"], l.supported_kinds.iter().map(|k| k.kind.as_str()).collect::<Vec<_>>());
    assert_eq!(("False", "InvalidRouteKinds"), (find(&l.conditions, "ResolvedRefs").status.as_str(), find(&l.conditions, "ResolvedRefs").reason.as_str()));
}

#[test]
fn tls_terminate_listeners_need_their_certificate() {
    let mut s = snapshot(&["tlsroute-terminate-simple-same-namespace"]);
    let (_, desired) = run(&s);
    let l = listener(&desired, "gateway-tlsroute-terminate", "tlsterminate");
    assert_eq!("InvalidCertificateRef", find(&l.conditions, "ResolvedRefs").reason);
    s.secrets.push(tls_secret("tls-terminate-checks-certificate"));
    let (config, desired) = run(&s);
    let l = listener(&desired, "gateway-tlsroute-terminate", "tlsterminate");
    assert_eq!(("True", 1), (find(&l.conditions, "Programmed").status.as_str(), l.attached_routes));
    let served = config.listeners.iter().find(|l| l.gateway.1 == "gateway-tlsroute-terminate").unwrap();
    assert!(served.tls.is_some(), "the data plane terminates with it");
    let parent = &desired.tls_routes[&(INFRA.to_string(), "tlsroute-terminated-test".to_string())][0];
    assert_eq!("True", find(&parent.conditions, "Accepted").status);
}

#[test]
fn tcp_and_udp_routes_attach_to_their_listeners() {
    let (config, desired) = run(&snapshot(&["tcproute-multiple-routes-attachment", "udproute-simple"]));
    let tcp = &desired.tcp_routes[&(INFRA.to_string(), "tcproute-attach-older".to_string())][0];
    assert_eq!("True", find(&tcp.conditions, "Accepted").status, "{tcp:?}");
    assert_eq!("True", find(&tcp.conditions, "ResolvedRefs").status);
    let udp = &desired.udp_routes[&(INFRA.to_string(), "udp-coredns".to_string())][0];
    assert_eq!("True", find(&udp.conditions, "Accepted").status, "{udp:?}");
    assert_eq!(1, listener(&desired, "udp-gateway", "coredns").attached_routes);
    let protocols: BTreeSet<&str> = config.listeners.iter().filter(|l| l.gateway.1.contains("tcp") || l.gateway.1.contains("udp")).map(|l| l.protocol.as_str()).collect();
    assert_eq!(BTreeSet::from(["TCP", "UDP"]), protocols);
}

#[test]
fn a_gateway_is_accepted_while_any_listener_is() {
    let (_, desired) = run(&snapshot(&["gateway-invalid-listeners-unsupported-protocol"]));
    let accepted = |gateway: &str| {
        let c = find(&desired.gateways[&(INFRA.to_string(), gateway.to_string())].conditions, "Accepted").clone();
        (c.status, c.reason)
    };
    assert_eq!(("False".to_string(), "ListenersNotValid".to_string()), accepted("gateway-only-unsupported-protocols"));
    assert_eq!(("True".to_string(), "ListenersNotValid".to_string()), accepted("gateway-supported-and-unsupported-protocols"));
    assert_eq!(("True".to_string(), "Accepted".to_string()), accepted("same-namespace"));
}

fn set_condition(desired: &Desired, set: &str, kind: &str) -> (String, String) {
    let c = find(&desired.listener_sets[&(INFRA.to_string(), set.to_string())].conditions, kind).clone();
    (c.status, c.reason)
}

fn set_listener<'a>(desired: &'a Desired, set: &str, name: &str) -> &'a ListenerStatus {
    desired.listener_sets[&(INFRA.to_string(), set.to_string())].listeners.iter().find(|l| l.name == name).unwrap()
}

/// The suite applies a namespace's objects in order, a second apart.
fn created_in_order(s: &mut Snapshot) {
    for (i, set) in s.listener_sets.iter_mut().enumerate() {
        set.metadata.creation_timestamp = Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(k8s_openapi::jiff::Timestamp::from_second(1_700_000_000 + i as i64).unwrap()));
    }
}

#[test]
fn listener_sets_need_the_gateways_permission() {
    let (_, desired) = run(&snapshot(&["listenerset-default-not-allowed"]));
    let ok = |status: &str, reason: &str| (status.to_string(), reason.to_string());
    assert_eq!(ok("False", "NotAllowed"), set_condition(&desired, "listenerset-default-not-allowed", "Accepted"));
    assert_eq!(ok("False", "NotAllowed"), set_condition(&desired, "listenerset-default-not-allowed", "Programmed"));
    assert_eq!(Some(0), desired.gateways[&(INFRA.to_string(), "gateway-default-does-not-allow-listenerset".to_string())].attached_listener_sets);
}

#[test]
fn a_listener_sets_listener_loses_conflicts_to_earlier_listeners() {
    let mut s = snapshot(&["listenerset-hostname-conflict"]);
    created_in_order(&mut s);
    let (_, desired) = run(&s);
    let ok = |status: &str, reason: &str| (status.to_string(), reason.to_string());
    let gateway = &desired.gateways[&(INFRA.to_string(), "gateway-with-listenerset-hostname-conflict".to_string())];
    assert_eq!(Some(2), gateway.attached_listener_sets);
    for name in ["gateway-listener", "hostname-conflict-with-gateway-listener"] {
        let l = gateway.listeners.iter().find(|l| l.name == name).unwrap();
        assert_eq!("True", find(&l.conditions, "Accepted").status, "{name}");
        assert_eq!("False", find(&l.conditions, "Conflicted").status, "{name}: the Gateway's listeners win");
    }
    let conflicted = |set: &str, name: &str| {
        let l = set_listener(&desired, set, name);
        (find(&l.conditions, "Accepted").status.clone(), find(&l.conditions, "Programmed").reason.clone(), find(&l.conditions, "Conflicted").status.clone())
    };
    let lost = ("False".to_string(), "HostnameConflict".to_string(), "True".to_string());
    assert_eq!(lost, conflicted("listenerset-with-hostname-conflict-with-gateway-1", "hostname-conflict-with-gateway-listener"));
    assert_eq!("True", find(&set_listener(&desired, "listenerset-with-hostname-conflict-with-gateway-1", "hostname-conflict-with-listener-set-listener").conditions, "Accepted").status, "first of its name");
    assert_eq!(lost, conflicted("listenerset-with-hostname-conflict-with-listener-set-2", "hostname-conflict-with-listener-set-listener"));
    assert_eq!(ok("True", "Accepted"), set_condition(&desired, "listenerset-with-hostname-conflict-with-gateway-1", "Accepted"));
    assert_eq!(ok("True", "Programmed"), set_condition(&desired, "listenerset-with-hostname-conflict-with-listener-set-1", "Programmed"));
    assert_eq!(ok("False", "ListenersNotValid"), set_condition(&desired, "listenerset-with-hostname-conflict-with-gateway-2", "Accepted"));
    assert_eq!(ok("False", "ListenersNotValid"), set_condition(&desired, "listenerset-with-hostname-conflict-with-listener-set-2", "Programmed"));
}

#[test]
fn routes_reach_listener_set_listeners_only_through_the_set() {
    let mut s = snapshot(&["listenerset-http-routing"]);
    created_in_order(&mut s);
    let (config, desired) = run(&s);
    let gateway = &desired.gateways[&(INFRA.to_string(), "gateway-with-listener-sets-http-routing".to_string())];
    assert_eq!(Some(2), gateway.attached_listener_sets);
    for route in ["attaches-to-all-listeners", "gateway-route", "listener-set-http-routing-1-route", "listener-set-http-routing-2-route"] {
        for parent in &desired.http_routes[&(INFRA.to_string(), route.to_string())] {
            assert_eq!("True", find(&parent.conditions, "Accepted").status, "{route} {:?}", parent.parent_ref);
        }
    }
    let served: Vec<(&str, Vec<&str>)> = config
        .listeners
        .iter()
        .filter(|l| l.gateway.1 == "gateway-with-listener-sets-http-routing")
        .map(|l| (l.hostname.as_deref().unwrap_or(""), l.http_routes.iter().map(|r| r.key.1.as_str()).collect()))
        .collect();
    let routes_on = |host: &str| served.iter().find(|(h, _)| *h == host).map(|(_, r)| r.clone()).unwrap_or_default();
    assert!(routes_on("gateway-listener-1.com").contains(&"gateway-route"));
    assert!(!routes_on("listener-set-http-routing-1-listener-1.com").contains(&"gateway-route"), "a Gateway parentRef doesn't reach a set's listeners");
    assert!(routes_on("listener-set-http-routing-1-listener-1.com").contains(&"listener-set-http-routing-1-route"));
    let service = desired.services.values().find(|svc| svc.labels().get(GATEWAY_NAME_LABEL).map(String::as_str) == Some("gateway-with-listener-sets-http-routing")).unwrap();
    assert!(!service.spec.as_ref().unwrap().ports.as_ref().unwrap().is_empty());
}
