use super::*;
use serde_json::json;

const MESH: &str = "gateway-conformance-mesh";
const CONSUMER: &str = "gateway-conformance-mesh-consumer";

fn service(namespace: &str, name: &str, ip: &str, ports: &[i32]) -> Service {
    serde_json::from_value(json!({
        "apiVersion": "v1", "kind": "Service",
        "metadata": {"name": name, "namespace": namespace},
        "spec": {"clusterIP": ip, "clusterIPs": [ip], "ports": ports.iter().map(|p| json!({"name": format!("p{p}"), "port": p})).collect::<Vec<_>>()}
    }))
    .unwrap()
}

fn slice(namespace: &str, service: &str, address: &str, ports: &[i32]) -> EndpointSlice {
    serde_json::from_value(json!({
        "apiVersion": "discovery.k8s.io/v1", "kind": "EndpointSlice",
        "metadata": {"name": format!("{service}-x"), "namespace": namespace, "labels": {"kubernetes.io/service-name": service}},
        "addressType": "IPv4",
        "endpoints": [{"addresses": [address], "conditions": {"ready": true}}],
        "ports": ports.iter().map(|p| json!({"name": format!("p{p}"), "port": p})).collect::<Vec<_>>()
    }))
    .unwrap()
}

/// An HTTPRoute in `namespace` with one parentRef to a Service, adding
/// `X-Header-Set: <value>` to responses and sending to `backend:80`.
fn route(namespace: &str, name: &str, parent: (&str, &str, Option<i32>), value: &str, backend: (&str, &str)) -> HttpRoute {
    let mut parent_ref = json!({"group": "", "kind": "Service", "name": parent.1, "namespace": parent.0});
    if let Some(port) = parent.2 {
        parent_ref["port"] = json!(port);
    }
    serde_json::from_value(json!({
        "apiVersion": "gateway.networking.k8s.io/v1", "kind": "HTTPRoute",
        "metadata": {"name": name, "namespace": namespace},
        "spec": {
            "parentRefs": [parent_ref],
            "rules": [{
                "filters": [{"type": "ResponseHeaderModifier", "responseHeaderModifier": {"set": [{"name": "X-Header-Set", "value": value}]}}],
                "backendRefs": [{"name": backend.1, "namespace": backend.0, "port": 80}]
            }]
        }
    }))
    .unwrap()
}

fn suite_snapshot() -> MeshSnapshot {
    MeshSnapshot {
        services: vec![service(MESH, "echo-v1", "10.96.0.1", &[80, 8080]), service(MESH, "echo-v2", "10.96.0.2", &[80, 8080])],
        endpoint_slices: vec![slice(MESH, "echo-v1", "10.244.0.1", &[80, 8080]), slice(MESH, "echo-v2", "10.244.0.2", &[80, 8080])],
        ..Default::default()
    }
}

fn header_value(table: &MeshTable, ip: &str, port: u16) -> Option<String> {
    let listeners = table.get(&(ip.to_string(), port))?;
    let filter = &listeners[0].http_routes[0].rules[0].filters[0];
    Some(filter.response_header_modifier.as_ref()?.set[0].value.clone())
}

#[test]
fn routes_apply_to_their_services_cluster_ip_only() {
    let mut mesh = suite_snapshot();
    mesh.http_routes.push(route(MESH, "r", (MESH, "echo-v2", None), "set", (MESH, "echo-v2")));
    let table = table(&mesh, MESH);
    assert_eq!(Some("set".to_string()), header_value(&table, "10.96.0.2", 80));
    assert_eq!(Some("set".to_string()), header_value(&table, "10.96.0.2", 8080), "no port: every Service port");
    assert_eq!(None, header_value(&table, "10.96.0.1", 80), "another Service");
    assert_eq!(None, header_value(&table, "10.244.0.2", 8080), "a pod IP");
    assert!(table[&("10.96.0.2".to_string(), 80)][0].http_routes[0].hostnames.is_empty());
}

#[test]
fn a_parent_port_limits_the_route() {
    let mut mesh = suite_snapshot();
    mesh.http_routes.push(route(MESH, "r", (MESH, "echo-v1", Some(80)), "v1", (MESH, "echo-v1")));
    let table = table(&mesh, MESH);
    assert_eq!(Some("v1".to_string()), header_value(&table, "10.96.0.1", 80));
    assert_eq!(None, header_value(&table, "10.96.0.1", 8080), "the excluded port goes through unchanged");
}

