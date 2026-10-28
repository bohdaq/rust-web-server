use super::*;
use crate::gateway_api::translate::Snapshot;

fn gateway(name: &str, listeners: serde_json::Value, extra: serde_json::Value) -> Gateway {
    let mut spec = serde_json::json!({"gatewayClassName": "rws", "listeners": listeners});
    if let serde_json::Value::Object(extra) = extra {
        spec.as_object_mut().unwrap().extend(extra);
    }
    serde_json::from_value(serde_json::json!({
        "apiVersion": "gateway.networking.k8s.io/v1", "kind": "Gateway",
        "metadata": {"name": name, "namespace": "ns", "uid": format!("uid-{name}")},
        "spec": spec
    }))
    .unwrap()
}

fn ports(service: &Service) -> Vec<(i32, String, i32)> {
    service.spec.as_ref().unwrap().ports.iter().flatten().map(|p| {
        let IntOrString::Int(target) = p.target_port.clone().unwrap() else { panic!() };
        (p.port, p.protocol.clone().unwrap(), target)
    }).collect()
}

#[test]
fn each_gateway_gets_a_service_with_one_port_per_listener_port() {
    let a = gateway("a", serde_json::json!([
        {"name": "http", "port": 80, "protocol": "HTTP"},
        {"name": "http-2", "port": 80, "protocol": "HTTP", "hostname": "x.example.com"},
        {"name": "dns", "port": 53, "protocol": "UDP"}
    ]), serde_json::json!({}));
    let b = gateway("b", serde_json::json!([{"name": "http", "port": 80, "protocol": "HTTP"}]), serde_json::json!({}));
    let snapshot = Snapshot::default();
    let services = desired(&snapshot, &[&a, &b], &ServiceSettings::default());
    let service_a = &services[&("ns".to_string(), "rws-a".to_string())];
    assert_eq!(vec![(53, "UDP".to_string(), 10000), (80, "TCP".to_string(), 10001)], ports(service_a));
    let service_b = &services[&("ns".to_string(), "rws-b".to_string())];
    assert_eq!(vec![(80, "TCP".to_string(), 10002)], ports(service_b), "container ports are unique across Gateways: the pods are shared");
    let owner = &service_a.metadata.owner_references.as_ref().unwrap()[0];
    assert_eq!(("Gateway", "a", Some(true)), (owner.kind.as_str(), owner.name.as_str(), owner.controller));
}

#[test]
fn allocations_are_kept_across_reconciles() {
    let mut a = gateway("a", serde_json::json!([{"name": "http", "port": 80, "protocol": "HTTP"}]), serde_json::json!({}));
    let mut snapshot = Snapshot::default();
    let first = desired(&snapshot, &[&a], &ServiceSettings::default()).into_values().next().unwrap();
    snapshot.services.push(first);
    // A new listener: 80 keeps its port, 443 gets the next free one.
    a.spec.listeners.push(serde_json::from_value(serde_json::json!({"name": "https", "port": 443, "protocol": "HTTPS"})).unwrap());
    let second = desired(&snapshot, &[&a], &ServiceSettings::default()).into_values().next().unwrap();
    assert_eq!(vec![(80, "TCP".to_string(), 10000), (443, "TCP".to_string(), 10001)], ports(&second));
    assert!(!same(&snapshot.services[0], &second));
    assert!(same(&second, &second));
}

#[test]
fn infrastructure_labels_and_annotations_reach_the_service() {
    let a = gateway("a", serde_json::json!([{"name": "http", "port": 80, "protocol": "HTTP"}]),
        serde_json::json!({"infrastructure": {"labels": {"team": "web"}, "annotations": {"lb.example/x": "1"}}}));
    let service = desired(&Snapshot::default(), &[&a], &ServiceSettings::default()).into_values().next().unwrap();
    assert_eq!(Some("web"), service.labels().get("team").map(String::as_str));
    assert_eq!(Some("1"), service.annotations().get("lb.example/x").map(String::as_str));
    assert_eq!(Some("rws-gateway"), service.labels().get("app.kubernetes.io/managed-by").map(String::as_str));
}

#[test]
fn an_exhausted_range_leaves_the_listener_out() {
    let a = gateway("a", serde_json::json!([{"name": "x", "port": 80, "protocol": "HTTP"}, {"name": "y", "port": 81, "protocol": "HTTP"}]), serde_json::json!({}));
    let settings = ServiceSettings { port_range: (10000, 10000), ..Default::default() };
    let service = desired(&Snapshot::default(), &[&a], &settings).into_values().next().unwrap();
    assert_eq!(1, ports(&service).len());
}

