//! The pure step of the controller: a [`Snapshot`] of the cluster in, the
//! configuration rws serves ([`GatewayConfig`]) and the status every object
//! should have ([`Desired`]) out. No I/O, so the Gateway API's acceptance
//! rules — which routes attach to which listeners, which references resolve,
//! which conditions and reasons are reported — are unit-tested here.

use std::collections::{BTreeMap, BTreeSet};

use k8s_openapi::api::core::v1::{Namespace, Secret, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, LabelSelector, Time};
use kube::ResourceExt;

use super::resources::{Gateway, GatewayClass, GrpcRoute, HttpRoute, ReferenceGrant};
use super::routes::CompiledHttpRoute;
use super::services::ServiceSettings;
use super::types::*;

/// What translation needs besides the cluster.
#[derive(Clone, Debug)]
pub struct Settings {
    pub controller_name: String,
    pub services: ServiceSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { controller_name: super::CONTROLLER_NAME.to_string(), services: ServiceSettings::default() }
    }
}

/// Everything the controller watches, as last seen.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub classes: Vec<GatewayClass>,
    pub gateways: Vec<Gateway>,
    pub http_routes: Vec<HttpRoute>,
    pub grpc_routes: Vec<GrpcRoute>,
    pub reference_grants: Vec<ReferenceGrant>,
    pub services: Vec<Service>,
    pub namespaces: Vec<Namespace>,
    pub secrets: Vec<Secret>,
    pub endpoint_slices: Vec<EndpointSlice>,
}

/// The label rws puts on the Service it creates for a Gateway (the name the
/// Gateway API recommends), next to [`MANAGED_BY`].
pub const GATEWAY_NAME_LABEL: &str = "gateway.networking.k8s.io/gateway-name";
/// `app.kubernetes.io/managed-by` on everything the controller creates.
pub const MANAGED_BY: &str = "rws-gateway";

/// A Gateway's addresses: its Service's load-balancer ingress.
pub fn gateway_addresses(snapshot: &Snapshot, namespace: &str, name: &str) -> Vec<GatewayStatusAddress> {
    let service = snapshot.services.iter().find(|s| {
        s.namespace().as_deref() == Some(namespace)
            && s.labels().get(GATEWAY_NAME_LABEL).map(String::as_str) == Some(name)
            && s.labels().get("app.kubernetes.io/managed-by").map(String::as_str) == Some(MANAGED_BY)
    });
    let ingress = service.and_then(|s| s.status.as_ref()).and_then(|s| s.load_balancer.as_ref()).and_then(|lb| lb.ingress.clone()).unwrap_or_default();
    ingress
        .into_iter()
        .filter_map(|i| match (i.ip, i.hostname) {
            (Some(ip), _) => Some(GatewayStatusAddress { address_type: Some("IPAddress".to_string()), value: ip }),
            (None, Some(hostname)) => Some(GatewayStatusAddress { address_type: Some("Hostname".to_string()), value: hostname }),
            _ => None,
        })
        .collect()
}

/// The status each object should have; written only where it differs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Desired {
    /// GatewayClass name → status.
    pub classes: BTreeMap<String, GatewayClassStatus>,
    /// (namespace, name) → status.
    pub gateways: BTreeMap<(String, String), GatewayStatus>,
    /// (namespace, name) → this controller's parent statuses (other
    /// controllers' entries are kept when written).
    pub http_routes: BTreeMap<(String, String), Vec<RouteParentStatus>>,
    /// The same for GRPCRoutes.
    pub grpc_routes: BTreeMap<(String, String), Vec<RouteParentStatus>>,
    /// (namespace, name) → the Service each of our Gateways should have.
    pub services: BTreeMap<(String, String), Service>,
}

/// What the data plane serves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GatewayConfig {
    pub listeners: Vec<ListenerConfig>,
}

/// One accepted listener, the container port it's served on, and the
/// routes attached to it. Only listeners whose Gateway's Service already maps
/// their port are served, so every replica agrees on the port.
#[derive(Clone, Debug, PartialEq)]
pub struct ListenerConfig {
    pub gateway: (String, String),
    pub name: String,
    pub port: i32,
    pub protocol: String,
    pub hostname: Option<String>,
    pub container_port: u16,
    /// PEM certificate chain and key, for HTTPS.
    pub tls: Option<(Vec<u8>, Vec<u8>)>,
    pub http_routes: Vec<CompiledHttpRoute>,
}

