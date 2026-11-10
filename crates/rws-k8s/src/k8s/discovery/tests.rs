use k8s_openapi::api::discovery::v1::{Endpoint, EndpointConditions, EndpointPort, EndpointSlice};

use super::*;

fn endpoint(ip: &str, ready: Option<bool>) -> Endpoint {
    Endpoint { addresses: vec![ip.to_string()], conditions: Some(EndpointConditions { ready, ..Default::default() }), ..Default::default() }
}

fn slice(kind: &str, ports: &[(&str, i32)], endpoints: Vec<Endpoint>) -> EndpointSlice {
    EndpointSlice {
        address_type: kind.to_string(),
        endpoints,
        ports: Some(ports.iter().map(|(n, p)| EndpointPort { name: Some(n.to_string()), port: Some(*p), ..Default::default() }).collect()),
        metadata: Default::default(),
    }
}

#[test]
fn ready_endpoints_with_the_named_port() {
    let a = slice("IPv4", &[("metrics", 9090), ("http", 8080)], vec![endpoint("10.0.0.2", Some(true)), endpoint("10.0.0.1", None), endpoint("10.0.0.3", Some(false))]);
    let b = slice("IPv6", &[("http", 8080)], vec![endpoint("fd00::1", Some(true))]);
    let dup = slice("IPv4", &[("http", 8080)], vec![endpoint("10.0.0.1", Some(true))]);
    assert_eq!(vec!["10.0.0.1:8080", "10.0.0.2:8080", "[fd00::1]:8080"], ready_addresses([&a, &b, &dup], Some("http")));
    assert_eq!(vec!["10.0.0.1:9090", "10.0.0.2:9090"], ready_addresses([&a], None), "the first port without a name");
    assert!(ready_addresses([&a], Some("grpc")).is_empty());
    assert!(ready_addresses([&slice("IPv4", &[], vec![endpoint("10.0.0.9", None)])], None).is_empty());
}
