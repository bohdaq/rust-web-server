//! The Service rws keeps for each Gateway: the Gateway's own address
//! (`LoadBalancer` by default), one port per listener port, each mapped to a
//! container port allocated from a range — the pods tell Gateways apart by
//! the port a connection arrives on, and never bind ports below 1024.
//!
//! Allocations live in the Services themselves: a listener port keeps its
//! container port for as long as its Service has it, so every replica, and
//! every restart, agrees on the mapping without any other storage.

use std::collections::{BTreeMap, BTreeSet};

use k8s_openapi::api::core::v1::{Service, ServicePort, ServiceSpec};
use k8s_openapi::api::discovery::v1::{Endpoint, EndpointConditions, EndpointPort, EndpointSlice};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::ResourceExt;

use super::resources::Gateway;
use super::translate::{Snapshot, GATEWAY_NAME_LABEL, MANAGED_BY};
use super::types::{GatewaySpecAddress, GATEWAY_GROUP};

/// How Gateways' Services are made.
#[derive(Clone, Debug)]
pub struct ServiceSettings {
    /// Labels of the pods that serve the traffic (the shared Deployment's).
    pub pod_selector: BTreeMap<String, String>,
    /// Their namespace. A Service only selects pods in its own namespace, so
    /// each Gateway's Service has no selector: rws writes its EndpointSlice.
    pub pod_namespace: String,
    /// Provisioned mode: each Gateway's own pods ([`super::provision`])
    /// instead of `pod_selector`.
    pub provisioned: bool,
    /// Container ports to allocate from (inclusive).
    pub port_range: (u16, u16),
    /// `LoadBalancer` (default), `NodePort` or `ClusterIP`.
    pub service_type: String,
}

impl Default for ServiceSettings {
    fn default() -> Self {
        ServiceSettings {
            pod_selector: BTreeMap::from([("app.kubernetes.io/name".to_string(), "rws".to_string())]),
            pod_namespace: "default".to_string(),
            provisioned: false,
            port_range: (10000, 10999),
            service_type: "LoadBalancer".to_string(),
        }
    }
}

/// The transport a listener protocol needs.
pub fn transport(protocol: &str) -> &'static str {
    if protocol == "UDP" { "UDP" } else { "TCP" }
}

/// The Service's name for a Gateway: `rws-<gateway>`, shortened with a hash
/// past the 63 characters a Service name may have.
pub fn service_name(gateway: &str) -> String {
    let name = format!("rws-{gateway}");
    if name.len() <= 63 {
        return name;
    }
    let hash: String = crate::core::sha256::sha256(gateway.as_bytes())[..4].iter().map(|b| format!("{b:02x}")).collect();
    let head: String = name.chars().take(54).collect();
    format!("{}-{hash}", head.trim_end_matches(['-', '.']))
}

fn is_ours(service: &Service) -> bool {
    service.labels().get("app.kubernetes.io/managed-by").map(String::as_str) == Some(MANAGED_BY)
}

/// The Service rws manages for this Gateway, as it is now.
pub fn current<'a>(snapshot: &'a Snapshot, gateway: &Gateway) -> Option<&'a Service> {
    let namespace = gateway.namespace().unwrap_or_default();
    let name = service_name(&gateway.name_any());
    snapshot.services.iter().find(|s| s.namespace().as_deref() == Some(namespace.as_str()) && s.name_any() == name && is_ours(s))
}

/// (listener port, transport) → container port, as the Gateway's current
/// Service maps it.
pub fn container_ports(service: Option<&Service>) -> BTreeMap<(i32, String), u16> {
    let mut ports = BTreeMap::new();
    for port in service.and_then(|s| s.spec.as_ref()).and_then(|s| s.ports.as_ref()).into_iter().flatten() {
        if let Some(IntOrString::Int(target)) = port.target_port {
            let transport = port.protocol.clone().unwrap_or_else(|| "TCP".to_string());
            ports.insert((port.port, transport), target as u16);
        }
    }
    ports
}

/// Address types rws can ask a load balancer for: `IPAddress` (the default).
pub fn address_supported(address: &GatewaySpecAddress) -> bool {
    address.address_type.as_deref().unwrap_or("IPAddress") == "IPAddress"
}

/// The static addresses a Gateway asks for (`spec.addresses` with a value;
/// one with only a type asks for any address).
pub fn requested_addresses(gateway: &Gateway) -> Vec<&str> {
    gateway.spec.addresses.iter().filter(|a| address_supported(a) && !a.value.is_empty()).map(|a| a.value.as_str()).collect()
}