/// The features rws reports on its GatewayClasses (names as in the suite's
/// `pkg/features` at Gateway API v1.6.3; sorted). Grows with each phase;
/// `FEATURES` in `.github/workflows/gateway-api.yml` must match.
pub const SUPPORTED_FEATURES: &[&str] = &[
    "GRPCRoute",
    "GRPCRouteNamedRouteRule",
    "Gateway",
    "HTTPRoute",
    "HTTPRoute303RedirectStatusCode",
    "HTTPRoute307RedirectStatusCode",
    "HTTPRoute308RedirectStatusCode",
    "HTTPRouteBackendProtocolH2C",
    "HTTPRouteBackendProtocolWebSocket",
    "HTTPRouteBackendRequestHeaderModification",
    "HTTPRouteBackendTimeout",
    "HTTPRouteCORS",
    "HTTPRouteHostRewrite",
    "HTTPRouteMethodMatching",
    "HTTPRouteNamedRouteRule",
    "HTTPRouteParentRefPort",
    "HTTPRoutePathRedirect",
    "HTTPRoutePathRewrite",
    "HTTPRoutePortRedirect",
    "HTTPRouteQueryParamMatching",
    "HTTPRouteRequestMirror",
    "HTTPRouteRequestMultipleMirrors",
    "HTTPRouteRequestPercentageMirror",
    "HTTPRouteRequestTimeout",
    "HTTPRouteResponseHeaderModification",
    "HTTPRouteSchemeRedirect",
    "ReferenceGrant",
];

