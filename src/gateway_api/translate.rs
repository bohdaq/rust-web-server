//! The pure step of the controller: a [`Snapshot`] of the cluster in, the
//! configuration rws serves ([`GatewayConfig`]) and the status every object
//! should have ([`Desired`]) out. No I/O, so the Gateway API's acceptance
//! rules — which routes attach to which listeners, which references resolve,
//! which conditions and reasons are reported — are unit-tested here.

use std::collections::{BTreeMap, BTreeSet};

use k8s_openapi::api::core::v1::{ConfigMap, Namespace, Pod, Secret, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, LabelSelector, Time};
use kube::ResourceExt;

use super::resources::{BackendTlsPolicy, Gateway, GatewayClass, GrpcRoute, HttpRoute, ListenerSet, ReferenceGrant, TcpRoute, TlsRoute, UdpRoute};
use super::routes::CompiledHttpRoute;
use super::services::ServiceSettings;
use super::types::*;

/// What translation needs besides the cluster.
#[derive(Clone, Debug)]
pub struct Settings {
    pub controller_name: String,
    pub services: ServiceSettings,
    /// Provisioned mode: a Deployment per Gateway.
    pub provision: Option<super::provision::ProvisionSettings>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { controller_name: super::CONTROLLER_NAME.to_string(), services: ServiceSettings::default(), provision: None }
    }
}

/// Everything the controller watches, as last seen.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub classes: Vec<GatewayClass>,
    pub gateways: Vec<Gateway>,
    pub http_routes: Vec<HttpRoute>,
    pub grpc_routes: Vec<GrpcRoute>,
    pub tls_routes: Vec<TlsRoute>,
    pub tcp_routes: Vec<TcpRoute>,
    pub udp_routes: Vec<UdpRoute>,
    pub listener_sets: Vec<ListenerSet>,
    pub backend_tls_policies: Vec<BackendTlsPolicy>,
    pub reference_grants: Vec<ReferenceGrant>,
    pub services: Vec<Service>,
    pub namespaces: Vec<Namespace>,
    pub secrets: Vec<Secret>,
    /// For client-certificate CAs (`spec.tls.frontend`).
    pub config_maps: Vec<ConfigMap>,
    pub endpoint_slices: Vec<EndpointSlice>,
    /// rws's own serving pods (in the pod namespace, by the pod selector).
    pub pods: Vec<Pod>,
    /// Provisioned Deployments, in the pod namespace.
    pub deployments: Vec<k8s_openapi::api::apps::v1::Deployment>,
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
    pub tls_routes: BTreeMap<(String, String), Vec<RouteParentStatus>>,
    pub tcp_routes: BTreeMap<(String, String), Vec<RouteParentStatus>>,
    pub udp_routes: BTreeMap<(String, String), Vec<RouteParentStatus>>,
    /// (namespace, name) → status of the ListenerSets attached to our Gateways.
    pub listener_sets: BTreeMap<(String, String), ListenerSetStatus>,
    /// (namespace, name) → this controller's ancestor statuses of each BackendTLSPolicy.
    pub backend_tls_policies: BTreeMap<(String, String), Vec<PolicyAncestorStatus>>,
    /// (namespace, name) → the Service each of our Gateways should have.
    pub services: BTreeMap<(String, String), Service>,
    /// (namespace, name) → those Services' EndpointSlices (rws's pods).
    pub endpoint_slices: BTreeMap<(String, String), EndpointSlice>,
    /// Provisioned mode: (Gateway namespace, name) → its Deployment.
    pub deployments: BTreeMap<(String, String), k8s_openapi::api::apps::v1::Deployment>,
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
    /// Client certificates to ask for (`spec.tls.frontend` for its port).
    pub client_ca: Option<ClientCa>,
    /// The certificate (PEM chain, key) its Gateway presents to TLS
    /// backends (`spec.tls.backend.clientCertificateRef`).
    pub backend_client_cert: Option<(Vec<u8>, Vec<u8>)>,
    pub http_routes: Vec<CompiledHttpRoute>,
}