/// The Service every one of `gateways` should have, allocating container
/// ports for new listener ports (keeping every existing allocation).
pub fn desired(snapshot: &Snapshot, gateways: &[&Gateway], settings: &ServiceSettings) -> BTreeMap<(String, String), Service> {
    // Container ports in use by any of our Services: the pods are shared.
    let mut used: BTreeSet<u16> = snapshot.services.iter().filter(|s| is_ours(s)).flat_map(|s| container_ports(Some(s)).into_values()).collect();
    let mut out = BTreeMap::new();
    for gateway in gateways {
        let namespace = gateway.namespace().unwrap_or_default();
        let existing = container_ports(current(snapshot, gateway));
        let mut wanted: BTreeSet<(i32, String)> = BTreeSet::new();
        for listener in &gateway.spec.listeners {
            wanted.insert((listener.port, transport(&listener.protocol).to_string()));
        }
        let mut ports = Vec::new();
        for (port, transport) in wanted {
            let container = match existing.get(&(port, transport.clone())) {
                Some(container) => *container,
                None => match (settings.port_range.0..=settings.port_range.1).find(|p| !used.contains(p)) {
                    Some(free) => {
                        used.insert(free);
                        free
                    }
                    None => continue, // range exhausted: the listener stays unprogrammed
                },
            };
            ports.push(ServicePort {
                name: Some(format!("{}-{port}", transport.to_ascii_lowercase())),
                port,
                protocol: Some(transport),
                target_port: Some(IntOrString::Int(container as i32)),
                ..Default::default()
            });
        }
        let infrastructure = gateway.spec.infrastructure.clone().unwrap_or_default();
        let mut labels = infrastructure.labels.clone();
        labels.insert(GATEWAY_NAME_LABEL.to_string(), gateway.name_any());
        labels.insert("app.kubernetes.io/managed-by".to_string(), MANAGED_BY.to_string());
        let service = Service {
            metadata: ObjectMeta {
                name: Some(service_name(&gateway.name_any())),
                namespace: Some(namespace.clone()),
                labels: Some(labels),
                annotations: (!infrastructure.annotations.is_empty()).then(|| infrastructure.annotations.clone()),
                owner_references: gateway.metadata.uid.clone().map(|uid| {
                    vec![OwnerReference {
                        api_version: format!("{GATEWAY_GROUP}/v1"),
                        kind: "Gateway".to_string(),
                        name: gateway.name_any(),
                        uid,
                        controller: Some(true),
                        block_owner_deletion: Some(true),
                    }]
                }),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                type_: Some(settings.service_type.clone()),
                // The first static address (a load balancer takes one per Service).
                load_balancer_ip: requested_addresses(gateway).first().map(|a| a.to_string()),
                selector: None,
                ports: Some(ports),
                ..Default::default()
            }),
            status: None,
        };
        out.insert((namespace, service_name(&gateway.name_any())), service);
    }
    out
}

/// `endpointslice.kubernetes.io/managed-by` on the EndpointSlices rws writes
/// (not the EndpointSlice controller's value, so it leaves them alone; a
/// label value, so no `/`).
pub const ENDPOINTS_MANAGED_BY: &str = "gateway-controller.rws8.tech";

impl ServiceSettings {
    /// The labels rws's serving pods are watched by.
    pub fn pod_watch_labels(&self) -> BTreeMap<String, String> {
        if self.provisioned {
            BTreeMap::from([("app.kubernetes.io/name".to_string(), super::provision::APP_NAME.to_string())])
        } else {
            self.pod_selector.clone()
        }
    }
}

/// The serving pods: in the pod namespace, matching `selector`, with an IP,
/// not being deleted; (IP, ready, node, name).
pub fn serving_pods(snapshot: &Snapshot, settings: &ServiceSettings, selector: &BTreeMap<String, String>) -> Vec<(String, bool, Option<String>, String)> {
    let mut pods: Vec<_> = snapshot
        .pods
        .iter()
        .filter(|p| p.namespace().as_deref() == Some(settings.pod_namespace.as_str()))
        .filter(|p| selector.iter().all(|(k, v)| p.labels().get(k) == Some(v)))
        .filter(|p| p.metadata.deletion_timestamp.is_none())
        .filter_map(|p| {
            let status = p.status.as_ref()?;
            let ip = status.pod_ip.clone().filter(|ip| !ip.is_empty())?;
            let ready = status.conditions.iter().flatten().any(|c| c.type_ == "Ready" && c.status == "True");
            Some((ip, ready, p.spec.as_ref().and_then(|s| s.node_name.clone()), p.name_any()))
        })
        .collect();
    pods.sort();
    pods
}