#[test]
fn consumer_routes_apply_only_to_their_namespace_and_win_there() {
    let mut mesh = suite_snapshot();
    mesh.http_routes.push(route(CONSUMER, "consumer", (MESH, "echo-v1", None), "consumer", (MESH, "echo-v1")));
    assert_eq!(Some("consumer".to_string()), header_value(&table(&mesh, CONSUMER), "10.96.0.1", 80));
    assert_eq!(None, header_value(&table(&mesh, MESH), "10.96.0.1", 80), "the producer's clients don't see it");
    mesh.http_routes.push(route(MESH, "producer", (MESH, "echo-v1", None), "producer", (MESH, "echo-v1")));
    assert_eq!(Some("consumer".to_string()), header_value(&table(&mesh, CONSUMER), "10.96.0.1", 80));
    assert_eq!(Some("producer".to_string()), header_value(&table(&mesh, MESH), "10.96.0.1", 80));
    assert_eq!(Some("producer".to_string()), header_value(&table(&mesh, "elsewhere"), "10.96.0.1", 80));
}

#[test]
fn a_consumer_route_reaches_its_parent_service_without_a_grant() {
    let mut mesh = suite_snapshot();
    mesh.http_routes.push(route(CONSUMER, "consumer", (MESH, "echo-v1", None), "c", (MESH, "echo-v1")));
    let table = table(&mesh, CONSUMER);
    let backend = &table[&("10.96.0.1".to_string(), 80)][0].http_routes[0].rules[0].backends[0];
    assert_eq!(Ok(vec!["10.244.0.1:80".to_string()]), backend.endpoints);
    // But not another Service in that namespace.
    let mut other = suite_snapshot();
    other.http_routes.push(route(CONSUMER, "consumer", (MESH, "echo-v1", None), "c", (MESH, "echo-v2")));
    let table = super::table(&other, CONSUMER);
    assert!(table[&("10.96.0.1".to_string(), 80)][0].http_routes[0].rules[0].backends[0].endpoints.is_err());
}

#[test]
fn headless_services_have_no_destination() {
    let mut mesh = suite_snapshot();
    mesh.services.push(service(MESH, "headless", "None", &[80]));
    mesh.http_routes.push(route(MESH, "r", (MESH, "headless", None), "x", (MESH, "echo-v1")));
    assert!(table(&mesh, MESH).keys().all(|(ip, _)| ip != "None"));
}

#[test]
fn the_snapshot_keeps_mesh_routes_and_the_services_they_name() {
    let mut from = Snapshot::default();
    let mesh = suite_snapshot();
    from.services = mesh.services.clone();
    from.services.push(service("other", "unrelated", "10.96.9.9", &[80]));
    from.endpoint_slices = mesh.endpoint_slices.clone();
    from.http_routes.push(route(MESH, "r", (MESH, "echo-v2", None), "set", (MESH, "echo-v1")));
    let mut gateway_route = route(MESH, "g", (MESH, "echo-v2", None), "set", (MESH, "echo-v1"));
    gateway_route.spec.parent_refs = vec![ParentReference { name: "gw".to_string(), ..Default::default() }];
    from.http_routes.push(gateway_route);
    let taken = snapshot(&from);
    assert_eq!(vec!["r"], taken.http_routes.iter().map(|r| r.name_any()).collect::<Vec<_>>());
    let mut services: Vec<String> = taken.services.iter().map(|s| s.name_any()).collect();
    services.sort();
    assert_eq!(vec!["echo-v1", "echo-v2"], services);
    assert_eq!(2, taken.endpoint_slices.len());
    // It survives the trip to a sidecar.
    let back: MeshSnapshot = serde_json::from_str(&serde_json::to_string(&taken).unwrap()).unwrap();
    assert_eq!(table(&taken, MESH).len(), table(&back, MESH).len());
}