/// The CAs a TLS-terminating listener verifies client certificates with.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientCa {
    /// PEM certificates, from the `ca.crt` of each referenced ConfigMap.
    pub pem: Vec<u8>,
    /// `AllowInsecureFallback`: ask, but let in clients without a valid one.
    pub insecure_fallback: bool,
}

/// The features rws reports on its GatewayClasses (names as in the suite's
/// `pkg/features` at Gateway API v1.6.3; sorted). Grows with each phase;
/// `FEATURES` in `.github/workflows/gateway-api.yml` must match.
pub const SUPPORTED_FEATURES: &[&str] = &[
    "BackendTLSPolicy",
    "BackendTLSPolicySANValidation",
    "GRPCRoute",
    "GRPCRouteNamedRouteRule",
    "Gateway",
    "GatewayAddressEmpty",
    "GatewayBackendClientCertificate",
    "GatewayFrontendClientCertificateValidation",
    "GatewayFrontendClientCertificateValidationInsecureFallback",
    "GatewayHTTPListenerIsolation",
    "GatewayHTTPSListenerDetectMisdirectedRequests",
    "GatewayInfrastructure",
    "GatewayPort8080",
    "GatewayStaticAddresses",
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
    "ListenerSet",
    "ReferenceGrant",
    "TCPRoute",
    "TLSRoute",
    "TLSRouteModeTerminate",
    "UDPRoute",
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

    // Gateways of our classes, and what each listener accepts: their own,
    // then those of the ListenerSets they allow (oldest first).
    let mut listeners: Vec<ListenerView> = Vec::new();
    let mut listener_sets: Vec<(&ListenerSet, (String, String), bool)> = Vec::new();
    let mut merged_gateways: Vec<Gateway> = Vec::new();
    for gateway in &snapshot.gateways {
        if !our_classes.contains(&gateway.spec.gateway_class_name) {
            continue;
        }
        let key = (gateway.namespace().unwrap_or_default(), gateway.name_any());
        let mut views = listener_views(gateway, &gateway.spec.listeners, &key.0, None, snapshot);
        let mut merged = gateway.clone();
        for (set, allowed) in attached_listener_sets(gateway, snapshot) {
            let set_key = (set.namespace().unwrap_or_default(), set.name_any());
            if allowed {
                views.extend(listener_views(gateway, &set.spec.listeners, &set_key.0, Some(set_key.clone()), snapshot));
                merged.spec.listeners.extend(set.spec.listeners.iter().cloned());
            }
            listener_sets.push((set, key.clone(), allowed));
        }
        mark_conflicts(&mut views);
        listeners.extend(views);
        merged_gateways.push(merged);
    }

    // Every Gateway of ours gets its Service, with a port for every listener
    // (its ListenerSets' too).
    let our_gateways: Vec<&Gateway> = merged_gateways.iter().collect();
    desired.services = super::services::desired(snapshot, &our_gateways, &settings.services);
    desired.endpoint_slices = super::services::endpoint_slices(snapshot, &desired.services, &settings.services);
    if let Some(provision) = &settings.provision {
        let gateways: Vec<&Gateway> = snapshot.gateways.iter().filter(|g| our_classes.contains(&g.spec.gateway_class_name)).collect();
        desired.deployments = super::provision::desired(&gateways, &settings.services.pod_namespace, controller_name, settings.services.port_range, provision);
    }

    // Routes: attach to listeners, resolve backends.
    let routes = snapshot
        .http_routes
        .iter()
        .map(|r| RouteIn::new("HTTPRoute", r, &r.spec.parent_refs, &r.spec.hostnames, http_references(&r.spec.rules), super::routes::compile(r, snapshot)))
        .chain(snapshot.grpc_routes.iter().map(|r| RouteIn::new("GRPCRoute", r, &r.spec.parent_refs, &r.spec.hostnames, grpc_references(&r.spec.rules), super::routes::compile_grpc(r, snapshot))))
        .chain([("TLSRoute", &snapshot.tls_routes), ("TCPRoute", &snapshot.tcp_routes), ("UDPRoute", &snapshot.udp_routes)].into_iter().flat_map(|(kind, routes)| {
            routes.iter().map(move |r| RouteIn::new(kind, r, &r.spec.parent_refs, &r.spec.hostnames, l4_references(&r.spec.rules), super::routes::compile_l4(r, kind, snapshot)))
        }));
    let mut attached: BTreeMap<ListenerKey, i32> = BTreeMap::new();
    // (Service namespace, name) → Gateways with an attached route to it.
    let mut service_gateways: BTreeMap<(String, String), BTreeSet<(String, String)>> = BTreeMap::new();
    for route in routes {
        let namespace = route.namespace.clone();
        let resolved = resolve_references(route.kind, &namespace, &route.references, snapshot);
        let mut parents = Vec::new();
        for parent_ref in route.parent_refs {
            if parent_ref.group() != GATEWAY_GROUP {
                continue;
            }
            let parent_key = (parent_ref.namespace.clone().unwrap_or_else(|| namespace.clone()), parent_ref.name.clone());
            // The Gateway, and for a ListenerSet parent the set whose listeners it means.
            let (gateway_key, owner) = match parent_ref.kind() {
                "Gateway" => (parent_key.clone(), None),
                "ListenerSet" => match listener_sets.iter().find(|(set, _, _)| is_named(*set, &parent_key)) {
                    Some((_, gateway, _)) => (gateway.clone(), Some(parent_key.clone())),
                    None => continue, // not attached to a Gateway of ours
                },
                _ => continue,
            };
            let candidates: Vec<&ListenerView> = listeners.iter().filter(|l| l.gateway == gateway_key && l.owner == owner).collect();
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
            if !attached_to.is_empty() {
                for reference in &route.references {
                    let service = (reference.namespace.clone().unwrap_or_else(|| namespace.clone()), reference.name.clone());
                    service_gateways.entry(service).or_default().insert(gateway_key.clone());
                }
            }
            for listener in &attached_to {
                *attached.entry((gateway_key.clone(), owner.clone(), listener.clone())).or_default() += 1;
                if let Some(view) = listeners.iter_mut().find(|l| l.gateway == gateway_key && l.owner == owner && &l.name == listener) {
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
            let statuses = match route.kind {
                "GRPCRoute" => &mut desired.grpc_routes,
                "TLSRoute" => &mut desired.tls_routes,
                "TCPRoute" => &mut desired.tcp_routes,
                "UDPRoute" => &mut desired.udp_routes,
                _ => &mut desired.http_routes,
            };
            statuses.insert((namespace, route.name), parents);
        }
    }

    // BackendTLSPolicies: status per Gateway whose routes reach a target.
    for ((namespace, name), evaluated) in super::backend_tls::evaluate(snapshot) {
        let Some(policy) = snapshot.backend_tls_policies.iter().find(|p| p.namespace().as_deref() == Some(namespace.as_str()) && p.name_any() == name) else { continue };
        let generation = policy.metadata.generation;
        let gateways: BTreeSet<(String, String)> = policy
            .spec
            .target_refs
            .iter()
            .filter_map(|t| service_gateways.get(&(namespace.clone(), t.name.clone())))
            .flatten()
            .cloned()
            .collect();
        let previous = policy.status.as_ref().map(|s| s.ancestors.as_slice()).unwrap_or_default();
        let ancestors: Vec<PolicyAncestorStatus> = gateways
            .into_iter()
            .map(|(gateway_namespace, gateway_name)| {
                let ancestor_ref = ParentReference {
                    group: Some(GATEWAY_GROUP.to_string()),
                    kind: Some("Gateway".to_string()),
                    namespace: Some(gateway_namespace),
                    name: gateway_name,
                    section_name: None,
                    port: None,
                };
                let old = previous.iter().find(|a| a.controller_name == controller_name && a.ancestor_ref == ancestor_ref).map(|a| a.conditions.as_slice()).unwrap_or_default();
                let conditions = vec![
                    match &evaluated.accepted {
                        Ok(()) => condition(old, "Accepted", true, "Accepted", "", generation, now),
                        Err((reason, message)) => condition(old, "Accepted", false, reason, message, generation, now),
                    },
                    match &evaluated.resolved {
                        Ok(()) => condition(old, "ResolvedRefs", true, "ResolvedRefs", "", generation, now),
                        Err((reason, message)) => condition(old, "ResolvedRefs", false, reason, message, generation, now),
                    },
                ];
                PolicyAncestorStatus { ancestor_ref, controller_name: controller_name.to_string(), conditions }
            })
            .collect();
        if !ancestors.is_empty() {
            desired.backend_tls_policies.insert((namespace, name), ancestors);
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
        let backend_client_cert = backend_client_certificate(gateway, snapshot);
        let mut listener_statuses = Vec::new();
        let mut set_statuses: BTreeMap<(String, String), Vec<ListenerStatus>> = BTreeMap::new();
        let mut any_valid = false;
        let (mut any_accepted, mut any_rejected) = (false, false);
        for view in listeners.iter().filter(|l| l.gateway == key) {
            let count = attached.get(&(key.clone(), view.owner.clone(), view.name.clone())).copied().unwrap_or(0);
            match &view.owner {
                None => {
                    any_valid |= view.accepted && view.programmed;
                    any_accepted |= view.accepted;
                    any_rejected |= !view.accepted;
                    let old = previous.listeners.iter().find(|l| l.name == view.name).map(|l| l.conditions.as_slice()).unwrap_or_default();
                    listener_statuses.push(listener_status(view, count, old, generation, now));
                }
                Some(set_key) => {
                    let set = snapshot.listener_sets.iter().find(|s| is_named(*s, set_key));
                    let old = set.and_then(|s| s.status.as_ref()).and_then(|s| s.listeners.iter().find(|l| l.name == view.name)).map(|l| l.conditions.clone()).unwrap_or_default();
                    let set_generation = set.and_then(|s| s.metadata.generation);
                    set_statuses.entry(set_key.clone()).or_default().push(listener_status(view, count, &old, set_generation, now));
                }
            }
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
                    client_ca: view.client_ca.clone(),
                    backend_client_cert: backend_client_cert.as_ref().and_then(|c| c.as_ref().ok()).cloned(),
                    http_routes: view.http_routes.clone(),
                });
            }
        }
        let addresses = gateway_addresses(snapshot, &key.0, &key.1);
        let addressed = !addresses.is_empty();
        let unsupported_address = gateway.spec.addresses.iter().find(|a| !super::services::address_supported(a));
        // Static addresses must all be the Service's.
        let requested = super::services::requested_addresses(gateway);
        let missing = requested.iter().find(|r| !addresses.iter().any(|a| &a.value == *r));
        let programmed = any_valid && addressed && missing.is_none() && unsupported_address.is_none();
        let (programmed_reason, programmed_message) = if let Some(address) = missing {
            ("AddressNotUsable", format!("The load balancer hasn't assigned {address}"))
        } else if !addressed {
            ("AddressNotAssigned", "Waiting for the Service's address".to_string())
        } else if unsupported_address.is_some() {
            ("Invalid", "An address type isn't supported".to_string())
        } else if !any_valid {
            ("Invalid", "No valid listener".to_string())
        } else {
            ("Programmed", "Serving".to_string())
        };
        // Accepted unless no listener is; `ListenersNotValid` when any isn't.
        // rws takes no parameters object: a reference can't be honoured.
        let invalid_parameters = gateway.spec.infrastructure.as_ref().is_some_and(|i| i.parameters_ref.is_some());
        let gateway_accepted = (gateway.spec.listeners.is_empty() || any_accepted) && unsupported_address.is_none() && !invalid_parameters;
        let accepted_reason = if invalid_parameters {
            "InvalidParameters"
        } else if unsupported_address.is_some() {
            "UnsupportedAddress"
        } else if any_rejected {
            "ListenersNotValid"
        } else {
            "Accepted"
        };

        // Its ListenerSets: accepted with at least one valid listener.
        let mut attached_sets = 0;
        for (set, _, allowed) in listener_sets.iter().filter(|(_, gateway, _)| *gateway == key) {
            let set_key = (set.namespace().unwrap_or_default(), set.name_any());
            let statuses = set_statuses.remove(&set_key).unwrap_or_default();
            let valid = statuses.iter().any(|l| l.conditions.iter().all(|c| c.type_ == "Conflicted" || c.type_ == "ResolvedRefs" || c.status == "True"));
            let (ok, reason, message) = if !allowed {
                (false, "NotAllowed", "The Gateway's allowedListeners doesn't admit this ListenerSet")
            } else if !valid {
                (false, "ListenersNotValid", "No valid listener")
            } else {
                (true, "Accepted", "")
            };
            attached_sets += ok as i32;
            let old = set.status.as_ref().map(|s| s.conditions.as_slice()).unwrap_or_default();
            let set_generation = set.metadata.generation;
            let conditions = vec![
                condition(old, "Accepted", ok, reason, message, set_generation, now),
                condition(old, "Programmed", ok, if ok { "Programmed" } else { reason }, message, set_generation, now),
            ];
            desired.listener_sets.insert(set_key, ListenerSetStatus { conditions, listeners: statuses });
        }
        let mut conditions = vec![
            condition(&previous.conditions, "Accepted", gateway_accepted, accepted_reason, "", generation, now),
            condition(&previous.conditions, "Programmed", programmed, programmed_reason, &programmed_message, generation, now),
        ];
        if let Some(certificate) = &backend_client_cert {
            conditions.push(match certificate {
                Ok(_) => condition(&previous.conditions, "ResolvedRefs", true, "ResolvedRefs", "", generation, now),
                Err((reason, message)) => condition(&previous.conditions, "ResolvedRefs", false, reason, message, generation, now),
            });
        }
        if insecure_frontend(gateway) {
            let message = "Client certificate validation allows clients without a valid certificate (AllowInsecureFallback)";
            conditions.push(condition(&previous.conditions, "InsecureFrontendValidationMode", true, "ConfigurationChanged", message, generation, now));
        }
        desired.gateways.insert(
            key,
            GatewayStatus { addresses, conditions, listeners: listener_statuses, attached_listener_sets: Some(attached_sets) },
        );
    }
    (config, desired)
}

