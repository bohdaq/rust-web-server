use std::time::Duration;

use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::ConfigMap;

use super::*;

#[test]
fn backoff_doubles_up_to_five_minutes() {
    let s = Duration::from_secs(5);
    assert_eq!(Duration::from_secs(5), backoff(s, 1));
    assert_eq!(Duration::from_secs(10), backoff(s, 2));
    assert_eq!(Duration::from_secs(40), backoff(s, 4));
    assert_eq!(Duration::from_secs(300), backoff(s, 12));
    assert_eq!(Duration::from_secs(300), backoff(s, 1000));
    assert_eq!(Duration::from_secs(5), backoff(s, 0));
}

#[test]
fn kinds_name_their_group_and_plural() {
    assert_eq!(("apps".to_string(), "deployments".to_string()), group_and_plural::<Deployment>());
    assert_eq!(("".to_string(), "configmaps".to_string()), group_and_plural::<ConfigMap>());
}

#[test]
fn metrics_text_lists_each_controller() {
    let c = counters_for("ControllerTestKind");
    c.reconciles.fetch_add(3, Ordering::Relaxed);
    c.errors.fetch_add(1, Ordering::Relaxed);
    let text = metrics_text();
    assert!(text.contains("rws_controller_reconciles_total{controller=\"ControllerTestKind\"} 3"), "{text}");
    assert!(text.contains("rws_controller_reconcile_errors_total{controller=\"ControllerTestKind\"} 1"), "{text}");
}

#[tokio::test]
async fn rules_cover_the_kinds_and_the_lease() {
    let client = crate::k8s::client::offline();
    let operator = Operator::new(client.clone(), "t")
        .leader_election("ns", "t")
        .controller(Controller::new(Api::<Deployment>::all(client.clone())).owns(Api::<ConfigMap>::all(client)).reconcile(|_d: Arc<Deployment>, _c| async { Ok(Action::await_change()) }));
    assert_eq!(vec!["Deployment".to_string()], operator.kinds());
    let rules = operator.rules();
    let has = |group: &str, resource: &str, verb: &str| rules.iter().any(|r| r.api_group == group && r.resources.iter().any(|x| x == resource) && r.verbs.iter().any(|v| v == verb));
    assert!(has("apps", "deployments", "watch") && has("apps", "deployments/status", "patch"));
    assert!(has("", "configmaps", "create") && has("coordination.k8s.io", "leases", "update") && has("events.k8s.io", "events", "create"));
}

#[test]
fn an_offline_client_works_outside_a_runtime() {
    let client = crate::k8s::client::offline();
    let _ = Controller::new(Api::<ConfigMap>::all(client)).rules();
}
