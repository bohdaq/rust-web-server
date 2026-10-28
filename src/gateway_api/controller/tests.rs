//! The controller loop against a fake API server: kube's `Client` takes any
//! tower `Service`, so list requests are answered from the conformance
//! fixtures, watches end at once, and status patches are recorded.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};

use super::*;
use crate::gateway_api::types::RouteStatus;

const INFRA: &str = "gateway-conformance-infra";

fn fixture(name: &str) -> Vec<serde_json::Value> {
    let path = format!("{}/src/gateway_api/testdata/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// Objects by list path; recorded `PATCH`es as (path, body).
#[derive(Clone, Default)]
struct FakeApi {
    lists: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
    patches: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl FakeApi {
    fn add(&self, mut object: serde_json::Value) {
        // What the API server stamps on every object.
        object["metadata"]["resourceVersion"] = serde_json::json!("1");
        object["metadata"]["generation"] = serde_json::json!(1);
        let path = match object["kind"].as_str().unwrap() {
            "GatewayClass" => "/apis/gateway.networking.k8s.io/v1/gatewayclasses",
            "Gateway" => "/apis/gateway.networking.k8s.io/v1/gateways",
            "HTTPRoute" => "/apis/gateway.networking.k8s.io/v1/httproutes",
            "ReferenceGrant" => "/apis/gateway.networking.k8s.io/v1beta1/referencegrants",
            "Service" => "/api/v1/services",
            "Namespace" => "/api/v1/namespaces",
            "Secret" => "/api/v1/secrets",
            "EndpointSlice" => "/apis/discovery.k8s.io/v1/endpointslices",
            other => panic!("{other}"),
        };
        self.lists.lock().unwrap().push((path.to_string(), object));
    }

    fn respond(&self, request: http::Request<kube::client::Body>, body: Vec<u8>) -> http::Response<Full<Bytes>> {
        let path = request.uri().path().to_string();
        let query = request.uri().query().unwrap_or("").to_string();
        let json = |value: serde_json::Value| http::Response::new(Full::new(Bytes::from(value.to_string())));
        if request.method() == http::Method::PATCH {
            let patch: serde_json::Value = serde_json::from_slice(&body).unwrap();
            self.patches.lock().unwrap().push((path.clone(), patch.clone()));
            // Answer with the object as patched, as the API server does.
            let base = path.trim_end_matches("/status");
            let name = base.rsplit('/').next().unwrap();
            let found = self.lists.lock().unwrap().iter().map(|(_, o)| o.clone()).find(|o| o["metadata"]["name"] == name);
            // A server-side apply may create the object: answer with the body.
            let Some(mut object) = found else { return json(patch) };
            if path.ends_with("/status") {
                object["status"] = patch["status"].clone();
            }
            return json(object);
        }
        if query.contains("watch=true") {
            // End the watch at once; the watcher reconnects with backoff.
            return http::Response::new(Full::new(Bytes::new()));
        }
        let items: Vec<serde_json::Value> = self.lists.lock().unwrap().iter().filter(|(p, _)| *p == path).map(|(_, o)| o.clone()).collect();
        json(serde_json::json!({"apiVersion": "v1", "kind": "List", "metadata": {"resourceVersion": "1"}, "items": items}))
    }

    fn client(&self) -> Client {
        let fake = self.clone();
        let service = tower::service_fn(move |request: http::Request<kube::client::Body>| {
            let fake = fake.clone();
            async move {
                let (parts, body) = request.into_parts();
                let bytes = body.collect().await.map(|b| b.to_bytes().to_vec()).unwrap_or_default();
                Ok::<_, std::convert::Infallible>(fake.respond(http::Request::from_parts(parts, kube::client::Body::empty()), bytes))
            }
        });
        Client::new(service, "default")
    }
}

fn fake_with(fixtures: &[&str]) -> FakeApi {
    let fake = FakeApi::default();
    fake.add(serde_json::json!({
        "apiVersion": "gateway.networking.k8s.io/v1", "kind": "GatewayClass",
        "metadata": {"name": "rws", "generation": 1, "resourceVersion": "1"},
        "spec": {"controllerName": crate::gateway_api::CONTROLLER_NAME}
    }));
    for doc in std::iter::once("base").chain(fixtures.iter().copied()).flat_map(fixture) {
        fake.add(doc);
    }
    fake
}

fn options(write_status: bool, configs: Arc<Mutex<Vec<GatewayConfig>>>) -> Options {
    Options {
        settings: Default::default(),
        leader: Arc::new(std::sync::atomic::AtomicBool::new(write_status)),
        on_config: Arc::new(move |config| configs.lock().unwrap().push(config)),
    }
}

async fn ready(stores: &Stores) {
    stores.classes.wait_until_ready().await.unwrap();
    stores.gateways.wait_until_ready().await.unwrap();
    stores.http_routes.wait_until_ready().await.unwrap();
    stores.reference_grants.wait_until_ready().await.unwrap();
    stores.services.wait_until_ready().await.unwrap();
    stores.namespaces.wait_until_ready().await.unwrap();
    stores.secrets.wait_until_ready().await.unwrap();
    stores.endpoint_slices.wait_until_ready().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reconcile_writes_class_gateway_and_route_status() {
    let fake = fake_with(&["httproute-simple-same-namespace"]);
    let client = fake.client();
    let (stores, _changed) = start_watches(&client, &Default::default());
    tokio::time::timeout(Duration::from_secs(10), ready(&stores)).await.expect("stores filled from the lists");
    let configs = Arc::new(Mutex::new(Vec::new()));
    reconcile(&client, &stores, &options(true, Arc::clone(&configs))).await.unwrap();

    let patches = fake.patches.lock().unwrap().clone();
    let patch_for = |suffix: &str| patches.iter().find(|(p, _)| p.ends_with(suffix)).unwrap_or_else(|| panic!("no PATCH to …{suffix}: {patches:?}")).1.clone();
    let class = patch_for("/gatewayclasses/rws/status");
    assert_eq!("Accepted", class["status"]["conditions"][0]["type"]);
    assert_eq!("True", class["status"]["conditions"][0]["status"]);
    let gateway = patch_for(&format!("/namespaces/{INFRA}/gateways/same-namespace/status"));
    assert_eq!(1, gateway["status"]["listeners"][0]["attachedRoutes"]);
    let route = patch_for(&format!("/namespaces/{INFRA}/httproutes/gateway-conformance-infra-test/status"));
    assert_eq!(crate::gateway_api::CONTROLLER_NAME, route["status"]["parents"][0]["controllerName"]);
    assert_eq!(1, route["status"]["parents"][0]["conditions"][0]["observedGeneration"], "{route}");
    assert!(route["metadata"]["resourceVersion"].is_string(), "optimistic concurrency on the shared parents list: {route}");
    assert_eq!(1, configs.lock().unwrap().len(), "the data plane got the configuration");
    let service = patch_for(&format!("/namespaces/{INFRA}/services/rws-same-namespace"));
    assert_eq!("LoadBalancer", service["spec"]["type"], "{service}");
    assert_eq!("same-namespace", service["metadata"]["labels"]["gateway.networking.k8s.io/gateway-name"]);
    assert!((10000..=10999).contains(&service["spec"]["ports"][0]["targetPort"].as_i64().unwrap()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_follower_serves_without_writing_status() {
    let fake = fake_with(&["httproute-simple-same-namespace"]);
    let client = fake.client();
    let (stores, _changed) = start_watches(&client, &Default::default());
    tokio::time::timeout(Duration::from_secs(10), ready(&stores)).await.unwrap();
    let configs = Arc::new(Mutex::new(Vec::new()));
    reconcile(&client, &stores, &options(false, Arc::clone(&configs))).await.unwrap();
    assert!(fake.patches.lock().unwrap().is_empty());
    assert_eq!(1, configs.lock().unwrap().len());
}

#[test]
fn other_controllers_route_parents_are_kept() {
    let entry = |controller: &str, gateway: &str| RouteParentStatus {
        parent_ref: crate::gateway_api::types::ParentReference { name: gateway.to_string(), ..Default::default() },
        controller_name: controller.to_string(),
        conditions: Vec::new(),
    };
    let current = vec![entry("example.com/other", "theirs"), entry("rws", "stale")];
    let ours = vec![entry("rws", "fresh")];
    let merged = merge_parents(&current, &ours, "rws");
    let names: Vec<(&str, &str)> = merged.iter().map(|p| (p.controller_name.as_str(), p.parent_ref.name.as_str())).collect();
    assert_eq!(vec![("example.com/other", "theirs"), ("rws", "fresh")], names);
    let _ = RouteStatus { parents: merged };
}
