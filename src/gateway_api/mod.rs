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

pub mod controller;
pub mod dataplane;
pub mod resources;
pub mod router;
pub mod routes;
pub mod services;
pub mod translate;
pub mod types;

/// The `controllerName` GatewayClasses name to be handled by rws.
pub const CONTROLLER_NAME: &str = "rws8.tech/gateway-controller";

/// The field manager rws writes status with.
pub const FIELD_MANAGER: &str = "rws-gateway";
