//! HTTPRoutes compiled for the data plane: matches, filters, and backends
//! resolved to the ready endpoints of their Services (through the Service's
//! port → EndpointSlice port), so the router does no lookups per request.

use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::ResourceExt;

use super::resources::{GrpcRoute, HttpRoute};
use super::translate::Snapshot;
use super::types::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledHttpRoute {
    /// (namespace, name).
    pub key: (String, String),
    pub hostnames: Vec<String>,
    /// `creationTimestamp` (RFC 3339): older routes win ties.
    pub created: String,
    pub rules: Vec<CompiledRule>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledRule {
    pub name: Option<String>,
    /// Never empty: no matches means one match on prefix `/`.
    pub matches: Vec<HttpRouteMatch>,
    pub filters: Vec<HttpRouteFilter>,
    pub backends: Vec<Backend>,
    pub timeouts: Option<HttpRouteTimeouts>,
    pub retry: Option<HttpRouteRetry>,
    /// The rule's `RequestMirror` filters, resolved.
    pub mirrors: Vec<Mirror>,
}

/// Where a `RequestMirror` filter sends copies, and how many.
#[derive(Clone, Debug, PartialEq)]
pub struct Mirror {
    pub endpoints: Result<Vec<String>, String>,
    /// Share of requests mirrored, 0.0–1.0 (`percent` or `fraction`;
    /// all when neither is set).
    pub share: f64,
}

impl Mirror {
    fn from_filter(filter: &HttpRequestMirrorFilter, namespace: &str, snapshot: &Snapshot) -> Mirror {
        let share = match (&filter.fraction, filter.percent) {
            (Some(f), _) => f.numerator as f64 / f.denominator.unwrap_or(100).max(1) as f64,
            (None, Some(percent)) => percent as f64 / 100.0,
            (None, None) => 1.0,
        };
        Mirror { endpoints: resolve(&filter.backend_ref, namespace, "HTTPRoute", snapshot).0, share: share.clamp(0.0, 1.0) }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Backend {
    pub weight: u32,
    /// `ip:port` of every ready endpoint; `Err` for a reference that doesn't
    /// resolve (its share of requests gets `500`, as the spec says).
    pub endpoints: Result<Vec<String>, String>,
    /// The Service port's `appProtocol` (`kubernetes.io/h2c`, `kubernetes.io/ws`).
    pub app_protocol: Option<String>,
    pub filters: Vec<HttpRouteFilter>,
}

pub fn compile(route: &HttpRoute, snapshot: &Snapshot) -> CompiledHttpRoute {
    let namespace = route.namespace().unwrap_or_default();
    let rules = route
        .spec
        .rules
        .iter()
        .map(|rule| CompiledRule {
            name: rule.name.clone(),
            matches: if rule.matches.is_empty() {
                vec![HttpRouteMatch { path: Some(HttpPathMatch { match_type: Some("PathPrefix".to_string()), value: Some("/".to_string()) }), ..Default::default() }]
            } else {
                rule.matches.clone()
            },
            filters: rule.filters.clone(),
            backends: rule
                .backend_refs
                .iter()
                .map(|b| {
                    let (endpoints, app_protocol) = resolve(&b.backend_ref.object, &namespace, "HTTPRoute", snapshot);
                    Backend { weight: b.backend_ref.weight.unwrap_or(1).max(0) as u32, endpoints, app_protocol, filters: b.filters.clone() }
                })
                .collect(),
            timeouts: rule.timeouts.clone(),
            retry: rule.retry.clone(),
            mirrors: rule
                .filters
                .iter()
                .filter(|f| f.filter_type == "RequestMirror")
                .filter_map(|f| f.request_mirror.as_ref())
                .map(|f| Mirror::from_filter(f, &namespace, snapshot))
                .collect(),
        })
        .collect();
    CompiledHttpRoute {
        key: (namespace, route.name_any()),
        hostnames: route.spec.hostnames.clone(),
        created: route.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()).unwrap_or_default(),
        rules,
    }
}

/// The `matches.path` type a gRPC `method` match with only a method
/// compiles to: the value is the method, matched as the path's second
/// segment of `/<service>/<method>` (not an API type; internal).
pub const GRPC_METHOD_MATCH: &str = "rws.GRPCMethod";

/// A GRPCRoute compiled to the same form as an HTTPRoute: a `method` match
/// becomes a path match on `/<service>/<method>` (`Exact` for both,
/// `PathPrefix` `/<service>` for a service alone, [`GRPC_METHOD_MATCH`] for
/// a method alone); `RegularExpression` matches nothing (implementation
/// specific, not supported). Backends are reached over h2c.
pub fn compile_grpc(route: &GrpcRoute, snapshot: &Snapshot) -> CompiledHttpRoute {
    let namespace = route.namespace().unwrap_or_default();
    let path = |kind: &str, value: String| Some(HttpPathMatch { match_type: Some(kind.to_string()), value: Some(value) });
    let rules = route
        .spec
        .rules
        .iter()
        .map(|rule| {
            let matches = if rule.matches.is_empty() {
                vec![HttpRouteMatch { path: path("PathPrefix", "/".to_string()), ..Default::default() }]
            } else {
                rule.matches
                    .iter()
                    .map(|m| {
                        let path = match &m.method {
                            None => path("PathPrefix", "/".to_string()),
                            Some(method) if method.match_type.as_deref().unwrap_or("Exact") != "Exact" => path("rws.Unsupported", String::new()),
                            Some(method) => match (&method.service, &method.method) {
                                (Some(service), Some(name)) => path("Exact", format!("/{service}/{name}")),
                                (Some(service), None) => path("PathPrefix", format!("/{service}")),
                                (None, Some(name)) => path(GRPC_METHOD_MATCH, name.clone()),
                                (None, None) => path("PathPrefix", "/".to_string()),
                            },
                        };
                        HttpRouteMatch { path, headers: m.headers.clone(), ..Default::default() }
                    })
                    .collect()
            };
            CompiledRule {
                name: rule.name.clone(),
                matches,
                filters: rule.filters.clone(),
                backends: rule
                    .backend_refs
                    .iter()
                    .map(|b| {
                        let (endpoints, _) = resolve(&b.backend_ref.object, &namespace, "GRPCRoute", snapshot);
                        Backend {
                            weight: b.backend_ref.weight.unwrap_or(1).max(0) as u32,
                            endpoints,
                            app_protocol: Some("kubernetes.io/h2c".to_string()),
                            filters: b.filters.clone(),
                        }
                    })
                    .collect(),
                timeouts: None,
                retry: None,
                mirrors: rule
                    .filters
                    .iter()
                    .filter(|f| f.filter_type == "RequestMirror")
                    .filter_map(|f| f.request_mirror.as_ref())
                    .map(|f| Mirror::from_filter(f, &namespace, snapshot))
                    .collect(),
            }
        })
        .collect();
    CompiledHttpRoute {
        key: (namespace, route.name_any()),
        hostnames: route.spec.hostnames.clone(),
        created: route.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()).unwrap_or_default(),
        rules,
    }
}

/// A backendRef → its ready endpoints and `appProtocol`.
pub fn resolve(reference: &BackendObjectReference, route_namespace: &str, route_kind: &str, snapshot: &Snapshot) -> (Result<Vec<String>, String>, Option<String>) {
    if !reference.group().is_empty() || reference.kind() != "Service" {
        return (Err(format!("unsupported backend kind {}", reference.kind())), None);
    }
    let namespace = reference.namespace.clone().unwrap_or_else(|| route_namespace.to_string());
    if namespace != route_namespace && !super::translate::granted(snapshot, GATEWAY_GROUP, route_kind, route_namespace, "", "Service", &namespace, &reference.name) {
        return (Err("not permitted by a ReferenceGrant".to_string()), None);
    }
    let Some(service) = snapshot.services.iter().find(|s| s.namespace().as_deref() == Some(namespace.as_str()) && s.name_any() == reference.name) else {
        return (Err(format!("Service {namespace}/{} not found", reference.name)), None);
    };
    let Some(port) = reference.port else {
        return (Err("a Service backend needs a port".to_string()), None);
    };
    let service_port = service.spec.as_ref().and_then(|s| s.ports.as_ref()).and_then(|ports| ports.iter().find(|p| p.port == port)).cloned();
    let Some(service_port) = service_port else {
        return (Err(format!("Service {namespace}/{} has no port {port}", reference.name)), None);
    };
    let endpoints = endpoints(snapshot, &namespace, &service.name_any(), service_port.name.as_deref());
    (Ok(endpoints), service_port.app_protocol.clone())
}

/// Ready endpoints of a Service port, from its EndpointSlices.
fn endpoints(snapshot: &Snapshot, namespace: &str, service: &str, port_name: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let slices = snapshot.endpoint_slices.iter().filter(|s: &&EndpointSlice| {
        s.namespace().as_deref() == Some(namespace) && s.labels().get("kubernetes.io/service-name").map(String::as_str) == Some(service)
    });
    for slice in slices {
        let Some(port) = slice
            .ports
            .iter()
            .flatten()
            .find(|p| p.name.as_deref().unwrap_or("") == port_name.unwrap_or(""))
            .and_then(|p| p.port)
        else {
            continue;
        };
        for endpoint in &slice.endpoints {
            if endpoint.conditions.as_ref().and_then(|c| c.ready) == Some(false) {
                continue;
            }
            for address in &endpoint.addresses {
                out.push(if address.contains(':') { format!("[{address}]:{port}") } else { format!("{address}:{port}") });
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests;
