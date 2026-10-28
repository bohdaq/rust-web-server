//! Where each Gateway API kind lives (group, version, plural), and the
//! object types: `kube::core::Object` over our typed spec and status, read
//! and written with `kube::Api::*_with(client, &api_resource)`.

use kube::core::{ApiResource, GroupVersionKind, Object};

use super::types::*;

pub type GatewayClass = Object<GatewayClassSpec, GatewayClassStatus>;
pub type Gateway = Object<GatewaySpec, GatewayStatus>;
pub type HttpRoute = Object<HttpRouteSpec, RouteStatus>;
pub type GrpcRoute = Object<GrpcRouteSpec, RouteStatus>;
pub type TlsRoute = Object<L4RouteSpec, RouteStatus>;
pub type TcpRoute = Object<L4RouteSpec, RouteStatus>;
pub type UdpRoute = Object<L4RouteSpec, RouteStatus>;
pub type ReferenceGrant = Object<ReferenceGrantSpec, NoStatus>;
pub type ListenerSet = Object<ListenerSetSpec, ListenerSetStatus>;
pub type BackendTlsPolicy = Object<BackendTlsPolicySpec, PolicyStatus>;

fn resource(version: &str, kind: &str, plural: &str) -> ApiResource {
    ApiResource::from_gvk_with_plural(&GroupVersionKind::gvk(GATEWAY_GROUP, version, kind), plural)
}

pub fn gateway_classes() -> ApiResource {
    resource("v1", "GatewayClass", "gatewayclasses")
}

pub fn gateways() -> ApiResource {
    resource("v1", "Gateway", "gateways")
}

pub fn http_routes() -> ApiResource {
    resource("v1", "HTTPRoute", "httproutes")
}

pub fn grpc_routes() -> ApiResource {
    resource("v1", "GRPCRoute", "grpcroutes")
}

pub fn tls_routes() -> ApiResource {
    resource("v1", "TLSRoute", "tlsroutes")
}

pub fn tcp_routes() -> ApiResource {
    resource("v1", "TCPRoute", "tcproutes")
}

pub fn udp_routes() -> ApiResource {
    resource("v1", "UDPRoute", "udproutes")
}

pub fn listener_sets() -> ApiResource {
    resource("v1", "ListenerSet", "listenersets")
}

pub fn backend_tls_policies() -> ApiResource {
    resource("v1", "BackendTLSPolicy", "backendtlspolicies")
}

pub fn reference_grants() -> ApiResource {
    resource("v1beta1", "ReferenceGrant", "referencegrants")
}
