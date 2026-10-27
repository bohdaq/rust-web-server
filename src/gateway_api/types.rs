//! Gateway API v1.6 objects — the fields rws acts on, as serde structs read
//! through `kube::core::Object<Spec, Status>` (see [`super::resources`]).
//! Field names follow the upstream Go types' JSON tags exactly; anything not
//! modeled is ignored on read and never written.

use std::collections::BTreeMap;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, LabelSelector};
use serde::{Deserialize, Serialize};

/// `group`/`kind` defaults: core resources have the empty group.
pub const GATEWAY_GROUP: &str = "gateway.networking.k8s.io";

// ── GatewayClass ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayClassSpec {
    pub controller_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters_ref: Option<ParametersReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParametersReference {
    pub group: String,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayClassStatus {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_features: Vec<SupportedFeature>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SupportedFeature {
    pub name: String,
}

// ── Gateway ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySpec {
    pub gateway_class_name: String,
    #[serde(default)]
    pub listeners: Vec<Listener>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<GatewaySpecAddress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infrastructure: Option<GatewayInfrastructure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_listeners: Option<AllowedListeners>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<GatewayTlsConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listener {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    pub port: i32,
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<ListenerTlsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_routes: Option<AllowedRoutes>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenerTlsConfig {
    /// `Terminate` (default) or `Passthrough`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub certificate_refs: Vec<SecretObjectReference>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowedRoutes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<RouteNamespaces>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<RouteGroupKind>,
}

/// `from`: `Same` (default), `All`, `Selector` (with `selector`); for
/// ListenerSets also `None`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteNamespaces {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<LabelSelector>,
}

pub type AllowedListeners = RouteNamespacesHolder;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteNamespacesHolder {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<RouteNamespaces>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteGroupKind {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub kind: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySpecAddress {
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub address_type: Option<String>,
    #[serde(default)]
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayInfrastructure {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters_ref: Option<LocalObjectReference>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayTlsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<GatewayBackendTls>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontend: Option<FrontendTlsConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayBackendTls {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_certificate_ref: Option<SecretObjectReference>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendTlsConfig {
    #[serde(default)]
    pub default: TlsConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub per_port: Vec<TlsPortConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<FrontendTlsValidation>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsPortConfig {
    pub port: i32,
    #[serde(default)]
    pub tls: TlsConfig,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendTlsValidation {
    #[serde(default)]
    pub ca_certificate_refs: Vec<ObjectReference>,
    /// `AllowValidOnly` (default) or `AllowInsecureFallback`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayStatus {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<GatewayStatusAddress>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub listeners: Vec<ListenerStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_listener_sets: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayStatusAddress {
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub address_type: Option<String>,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenerStatus {
    pub name: String,
    #[serde(default)]
    pub supported_kinds: Vec<RouteGroupKind>,
    pub attached_routes: i32,
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

// ── references ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParentReference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<i32>,
}

impl ParentReference {
    /// Group, defaulting to the Gateway API group.
    pub fn group(&self) -> &str {
        self.group.as_deref().unwrap_or(GATEWAY_GROUP)
    }

    /// Kind, defaulting to `Gateway`.
    pub fn kind(&self) -> &str {
        self.kind.as_deref().unwrap_or("Gateway")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendObjectReference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<i32>,
}

impl BackendObjectReference {
    /// Group, defaulting to core (`""`).
    pub fn group(&self) -> &str {
        self.group.as_deref().unwrap_or("")
    }

    /// Kind, defaulting to `Service`.
    pub fn kind(&self) -> &str {
        self.kind.as_deref().unwrap_or("Service")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendRef {
    #[serde(flatten)]
    pub object: BackendObjectReference,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretObjectReference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectReference {
    #[serde(default)]
    pub group: String,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalObjectReference {
    #[serde(default)]
    pub group: String,
    pub kind: String,
    pub name: String,
}

// ── HTTPRoute ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteSpec {
    #[serde(default)]
    pub parent_refs: Vec<ParentReference>,
    #[serde(default)]
    pub hostnames: Vec<String>,
    #[serde(default)]
    pub rules: Vec<HttpRouteRule>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub matches: Vec<HttpRouteMatch>,
    #[serde(default)]
    pub filters: Vec<HttpRouteFilter>,
    #[serde(default)]
    pub backend_refs: Vec<HttpBackendRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeouts: Option<HttpRouteTimeouts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<HttpRouteRetry>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteTimeouts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_request: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteRetry {
    #[serde(default)]
    pub codes: Vec<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempts: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backoff: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteMatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<HttpPathMatch>,
    #[serde(default)]
    pub headers: Vec<HttpHeaderMatch>,
    #[serde(default)]
    pub query_params: Vec<HttpHeaderMatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

/// `type`: `Exact`, `PathPrefix` (default), `RegularExpression`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HttpPathMatch {
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub match_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// A header or query-parameter match: `type` `Exact` (default) or
/// `RegularExpression`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HttpHeaderMatch {
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub match_type: Option<String>,
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteFilter {
    #[serde(rename = "type")]
    pub filter_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_header_modifier: Option<HttpHeaderFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_header_modifier: Option<HttpHeaderFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_mirror: Option<HttpRequestMirrorFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_redirect: Option<HttpRequestRedirectFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_rewrite: Option<HttpUrlRewriteFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cors: Option<HttpCorsFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_ref: Option<LocalObjectReference>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HttpHeaderFilter {
    #[serde(default)]
    pub set: Vec<HttpHeader>,
    #[serde(default)]
    pub add: Vec<HttpHeader>,
    #[serde(default)]
    pub remove: Vec<String>,
}

/// `type`: `ReplaceFullPath` or `ReplacePrefixMatch`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpPathModifier {
    #[serde(rename = "type")]
    pub modifier_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replace_full_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replace_prefix_match: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestRedirectFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<HttpPathModifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HttpUrlRewriteFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<HttpPathModifier>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestMirrorFilter {
    pub backend_ref: BackendObjectReference,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fraction: Option<Fraction>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fraction {
    pub numerator: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denominator: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpCorsFilter {
    #[serde(default)]
    pub allow_origins: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_credentials: Option<bool>,
    #[serde(default)]
    pub allow_methods: Vec<String>,
    #[serde(default)]
    pub allow_headers: Vec<String>,
    #[serde(default)]
    pub expose_headers: Vec<String>,
    #[serde(default)]
    pub max_age: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpBackendRef {
    #[serde(flatten)]
    pub backend_ref: BackendRef,
    #[serde(default)]
    pub filters: Vec<HttpRouteFilter>,
}

// ── GRPCRoute (v1) ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrpcRouteSpec {
    #[serde(default)]
    pub parent_refs: Vec<ParentReference>,
    #[serde(default)]
    pub hostnames: Vec<String>,
    #[serde(default)]
    pub rules: Vec<GrpcRouteRule>,
}

/// Its filters and backendRefs have HTTPRoute's shape (GRPCRoute allows
/// the header modifiers, `RequestMirror` and `ExtensionRef`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrpcRouteRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub matches: Vec<GrpcRouteMatch>,
    #[serde(default)]
    pub filters: Vec<HttpRouteFilter>,
    #[serde(default)]
    pub backend_refs: Vec<HttpBackendRef>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GrpcRouteMatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<GrpcMethodMatch>,
    #[serde(default)]
    pub headers: Vec<HttpHeaderMatch>,
}

/// `type` `Exact` (default) or `RegularExpression`; `service` and/or `method`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GrpcMethodMatch {
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub match_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

// ── TLSRoute, TCPRoute, UDPRoute (v1) ─────────────────────────────────────────

/// TLSRoute's spec; TCPRoute and UDPRoute have the same without hostnames.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L4RouteSpec {
    #[serde(default)]
    pub parent_refs: Vec<ParentReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hostnames: Vec<String>,
    #[serde(default)]
    pub rules: Vec<L4RouteRule>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L4RouteRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub backend_refs: Vec<BackendRef>,
}

/// The status every route kind has.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RouteStatus {
    #[serde(default)]
    pub parents: Vec<RouteParentStatus>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteParentStatus {
    pub parent_ref: ParentReference,
    pub controller_name: String,
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

// ── ReferenceGrant (v1beta1) ──────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReferenceGrantSpec {
    #[serde(default)]
    pub from: Vec<ReferenceGrantFrom>,
    #[serde(default)]
    pub to: Vec<ReferenceGrantTo>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReferenceGrantFrom {
    #[serde(default)]
    pub group: String,
    pub kind: String,
    pub namespace: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReferenceGrantTo {
    #[serde(default)]
    pub group: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// ReferenceGrant has no status.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NoStatus {}

#[cfg(test)]
mod tests;
