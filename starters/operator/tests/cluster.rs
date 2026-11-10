//! The controller against a real API server: with `RWS_TEST_KUBECONFIG`
//! set (a kind or k3d cluster; never one that matters), skipped otherwise.
//! The webhooks aren't installed here — `ci/e2e/operator.sh` runs the
//! whole operator in a cluster.

use std::time::Duration;

use {{crate_name}}::{operator, Greeting, GreetingSpec};
use rust_web_server::k8s::k8s_openapi::api::core::v1::ConfigMap;
use rust_web_server::k8s::kube::api::{DeleteParams, PostParams};
use rust_web_server::k8s::testing::{eventually, TestCluster};

#[tokio::test]
async fn greetings_get_their_config_map() {
    let Some(cluster) = TestCluster::from_env().await else { return };
    cluster.install_crd::<Greeting>().await.unwrap();
    let ns = cluster.namespace("greetings").await.unwrap();
    let running = tokio::spawn(operator(cluster.client.clone(), ns.name()).run());

    let greetings = ns.api::<Greeting>();
    greetings.create(&PostParams::default(), &Greeting::new("hello", GreetingSpec { message: "hi".into() })).await.unwrap();
    let config_maps = ns.api::<ConfigMap>();
    let written = eventually(Duration::from_secs(60), || async { config_maps.get_opt("hello-greeting").await.ok().flatten() }).await.expect("the ConfigMap");
    assert_eq!(Some("hi"), written.data.as_ref().and_then(|d| d.get("message")).map(String::as_str));
    let ready = eventually(Duration::from_secs(30), || async { greetings.get("hello").await.ok().and_then(|g| g.status).filter(|s| s.ready) }).await;
    assert!(ready.is_some());

    greetings.delete("hello", &DeleteParams::default()).await.unwrap();
    let gone = eventually(Duration::from_secs(30), || async { greetings.get_opt("hello").await.ok().filter(|g| g.is_none()).map(|_| ()) }).await;
    assert!(gone.is_some(), "the finalizer ran and was removed");
    running.abort();
    ns.delete().await;
}