pub fn translate(snapshot: &Snapshot, settings: &Settings, now: &Time) -> (GatewayConfig, Desired) {
    let controller_name = settings.controller_name.as_str();
    let mut desired = Desired::default();
    let mut config = GatewayConfig::default();

    // GatewayClasses: ours are accepted.
    let mut our_classes = BTreeSet::new();
    for class in &snapshot.classes {
        if class.spec.controller_name != controller_name {
            continue;
        }
        let name = class.name_any();
        our_classes.insert(name.clone());
        let previous = class.status.as_ref().map(|s| s.conditions.as_slice()).unwrap_or_default();
        let generation = class.metadata.generation;
        let conditions = vec![condition(previous, "Accepted", true, "Accepted", "Handled by rws", generation, now)];
        let supported_features = SUPPORTED_FEATURES.iter().map(|f| SupportedFeature { name: f.to_string() }).collect();
        desired.classes.insert(name, GatewayClassStatus { conditions, supported_features });
    }

    // Gateways of our classes, and what each listener accepts.
    let mut listeners: Vec<ListenerView> = Vec::new();
    for gateway in &snapshot.gateways {
        if !our_classes.contains(&gateway.spec.gateway_class_name) {
            continue;
        }
        listeners.extend(listener_views(gateway, snapshot));
    }

    // Every Gateway of ours gets its Service.
    let our_gateways: Vec<&Gateway> = snapshot.gateways.iter().filter(|g| our_classes.contains(&g.spec.gateway_class_name)).collect();
    desired.services = super::services::desired(snapshot, &our_gateways, &settings.services);

    // Routes: attach to listeners, resolve backends.
    let routes = snapshot
        .http_routes
        .iter()
        .map(|r| RouteIn::new("HTTPRoute", r, &r.spec.parent_refs, &r.spec.hostnames, http_references(&r.spec.rules), super::routes::compile(r, snapshot)))
        .chain(snapshot.grpc_routes.iter().map(|r| RouteIn::new("GRPCRoute", r, &r.spec.parent_refs, &r.spec.hostnames, grpc_references(&r.spec.rules), super::routes::compile_grpc(r, snapshot))));
    let mut attached: BTreeMap<(String, String, String), i32> = BTreeMap::new();
    for route in routes {
        let namespace = route.namespace.clone();
        let resolved = resolve_references(route.kind, &namespace, &route.references, snapshot);
        let mut parents = Vec::new();
        for parent_ref in route.parent_refs {
            if parent_ref.group() != GATEWAY_GROUP || parent_ref.kind() != "Gateway" {
                continue;
            }
            let gateway_namespace = parent_ref.namespace.clone().unwrap_or_else(|| namespace.clone());
            let gateway_key = (gateway_namespace.clone(), parent_ref.name.clone());
            let candidates: Vec<&ListenerView> = listeners.iter().filter(|l| l.gateway == gateway_key).collect();
            if candidates.is_empty() && !snapshot.gateways.iter().any(|g| is_named(g, &gateway_key) && our_classes.contains(&g.spec.gateway_class_name)) {
                continue; // not one of ours: another controller reports it
            }
            let old = route
                .previous
                .iter()
                .find(|p| p.controller_name == controller_name && same_parent(&p.parent_ref, parent_ref, &namespace))
                .map(|p| p.conditions.as_slice())
                .unwrap_or_default();
            let (accepted, reason, message, attached_to) = attach(route.kind, route.hostnames, parent_ref, &namespace, &candidates, snapshot);
            for listener in &attached_to {
                *attached.entry((gateway_key.0.clone(), gateway_key.1.clone(), listener.clone())).or_default() += 1;
                if let Some(view) = listeners.iter_mut().find(|l| l.gateway == gateway_key && &l.name == listener) {
                    view.http_routes.push(route.compiled.clone());
                }
            }
            let conditions = vec![
                condition(old, "Accepted", accepted, reason, &message, route.generation, now),
                match &resolved {
                    Ok(()) => condition(old, "ResolvedRefs", true, "ResolvedRefs", "All references resolved", route.generation, now),
                    Err((reason, message)) => condition(old, "ResolvedRefs", false, reason, message, route.generation, now),
                },
            ];
            parents.push(RouteParentStatus { parent_ref: parent_ref.clone(), controller_name: controller_name.to_string(), conditions });
        }
        if !parents.is_empty() {
            let statuses = if route.kind == "GRPCRoute" { &mut desired.grpc_routes } else { &mut desired.http_routes };
            statuses.insert((namespace, route.name), parents);
        }
    }

    // Gateway status now that attached routes are counted.
    for gateway in &snapshot.gateways {
        if !our_classes.contains(&gateway.spec.gateway_class_name) {
            continue;
        }
        let key = (gateway.namespace().unwrap_or_default(), gateway.name_any());
        let generation = gateway.metadata.generation;
        let previous = gateway.status.clone().unwrap_or_default();
        let container_ports = super::services::container_ports(super::services::current(snapshot, gateway));
        let mut listener_statuses = Vec::new();
        let mut any_valid = false;
        for view in listeners.iter().filter(|l| l.gateway == key) {
            let old = previous.listeners.iter().find(|l| l.name == view.name).map(|l| l.conditions.as_slice()).unwrap_or_default();
            let count = attached.get(&(key.0.clone(), key.1.clone(), view.name.clone())).copied().unwrap_or(0);
            any_valid |= view.accepted && view.programmed;
            listener_statuses.push(ListenerStatus {
                name: view.name.clone(),
                supported_kinds: view.supported_kinds.clone(),
                attached_routes: count,
                conditions: vec![
                    condition(old, "Accepted", view.accepted, view.accepted_reason, &view.message, generation, now),
                    condition(old, "Programmed", view.programmed, if view.programmed { "Programmed" } else { "Invalid" }, &view.message, generation, now),
                    condition(old, "ResolvedRefs", view.resolved_refs.is_ok(), view.resolved_refs.as_ref().err().map_or("ResolvedRefs", |(r, _)| r), view.resolved_refs.as_ref().err().map_or("", |(_, m)| m.as_str()), generation, now),
                    condition(old, "Conflicted", view.conflicted.is_some(), view.conflicted.unwrap_or("NoConflicts"), "", generation, now),
                ],
            });
            let container_port = container_ports.get(&(view.port, super::services::transport(&view.protocol).to_string()));
            if let (true, true, Some(container_port)) = (view.accepted, view.programmed, container_port) {
                config.listeners.push(ListenerConfig {
                    gateway: key.clone(),
                    name: view.name.clone(),
                    port: view.port,
                    protocol: view.protocol.clone(),
                    hostname: view.hostname.clone(),
                    container_port: *container_port,
                    tls: view.tls.clone(),
                    http_routes: view.http_routes.clone(),
                });
            }
        }
        let addresses = gateway_addresses(snapshot, &key.0, &key.1);
        let addressed = !addresses.is_empty();
        let programmed = any_valid && addressed;
        let (programmed_reason, programmed_message) = if !addressed {
            ("AddressNotAssigned", "Waiting for the Service's address")
        } else if !any_valid {
            ("Invalid", "No valid listener")
        } else {
            ("Programmed", "Serving")
        };
        let accepted_reason = if gateway.spec.listeners.is_empty() || any_valid { "Accepted" } else { "ListenersNotValid" };
        let conditions = vec![
            condition(&previous.conditions, "Accepted", true, accepted_reason, "", generation, now),
            condition(&previous.conditions, "Programmed", programmed, programmed_reason, programmed_message, generation, now),
        ];
        desired.gateways.insert(
            key,
            GatewayStatus { addresses, conditions, listeners: listener_statuses, attached_listener_sets: None },
        );
    }
    (config, desired)
}

