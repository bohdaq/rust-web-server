//! The Gateway API **mesh profile** (GAMMA), stage 1 — routing only, no mTLS
//! (`spec/GATEWAY_MESH.md`).
//!
//! HTTPRoutes and GRPCRoutes whose `parentRefs` name a Service (`group: ""`,
//! `kind: Service`) route the traffic pods send to that Service. A sidecar
//! in each pod ([`sidecar`]) receives the pod's outbound TCP connections; the
//! connection's **original destination** — the Service's ClusterIP and port,
//! not its `Host` header — picks the routes, and the gateway's
//! [`PortRouter`](super::router::PortRouter) applies them (matches, filters,
//! weights, redirects, mirrors, timeouts, retries). Anything else goes to
//! its original destination unchanged.
//!
//! The controller hands sidecars a [`MeshSnapshot`] — the routes, the
//! Services they name and those Services' EndpointSlices — over HTTP
//! ([`config`]); each sidecar compiles it with [`table`] for its own
//! namespace, so it needs no Kubernetes API access.
//!
//! Which routes apply to a client in namespace `ns`, for a Service in
//! namespace `s`:
//! - routes in `ns` naming the Service (**consumer routes**) when `ns != s`
//!   and there are any;
//! - else the routes in `s` naming it (**producer routes**).
//!
//! A route's `parentRef.port` limits it to that Service port.

pub mod config;
pub mod inject;
pub mod sidecar;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::ResourceExt;
use serde::{Deserialize, Serialize};

use super::resources::{GrpcRoute, HttpRoute, ReferenceGrant};
use super::routes::CompiledHttpRoute;
use super::translate::{ListenerConfig, Snapshot};
use super::types::*;

/// What a sidecar needs to route: the mesh routes, the Services they name
/// as parents or backends, and those Services' EndpointSlices.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshSnapshot {
    #[serde(default)]
    pub http_routes: Vec<HttpRoute>,
    #[serde(default)]
    pub grpc_routes: Vec<GrpcRoute>,
    #[serde(default)]
    pub services: Vec<Service>,
    #[serde(default)]
    pub endpoint_slices: Vec<EndpointSlice>,
    #[serde(default)]
    pub reference_grants: Vec<ReferenceGrant>,
}

/// Whether `parent` names a Service (`group: ""`, `kind: Service`).
pub fn is_service_parent(parent: &ParentReference) -> bool {
    parent.group() == "" && parent.kind() == "Service"
}

/// The Service a parentRef names: (namespace, name).
fn parent_service(parent: &ParentReference, route_namespace: &str) -> (String, String) {
    (parent.namespace.clone().unwrap_or_else(|| route_namespace.to_string()), parent.name.clone())
}

/// The mesh part of what the controller watches.
pub fn snapshot(from: &Snapshot) -> MeshSnapshot {
    let meshy = |parents: &[ParentReference]| parents.iter().any(is_service_parent);
    let http_routes: Vec<HttpRoute> = from.http_routes.iter().filter(|r| meshy(&r.spec.parent_refs)).cloned().collect();
    let grpc_routes: Vec<GrpcRoute> = from.grpc_routes.iter().filter(|r| meshy(&r.spec.parent_refs)).cloned().collect();
    // Services named as parents or backends.
    let mut wanted: std::collections::BTreeSet<(String, String)> = Default::default();
    let mut note = |namespace: &str, parents: &[ParentReference], backends: Vec<&BackendObjectReference>| {
        for p in parents.iter().filter(|p| is_service_parent(p)) {
            wanted.insert(parent_service(p, namespace));
        }
        for b in backends {
            wanted.insert((b.namespace.clone().unwrap_or_else(|| namespace.to_string()), b.name.clone()));
        }
    };
    for r in &http_routes {
        let namespace = r.namespace().unwrap_or_default();
        note(&namespace, &r.spec.parent_refs, r.spec.rules.iter().flat_map(|rule| rule.backend_refs.iter().map(|b| &b.backend_ref.object)).collect());
    }
    for r in &grpc_routes {
        let namespace = r.namespace().unwrap_or_default();
        note(&namespace, &r.spec.parent_refs, r.spec.rules.iter().flat_map(|rule| rule.backend_refs.iter().map(|b| &b.backend_ref.object)).collect());
    }
    let named = |namespace: Option<String>, name: String| wanted.contains(&(namespace.unwrap_or_default(), name));
    let services = from.services.iter().filter(|s| named(s.namespace(), s.name_any())).cloned().collect();
    let endpoint_slices = from
        .endpoint_slices
        .iter()
        .filter(|e| e.labels().get("kubernetes.io/service-name").is_some_and(|service| named(e.namespace(), service.clone())))
        .cloned()
        .collect();
    MeshSnapshot { http_routes, grpc_routes, services, endpoint_slices, reference_grants: from.reference_grants.clone() }
}

