//! The provisioned mode (`RWS_CONFIG_GATEWAY_API_MODE=provisioned`): every
//! Gateway gets its own Deployment of rws data-plane pods, in the
//! controller's namespace and under its ServiceAccount, so no Gateway
//! shares pods with another. Those pods run with
//! `RWS_CONFIG_GATEWAY_API_GATEWAY=<namespace>/<name>`: they watch like the
//! controller but serve only that Gateway and never write. The Gateway's
//! Service lists them in its EndpointSlice ([`super::services`]). A
//! Deployment can't be owned by an object in another namespace, so the
//! controller deletes those whose Gateway is gone ([`stale`]).

use std::collections::BTreeMap;

use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{Container, EnvVar, EnvVarSource, HTTPGetAction, ObjectFieldSelector, PodSpec, PodTemplateSpec, Probe};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::ResourceExt;

use super::resources::Gateway;
use super::translate::{GATEWAY_NAME_LABEL, MANAGED_BY};

/// `app.kubernetes.io/name` of provisioned pods.
pub const APP_NAME: &str = "rws-gateway";
/// The Gateway's namespace on its Deployment and pods.
pub const GATEWAY_NAMESPACE_LABEL: &str = "rws8.tech/gateway-namespace";

#[derive(Clone, Debug, PartialEq)]
pub struct ProvisionSettings {
    /// The rws image provisioned pods run (the chart's).
    pub image: String,
    /// The controller's ServiceAccount, which they share.
    pub service_account: String,
    /// rws's HTTP port in the pods, for the probes.
    pub health_port: i32,
    /// Replicas per Gateway.
    pub replicas: i32,
}

impl Default for ProvisionSettings {
    fn default() -> Self {
        ProvisionSettings { image: String::new(), service_account: "default".to_string(), health_port: 7878, replicas: 1 }
    }
}

/// The labels of a Gateway's pods (and its Deployment's selector).
pub fn pod_labels(namespace: &str, name: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("app.kubernetes.io/name".to_string(), APP_NAME.to_string()),
        (GATEWAY_NAME_LABEL.to_string(), name.to_string()),
        (GATEWAY_NAMESPACE_LABEL.to_string(), namespace.to_string()),
    ])
}

/// `rws-gw-<namespace>-<name>`, shortened with a hash past 63 characters.
pub fn deployment_name(namespace: &str, name: &str) -> String {
    let full = format!("rws-gw-{namespace}-{name}");
    if full.len() <= 63 {
        return full;
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    full.hash(&mut hasher);
    format!("{}-{:08x}", &full[..54], hasher.finish() as u32)
}

/// The Deployment each Gateway should have, keyed by (its namespace, name).
pub fn desired(gateways: &[&Gateway], pod_namespace: &str, controller_name: &str, port_range: (u16, u16), settings: &ProvisionSettings) -> BTreeMap<(String, String), Deployment> {
    let mut out = BTreeMap::new();
    for gateway in gateways {
        let (namespace, name) = (gateway.namespace().unwrap_or_default(), gateway.name_any());
        let labels = pod_labels(&namespace, &name);
        let infrastructure = gateway.spec.infrastructure.clone().unwrap_or_default();
        let mut pod_labels_all = infrastructure.labels.clone();
        pod_labels_all.extend(labels.clone());
        let mut deployment_labels = labels.clone();
        deployment_labels.insert("app.kubernetes.io/managed-by".to_string(), MANAGED_BY.to_string());
        let env = |key: &str, value: String| EnvVar { name: key.to_string(), value: Some(value), value_from: None };
        let field = |key: &str, path: &str| EnvVar {
            name: key.to_string(),
            value: None,
            value_from: Some(EnvVarSource { field_ref: Some(ObjectFieldSelector { field_path: path.to_string(), api_version: None }), ..Default::default() }),
        };
        let probe = |path: &str| Probe {
            http_get: Some(HTTPGetAction { path: Some(path.to_string()), port: IntOrString::Int(settings.health_port), ..Default::default() }),
            period_seconds: Some(5),
            ..Default::default()
        };
        let container = Container {
            name: "rws".to_string(),
            image: Some(settings.image.clone()),
            env: Some(vec![
                env(super::RWS_CONFIG_GATEWAY_API_ENABLED, "true".to_string()),
                env(super::RWS_CONFIG_GATEWAY_API_GATEWAY, format!("{namespace}/{name}")),
                env(super::RWS_CONFIG_GATEWAY_API_CONTROLLER_NAME, controller_name.to_string()),
                env(super::RWS_CONFIG_GATEWAY_API_PORT_RANGE, format!("{}-{}", port_range.0, port_range.1)),
                field("POD_NAME", "metadata.name"),
                field("POD_NAMESPACE", "metadata.namespace"),
            ]),
            readiness_probe: Some(probe("/readyz")),
            liveness_probe: Some(probe("/healthz")),
            ..Default::default()
        };
        let deployment = Deployment {
            metadata: ObjectMeta {
                name: Some(deployment_name(&namespace, &name)),
                namespace: Some(pod_namespace.to_string()),
                labels: Some(deployment_labels),
                annotations: (!infrastructure.annotations.is_empty()).then(|| infrastructure.annotations.clone()),
                ..Default::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(settings.replicas),
                selector: LabelSelector { match_labels: Some(labels), match_expressions: None },
                template: PodTemplateSpec {
                    metadata: Some(ObjectMeta {
                        labels: Some(pod_labels_all),
                        annotations: (!infrastructure.annotations.is_empty()).then(|| infrastructure.annotations.clone()),
                        ..Default::default()
                    }),
                    spec: Some(PodSpec { service_account_name: Some(settings.service_account.clone()), containers: vec![container], ..Default::default() }),
                },
                ..Default::default()
            }),
            status: None,
        };
        out.insert((namespace, name), deployment);
    }
    out
}

/// Whether `current` already runs what `desired` asks for (the fields rws sets).
pub fn same(current: &Deployment, desired: &Deployment) -> bool {
    let spec = |d: &Deployment| {
        let s = d.spec.as_ref();
        let template = s.map(|s| &s.template);
        let container = template.and_then(|t| t.spec.as_ref()).and_then(|p| p.containers.first());
        (
            s.and_then(|s| s.replicas),
            template.and_then(|t| t.metadata.as_ref()).and_then(|m| m.labels.clone()),
            template.and_then(|t| t.metadata.as_ref()).and_then(|m| m.annotations.clone()),
            container.and_then(|c| c.image.clone()),
            container.and_then(|c| c.env.clone()),
        )
    };
    spec(current) == spec(desired)
}

/// Provisioned Deployments in `deployments` whose Gateway no longer wants one.
pub fn stale<'a>(deployments: &'a [Deployment], wanted: &BTreeMap<(String, String), Deployment>) -> Vec<&'a Deployment> {
    deployments
        .iter()
        .filter(|d| d.labels().get("app.kubernetes.io/managed-by").map(String::as_str) == Some(MANAGED_BY))
        .filter(|d| !wanted.values().any(|w| w.metadata.name == d.metadata.name && w.metadata.namespace == d.metadata.namespace))
        .collect()
}

#[cfg(test)]
mod tests;
