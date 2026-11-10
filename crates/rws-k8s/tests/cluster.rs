//! Against a real API server (`RWS_TEST_KUBECONFIG`; skipped without it).
//! CI runs them on kind; locally a k3d or kind cluster does. Each test works
//! in a namespace of its own; cluster-wide objects get unique names.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use k8s_openapi::api::core::v1::{ConfigMap, Secret, Service, ServicePort, ServiceSpec};
use k8s_openapi::api::discovery::v1::{Endpoint, EndpointConditions, EndpointPort, EndpointSlice};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, DeleteParams, Patch, PatchParams, PostParams};
use kube::{CustomResource, ResourceExt};
use rws_k8s::k8s::controller::{Action, Context, Controller, Operator};
use rws_k8s::k8s::testing::{eventually, TestCluster};
use rws_k8s::k8s::{config, discovery, leader::LeaderElector, manifests, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const WAIT: Duration = Duration::from_secs(30);

fn meta(name: &str) -> ObjectMeta {
    ObjectMeta { name: Some(name.to_string()), ..Default::default() }
}

#[tokio::test]
async fn config_maps_and_secrets_stay_current() {
    let Some(cluster) = TestCluster::from_env().await else { return };
    let ns = cluster.namespace("rws-config").await.unwrap();
    let cms: Api<ConfigMap> = ns.api();
    let cm = ConfigMap { metadata: meta("settings"), data: Some(BTreeMap::from([("page_size".into(), "20".into())])), ..Default::default() };
    cms.create(&PostParams::default(), &cm).await.unwrap();

    let watch = config::ConfigWatch::config_map(cluster.client.clone(), ns.name(), "settings").await.unwrap();
    assert_eq!(Some("20".to_string()), watch.get("page_size"));
    let changes = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&changes);
    watch.on_change(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });
    let patch = serde_json::json!({ "data": { "page_size": "50" } });
    cms.patch("settings", &PatchParams::default(), &Patch::Merge(&patch)).await.unwrap();
    let seen = eventually(WAIT, || async { (watch.get("page_size").as_deref() == Some("50")).then_some(()) }).await;
    assert!(seen.is_some(), "the watch saw the change");
    assert!(changes.load(Ordering::SeqCst) >= 1);
    assert!(config::ConfigWatch::config_map(cluster.client.clone(), ns.name(), "missing").await.is_err());

    let secrets: Api<Secret> = ns.api();
    let secret = Secret { metadata: meta("db"), string_data: Some(BTreeMap::from([("password".into(), "s3cret".into())])), ..Default::default() };
    secrets.create(&PostParams::default(), &secret).await.unwrap();
    let reference = format!("k8s://{}/db#password", ns.name());
    assert_eq!("s3cret", config::resolve(&cluster.client, &reference).await.unwrap());
    assert!(config::resolve(&cluster.client, &format!("k8s://{}/db#user", ns.name())).await.is_err());
    let watched = config::ConfigWatch::secret(cluster.client.clone(), ns.name(), "db").await.unwrap();
    assert_eq!(Some("s3cret".to_string()), watched.get("password"));
    ns.delete().await;
}

fn slice(name: &str, service: &str, ips: &[(&str, bool)]) -> EndpointSlice {
    EndpointSlice {
        metadata: ObjectMeta { name: Some(name.into()), labels: Some(BTreeMap::from([("kubernetes.io/service-name".into(), service.into())])), ..Default::default() },
        address_type: "IPv4".into(),
        endpoints: ips.iter().map(|(ip, ready)| Endpoint { addresses: vec![ip.to_string()], conditions: Some(EndpointConditions { ready: Some(*ready), ..Default::default() }), ..Default::default() }).collect(),
        ports: Some(vec![EndpointPort { name: Some("http".into()), port: Some(8080), protocol: Some("TCP".into()), ..Default::default() }]),
    }
}