#[test]
fn service_parents_are_accepted_when_the_service_and_port_exist() {
    let services = suite_snapshot().services;
    let parent = |name: &str, port: Option<i32>| ParentReference { group: Some(String::new()), kind: Some("Service".to_string()), name: name.to_string(), port, ..Default::default() };
    assert_eq!((true, "Accepted"), {
        let (a, r, _) = parent_acceptance(&parent("echo-v1", Some(80)), MESH, &services);
        (a, r)
    });
    assert!(!parent_acceptance(&parent("missing", None), MESH, &services).0);
    assert!(!parent_acceptance(&parent("echo-v1", Some(9999)), MESH, &services).0);
    assert!(is_service_parent(&parent("x", None)));
    assert!(!is_service_parent(&ParentReference { name: "gw".to_string(), ..Default::default() }));
}

#[test]
fn http_is_told_from_other_protocols_by_its_first_bytes() {
    use sidecar::looks_like_http;
    assert_eq!(Some(true), looks_like_http(b"GET / HTTP/1.1\r\n"));
    assert_eq!(Some(true), looks_like_http(b"PRI * HTTP/2.0\r\n"));
    assert_eq!(Some(true), looks_like_http(b"PRI "));
    assert_eq!(None, looks_like_http(b"GE"));
    assert_eq!(Some(false), looks_like_http(&[0x16, 0x03, 0x01]), "a TLS ClientHello");
    assert_eq!(Some(false), looks_like_http(b" GET"));
    assert_eq!(Some(false), looks_like_http(b"get / HTTP/1.1"));
}

#[test]
fn the_requested_version_is_read_from_the_query() {
    assert_eq!(Some(7), config::requested_version("/mesh/v1/snapshot?version=7"));
    assert_eq!(Some(0), config::requested_version("/mesh/v1/snapshot?a=b&version=0"));
    assert_eq!(None, config::requested_version("/mesh/v1/snapshot"));
}

fn get(uri: &str) -> crate::request::Request {
    crate::request::Request { method: "GET".to_string(), request_uri: uri.to_string(), http_version: "HTTP/1.1".to_string(), headers: vec![], body: Default::default() }
}

fn connection() -> crate::server::ConnectionInfo {
    crate::server::ConnectionInfo {
        client: crate::server::Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: crate::server::Address { ip: "127.0.0.1".to_string(), port: 2 },
        request_size: 0,
        sni_hostname: None,
        tls: false,
    }
}

