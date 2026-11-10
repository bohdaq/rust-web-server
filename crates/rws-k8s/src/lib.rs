//! Kubernetes integration and an operator framework for rust-web-server
//! (18.36.0): configuration, secrets and service discovery from the
//! Kubernetes API, leader election, controllers, admission and conversion
//! webhooks, CRD and RBAC manifests, and tests against a real API server.
//! Used through the facade's `k8s` module (`k8s` feature).
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "leader")]
pub mod k8s;
