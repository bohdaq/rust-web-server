//! Where each Gateway API kind lives (group, version, plural), and the
//! object types: `kube::core::Object` over our typed spec and status, read
//! and written with `kube::Api::*_with(client, &api_resource)`.

use kube::core::{ApiResource, GroupVersionKind, Object};

use super::types::*;

pub type GatewayClass = Object<GatewayClassSpec, GatewayClassStatus>;
pub type Gateway = Object<GatewaySpec, GatewayStatus>;
pub type HttpRoute = Object<HttpRouteSpec, RouteStatus>;
pub type ReferenceGrant = Object<ReferenceGrantSpec, NoStatus>;

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

pub fn reference_grants() -> ApiResource {
    resource("v1beta1", "ReferenceGrant", "referencegrants")
}