// ── listeners ─────────────────────────────────────────────────────────────────

/// (Gateway, ListenerSet or `None` for the Gateway's own, listener name).
type ListenerKey = ((String, String), Option<(String, String)>, String);

struct ListenerView {
    gateway: (String, String),
    /// The ListenerSet it comes from; `None` for the Gateway's own.
    owner: Option<(String, String)>,
    /// Its object's namespace (`from: Same` for routes, Secrets).
    namespace: String,
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
    client_ca: Option<ClientCa>,
    /// A TLS listener in `Passthrough` mode.
    passthrough: bool,
    http_routes: Vec<CompiledHttpRoute>,
}

/// HTTPS listeners, and TLS listeners in `Terminate` mode (the default),
/// decrypt with their certificates; `Passthrough` forwards the TLS stream.
fn terminates_tls(listener: &Listener) -> bool {
    match listener.protocol.as_str() {
        "HTTPS" => true,
        "TLS" => listener.tls.as_ref().and_then(|t| t.mode.as_deref()).unwrap_or("Terminate") == "Terminate",
        _ => false,
    }
}

/// The route kinds a protocol carries.
fn kinds_for(protocol: &str) -> &'static [&'static str] {
    match protocol {
        "HTTP" | "HTTPS" => &["HTTPRoute", "GRPCRoute"],
        "TLS" => &["TLSRoute"],
        "TCP" => &["TCPRoute"],
        "UDP" => &["UDPRoute"],
        _ => &[],
    }
}