// ── listeners ─────────────────────────────────────────────────────────────────

struct ListenerView {
    gateway: (String, String),
    name: String,
    port: i32,
    protocol: String,
    hostname: Option<String>,
    allowed: AllowedRoutes,
    supported_kinds: Vec<RouteGroupKind>,
    accepted: bool,
    accepted_reason: &'static str,
    programmed: bool,
    resolved_refs: Result<(), (&'static str, String)>,
    conflicted: Option<&'static str>,
    message: String,
    tls: Option<(Vec<u8>, Vec<u8>)>,
    http_routes: Vec<CompiledHttpRoute>,
}

/// The route kinds a protocol carries.
fn kinds_for(protocol: &str) -> &'static [&'static str] {
    match protocol {
        "HTTP" | "HTTPS" => &["HTTPRoute", "GRPCRoute"],
        _ => &[],
    }
}

fn listener_views(gateway: &Gateway, snapshot: &Snapshot) -> Vec<ListenerView> {
    let gateway_namespace = gateway.namespace().unwrap_or_default();
    let key = (gateway_namespace.clone(), gateway.name_any());
    let mut views: Vec<ListenerView> = gateway
        .spec
        .listeners
        .iter()
        .map(|l| {
            let allowed = l.allowed_routes.clone().unwrap_or_default();
            let carried = kinds_for(&l.protocol);
            let mut supported_kinds = Vec::new();
            let mut resolved_refs = Ok(());
            if allowed.kinds.is_empty() {
                supported_kinds = carried.iter().map(|k| RouteGroupKind { group: Some(GATEWAY_GROUP.to_string()), kind: k.to_string() }).collect();
            } else {
                for kind in &allowed.kinds {
                    let group_ok = kind.group.as_deref().unwrap_or(GATEWAY_GROUP) == GATEWAY_GROUP;
                    if group_ok && carried.contains(&kind.kind.as_str()) {
                        supported_kinds.push(RouteGroupKind { group: Some(GATEWAY_GROUP.to_string()), kind: kind.kind.clone() });
                    } else {
                        resolved_refs = Err(("InvalidRouteKinds", format!("{} isn't served on a {} listener", kind.kind, l.protocol)));
                    }
                }
            }
            let (accepted, accepted_reason, message) = if carried.is_empty() {
                (false, "UnsupportedProtocol", format!("protocol {} isn't supported", l.protocol))
            } else {
                (true, "Accepted", String::new())
            };
            let mut tls = None;
            if accepted && l.protocol == "HTTPS" {
                match certificates(l, &gateway_namespace, snapshot) {
                    Ok(material) => tls = Some(material),
                    Err(e) => resolved_refs = Err(e),
                }
            }
            // Invalid route kinds alone don't stop the listener serving the valid ones.
            let programmed = accepted && resolved_refs.as_ref().err().is_none_or(|(reason, _)| *reason == "InvalidRouteKinds");
            ListenerView {
                gateway: key.clone(),
                name: l.name.clone(),
                port: l.port,
                protocol: l.protocol.clone(),
                hostname: l.hostname.clone(),
                allowed,
                supported_kinds,
                accepted,
                accepted_reason,
                programmed,
                resolved_refs,
                conflicted: None,
                message,
                tls,
                http_routes: Vec::new(),
            }
        })
        .collect();

    // Conflicts: listeners on one port must agree on the protocol (HTTP vs
    // HTTPS/TLS) and differ in hostname.
    for i in 0..views.len() {
        for j in 0..views.len() {
            if i == j || views[i].port != views[j].port {
                continue;
            }
            let reason = if views[i].protocol != views[j].protocol {
                Some("ProtocolConflict")
            } else if views[i].hostname == views[j].hostname {
                Some("HostnameConflict")
            } else {
                None
            };
            if let Some(reason) = reason {
                views[i].conflicted = Some(reason);
                views[i].programmed = false;
            }
        }
    }
    views
}