#[tokio::test]
async fn service_endpoints_follow_endpoint_slices() {
    let Some(cluster) = TestCluster::from_env().await else { return };
    let ns = cluster.namespace("rws-discovery").await.unwrap();
    // A Service without a selector: its slices are ours to write.
    let svc = Service {
        metadata: meta("orders"),
        spec: Some(ServiceSpec { ports: Some(vec![ServicePort { name: Some("http".into()), port: 80, ..Default::default() }]), ..Default::default() }),
        ..Default::default()
    };
    ns.api::<Service>().create(&PostParams::default(), &svc).await.unwrap();
    let slices: Api<EndpointSlice> = ns.api();
    slices.create(&PostParams::default(), &slice("orders-a", "orders", &[("10.1.0.1", true), ("10.1.0.2", false)])).await.unwrap();

    let pool = discovery::service_endpoints(cluster.client.clone(), ns.name(), "orders", Some("http")).await.unwrap();
    assert_eq!(vec!["10.1.0.1:8080"], pool.backends());
    slices.create(&PostParams::default(), &slice("orders-b", "orders", &[("10.1.0.3", true)])).await.unwrap();
    let both = eventually(WAIT, || async { (pool.backends().len() == 2).then(|| pool.backends()) }).await;
    assert_eq!(Some(vec!["10.1.0.1:8080".to_string(), "10.1.0.3:8080".to_string()]), both);
    slices.delete("orders-a", &DeleteParams::default()).await.unwrap();
    let one = eventually(WAIT, || async { (pool.backends() == vec!["10.1.0.3:8080"]).then_some(()) }).await;
    assert!(one.is_some(), "{:?}", pool.backends());
    ns.delete().await;
}