/// A Service port's routes, as the sidecar serves them: one listener (the
/// gateway router's unit) holding the routes, by original destination.
pub type MeshTable = HashMap<(String, u16), Arc<Vec<ListenerConfig>>>;

/// A route's Service parents with the port each limits it to.
fn service_parents(parents: &[ParentReference], namespace: &str) -> Vec<((String, String), Option<i32>)> {
    parents.iter().filter(|p| is_service_parent(p)).map(|p| (parent_service(p, namespace), p.port)).collect()
}

/// The routes for clients in `client_namespace`, by (ClusterIP, port).
pub fn table(mesh: &MeshSnapshot, client_namespace: &str) -> MeshTable {
    let snapshot = compile_snapshot(mesh);
    // (Service, port limit, route namespace, compiled route).
    let mut attached: Vec<((String, String), Option<i32>, String, CompiledHttpRoute)> = Vec::new();
    for r in &mesh.http_routes {
        let namespace = r.namespace().unwrap_or_default();
        let compiled = without_hostnames(super::routes::compile(r, &snapshot));
        for (service, port) in service_parents(&r.spec.parent_refs, &namespace) {
            attached.push((service, port, namespace.clone(), compiled.clone()));
        }
    }
    for r in &mesh.grpc_routes {
        let namespace = r.namespace().unwrap_or_default();
        let compiled = without_hostnames(super::routes::compile_grpc(r, &snapshot));
        for (service, port) in service_parents(&r.spec.parent_refs, &namespace) {
            attached.push((service, port, namespace.clone(), compiled.clone()));
        }
    }
    let mut table = MeshTable::new();
    for service in &mesh.services {
        let key = (service.namespace().unwrap_or_default(), service.name_any());
        let routes_here = |namespace: &str| attached.iter().filter(|(s, _, n, _)| *s == key && n == namespace).collect::<Vec<_>>();
        // Consumer routes win for their own namespace's clients.
        let consumer = if client_namespace != key.0 { routes_here(client_namespace) } else { Vec::new() };
        let chosen = if consumer.is_empty() { routes_here(&key.0) } else { consumer };
        if chosen.is_empty() {
            continue;
        }
        let Some(spec) = &service.spec else { continue };
        let ips = cluster_ips(spec);
        for port in spec.ports.iter().flatten() {
            let routes: Vec<CompiledHttpRoute> = chosen.iter().filter(|(_, limit, _, _)| limit.is_none_or(|p| p == port.port)).map(|(_, _, _, r)| r.clone()).collect();
            if routes.is_empty() {
                continue;
            }
            let listener = ListenerConfig {
                gateway: key.clone(),
                name: port.name.clone().unwrap_or_else(|| port.port.to_string()),
                port: port.port,
                protocol: "HTTP".to_string(),
                hostname: None,
                container_port: port.port as u16,
                tls: None,
                client_ca: None,
                backend_client_cert: None,
                request_filters: Vec::new(),
                http_routes: routes,
            };
            let listeners = Arc::new(vec![listener]);
            for ip in &ips {
                table.insert((ip.clone(), port.port as u16), Arc::clone(&listeners));
            }
        }
    }
    table
}