/// The EndpointSlice of each Gateway Service: the serving pods on the
/// Service's container ports (one slice per address family in use).
pub fn endpoint_slices(snapshot: &Snapshot, services: &BTreeMap<(String, String), Service>, settings: &ServiceSettings) -> BTreeMap<(String, String), EndpointSlice> {
    let shared = serving_pods(snapshot, settings, &settings.pod_selector);
    let mut out = BTreeMap::new();
    for ((namespace, name), service) in services {
        // Provisioned: the Gateway's own pods.
        let own;
        let pods = if settings.provisioned {
            let gateway = service.labels().get(GATEWAY_NAME_LABEL).cloned().unwrap_or_default();
            own = serving_pods(snapshot, settings, &super::provision::pod_labels(namespace, &gateway));
            &own
        } else {
            &shared
        };
        let ports: Vec<EndpointPort> = service
            .spec
            .iter()
            .flat_map(|s| s.ports.iter().flatten())
            .filter_map(|p| match p.target_port {
                Some(IntOrString::Int(target)) => Some(EndpointPort { name: p.name.clone(), port: Some(target), protocol: p.protocol.clone(), app_protocol: None }),
                _ => None,
            })
            .collect();
        for (family, v6) in [("IPv4", false), ("IPv6", true)] {
            let endpoints: Vec<Endpoint> = pods
                .iter()
                .filter(|(ip, ..)| ip.contains(':') == v6)
                .map(|(ip, ready, node, pod)| Endpoint {
                    addresses: vec![ip.clone()],
                    conditions: Some(EndpointConditions { ready: Some(*ready), serving: Some(*ready), terminating: Some(false) }),
                    node_name: node.clone(),
                    target_ref: Some(k8s_openapi::api::core::v1::ObjectReference {
                        kind: Some("Pod".to_string()),
                        name: Some(pod.clone()),
                        namespace: Some(settings.pod_namespace.clone()),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect();
            if endpoints.is_empty() && v6 {
                continue;
            }
            let slice_name = format!("{name}-{}", family.to_ascii_lowercase());
            let slice = EndpointSlice {
                metadata: ObjectMeta {
                    name: Some(slice_name.clone()),
                    namespace: Some(namespace.clone()),
                    labels: Some(BTreeMap::from([
                        ("kubernetes.io/service-name".to_string(), name.clone()),
                        ("endpointslice.kubernetes.io/managed-by".to_string(), ENDPOINTS_MANAGED_BY.to_string()),
                        ("app.kubernetes.io/managed-by".to_string(), MANAGED_BY.to_string()),
                    ])),
                    owner_references: service.metadata.owner_references.clone(),
                    ..Default::default()
                },
                address_type: family.to_string(),
                endpoints,
                ports: Some(ports.clone()),
            };
            out.insert((namespace.clone(), slice_name), slice);
        }
    }
    out
}

/// Whether an EndpointSlice already has what `desired` asks for.
pub fn same_slice(current: &EndpointSlice, desired: &EndpointSlice) -> bool {
    let endpoints = |s: &EndpointSlice| {
        let mut e: Vec<_> = s.endpoints.iter().map(|e| (e.addresses.clone(), e.conditions.as_ref().and_then(|c| c.ready))).collect();
        e.sort();
        e
    };
    let ports = |s: &EndpointSlice| {
        let mut p: Vec<_> = s.ports.iter().flatten().map(|p| (p.name.clone(), p.port, p.protocol.clone())).collect();
        p.sort();
        p
    };
    current.address_type == desired.address_type && endpoints(current) == endpoints(desired) && ports(current) == ports(desired)
}

/// Whether `current` already is what `desired` asks for (the fields rws
/// sets; the API server fills in others, like `clusterIP` and node ports).
pub fn same(current: &Service, desired: &Service) -> bool {
    let labels_ok = desired.metadata.labels.iter().flatten().all(|(k, v)| current.labels().get(k) == Some(v));
    let annotations_ok = desired.metadata.annotations.iter().flatten().all(|(k, v)| current.annotations().get(k) == Some(v));
    let (Some(c), Some(d)) = (current.spec.as_ref(), desired.spec.as_ref()) else { return false };
    let ports = |s: &ServiceSpec| -> Vec<(i32, Option<String>, Option<IntOrString>)> {
        let mut p: Vec<_> = s.ports.iter().flatten().map(|p| (p.port, p.protocol.clone(), p.target_port.clone())).collect();
        p.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        p
    };
    labels_ok && annotations_ok && c.type_ == d.type_ && c.selector == d.selector && c.load_balancer_ip == d.load_balancer_ip && ports(c) == ports(d)
}

#[cfg(test)]
mod tests;