#[tokio::test]
async fn one_replica_leads_at_a_time() {
    let Some(cluster) = TestCluster::from_env().await else { return };
    let ns = cluster.namespace("rws-leader").await.unwrap();
    let timing = (Duration::from_secs(3), Duration::from_millis(500));
    let a = LeaderElector::new(cluster.client.clone(), ns.name(), "lock").identity("a").timing(timing.0, timing.1);
    let b = LeaderElector::new(cluster.client.clone(), ns.name(), "lock").identity("b").timing(timing.0, timing.1);
    assert!(a.step().await, "the first takes the new Lease");
    assert!(!b.step().await, "the second follows");
    assert!(a.step().await && !b.step().await, "renewing keeps it");
    // a stops renewing: b takes over after the duration.
    let took_over = eventually(Duration::from_secs(10), || async { b.step().await.then_some(()) }).await;
    assert!(took_over.is_some());
    assert!(!a.step().await, "a follows now");
    // Releasing hands over at once.
    b.release().await.unwrap();
    assert!(!b.handle().is_leader());
    assert!(a.step().await);
    ns.delete().await;
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "rws-test.example.com", version = "v1", kind = "Greeting", namespaced, status = "GreetingStatus")]
pub struct GreetingSpec {
    pub message: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
pub struct GreetingStatus {
    pub written: bool,
}

#[tokio::test]
async fn a_controller_reconciles_owns_and_cleans_up() {
    let Some(cluster) = TestCluster::from_env().await else { return };
    cluster.install_crd::<Greeting>().await.unwrap();
    let ns = cluster.namespace("rws-controller").await.unwrap();
    let cleaned = Arc::new(AtomicUsize::new(0));
    let cleaned_by = Arc::clone(&cleaned);

    let controller = Controller::new(ns.api::<Greeting>())
        .owns(ns.api::<ConfigMap>())
        .reconcile(|greeting: Arc<Greeting>, ctx: Arc<Context>| async move {
            let cm = ConfigMap {
                metadata: meta(&format!("{}-greeting", greeting.name_any())),
                data: Some(BTreeMap::from([("message".into(), greeting.spec.message.clone())])),
                ..Default::default()
            };
            ctx.apply_owned(&*greeting, cm).await?;
            ctx.patch_status(&*greeting, serde_json::json!({ "written": true })).await?;
            ctx.event(&*greeting, false, "Written", "the ConfigMap is up to date").await?;
            Ok::<_, Error>(Action::requeue(Duration::from_secs(300)))
        })
        .finalizer("rws-test.example.com/cleanup", move |_g, _ctx| {
            let cleaned = Arc::clone(&cleaned_by);
            async move {
                cleaned.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
    let operator = Operator::new(cluster.client.clone(), "rws-test-operator").controller(controller);
    let rules = operator.rules();
    assert!(rules.iter().any(|r| r.api_group == "rws-test.example.com" && r.resources.contains(&"greetings/status".to_string())), "{rules:?}");
    let running = tokio::spawn(operator.run());

    let greetings: Api<Greeting> = ns.api();
    greetings.create(&PostParams::default(), &Greeting::new("hello", GreetingSpec { message: "hi".into() })).await.unwrap();
    let cms: Api<ConfigMap> = ns.api();
    let written = eventually(WAIT, || async { cms.get_opt("hello-greeting").await.ok().flatten() }).await.expect("the owned ConfigMap");
    assert_eq!(Some("hi"), written.data.as_ref().and_then(|d| d.get("message")).map(String::as_str));
    assert_eq!("Greeting", written.owner_references()[0].kind);
    let status = eventually(WAIT, || async { greetings.get("hello").await.ok().and_then(|g| g.status).filter(|s| s.written) }).await;
    assert!(status.is_some(), "status written");
    assert!(greetings.get("hello").await.unwrap().finalizers().contains(&"rws-test.example.com/cleanup".to_string()));

    // A change of spec is reconciled.
    let patch = serde_json::json!({ "spec": { "message": "hello again" } });
    greetings.patch("hello", &PatchParams::default(), &Patch::Merge(&patch)).await.unwrap();
    let updated = eventually(WAIT, || async {
        cms.get("hello-greeting").await.ok().filter(|c| c.data.as_ref().and_then(|d| d.get("message")).map(String::as_str) == Some("hello again"))
    })
    .await;
    assert!(updated.is_some());

    // Deleting runs the cleanup, then the object goes.
    greetings.delete("hello", &DeleteParams::default()).await.unwrap();
    let gone = eventually(WAIT, || async { greetings.get_opt("hello").await.ok().filter(|g| g.is_none()).map(|_| ()) }).await;
    assert!(gone.is_some(), "the finalizer was removed");
    assert_eq!(1, cleaned.load(Ordering::SeqCst));
    let text = rws_k8s::k8s::controller::metrics_text();
    assert!(text.contains("rws_controller_reconciles_total{controller=\"Greeting\"}"), "{text}");
    running.abort();
    ns.delete().await;
}

#[tokio::test]
async fn generated_manifests_are_accepted() {
    let Some(cluster) = TestCluster::from_env().await else { return };
    let ns = cluster.namespace("rws-manifests").await.unwrap();
    let rbac = manifests::Rbac::new("rws-test-op", ns.name()).rule(manifests::Rule::new("", &["configmaps"], &["get", "list"])).leader_election();
    let ca = rws_k8s::k8s::webhook::WebhookCertificate::generate("rws-test-op", ns.name()).unwrap().ca_pem;
    let target = manifests::WebhookTarget { service: "rws-test-op".into(), namespace: ns.name().into(), port: 443, path: "/validate".into(), ca_pem: ca };
    let webhook = manifests::webhook_configuration(&format!("rws-test-{}", ns.name()), false, &target, &[manifests::WebhookRule::new("", "v1", "configmaps", &["CREATE"])], false);
    let mut documents = rbac.documents();
    documents.push(webhook);
    documents.push(manifests::crd::<Greeting>());
    // Server-side dry run of each: the API server validates them.
    let discovery = kube::Discovery::new(cluster.client.clone()).run().await.unwrap();
    for doc in documents {
        let obj: kube::api::DynamicObject = serde_json::from_value(doc.clone()).unwrap();
        let gvk = kube::api::GroupVersionKind::try_from(obj.types.as_ref().unwrap()).unwrap();
        let (resource, caps) = discovery.resolve_gvk(&gvk).unwrap();
        let api: Api<kube::api::DynamicObject> = match (caps.scope, obj.namespace()) {
            (kube::discovery::Scope::Namespaced, Some(n)) => Api::namespaced_with(cluster.client.clone(), &n, &resource),
            _ => Api::all_with(cluster.client.clone(), &resource),
        };
        let params = PatchParams::apply("rws-test").dry_run().force();
        api.patch(&obj.name_any(), &params, &Patch::Apply(&doc)).await.unwrap_or_else(|e| panic!("{} rejected: {e}", gvk.kind));
    }
    ns.delete().await;
}

#[tokio::test]
async fn replicas_share_one_webhook_certificate() {
    let Some(cluster) = TestCluster::from_env().await else { return };
    let ns = cluster.namespace("rws-webhook-cert").await.unwrap();
    let (a, b) = tokio::join!(
        rws_k8s::k8s::webhook::WebhookCertificate::shared(&cluster.client, ns.name(), "op-tls", "op"),
        rws_k8s::k8s::webhook::WebhookCertificate::shared(&cluster.client, ns.name(), "op-tls", "op"),
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a.ca_pem, b.ca_pem, "both replicas use the stored one");
    let again = rws_k8s::k8s::webhook::WebhookCertificate::shared(&cluster.client, ns.name(), "op-tls", "op").await.unwrap();
    assert_eq!(a.cert_pem, again.cert_pem);
    ns.delete().await;
}
