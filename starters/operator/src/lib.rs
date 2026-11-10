//! A Kubernetes operator on rust-web-server: a custom resource (`Greeting`),
//! a controller that reconciles it, validating and mutating webhooks, and
//! its manifests (CRD, RBAC, webhooks, Deployment) printed by the binary.
//!
//! The conventional rws layout (`rws.app.toml`; an operator's "routes" are
//! its controllers, in `src/controllers/`): this file builds the operator
//! and the HTTPS server, `src/main.rs` only runs them, `tests/` tests them.
//! See AGENTS.md.

pub mod controllers;
pub mod webhooks;

use rust_web_server::dev_mcp::DevTools;
use rust_web_server::k8s::controller::Operator;
use rust_web_server::k8s::kube::Client;
use rust_web_server::k8s::manifests::{self, Rbac, Rule, WebhookRule, WebhookTarget};
use rust_web_server::k8s::webhook::Webhooks;
use serde_json::{json, Value};

pub use controllers::greeting::{Greeting, GreetingSpec, GreetingStatus};

/// The operator's name: field manager, event reporter, Lease, ServiceAccount,
/// Service and webhook configurations.
pub const NAME: &str = "{{project-name}}";

/// The port the HTTPS server listens on in the pod.
pub const PORT: u16 = 8443;

/// The controllers, under leader election in `namespace`.
pub fn operator(client: Client, namespace: &str) -> Operator {
    Operator::new(client.clone(), NAME).leader_election(namespace, NAME).controller(controllers::greeting::controller(client))
}

/// The HTTPS server: the webhooks, and rws's probes and metrics for
/// everything else.
pub fn server() -> Webhooks {
    use rust_web_server::core::New;
    webhooks::webhooks().wrap(rust_web_server::app::App::new())
}

/// The dev MCP endpoint, for a coding agent working on this project.
/// Mounted with `RWS_ENV=dev` or `DEV_MCP_TOKEN` set.
pub fn dev_tools() -> DevTools {
    DevTools::new().at("/_dev/mcp").token_env("DEV_MCP_TOKEN").app_dir(env!("CARGO_MANIFEST_DIR"))
}

fn target(namespace: &str, path: &str, ca_pem: &str) -> WebhookTarget {
    WebhookTarget { service: NAME.into(), namespace: namespace.into(), port: 443, path: path.into(), ca_pem: ca_pem.into() }
}

/// Everything to apply: the CRD, the RBAC, both webhook configurations,
/// the Deployment and its Service. `ca_pem` empty: the operator puts its
/// CA into the webhook configurations when it starts.
pub fn manifests(namespace: &str, image: &str, ca_pem: &str) -> Vec<Value> {
    let rules = vec![WebhookRule::new("example.com", "v1", "greetings", &["CREATE", "UPDATE"])];
    let rbac = Rbac::new(NAME, namespace)
        .rules(operator(rust_web_server::k8s::client::offline(), namespace).rules())
        .rule(Rule::new("admissionregistration.k8s.io", &["validatingwebhookconfigurations", "mutatingwebhookconfigurations"], &["get", "patch"]))
        // The webhook certificate shared by the replicas.
        .rule(Rule::new("", &["secrets"], &["get", "create"]));
    let mut documents = vec![manifests::crd::<Greeting>()];
    documents.extend(rbac.documents());
    documents.push(manifests::webhook_configuration(NAME, false, &target(namespace, "/validate", ca_pem), &rules, true));
    documents.push(manifests::webhook_configuration(NAME, true, &target(namespace, "/mutate", ca_pem), &rules, true));
    documents.push(deployment(namespace, image));
    documents.push(json!({
        "apiVersion": "v1", "kind": "Service",
        "metadata": { "name": NAME, "namespace": namespace },
        "spec": { "selector": { "app.kubernetes.io/name": NAME }, "ports": [{ "name": "https", "port": 443, "targetPort": PORT }] }
    }));
    documents
}

fn deployment(namespace: &str, image: &str) -> Value {
    let probe = |path: &str, period: u32| json!({ "httpGet": { "path": path, "port": PORT, "scheme": "HTTPS" }, "periodSeconds": period, "timeoutSeconds": 3 });
    json!({
        "apiVersion": "apps/v1", "kind": "Deployment",
        "metadata": { "name": NAME, "namespace": namespace, "labels": { "app.kubernetes.io/name": NAME } },
        "spec": {
            "replicas": 2,
            "selector": { "matchLabels": { "app.kubernetes.io/name": NAME } },
            "template": {
                "metadata": { "labels": { "app.kubernetes.io/name": NAME } },
                "spec": {
                    "serviceAccountName": NAME,
                    "terminationGracePeriodSeconds": 35,
                    "securityContext": { "runAsNonRoot": true, "seccompProfile": { "type": "RuntimeDefault" } },
                    "containers": [{
                        "name": "operator",
                        "image": image,
                        "imagePullPolicy": "IfNotPresent",
                        "ports": [{ "name": "https", "containerPort": PORT }],
                        "env": [
                            { "name": "RWS_CONFIG_IP", "value": "0.0.0.0" },
                            { "name": "RWS_CONFIG_PORT", "value": PORT.to_string() },
                            { "name": "WEBHOOK_TLS_DIR", "value": "/tmp/webhook-tls" },
                            { "name": "POD_NAME", "valueFrom": { "fieldRef": { "fieldPath": "metadata.name" } } },
                            { "name": "POD_NAMESPACE", "valueFrom": { "fieldRef": { "fieldPath": "metadata.namespace" } } }
                        ],
                        "readinessProbe": probe("/readyz", 2),
                        "livenessProbe": probe("/livez", 10),
                        "resources": { "requests": { "cpu": "50m", "memory": "64Mi" }, "limits": { "memory": "256Mi" } },
                        "securityContext": { "allowPrivilegeEscalation": false, "readOnlyRootFilesystem": true, "capabilities": { "drop": ["ALL"] } },
                        "volumeMounts": [{ "name": "tmp", "mountPath": "/tmp" }]
                    }],
                    "volumes": [{ "name": "tmp", "emptyDir": {} }]
                }
            }
        }
    })
}
