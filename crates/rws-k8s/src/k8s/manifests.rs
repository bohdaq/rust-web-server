//! Manifests an operator ships: CRDs generated from its Rust types, the
//! RBAC its controllers need, the webhook configurations — as JSON, which
//! is valid YAML, joined with `---` for `kubectl apply -f -`.
//!
//! ```rust,no_run
//! use rws_k8s::k8s::manifests::{self, Rbac};
//! # #[derive(rws_k8s::k8s::kube::CustomResource, serde::Deserialize, serde::Serialize, Clone, Debug, rws_k8s::k8s::schemars::JsonSchema)]
//! # #[kube(group = "example.com", version = "v1", kind = "Backup", namespaced)]
//! # pub struct BackupSpec { pub schedule: String }
//! # fn operator() -> rws_k8s::k8s::controller::Operator { unimplemented!() }
//!
//! let rbac = Rbac::new("backup-operator", "backups").rules(operator().rules());
//! let all = [vec![manifests::crd::<Backup>()], rbac.documents()].concat();
//! print!("{}", manifests::to_yaml(&all));      // kubectl apply -f -
//! ```
//!
//! [`print_on_request`] gives an operator binary a `--manifests` flag that
//! prints them and exits.

use std::collections::BTreeMap;

use serde_json::{json, Value};

/// One RBAC policy rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rule {
    /// `""` for the core group.
    pub api_group: String,
    pub resources: Vec<String>,
    pub verbs: Vec<String>,
}

impl Rule {
    pub fn new(api_group: &str, resources: &[&str], verbs: &[&str]) -> Self {
        Rule { api_group: api_group.to_string(), resources: resources.iter().map(|s| s.to_string()).collect(), verbs: verbs.iter().map(|s| s.to_string()).collect() }
    }

    pub fn to_json(&self) -> Value {
        json!({ "apiGroups": [self.api_group], "resources": self.resources, "verbs": self.verbs })
    }
}

/// `rules` with the verbs of each (group, resource) merged, one rule per
/// (group, verb set), sorted.
pub fn merge_rules(rules: Vec<Rule>) -> Vec<Rule> {
    let mut by_resource: BTreeMap<(String, String), std::collections::BTreeSet<String>> = BTreeMap::new();
    for rule in rules {
        for resource in &rule.resources {
            by_resource.entry((rule.api_group.clone(), resource.clone())).or_default().extend(rule.verbs.iter().cloned());
        }
    }
    let mut grouped: BTreeMap<(String, Vec<String>), Vec<String>> = BTreeMap::new();
    for ((group, resource), verbs) in by_resource {
        grouped.entry((group, verbs.into_iter().collect())).or_default().push(resource);
    }
    grouped.into_iter().map(|((api_group, verbs), resources)| Rule { api_group, resources, verbs }).collect()
}

/// A ServiceAccount, a ClusterRole with the rules and its binding — what
/// an operator's pod runs as.
pub struct Rbac {
    name: String,
    namespace: String,
    rules: Vec<Rule>,
}

impl Rbac {
    /// The account `name` in `namespace`.
    pub fn new(name: &str, namespace: &str) -> Self {
        Rbac { name: name.to_string(), namespace: namespace.to_string(), rules: Vec::new() }
    }

    pub fn rules(mut self, rules: Vec<Rule>) -> Self {
        self.rules.extend(rules);
        self
    }

    pub fn rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    /// The rules a [`LeaderElector`](super::leader::LeaderElector) needs.
    pub fn leader_election(self) -> Self {
        self.rule(Rule::new("coordination.k8s.io", &["leases"], &["get", "create", "update"]))
    }

