//! Kubernetes integration and an operator framework (`k8s` feature,
//! 18.36.0), on [kube-rs](https://kube.rs):
//!
//! | Module | What |
//! |---|---|
//! | [`client`] | A client: in a pod its service account, else `KUBECONFIG` / `~/.kube/config` |
//! | [`leader`] | [`LeaderElector`](leader::LeaderElector): one replica does the work, over a `Lease` |
//! | [`config`] | ConfigMaps and Secrets watched live; `k8s://` secret references |
//! | [`discovery`] | A Service's ready endpoints (EndpointSlices) as a `BackendPool` |
//! | [`controller`] | Controllers: watch, reconcile, requeue, finalizers, status |
//! | [`webhook`] | Admission (validating, mutating) and conversion webhooks, with their TLS certificate |
//! | [`manifests`] | CRDs from Rust types, RBAC, webhook configurations, as YAML-compatible JSON |
//! | [`testing`] | Tests against a real API server: a throwaway namespace per test |
//!
//! `kube`, `k8s_openapi` and `schemars` are re-exported for the types they
//! define (`kube::CustomResource`, `k8s_openapi::api::core::v1::Pod`, …).

pub use k8s_openapi;
pub use kube;
#[cfg(feature = "k8s")]
pub use schemars;

pub mod client;
pub mod leader;
#[cfg(feature = "k8s")]
pub mod config;
#[cfg(feature = "k8s")]
pub mod controller;
#[cfg(feature = "k8s")]
pub mod discovery;
#[cfg(feature = "k8s")]
pub mod manifests;
#[cfg(feature = "k8s")]
pub mod testing;
#[cfg(feature = "k8s")]
pub mod webhook;

/// One error type for the module: kube's errors and our own messages.
#[derive(Debug)]
pub enum Error {
    Kube(kube::Error),
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Kube(e) => write!(f, "{e}"),
            Error::Other(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for Error {}

impl From<kube::Error> for Error {
    fn from(e: kube::Error) -> Self {
        Error::Kube(e)
    }
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Error::Other(e)
    }
}

impl From<&str> for Error {
    fn from(e: &str) -> Self {
        Error::Other(e.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An `Api` for a kind whatever its scope: namespaced kinds in a namespace
/// (or across all), cluster-wide kinds everywhere. Implemented for
/// k8s-openapi's two scope types.
pub trait Scope {
    fn api<K: kube::Resource<Scope = Self, DynamicType = ()>>(client: kube::Client, namespace: Option<&str>) -> kube::Api<K>;
}

impl Scope for k8s_openapi::NamespaceResourceScope {
    fn api<K: kube::Resource<Scope = Self, DynamicType = ()>>(client: kube::Client, namespace: Option<&str>) -> kube::Api<K> {
        match namespace {
            Some(ns) => kube::Api::namespaced(client, ns),
            None => kube::Api::all(client),
        }
    }
}

impl Scope for k8s_openapi::ClusterResourceScope {
    fn api<K: kube::Resource<Scope = Self, DynamicType = ()>>(client: kube::Client, _namespace: Option<&str>) -> kube::Api<K> {
        kube::Api::all(client)
    }
}

/// The `Api` for `obj`'s kind, in its namespace when it has one.
pub fn api_for<K>(client: &kube::Client, obj: &K) -> kube::Api<K>
where
    K: kube::Resource<DynamicType = ()>,
    K::Scope: Scope,
{
    <K::Scope as Scope>::api::<K>(client.clone(), obj.meta().namespace.as_deref())
}