/// The views of `listeners`, of the Gateway `key` (from the ListenerSet
/// `owner`, if any, in `namespace`); conflicts are marked later, over the
/// Gateway's merged list ([`mark_conflicts`]).
fn listener_views(gateway: &Gateway, listeners: &[Listener], namespace: &str, owner: Option<(String, String)>, snapshot: &Snapshot) -> Vec<ListenerView> {
    let key = (gateway.namespace().unwrap_or_default(), gateway.name_any());
    listeners
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
            let (mut accepted, mut accepted_reason, message) = if carried.is_empty() {
                (false, "UnsupportedProtocol", format!("protocol {} isn't supported", l.protocol))
            } else {
                (true, "Accepted", String::new())
            };
            let (mut tls, mut client_ca) = (None, None);
            if accepted && terminates_tls(l) {
                match certificates(l, namespace, snapshot) {
                    Ok(material) => tls = Some(material),
                    Err(e) => resolved_refs = Err(e),
                }
                if let Some(validation) = frontend_validation(gateway, l.port) {
                    match client_cas(validation, &key.0, snapshot) {
                        Ok(ca) => client_ca = Some(ca),
                        Err(e) => {
                            resolved_refs = Err(e);
                            accepted = false;
                            accepted_reason = "NoValidCACertificate";
                        }
                    }
                }
            }
            // Invalid route kinds alone don't stop the listener serving the valid ones.
            let programmed = accepted && resolved_refs.as_ref().err().is_none_or(|(reason, _)| *reason == "InvalidRouteKinds");
            ListenerView {
                gateway: key.clone(),
                owner: owner.clone(),
                namespace: namespace.to_string(),
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
                client_ca,
                passthrough: l.protocol == "TLS" && !terminates_tls(l),
                http_routes: Vec::new(),
            }
        })
        .collect()
}