    /// ServiceAccount, ClusterRole, ClusterRoleBinding.
    pub fn documents(&self) -> Vec<Value> {
        let rules: Vec<Value> = merge_rules(self.rules.clone()).iter().map(Rule::to_json).collect();
        vec![
            json!({ "apiVersion": "v1", "kind": "ServiceAccount", "metadata": { "name": self.name, "namespace": self.namespace } }),
            json!({ "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "ClusterRole", "metadata": { "name": self.name }, "rules": rules }),
            json!({
                "apiVersion": "rbac.authorization.k8s.io/v1",
                "kind": "ClusterRoleBinding",
                "metadata": { "name": self.name },
                "roleRef": { "apiGroup": "rbac.authorization.k8s.io", "kind": "ClusterRole", "name": self.name },
                "subjects": [{ "kind": "ServiceAccount", "name": self.name, "namespace": self.namespace }]
            }),
        ]
    }
}

/// The CustomResourceDefinition of a `#[derive(CustomResource)]` type.
pub fn crd<K: kube::CustomResourceExt>() -> Value {
    serde_json::to_value(K::crd()).unwrap_or(Value::Null)
}

/// Which operations of which resources a webhook sees.
#[derive(Debug, Clone)]
pub struct WebhookRule {
    pub api_group: String,
    pub api_version: String,
    pub resource: String,
    /// `CREATE`, `UPDATE`, `DELETE`, `CONNECT`, or `*`.
    pub operations: Vec<String>,
}

impl WebhookRule {
    pub fn new(api_group: &str, api_version: &str, resource: &str, operations: &[&str]) -> Self {
        WebhookRule { api_group: api_group.into(), api_version: api_version.into(), resource: resource.into(), operations: operations.iter().map(|s| s.to_string()).collect() }
    }

    fn to_json(&self) -> Value {
        json!({ "apiGroups": [self.api_group], "apiVersions": [self.api_version], "resources": [self.resource], "operations": self.operations, "scope": "*" })
    }
}

/// Where the API server reaches a webhook: a Service and a path.
#[derive(Debug, Clone)]
pub struct WebhookTarget {
    pub service: String,
    pub namespace: String,
    pub port: u16,
    pub path: String,
    /// The PEM CA that signed the serving certificate.
    pub ca_pem: String,
}

impl WebhookTarget {
    fn client_config(&self) -> Value {
        json!({
            "service": { "name": self.service, "namespace": self.namespace, "port": self.port, "path": self.path },
            "caBundle": base64(self.ca_pem.as_bytes()),
        })
    }
}

/// A ValidatingWebhookConfiguration (or Mutating, `mutating = true`) with
/// one webhook. `fail_closed`: refuse the request when the webhook can't
/// be reached (`failurePolicy: Fail`), else let it through (`Ignore`).
pub fn webhook_configuration(name: &str, mutating: bool, target: &WebhookTarget, rules: &[WebhookRule], fail_closed: bool) -> Value {
    let kind = if mutating { "MutatingWebhookConfiguration" } else { "ValidatingWebhookConfiguration" };
    json!({
        "apiVersion": "admissionregistration.k8s.io/v1",
        "kind": kind,
        "metadata": { "name": name },
        "webhooks": [{
            "name": format!("{name}.{}.svc", target.namespace),
            "admissionReviewVersions": ["v1"],
            "sideEffects": "None",
            "timeoutSeconds": 10,
            "failurePolicy": if fail_closed { "Fail" } else { "Ignore" },
            "clientConfig": target.client_config(),
            "rules": rules.iter().map(WebhookRule::to_json).collect::<Vec<_>>(),
        }]
    })
}

/// `crd` with a conversion webhook at `target` (the CRD lists several
/// versions; the API server asks the webhook to convert between them).
pub fn with_conversion_webhook(mut crd: Value, target: &WebhookTarget) -> Value {
    crd["spec"]["conversion"] = json!({
        "strategy": "Webhook",
        "webhook": { "conversionReviewVersions": ["v1"], "clientConfig": target.client_config() },
    });
    crd
}

/// Documents as a YAML stream: each JSON document (which YAML accepts)
/// after a `---` line. The stream starts with one too: kubectl reads a
/// stream that starts with `{` as JSON, and panics when a document longer
/// than its 2 KiB sniff buffer is followed by `---`.
pub fn to_yaml(documents: &[Value]) -> String {
    documents.iter().map(|d| format!("---\n{}\n", serde_json::to_string_pretty(d).unwrap_or_default())).collect()
}

/// When the program's arguments contain `--manifests`, print `documents`
/// as YAML and exit — `my-operator --manifests | kubectl apply -f -`.
pub fn print_on_request(documents: impl FnOnce() -> Vec<Value>) {
    if std::env::args().any(|a| a == "--manifests") {
        print!("{}", to_yaml(&documents()));
        std::process::exit(0);
    }
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests;
