//! The webhooks and the server, through `TestClient` (no socket, no
//! cluster): AdmissionReviews as the API server sends them.

use {{crate_name}}::{server, webhooks::LABEL};
use rust_web_server::test_client::TestClient;
use serde_json::{json, Value};

fn review(message: &str) -> String {
    json!({
        "apiVersion": "admission.k8s.io/v1",
        "kind": "AdmissionReview",
        "request": {
            "uid": "e911857d-c318-11e8-bbad-025000000001",
            "kind": {"group": "example.com", "version": "v1", "kind": "Greeting"},
            "resource": {"group": "example.com", "version": "v1", "resource": "greetings"},
            "name": "hello", "namespace": "default", "operation": "CREATE",
            "userInfo": {"username": "ann"},
            "object": {"apiVersion": "example.com/v1", "kind": "Greeting", "metadata": {"name": "hello", "namespace": "default"}, "spec": {"message": message}},
            "oldObject": null, "dryRun": false
        }
    })
    .to_string()
}

fn post(path: &str, body: &str) -> Value {
    let response = TestClient::new(server()).post(path).body_json(body).send();
    assert_eq!(200, response.status(), "{}", response.body_text());
    serde_json::from_slice(response.body_bytes()).unwrap()
}

#[test]
fn greetings_need_a_message() {
    assert_eq!(true, post("/validate", &review("hi"))["response"]["allowed"]);
    let denied = post("/validate", &review("   "));
    assert_eq!((false, "spec.message is required"), (denied["response"]["allowed"].as_bool().unwrap(), denied["response"]["status"]["message"].as_str().unwrap()));
    let long = post("/validate", &review(&"x".repeat(1001)));
    assert_eq!(false, long["response"]["allowed"]);
}

#[test]
fn greetings_get_the_label() {
    let answer = post("/mutate", &review("hi"));
    assert_eq!(true, answer["response"]["allowed"]);
    assert_eq!("JSONPatch", answer["response"]["patchType"]);
    assert!(answer["response"]["patch"].as_str().is_some_and(|p| !p.is_empty()), "{answer}");
    let _ = LABEL;
}

#[test]
fn probes_answer_beside_the_webhooks() {
    let client = TestClient::new(server());
    assert_eq!(200, client.get("/livez").send().status());
    assert_eq!(400, client.post("/validate").body_json("{}").send().status());
}

#[test]
fn manifests_name_everything_to_apply() {
    let docs = {{crate_name}}::manifests("ops", "ghcr.io/acme/op:1", "");
    let kinds: Vec<&str> = docs.iter().map(|d| d["kind"].as_str().unwrap()).collect();
    assert_eq!(vec!["CustomResourceDefinition", "ServiceAccount", "ClusterRole", "ClusterRoleBinding", "ValidatingWebhookConfiguration", "MutatingWebhookConfiguration", "Deployment", "Service"], kinds);
    assert_eq!("greetings.example.com", docs[0]["metadata"]["name"]);
    let rules = docs[2]["rules"].to_string();
    assert!(rules.contains("greetings/status") && rules.contains("leases") && rules.contains("validatingwebhookconfigurations"), "{rules}");
    assert_eq!("ghcr.io/acme/op:1", docs[6]["spec"]["template"]["spec"]["containers"][0]["image"]);
    assert_eq!("/validate", docs[4]["webhooks"][0]["clientConfig"]["service"]["path"]);
}