/// Listeners on one port must agree on the protocol and differ in hostname.
/// Among the Gateway's own listeners every party to a conflict is marked; a
/// ListenerSet's listener that conflicts with one earlier in the merged list
/// (the Gateway's, then older ListenerSets') is the one that loses, and
/// isn't accepted.
fn mark_conflicts(views: &mut [ListenerView]) {
    for j in 0..views.len() {
        for i in 0..views.len() {
            if i == j || views[i].port != views[j].port {
                continue;
            }
            // TCP and UDP listeners on one port number don't share a socket.
            if super::services::transport(&views[i].protocol) != super::services::transport(&views[j].protocol) {
                continue;
            }
            // A ListenerSet's listener only loses to earlier ones.
            if views[j].owner.is_some() && (i > j || views[i].conflicted.is_some()) {
                continue;
            }
            if views[j].owner.is_none() && views[i].owner.is_some() {
                continue;
            }
            // Terminate and Passthrough on one port would need TLSRouteModeMixed.
            let mixed_tls = views[i].protocol == "TLS" && views[j].protocol == "TLS" && views[i].passthrough != views[j].passthrough;
            let reason = if views[i].protocol != views[j].protocol || mixed_tls {
                "ProtocolConflict"
            } else if views[i].hostname == views[j].hostname {
                "HostnameConflict"
            } else {
                continue;
            };
            views[j].conflicted = Some(reason);
            views[j].programmed = false;
            if views[j].owner.is_some() || mixed_tls {
                views[j].accepted = false;
                views[j].accepted_reason = reason;
            }
            // A listener that can't be served carries no route kinds (the suite
            // expects none for the mixed TLS modes).
            if mixed_tls {
                views[j].supported_kinds.clear();
            }
        }
    }
}