/// An HTTPS listener's certificates resolve: TLS Secrets in its namespace,
/// or in another one a ReferenceGrant opens.
/// A listener's PEM certificate chain and key.
type TlsMaterial = (Vec<u8>, Vec<u8>);

fn certificates(listener: &Listener, gateway_namespace: &str, snapshot: &Snapshot) -> Result<TlsMaterial, (&'static str, String)> {
    let Some(tls) = &listener.tls else {
        return Err(("InvalidCertificateRef", "an HTTPS listener needs tls.certificateRefs".to_string()));
    };
    if tls.certificate_refs.is_empty() {
        return Err(("InvalidCertificateRef", "an HTTPS listener needs tls.certificateRefs".to_string()));
    }
    let mut material = None;
    for cert in &tls.certificate_refs {
        let group = cert.group.as_deref().unwrap_or("");
        let kind = cert.kind.as_deref().unwrap_or("Secret");
        if !group.is_empty() || kind != "Secret" {
            return Err(("InvalidCertificateRef", format!("{kind} isn't a supported certificate reference")));
        }
        let namespace = cert.namespace.clone().unwrap_or_else(|| gateway_namespace.to_string());
        if namespace != gateway_namespace && !granted(snapshot, GATEWAY_GROUP, "Gateway", gateway_namespace, "", "Secret", &namespace, &cert.name) {
            return Err(("RefNotPermitted", format!("no ReferenceGrant in {namespace} lets Gateways in {gateway_namespace} use Secret {}", cert.name)));
        }
        let secret = snapshot.secrets.iter().find(|s| s.namespace().as_deref() == Some(namespace.as_str()) && s.name_any() == cert.name);
        let valid = secret.is_some_and(|s| {
            let data = s.data.as_ref();
            let pem = |key: &str| data.and_then(|d| d.get(key)).is_some_and(|v| !v.0.is_empty());
            pem("tls.crt") && pem("tls.key") && crate::core::x509::cert_info_from_pem(&data.unwrap()["tls.crt"].0).is_some()
        });
        if !valid {
            return Err(("InvalidCertificateRef", format!("Secret {namespace}/{} isn't a valid TLS Secret", cert.name)));
        }
        if material.is_none() {
            let data = secret.and_then(|s| s.data.as_ref()).expect("checked above");
            material = Some((data["tls.crt"].0.clone(), data["tls.key"].0.clone()));
        }
    }
    material.ok_or(("InvalidCertificateRef", "no certificate".to_string()))
}

// ── routes ────────────────────────────────────────────────────────────────────

/// Whether a parentRef in a route's status names the same parent.
fn same_parent(a: &ParentReference, b: &ParentReference, route_namespace: &str) -> bool {
    a.group() == b.group()
        && a.kind() == b.kind()
        && a.name == b.name
        && a.namespace.as_deref().unwrap_or(route_namespace) == b.namespace.as_deref().unwrap_or(route_namespace)
        && a.section_name == b.section_name
        && a.port == b.port
}

fn is_named<K: kube::Resource>(object: &K, key: &(String, String)) -> bool {
    object.meta().namespace.as_deref() == Some(key.0.as_str()) && object.meta().name.as_deref() == Some(key.1.as_str())
}