#[test]
fn service_names_fit_in_63_characters() {
    assert_eq!("rws-web", service_name("web"));
    let long = "g".repeat(80);
    let name = service_name(&long);
    assert!(name.len() <= 63 && name.starts_with("rws-g"), "{name}");
    assert_ne!(name, service_name(&"g".repeat(81)), "different Gateways, different names");
}

#[test]
fn each_gateway_service_gets_an_endpoint_slice_of_the_serving_pods() {
    let pod = |name: &str, namespace: &str, app: &str, ip: &str, ready: bool| -> k8s_openapi::api::core::v1::Pod {
        serde_json::from_value(serde_json::json!({
            "metadata": {"name": name, "namespace": namespace, "labels": {"app.kubernetes.io/name": app}},
            "spec": {"containers": [], "nodeName": "node-1"},
            "status": {"podIP": ip, "conditions": [{"type": "Ready", "status": if ready { "True" } else { "False" }}]}
        }))
        .unwrap()
    };
    let settings = ServiceSettings { pod_namespace: "rws-system".to_string(), ..Default::default() };
    let snapshot = Snapshot {
        pods: vec![
            pod("rws-a", "rws-system", "rws", "10.244.0.6", true),
            pod("rws-b", "rws-system", "rws", "10.244.0.7", false),
            pod("rws-c", "rws-system", "rws", "fd00::7", true),
            pod("other", "rws-system", "nginx", "10.244.0.8", true),
            pod("elsewhere", "default", "rws", "10.244.0.9", true),
        ],
        ..Default::default()
    };
    let service: Service = serde_json::from_value(serde_json::json!({
        "metadata": {"name": "rws-gw", "namespace": "apps"},
        "spec": {"ports": [{"name": "tcp-80", "port": 80, "protocol": "TCP", "targetPort": 10000}]}
    }))
    .unwrap();
    let slices = endpoint_slices(&snapshot, &BTreeMap::from([(("apps".to_string(), "rws-gw".to_string()), service)]), &settings);
    let v4 = &slices[&("apps".to_string(), "rws-gw-ipv4".to_string())];
    assert_eq!(Some("rws-gw"), v4.labels().get("kubernetes.io/service-name").map(String::as_str));
    assert_eq!(Some(ENDPOINTS_MANAGED_BY), v4.labels().get("endpointslice.kubernetes.io/managed-by").map(String::as_str));
    let endpoints: Vec<(String, Option<bool>)> = v4.endpoints.iter().map(|e| (e.addresses[0].clone(), e.conditions.as_ref().and_then(|c| c.ready))).collect();
    assert_eq!(vec![("10.244.0.6".to_string(), Some(true)), ("10.244.0.7".to_string(), Some(false))], endpoints, "only rws's pods, in its namespace");
    assert_eq!(Some(10000), v4.ports.as_ref().unwrap()[0].port, "the container port");
    assert_eq!(vec!["fd00::7".to_string()], slices[&("apps".to_string(), "rws-gw-ipv6".to_string())].endpoints[0].addresses);
    assert!(same_slice(v4, v4));
    let mut changed = v4.clone();
    changed.endpoints.pop();
    assert!(!same_slice(v4, &changed));
}

#[test]
fn provisioned_gateways_list_only_their_own_pods() {
    let pod = |name: &str, gateway: &str| -> k8s_openapi::api::core::v1::Pod {
        let mut labels = super::super::provision::pod_labels("apps", gateway);
        labels.insert("x".to_string(), "y".to_string());
        serde_json::from_value(serde_json::json!({
            "metadata": {"name": name, "namespace": "rws-system", "labels": labels},
            "status": {"podIP": format!("10.244.1.{}", name.len()), "conditions": [{"type": "Ready", "status": "True"}]}
        }))
        .unwrap()
    };
    let settings = ServiceSettings { pod_namespace: "rws-system".to_string(), provisioned: true, ..Default::default() };
    let snapshot = Snapshot { pods: vec![pod("web-a", "web"), pod("api-bb", "api")], ..Default::default() };
    let service: Service = serde_json::from_value(serde_json::json!({
        "metadata": {"name": "rws-web", "namespace": "apps", "labels": {GATEWAY_NAME_LABEL: "web"}},
        "spec": {"ports": [{"name": "tcp-80", "port": 80, "protocol": "TCP", "targetPort": 10000}]}
    }))
    .unwrap();
    let slices = endpoint_slices(&snapshot, &BTreeMap::from([(("apps".to_string(), "rws-web".to_string()), service)]), &settings);
    let addresses: Vec<&str> = slices[&("apps".to_string(), "rws-web-ipv4".to_string())].endpoints.iter().map(|e| e.addresses[0].as_str()).collect();
    assert_eq!(vec!["10.244.1.5"], addresses, "web's pod only");
    assert_eq!(Some("rws-gateway"), settings.pod_watch_labels().get("app.kubernetes.io/name").map(String::as_str));
}
