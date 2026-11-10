use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::*;

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "example.com", version = "v1", kind = "Widget", namespaced)]
pub struct WidgetSpec {
    pub size: u32,
}

#[test]
fn rules_merge_by_resource_and_verbs() {
    let merged = merge_rules(vec![
        Rule::new("apps", &["deployments"], &["get", "list"]),
        Rule::new("apps", &["deployments"], &["watch", "get"]),
        Rule::new("", &["configmaps", "secrets"], &["get"]),
        Rule::new("apps", &["statefulsets"], &["get", "list", "watch"]),
    ]);
    assert_eq!(
        vec![
            Rule::new("", &["configmaps", "secrets"], &["get"]),
            Rule::new("apps", &["deployments", "statefulsets"], &["get", "list", "watch"]),
        ],
        merged
    );
}

#[test]
fn rbac_documents_bind_the_account() {
    let docs = Rbac::new("op", "ops").rule(Rule::new("example.com", &["widgets"], &["get"])).leader_election().documents();
    assert_eq!(3, docs.len());
    assert_eq!(("ServiceAccount", "ops"), (docs[0]["kind"].as_str().unwrap(), docs[0]["metadata"]["namespace"].as_str().unwrap()));
    assert_eq!(2, docs[1]["rules"].as_array().unwrap().len());
    assert_eq!("op", docs[2]["subjects"][0]["name"]);
    assert_eq!("ClusterRole", docs[2]["roleRef"]["kind"]);
}

#[test]
fn crds_come_from_the_type() {
    let crd = crd::<Widget>();
    assert_eq!("widgets.example.com", crd["metadata"]["name"]);
    assert_eq!("Namespaced", crd["spec"]["scope"]);
    let schema = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]["size"];
    assert_eq!("integer", schema["type"], "{crd}");
    let target = WebhookTarget { service: "op".into(), namespace: "ops".into(), port: 443, path: "/convert".into(), ca_pem: "CA".into() };
    let converted = with_conversion_webhook(crd, &target);
    assert_eq!("Webhook", converted["spec"]["conversion"]["strategy"]);
    assert_eq!("Q0E=", converted["spec"]["conversion"]["webhook"]["clientConfig"]["caBundle"]);
}

#[test]
fn webhook_configurations() {
    let target = WebhookTarget { service: "op".into(), namespace: "ops".into(), port: 8443, path: "/validate".into(), ca_pem: "-----BEGIN CERTIFICATE-----".into() };
    let v = webhook_configuration("widgets", false, &target, &[WebhookRule::new("example.com", "v1", "widgets", &["CREATE", "UPDATE"])], true);
    assert_eq!("ValidatingWebhookConfiguration", v["kind"]);
    assert_eq!("widgets.ops.svc", v["webhooks"][0]["name"]);
    assert_eq!("Fail", v["webhooks"][0]["failurePolicy"]);
    assert_eq!(8443, v["webhooks"][0]["clientConfig"]["service"]["port"]);
    assert_eq!("widgets", v["webhooks"][0]["rules"][0]["resources"][0]);
    let m = webhook_configuration("widgets", true, &target, &[], false);
    assert_eq!(("MutatingWebhookConfiguration", "Ignore"), (m["kind"].as_str().unwrap(), m["webhooks"][0]["failurePolicy"].as_str().unwrap()));
}

#[test]
fn yaml_stream_and_base64() {
    let text = to_yaml(&[json!({"a": 1}), json!({"b": 2})]);
    assert_eq!("---\n{\n  \"a\": 1\n}\n---\n{\n  \"b\": 2\n}\n", text);
    assert_eq!("", to_yaml(&[]));
    assert_eq!(("", "Zg==", "Zm8=", "Zm9v", "Zm9vYg=="), (base64(b"").as_str(), base64(b"f").as_str(), base64(b"fo").as_str(), base64(b"foo").as_str(), base64(b"foob").as_str()));
}
