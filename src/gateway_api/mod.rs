//! A Kubernetes Gateway API controller (`gateway-api` feature, 18.8.0, #19):
//! rws watches GatewayClasses, Gateways and routes, serves their traffic, and
//! reports what it accepted in their status. The plan and progress are in
//! `spec/GATEWAY_API.md`.
//!
//! - [`types`] — the Gateway API v1.6 objects rws reads (serde structs).
//! - [`resources`] — where each kind lives in the API, and the object types.
//! - [`translate`] — the pure step from a snapshot of the cluster to the
//!   configuration rws serves and the status every object should have.
//! - [`services`] — the Service each Gateway gets, and its port allocation.
//! - [`routes`] — routes compiled for the data plane, backends resolved to
//!   endpoints.
//! - [`dataplane`] / [`router`] — the ports served and how a request on one
//!   finds its listener, rule, backend and endpoint.
//! - [`controller`] — watches, the reconcile loop and status writes.

pub mod backend_tls;
pub mod controller;
pub mod dataplane;
pub mod filters;
pub mod l4;
pub mod lease;
pub mod provision;
pub mod resources;
pub mod router;
pub mod routes;
pub mod services;
pub mod translate;
pub mod types;
pub mod upgrade;

/// The `controllerName` GatewayClasses name to be handled by rws.
pub const CONTROLLER_NAME: &str = "rws8.tech/gateway-controller";

/// The field manager rws writes status with.
pub const FIELD_MANAGER: &str = "rws-gateway";

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// `RWS_CONFIG_GATEWAY_API_ENABLED` (`[gateway_api] enabled`): run the
/// controller and serve Gateways.
pub const RWS_CONFIG_GATEWAY_API_ENABLED: &str = "RWS_CONFIG_GATEWAY_API_ENABLED";
/// The `controllerName` handled (default [`CONTROLLER_NAME`]).
pub const RWS_CONFIG_GATEWAY_API_CONTROLLER_NAME: &str = "RWS_CONFIG_GATEWAY_API_CONTROLLER_NAME";
/// The labels of the pods Gateways' Services select, `key=value,…`.
pub const RWS_CONFIG_GATEWAY_API_POD_SELECTOR: &str = "RWS_CONFIG_GATEWAY_API_POD_SELECTOR";
/// Container ports to allocate, `first-last` (default `10000-10999`).
pub const RWS_CONFIG_GATEWAY_API_PORT_RANGE: &str = "RWS_CONFIG_GATEWAY_API_PORT_RANGE";
/// Gateways' Service type (default `LoadBalancer`).
pub const RWS_CONFIG_GATEWAY_API_SERVICE_TYPE: &str = "RWS_CONFIG_GATEWAY_API_SERVICE_TYPE";
/// Where the leader-election Lease lives (default: the pod's namespace).
pub const RWS_CONFIG_GATEWAY_API_LEASE_NAMESPACE: &str = "RWS_CONFIG_GATEWAY_API_LEASE_NAMESPACE";
/// `shared` (default: the controller's pods serve every Gateway) or
/// `provisioned` (a Deployment per Gateway, [`provision`]).
pub const RWS_CONFIG_GATEWAY_API_MODE: &str = "RWS_CONFIG_GATEWAY_API_MODE";
/// The image provisioned Deployments run.
pub const RWS_CONFIG_GATEWAY_API_IMAGE: &str = "RWS_CONFIG_GATEWAY_API_IMAGE";
/// The ServiceAccount provisioned pods run as (default: the pod's, from
/// `POD_SERVICE_ACCOUNT`).
pub const RWS_CONFIG_GATEWAY_API_SERVICE_ACCOUNT: &str = "RWS_CONFIG_GATEWAY_API_SERVICE_ACCOUNT";
/// Set on provisioned pods: `namespace/name` of the one Gateway they serve;
/// they don't take the Lease or write anything.
pub const RWS_CONFIG_GATEWAY_API_GATEWAY: &str = "RWS_CONFIG_GATEWAY_API_GATEWAY";