/// A Service's ClusterIPs (none for a headless one).
fn cluster_ips(spec: &k8s_openapi::api::core::v1::ServiceSpec) -> Vec<String> {
    let mut ips: Vec<String> = spec.cluster_ips.clone().unwrap_or_default();
    if let Some(ip) = &spec.cluster_ip {
        if !ips.contains(ip) {
            ips.push(ip.clone());
        }
    }
    ips.retain(|ip| !ip.is_empty() && ip != "None");
    ips
}

/// Mesh routes ignore `hostnames`: the destination picked them.
fn without_hostnames(mut route: CompiledHttpRoute) -> CompiledHttpRoute {
    route.hostnames.clear();
    route
}

/// The snapshot routes are compiled against. A consumer route may send to
/// the Service it's attached to in another namespace without a
/// ReferenceGrant (GAMMA); those grants are added here.
fn compile_snapshot(mesh: &MeshSnapshot) -> Snapshot {
    let mut grants = mesh.reference_grants.clone();
    let mut implied = |kind: &str, namespace: &str, parents: &[ParentReference]| {
        for ((service_namespace, name), _) in service_parents(parents, namespace) {
            if service_namespace != namespace {
                grants.push(implied_grant(kind, namespace, &service_namespace, &name));
            }
        }
    };
    for r in &mesh.http_routes {
        implied("HTTPRoute", &r.namespace().unwrap_or_default(), &r.spec.parent_refs);
    }
    for r in &mesh.grpc_routes {
        implied("GRPCRoute", &r.namespace().unwrap_or_default(), &r.spec.parent_refs);
    }
    Snapshot { services: mesh.services.clone(), endpoint_slices: mesh.endpoint_slices.clone(), reference_grants: grants, ..Default::default() }
}

fn implied_grant(kind: &str, from_namespace: &str, to_namespace: &str, to_name: &str) -> ReferenceGrant {
    let mut grant = ReferenceGrant::new(
        "rws-mesh-consumer",
        &super::resources::reference_grants(),
        ReferenceGrantSpec {
            from: vec![ReferenceGrantFrom { group: GATEWAY_GROUP.to_string(), kind: kind.to_string(), namespace: from_namespace.to_string() }],
            to: vec![ReferenceGrantTo { group: String::new(), kind: "Service".to_string(), name: Some(to_name.to_string()) }],
        },
    );
    grant.metadata.namespace = Some(to_namespace.to_string());
    grant
}

/// Accepted/ResolvedRefs of a route's Service parent: (accepted, reason,
/// message). The Service must exist; that it has the `port` too when set.
pub fn parent_acceptance(parent: &ParentReference, route_namespace: &str, services: &[Service]) -> (bool, &'static str, String) {
    let (namespace, name) = parent_service(parent, route_namespace);
    let Some(service) = services.iter().find(|s| s.namespace().as_deref() == Some(namespace.as_str()) && s.name_any() == name) else {
        return (false, "NoMatchingParent", format!("Service {namespace}/{name} not found"));
    };
    if let Some(port) = parent.port {
        let has_port = service.spec.as_ref().and_then(|s| s.ports.as_ref()).is_some_and(|ports| ports.iter().any(|p| p.port == port));
        if !has_port {
            return (false, "NoMatchingParent", format!("Service {namespace}/{name} has no port {port}"));
        }
    }
    (true, "Accepted", "Route is accepted".to_string())
}

/// Whether every backend of a route outside its namespace is one of its own
/// Service parents (what a consumer route may reference without a grant).
pub fn only_parent_references(references: &[&BackendObjectReference], parents: &[ParentReference], route_namespace: &str) -> bool {
    let parents = service_parents(parents, route_namespace);
    references.iter().all(|r| {
        let namespace = r.namespace.clone().unwrap_or_else(|| route_namespace.to_string());
        namespace == route_namespace || parents.iter().any(|((n, name), _)| *n == namespace && *name == r.name)
    })
}

/// The routes of a table by destination, for logs: `ip:port → n routes`.
pub fn describe(table: &MeshTable) -> BTreeMap<String, usize> {
    table.iter().map(|((ip, port), listeners)| (format!("{ip}:{port}"), listeners.iter().map(|l| l.http_routes.len()).sum())).collect()
}

#[cfg(test)]
mod tests;