/// The ListenerSets naming `gateway` as their parent, oldest first, and
/// whether its `allowedListeners` admits each.
fn attached_listener_sets<'a>(gateway: &Gateway, snapshot: &'a Snapshot) -> Vec<(&'a ListenerSet, bool)> {
    let gateway_namespace = gateway.namespace().unwrap_or_default();
    let mut sets: Vec<&ListenerSet> = snapshot
        .listener_sets
        .iter()
        .filter(|set| {
            let parent = &set.spec.parent_ref;
            let namespace = parent.namespace.clone().or_else(|| set.namespace()).unwrap_or_default();
            parent.group.as_deref().unwrap_or(GATEWAY_GROUP) == GATEWAY_GROUP
                && parent.kind.as_deref().unwrap_or("Gateway") == "Gateway"
                && parent.name == gateway.name_any()
                && namespace == gateway_namespace
        })
        .collect();
    sets.sort_by_key(|set| (set.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string()).unwrap_or_default(), set.namespace(), set.name_any()));
    let admitted = gateway.spec.allowed_listeners.as_ref().and_then(|a| a.namespaces.as_ref());
    sets.into_iter()
        .map(|set| {
            let namespace = set.namespace().unwrap_or_default();
            let allowed = namespace_admitted(admitted.and_then(|n| n.from.as_deref()).unwrap_or("None"), admitted.and_then(|n| n.selector.as_ref()), &namespace, &gateway_namespace, snapshot);
            (set, allowed)
        })
        .collect()
}