/// Attach `route` through `parent_ref` to the matching listeners: (accepted,
/// reason, message, listener names).
fn attach(kind: &str, hostnames: &[String], parent_ref: &ParentReference, route_namespace: &str, candidates: &[&ListenerView], snapshot: &Snapshot) -> (bool, &'static str, String, Vec<String>) {
    let mut named: Vec<&&ListenerView> = candidates
        .iter()
        .filter(|l| parent_ref.section_name.as_ref().is_none_or(|s| *s == l.name))
        .filter(|l| parent_ref.port.is_none_or(|p| p == l.port))
        .collect();
    if named.is_empty() {
        return (false, "NoMatchingParent", "no listener matches the parentRef's sectionName/port".to_string(), Vec::new());
    }
    named.retain(|l| l.accepted && l.conflicted.is_none());
    let allowed: Vec<&&ListenerView> = named.iter().copied().filter(|l| allows(l, kind, route_namespace, snapshot)).collect();
    if allowed.is_empty() {
        return (false, "NotAllowedByListeners", "no listener allows this route".to_string(), Vec::new());
    }
    let matching: Vec<String> = allowed
        .iter()
        .filter(|l| hostnames_intersect(l.hostname.as_deref(), hostnames))
        .map(|l| l.name.clone())
        .collect();
    if matching.is_empty() {
        return (false, "NoMatchingListenerHostname", "no listener hostname matches the route's hostnames".to_string(), Vec::new());
    }
    (true, "Accepted", String::new(), matching)
}

/// A listener allows routes of `kind` from `namespace`.
fn allows(listener: &ListenerView, kind: &str, namespace: &str, snapshot: &Snapshot) -> bool {
    if !listener.supported_kinds.iter().any(|k| k.kind == kind) {
        return false;
    }
    let from = listener.allowed.namespaces.as_ref().and_then(|n| n.from.as_deref()).unwrap_or("Same");
    match from {
        "All" => true,
        "Same" => namespace == listener.gateway.0,
        "Selector" => {
            let selector = listener.allowed.namespaces.as_ref().and_then(|n| n.selector.as_ref());
            let mut labels = snapshot.namespaces.iter().find(|n| n.name_any() == namespace).map(|n| n.labels().clone()).unwrap_or_default();
            // The API server sets this on every namespace; selectors use it.
            labels.entry("kubernetes.io/metadata.name".to_string()).or_insert_with(|| namespace.to_string());
            selector.is_some_and(|s| selector_matches(s, &labels))
        }
        _ => false,
    }
}

pub(crate) fn selector_matches(selector: &LabelSelector, labels: &BTreeMap<String, String>) -> bool {
    let labels_ok = selector.match_labels.as_ref().is_none_or(|m| m.iter().all(|(k, v)| labels.get(k) == Some(v)));
    let expressions_ok = selector.match_expressions.as_ref().is_none_or(|exprs| {
        exprs.iter().all(|e| {
            let value = labels.get(&e.key);
            let values = e.values.clone().unwrap_or_default();
            match e.operator.as_str() {
                "In" => value.is_some_and(|v| values.contains(v)),
                "NotIn" => value.is_none_or(|v| !values.contains(v)),
                "Exists" => value.is_some(),
                "DoesNotExist" => value.is_none(),
                _ => false,
            }
        })
    });
    labels_ok && expressions_ok
}

/// A listener's hostname and a route's hostnames have something in common
/// (either side empty = any; `*.example.com` covers one or more labels).
pub(crate) fn hostnames_intersect(listener: Option<&str>, route: &[String]) -> bool {
    let Some(listener) = listener.filter(|h| !h.is_empty()) else { return true };
    route.is_empty() || route.iter().any(|r| hostname_overlap(listener, r))
}

fn hostname_overlap(a: &str, b: &str) -> bool {
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    match (a.strip_prefix("*."), b.strip_prefix("*.")) {
        (None, None) => a == b,
        (Some(suffix), None) => b.ends_with(&format!(".{suffix}")),
        (None, Some(suffix)) => a.ends_with(&format!(".{suffix}")),
        (Some(x), Some(y)) => x == y || x.ends_with(&format!(".{y}")) || y.ends_with(&format!(".{x}")),
    }
}

/// A route of any kind, as the attachment loop needs it.
struct RouteIn<'a> {
    kind: &'static str,
    namespace: String,
    name: String,
    generation: Option<i64>,
    parent_refs: &'a [ParentReference],
    hostnames: &'a [String],
    previous: Vec<RouteParentStatus>,
    /// Every Service the route sends to (backendRefs and mirrors).
    references: Vec<&'a BackendObjectReference>,
    compiled: CompiledHttpRoute,
}