#[tokio::test]
async fn the_endpoint_serves_versions_and_waits_for_the_next() {
    use crate::application::Application;
    let publisher = config::Publisher::new();
    let c = connection();
    let first = publisher.execute(&get(config::PATH), &c).unwrap();
    assert_eq!(503, first.status_code, "nothing published yet");
    publisher.publish(&suite_snapshot());
    let first = publisher.current().0;
    publisher.publish(&suite_snapshot());
    assert_eq!(first, publisher.current().0, "the same snapshot keeps its version");
    let other = config::Publisher::new();
    other.publish(&suite_snapshot());
    assert_eq!(first, other.current().0, "and another replica gives it the same one");
    let request = get(&format!("{}?version={first}", config::PATH));
    let waiting = {
        let publisher = publisher.clone();
        tokio::spawn(async move {
            let c = connection();
            let response = publisher.execute_async(&request, &c).unwrap().await.unwrap();
            response.headers.iter().find(|h| h.name.eq_ignore_ascii_case(config::VERSION_HEADER)).map(|h| h.value.to_string())
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!waiting.is_finished(), "waits while that version is current");
    let mut next = suite_snapshot();
    next.http_routes.push(route(MESH, "r", (MESH, "echo-v2", None), "set", (MESH, "echo-v2")));
    publisher.publish(&next);
    let announced = waiting.await.unwrap().unwrap();
    assert_ne!(first.to_string(), announced);
    assert_eq!(publisher.current().0.to_string(), announced);
    let other = publisher.execute(&get("/other"), &c).unwrap();
    assert_eq!(404, other.status_code);
}

/// A backend answering every request with `200` and the path it saw.
fn path_echo() -> u16 {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buffer = [0u8; 4096];
                let n = stream.read(&mut buffer).unwrap_or(0);
                let head = String::from_utf8_lossy(&buffer[..n]).to_string();
                let path = head.split(' ').nth(1).unwrap_or("").to_string();
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{path}", path.len());
            });
        }
    });
    port
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sidecar_router_matches_filters_and_forwards() {
    use crate::application::Application;
    let backend = path_echo();
    let mut mesh = MeshSnapshot {
        services: vec![service(MESH, "echo", "10.96.0.3", &[80]), service(MESH, "echo-v1", "10.96.0.1", &[80])],
        endpoint_slices: vec![slice(MESH, "echo-v1", "127.0.0.1", &[backend as i32])],
        ..Default::default()
    };
    // Port 80 of echo-v1 maps to the backend's port by the slice's port name.
    mesh.services[1] = serde_json::from_value(json!({
        "apiVersion": "v1", "kind": "Service",
        "metadata": {"name": "echo-v1", "namespace": MESH},
        "spec": {"clusterIP": "10.96.0.1", "ports": [{"name": "p80", "port": 80, "targetPort": backend}]}
    }))
    .unwrap();
    mesh.endpoint_slices[0] = serde_json::from_value(json!({
        "apiVersion": "discovery.k8s.io/v1", "kind": "EndpointSlice",
        "metadata": {"name": "echo-v1-x", "namespace": MESH, "labels": {"kubernetes.io/service-name": "echo-v1"}},
        "addressType": "IPv4",
        "endpoints": [{"addresses": ["127.0.0.1"], "conditions": {"ready": true}}],
        "ports": [{"name": "p80", "port": backend}]
    }))
    .unwrap();
    let mut split = route(MESH, "split", (MESH, "echo", None), "split", (MESH, "echo-v1"));
    split.spec.rules[0].matches = vec![HttpRouteMatch { path: Some(HttpPathMatch { match_type: Some("Exact".to_string()), value: Some("/v1".to_string()) }), ..Default::default() }];
    mesh.http_routes.push(split);
    let routers = sidecar::build_routers(&mesh, MESH);
    let router = routers[&("10.96.0.3".to_string(), 80)].clone();
    let (matched, unmatched) = tokio::task::spawn_blocking(move || {
        let c = connection();
        let mut request = get("/v1");
        request.headers.push(crate::header::Header::new("Host", "echo"));
        let matched = router.execute(&request, &c).unwrap();
        let mut request = get("/v2");
        request.headers.push(crate::header::Header::new("Host", "echo"));
        (matched, router.execute(&request, &c).unwrap())
    })
    .await
    .unwrap();
    assert_eq!(200, matched.status_code);
    assert!(matched.headers.iter().any(|h| h.name == "X-Header-Set" && h.value == "split"));
    assert_eq!(404, unmatched.status_code, "a Service with routes answers 404 when no rule matches");
}

fn inject_settings() -> inject::InjectSettings {
    inject::InjectSettings { sidecar_image: "rws:test".to_string(), init_image: "rws-mesh-init:test".to_string(), config_url: "http://rws-mesh.rws-system:15010".to_string() }
}

/// `pod` with the JSON patch applied (only the `add`s injection makes).
fn applied(mut pod: serde_json::Value, ops: &[serde_json::Value]) -> serde_json::Value {
    for op in ops {
        assert_eq!("add", op["op"]);
        let path = op["path"].as_str().unwrap();
        let (parent, last) = path.rsplit_once('/').unwrap();
        let last = last.replace("~1", "/").replace("~0", "~");
        let target = pod.pointer_mut(parent).unwrap();
        match target {
            serde_json::Value::Array(items) => items.insert(last.parse().unwrap(), op["value"].clone()),
            serde_json::Value::Object(map) => {
                map.insert(last, op["value"].clone());
            }
            _ => panic!("can't add at {path}"),
        }
    }
    pod
}