/// `from` (`All`, `Same`, `Selector`, `None`) admits objects in `namespace`
/// for an object in `home`.
fn namespace_admitted(from: &str, selector: Option<&LabelSelector>, namespace: &str, home: &str, snapshot: &Snapshot) -> bool {
    match from {
        "All" => true,
        "Same" => namespace == home,
        "Selector" => {
            let mut labels = snapshot.namespaces.iter().find(|n| n.name_any() == namespace).map(|n| n.labels().clone()).unwrap_or_default();
            // The API server sets this on every namespace; selectors use it.
            labels.entry("kubernetes.io/metadata.name".to_string()).or_insert_with(|| namespace.to_string());
            selector.is_some_and(|s| selector_matches(s, &labels))
        }
        _ => false,
    }
}

/// A listener's status from its view.
fn listener_status(view: &ListenerView, attached_routes: i32, old: &[Condition], generation: Option<i64>, now: &Time) -> ListenerStatus {
    let programmed_reason = match (view.programmed, view.conflicted) {
        (true, _) => "Programmed",
        (false, Some(reason)) if view.owner.is_some() => reason,
        _ => "Invalid",
    };
    ListenerStatus {
        name: view.name.clone(),
        supported_kinds: view.supported_kinds.clone(),
        attached_routes,
        conditions: vec![
            condition(old, "Accepted", view.accepted, view.accepted_reason, &view.message, generation, now),
            condition(old, "Programmed", view.programmed, programmed_reason, &view.message, generation, now),
            condition(old, "ResolvedRefs", view.resolved_refs.is_ok(), view.resolved_refs.as_ref().err().map_or("ResolvedRefs", |(r, _)| r), view.resolved_refs.as_ref().err().map_or("", |(_, m)| m.as_str()), generation, now),
            condition(old, "Conflicted", view.conflicted.is_some(), view.conflicted.unwrap_or("NoConflicts"), "", generation, now),
        ],
    }
}

/// An HTTPS listener's certificates resolve: TLS Secrets in its namespace,
/// or in another one a ReferenceGrant opens.
/// A listener's PEM certificate chain and key.
type TlsMaterial = (Vec<u8>, Vec<u8>);

