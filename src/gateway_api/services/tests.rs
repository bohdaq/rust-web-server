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