impl<'a> RouteIn<'a> {
    fn new<S>(
        kind: &'static str,
        route: &'a kube::core::Object<S, RouteStatus>,
        parent_refs: &'a [ParentReference],
        hostnames: &'a [String],
        references: Vec<&'a BackendObjectReference>,
        compiled: CompiledHttpRoute,
    ) -> RouteIn<'a>
    where
        S: Clone,
    {
        RouteIn {
            kind,
            namespace: route.namespace().unwrap_or_default(),
            name: route.name_any(),
            generation: route.metadata.generation,
            parent_refs,
            hostnames,
            previous: route.status.as_ref().map(|s| s.parents.clone()).unwrap_or_default(),
            references,
            compiled,
        }
    }
}

/// The Services a rule's backendRefs and `RequestMirror` filters (on the
/// rule or a backendRef) name.
fn rule_references<'a>(backend_refs: &'a [HttpBackendRef], filters: &'a [HttpRouteFilter]) -> impl Iterator<Item = &'a BackendObjectReference> {
    let mirrors = filters.iter().chain(backend_refs.iter().flat_map(|b| &b.filters)).filter_map(|f| f.request_mirror.as_ref());
    backend_refs.iter().map(|b| &b.backend_ref.object).chain(mirrors.map(|m| &m.backend_ref))
}

fn http_references(rules: &[HttpRouteRule]) -> Vec<&BackendObjectReference> {
    rules.iter().flat_map(|r| rule_references(&r.backend_refs, &r.filters)).collect()
}

fn grpc_references(rules: &[GrpcRouteRule]) -> Vec<&BackendObjectReference> {
    rules.iter().flat_map(|r| rule_references(&r.backend_refs, &r.filters)).collect()
}

/// Every reference resolves: a Service, in the route's namespace or one a
/// ReferenceGrant opens to routes of `kind`, that exists.
fn resolve_references(kind: &str, namespace: &str, references: &[&BackendObjectReference], snapshot: &Snapshot) -> Result<(), (&'static str, String)> {
    for reference in references {
        if !reference.group().is_empty() || reference.kind() != "Service" {
            return Err(("InvalidKind", format!("{} isn't a supported backend kind", reference.kind())));
        }
        let target = reference.namespace.clone().unwrap_or_else(|| namespace.to_string());
        if target != namespace && !granted(snapshot, GATEWAY_GROUP, kind, namespace, "", "Service", &target, &reference.name) {
            return Err(("RefNotPermitted", format!("no ReferenceGrant in {target} lets {kind}s in {namespace} use Service {}", reference.name)));
        }
        if !snapshot.services.iter().any(|s| s.namespace().as_deref() == Some(target.as_str()) && s.name_any() == reference.name) {
            return Err(("BackendNotFound", format!("Service {target}/{} doesn't exist", reference.name)));
        }
    }
    Ok(())
}

/// A ReferenceGrant in `to_namespace` lets `from_kind`s in `from_namespace`
/// refer to the `to_kind` named `to_name`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn granted(snapshot: &Snapshot, from_group: &str, from_kind: &str, from_namespace: &str, to_group: &str, to_kind: &str, to_namespace: &str, to_name: &str) -> bool {
    snapshot.reference_grants.iter().filter(|g| g.namespace().as_deref() == Some(to_namespace)).any(|g| {
        g.spec.from.iter().any(|f| f.group == from_group && f.kind == from_kind && f.namespace == from_namespace)
            && g.spec.to.iter().any(|t| t.group == to_group && t.kind == to_kind && t.name.as_deref().is_none_or(|n| n == to_name))
    })
}

// ── conditions ────────────────────────────────────────────────────────────────

/// A condition, keeping `lastTransitionTime` from `previous` when the status
/// didn't change.
fn condition(previous: &[Condition], kind: &str, status: bool, reason: &str, message: &str, generation: Option<i64>, now: &Time) -> Condition {
    let status = if status { "True" } else { "False" }.to_string();
    let since = previous
        .iter()
        .find(|c| c.type_ == kind && c.status == status)
        .map(|c| c.last_transition_time.clone())
        .unwrap_or_else(|| now.clone());
    Condition {
        type_: kind.to_string(),
        status,
        reason: reason.to_string(),
        message: message.to_string(),
        observed_generation: generation,
        last_transition_time: since,
    }
}

#[cfg(test)]
mod tests;