fn setting(key: &str) -> Option<String> {
    std::env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Whether the controller is turned on.
pub fn enabled() -> bool {
    setting(RWS_CONFIG_GATEWAY_API_ENABLED).is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on"))
}

/// `key=value,key=value`.
pub fn parse_labels(value: &str) -> BTreeMap<String, String> {
    value.split(',').filter_map(|pair| pair.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).filter(|(k, _)| !k.is_empty()).collect()
}

/// `first-last`.
pub fn parse_port_range(value: &str) -> Option<(u16, u16)> {
    let (first, last) = value.split_once('-')?;
    let (first, last) = (first.trim().parse::<u16>().ok()?, last.trim().parse::<u16>().ok()?);
    (first >= 1024 && first <= last).then_some((first, last))
}

/// The translation settings, from `RWS_CONFIG_GATEWAY_API_*`.
pub fn settings_from_env() -> translate::Settings {
    let mut settings = translate::Settings::default();
    if let Some(name) = setting(RWS_CONFIG_GATEWAY_API_CONTROLLER_NAME) {
        settings.controller_name = name;
    }
    if let Some(selector) = setting(RWS_CONFIG_GATEWAY_API_POD_SELECTOR).map(|v| parse_labels(&v)).filter(|l| !l.is_empty()) {
        settings.services.pod_selector = selector;
    }
    if let Some(range) = setting(RWS_CONFIG_GATEWAY_API_PORT_RANGE).and_then(|v| parse_port_range(&v)) {
        settings.services.port_range = range;
    }
    if let Some(kind) = setting(RWS_CONFIG_GATEWAY_API_SERVICE_TYPE) {
        settings.services.service_type = kind;
    }
    settings.services.pod_namespace = pod_namespace();
    if setting(RWS_CONFIG_GATEWAY_API_MODE).is_some_and(|m| m.eq_ignore_ascii_case("provisioned")) {
        settings.services.provisioned = true;
        settings.provision = Some(provision::ProvisionSettings {
            image: setting(RWS_CONFIG_GATEWAY_API_IMAGE).unwrap_or_else(|| format!("ghcr.io/bohdaq/rws:{}", env!("CARGO_PKG_VERSION"))),
            service_account: setting(RWS_CONFIG_GATEWAY_API_SERVICE_ACCOUNT).or_else(|| setting("POD_SERVICE_ACCOUNT")).unwrap_or_else(|| "default".to_string()),
            ..Default::default()
        });
    }
    settings
}

/// `namespace/name` from [`RWS_CONFIG_GATEWAY_API_GATEWAY`]: this pod is a
/// provisioned data plane for that Gateway only.
pub fn own_gateway() -> Option<(String, String)> {
    let value = setting(RWS_CONFIG_GATEWAY_API_GATEWAY)?;
    let (namespace, name) = value.split_once('/')?;
    (!namespace.is_empty() && !name.is_empty()).then(|| (namespace.to_string(), name.to_string()))
}

/// The pod's namespace, where the serving pods run: `POD_NAMESPACE`, or
/// the service account's.
fn pod_namespace() -> String {
    setting("POD_NAMESPACE")
        .or_else(|| std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/namespace").ok().map(|n| n.trim().to_string()))
        .unwrap_or_else(|| "default".to_string())
}

/// Where the Lease lives: the setting, else the pod's namespace.
fn lease_namespace() -> String {
    setting(RWS_CONFIG_GATEWAY_API_LEASE_NAMESPACE).unwrap_or_else(pod_namespace)
}

/// When enabled, start the controller, leader election and data plane on
/// the current tokio runtime, against the cluster rws runs in (or the
/// kubeconfig). Returns at once.
pub fn start_from_env() {
    if !enabled() {
        return;
    }
    tokio::spawn(async {
        let client = match kube::Client::try_default().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("[gateway-api] no Kubernetes API: {e}");
                return;
            }
        };
        let settings = settings_from_env();
        let leader = Arc::new(AtomicBool::new(false));
        let became_leader = Arc::new(tokio::sync::Notify::new());
        let own = own_gateway();
        match &own {
            // A provisioned data plane: never the leader.
            Some((namespace, name)) => println!("[gateway-api] serving Gateway {namespace}/{name}"),
            None => {
                println!("[gateway-api] serving GatewayClasses with controllerName {}", settings.controller_name);
                let identity = setting("POD_NAME").or_else(|| setting("HOSTNAME")).unwrap_or_else(|| format!("rws-{}", std::process::id()));
                let lease = lease::LeaseSettings { namespace: lease_namespace(), name: "rws-gateway-controller".to_string(), identity };
                tokio::spawn(lease::run(client.clone(), lease, Arc::clone(&leader), Arc::clone(&became_leader)));
            }
        }
        let plane = Arc::new(dataplane::DataPlane::new("0.0.0.0"));
        let provisioned = settings.provision.is_some();
        let on_config = move |mut config: translate::GatewayConfig| {
            // Provisioned: each pod serves its own Gateway; the controller's serve none.
            match &own {
                Some(gateway) => config.listeners.retain(|l| &l.gateway == gateway),
                None if provisioned => config.listeners.clear(),
                None => {}
            }
            plane.apply(&config)
        };
        let options = controller::Options { settings, leader, on_config: Arc::new(on_config) };
        controller::run(client, options, became_leader).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_parse() {
        assert_eq!(BTreeMap::from([("a".to_string(), "1".to_string()), ("b".to_string(), "x".to_string())]), parse_labels(" a=1, b = x ,junk"));
        assert_eq!(Some((10000, 10999)), parse_port_range("10000-10999"));
        assert_eq!(None, parse_port_range("80-90"), "no privileged ports");
        assert_eq!(None, parse_port_range("2000-1000"));
        assert_eq!(None, parse_port_range("x"));
    }
}