fn certificates(listener: &Listener, gateway_namespace: &str, snapshot: &Snapshot) -> Result<TlsMaterial, (&'static str, String)> {
    let Some(tls) = &listener.tls else {
        return Err(("InvalidCertificateRef", "a listener that terminates TLS needs tls.certificateRefs".to_string()));
    };
    if tls.certificate_refs.is_empty() {
        return Err(("InvalidCertificateRef", "a listener that terminates TLS needs tls.certificateRefs".to_string()));
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

/// The client-certificate validation for a listener port: the Gateway's
/// `spec.tls.frontend.perPort` entry for it, else the default.
fn frontend_validation(gateway: &Gateway, port: i32) -> Option<&FrontendTlsValidation> {
    let frontend = gateway.spec.tls.as_ref()?.frontend.as_ref()?;
    match frontend.per_port.iter().find(|p| p.port == port) {
        Some(per_port) => per_port.tls.validation.as_ref(),
        None => frontend.default.validation.as_ref(),
    }
}

/// The Gateway's backend client certificate, when it names one: a TLS
/// Secret in its namespace, or one a ReferenceGrant opens.
fn backend_client_certificate(gateway: &Gateway, snapshot: &Snapshot) -> Option<Result<TlsMaterial, (&'static str, String)>> {
    let reference = gateway.spec.tls.as_ref()?.backend.as_ref()?.client_certificate_ref.as_ref()?;
    let gateway_namespace = gateway.namespace().unwrap_or_default();
    let group = reference.group.as_deref().unwrap_or("");
    let kind = reference.kind.as_deref().unwrap_or("Secret");
    if !group.is_empty() || kind != "Secret" {
        return Some(Err(("InvalidClientCertificateRef", format!("{group}/{kind} isn't a supported client certificate reference"))));
    }
    let namespace = reference.namespace.clone().unwrap_or_else(|| gateway_namespace.clone());
    if namespace != gateway_namespace && !granted(snapshot, GATEWAY_GROUP, "Gateway", &gateway_namespace, "", "Secret", &namespace, &reference.name) {
        return Some(Err(("RefNotPermitted", format!("no ReferenceGrant in {namespace} lets Gateways in {gateway_namespace} use Secret {}", reference.name))));
    }
    let data = snapshot
        .secrets
        .iter()
        .find(|s| s.namespace().as_deref() == Some(namespace.as_str()) && s.name_any() == reference.name)
        .and_then(|s| s.data.as_ref());
    let pair = data.and_then(|d| Some((d.get("tls.crt")?.0.clone(), d.get("tls.key")?.0.clone())));
    let valid = pair.as_ref().filter(|(cert, key)| crate::tls::certified_key_from_pem(cert, key).is_ok());
    Some(valid.cloned().ok_or(("InvalidClientCertificateRef", format!("Secret {namespace}/{} isn't a valid TLS Secret", reference.name))))
}

/// Whether any of the Gateway's validation settings is `AllowInsecureFallback`.
fn insecure_frontend(gateway: &Gateway) -> bool {
    let Some(frontend) = gateway.spec.tls.as_ref().and_then(|t| t.frontend.as_ref()) else { return false };
    std::iter::once(&frontend.default)
        .chain(frontend.per_port.iter().map(|p| &p.tls))
        .filter_map(|t| t.validation.as_ref())
        .any(|v| v.mode.as_deref() == Some("AllowInsecureFallback"))
}

/// The CA certificates a validation references: ConfigMaps with `ca.crt`,
/// in the Gateway's namespace or one a ReferenceGrant opens.
fn client_cas(validation: &FrontendTlsValidation, gateway_namespace: &str, snapshot: &Snapshot) -> Result<ClientCa, (&'static str, String)> {
    let mut pem = Vec::new();
    for reference in &validation.ca_certificate_refs {
        if !reference.group.is_empty() || reference.kind != "ConfigMap" {
            return Err(("InvalidCACertificateKind", format!("{} isn't a supported CA certificate reference", reference.kind)));
        }
        let namespace = reference.namespace.clone().unwrap_or_else(|| gateway_namespace.to_string());
        if namespace != gateway_namespace && !granted(snapshot, GATEWAY_GROUP, "Gateway", gateway_namespace, "", "ConfigMap", &namespace, &reference.name) {
            return Err(("RefNotPermitted", format!("no ReferenceGrant in {namespace} lets Gateways in {gateway_namespace} use ConfigMap {}", reference.name)));
        }
        let ca = snapshot
            .config_maps
            .iter()
            .find(|c| c.namespace().as_deref() == Some(namespace.as_str()) && c.name_any() == reference.name)
            .and_then(|c| c.data.as_ref())
            .and_then(|d| d.get("ca.crt"))
            .filter(|ca| crate::core::x509::cert_info_from_pem(ca.as_bytes()).is_some());
        let Some(ca) = ca else {
            return Err(("InvalidCACertificateRef", format!("ConfigMap {namespace}/{} has no valid ca.crt", reference.name)));
        };
        pem.extend_from_slice(ca.as_bytes());
        pem.push(b'\n');
    }
    if pem.is_empty() {
        return Err(("InvalidCACertificateRef", "no caCertificateRefs".to_string()));
    }
    Ok(ClientCa { pem, insecure_fallback: validation.mode.as_deref() == Some("AllowInsecureFallback") })
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
    let namespaces = listener.allowed.namespaces.as_ref();
    let from = namespaces.and_then(|n| n.from.as_deref()).unwrap_or("Same");
    namespace_admitted(from, namespaces.and_then(|n| n.selector.as_ref()), namespace, &listener.namespace, snapshot)
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

fn l4_references(rules: &[L4RouteRule]) -> Vec<&BackendObjectReference> {
    rules.iter().flat_map(|r| r.backend_refs.iter().map(|b| &b.object)).collect()
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