#[test]
fn injection_puts_the_init_container_then_the_sidecar_first() {
    let settings = inject_settings();
    let bare = json!({"metadata": {"name": "p"}, "spec": {"containers": [{"name": "app"}]}});
    let pod = applied(bare.clone(), &inject::patch(&bare, &settings).unwrap());
    let init = pod["spec"]["initContainers"].as_array().unwrap();
    assert_eq!(vec!["rws-mesh-init", "rws-mesh"], init.iter().map(|c| c["name"].as_str().unwrap()).collect::<Vec<_>>());
    assert_eq!("Always", init[1]["restartPolicy"], "a native sidecar");
    assert_eq!(1337, init[1]["securityContext"]["runAsUser"]);
    assert_eq!("rws:test", init[1]["image"]);
    assert_eq!("true", pod["metadata"]["annotations"]["mesh.rws8.tech/injected"]);
    let env = init[1]["env"].as_array().unwrap();
    assert!(env.iter().any(|e| e["name"] == "RWS_CONFIG_GATEWAY_API_MODE" && e["value"] == "mesh-sidecar"));
    assert!(init[0]["securityContext"]["capabilities"]["add"].as_array().unwrap().contains(&json!("NET_ADMIN")));

    // Existing init containers and annotations are kept, after ours.
    let busy = json!({"metadata": {"annotations": {"a": "b"}}, "spec": {"initContainers": [{"name": "migrate"}], "containers": [{"name": "app"}]}});
    let pod = applied(busy.clone(), &inject::patch(&busy, &settings).unwrap());
    let names: Vec<&str> = pod["spec"]["initContainers"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(vec!["rws-mesh-init", "rws-mesh", "migrate"], names);
    assert_eq!("b", pod["metadata"]["annotations"]["a"]);
    assert_eq!("true", pod["metadata"]["annotations"]["mesh.rws8.tech/injected"]);
}

#[test]
fn pods_that_opt_out_use_the_host_network_or_have_the_sidecar_are_left_alone() {
    let settings = inject_settings();
    assert!(inject::patch(&json!({"metadata": {"annotations": {"mesh.rws8.tech/inject": "false"}}, "spec": {}}), &settings).is_none());
    assert!(inject::patch(&json!({"metadata": {}, "spec": {"hostNetwork": true}}), &settings).is_none());
    assert!(inject::patch(&json!({"metadata": {}, "spec": {"initContainers": [{"name": "rws-mesh"}]}}), &settings).is_none());
}

#[test]
fn the_webhook_answers_admission_reviews() {
    let settings = inject_settings();
    let body = |kind: &str, operation: &str| {
        serde_json::to_vec(&json!({
            "apiVersion": "admission.k8s.io/v1", "kind": "AdmissionReview",
            "request": {"uid": "u-1", "kind": {"group": "", "version": "v1", "kind": kind}, "operation": operation,
                        "object": {"metadata": {"name": "p"}, "spec": {"containers": [{"name": "app"}]}}}
        }))
        .unwrap()
    };
    let answer: serde_json::Value = serde_json::from_slice(&inject::review(&body("Pod", "CREATE"), &settings).unwrap()).unwrap();
    assert_eq!("u-1", answer["response"]["uid"]);
    assert_eq!(true, answer["response"]["allowed"]);
    assert_eq!("JSONPatch", answer["response"]["patchType"]);
    let patch = crate::core::base64::Base64::decode(answer["response"]["patch"].as_str().unwrap().to_string()).unwrap();
    let ops: Vec<serde_json::Value> = serde_json::from_slice(&patch).unwrap();
    assert!(!ops.is_empty());
    // Anything else is allowed unchanged.
    let update: serde_json::Value = serde_json::from_slice(&inject::review(&body("Pod", "UPDATE"), &settings).unwrap()).unwrap();
    assert_eq!(true, update["response"]["allowed"]);
    assert!(update["response"].get("patch").is_none());
    assert!(inject::review(b"not json", &settings).is_err());
}

#[test]
fn status_and_bookkeeping_dont_change_the_snapshot() {
    let mut from = Snapshot::default();
    let mesh = suite_snapshot();
    from.services = mesh.services.clone();
    from.endpoint_slices = mesh.endpoint_slices.clone();
    from.http_routes.push(route(MESH, "r", (MESH, "echo-v2", None), "set", (MESH, "echo-v1")));
    let before = serde_json::to_string(&snapshot(&from)).unwrap();
    from.http_routes[0].metadata.resource_version = Some("42".to_string());
    from.http_routes[0].status = Some(RouteStatus { parents: vec![RouteParentStatus { parent_ref: ParentReference { name: "x".to_string(), ..Default::default() }, controller_name: "rws".to_string(), conditions: vec![] }] });
    let after = serde_json::to_string(&snapshot(&from)).unwrap();
    assert_eq!(before, after, "the leader's status writes don't wake every sidecar");
}
