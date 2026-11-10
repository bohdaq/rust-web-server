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
