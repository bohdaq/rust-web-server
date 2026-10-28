//! The controller loop: one watcher per kind feeding a shared store, a
//! debounced reconcile that runs [`translate`] over a snapshot of those
//! stores, hands the result to the data plane, and writes the status that
//! changed.
//!
//! Status is written with a JSON merge patch on `/status`. For routes,
//! `status.parents` is one list shared with other controllers: rws replaces
//! only its own entries (by `controllerName`) and sends the object's
//! `resourceVersion`, so a concurrent write by someone else makes the API
//! server refuse ours (`409`) rather than lose theirs; the next reconcile
//! retries from fresh state.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use k8s_openapi::api::core::v1::{ConfigMap, Namespace, Pod, Secret, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::api::{Api, Patch, PatchParams};
use kube::runtime::reflector::{self, store::Writer, Store};
use kube::runtime::{watcher, WatchStreamExt};
use kube::{Client, Resource, ResourceExt};
use serde::de::DeserializeOwned;
use tokio::sync::Notify;

use super::resources::{self, BackendTlsPolicy, Gateway, GatewayClass, GrpcRoute, HttpRoute, ListenerSet, ReferenceGrant, TcpRoute, TlsRoute, UdpRoute};
use super::translate::{translate, GatewayConfig, Settings, Snapshot};
use super::types::RouteParentStatus;

/// How long changes are gathered before a reconcile.
pub const DEBOUNCE: Duration = Duration::from_millis(100);
/// A reconcile at least this often, changes or not (status drift, a lost event).
pub const RESYNC: Duration = Duration::from_secs(30);

/// What the controller needs besides the cluster.
pub struct Options {
    pub settings: Settings,
    /// Whether this replica holds the Lease: the leader writes status and
    /// Services, every replica serves ([`super::lease`]).
    pub leader: Arc<AtomicBool>,
    /// Called with every new configuration.
    pub on_config: Arc<dyn Fn(GatewayConfig) + Send + Sync>,
}

/// The watched stores.
#[derive(Clone)]
pub struct Stores {
    pub classes: Store<GatewayClass>,
    pub gateways: Store<Gateway>,
    pub http_routes: Store<HttpRoute>,
    pub grpc_routes: Store<GrpcRoute>,
    pub tls_routes: Store<TlsRoute>,
    pub tcp_routes: Store<TcpRoute>,
    pub udp_routes: Store<UdpRoute>,
    pub listener_sets: Store<ListenerSet>,
    pub backend_tls_policies: Store<BackendTlsPolicy>,
    pub reference_grants: Store<ReferenceGrant>,
    pub services: Store<Service>,
    pub namespaces: Store<Namespace>,
    pub secrets: Store<Secret>,
    pub config_maps: Store<ConfigMap>,
    pub endpoint_slices: Store<EndpointSlice>,
    pub pods: Store<Pod>,
    pub deployments: Store<k8s_openapi::api::apps::v1::Deployment>,
}

impl Stores {
    pub fn snapshot(&self) -> Snapshot {
        fn all<K: Clone + kube::Resource>(store: &Store<K>) -> Vec<K>
        where
            K::DynamicType: std::hash::Hash + Eq + Clone,
        {
            store.state().iter().map(|o| (**o).clone()).collect()
        }
        Snapshot {
            classes: all(&self.classes),
            gateways: all(&self.gateways),
            http_routes: all(&self.http_routes),
            grpc_routes: all(&self.grpc_routes),
            tls_routes: all(&self.tls_routes),
            tcp_routes: all(&self.tcp_routes),
            udp_routes: all(&self.udp_routes),
            listener_sets: all(&self.listener_sets),
            backend_tls_policies: all(&self.backend_tls_policies),
            reference_grants: all(&self.reference_grants),
            services: all(&self.services),
            namespaces: all(&self.namespaces),
            secrets: all(&self.secrets),
            config_maps: all(&self.config_maps),
            endpoint_slices: all(&self.endpoint_slices),
            pods: all(&self.pods),
            deployments: all(&self.deployments),
        }
    }
}

/// Start a watcher for `api` feeding a new store; every change wakes `changed`.
fn watch<K>(api: Api<K>, dyntype: K::DynamicType, changed: Arc<Notify>) -> Store<K>
where
    K: Resource + Clone + DeserializeOwned + std::fmt::Debug + Send + Sync + 'static,
    K::DynamicType: std::hash::Hash + Eq + Clone + Send + Sync,
{
    watch_with(api, dyntype, watcher::Config::default(), changed)
}

fn watch_with<K>(api: Api<K>, dyntype: K::DynamicType, config: watcher::Config, changed: Arc<Notify>) -> Store<K>
where
    K: Resource + Clone + DeserializeOwned + std::fmt::Debug + Send + Sync + 'static,
    K::DynamicType: std::hash::Hash + Eq + Clone + Send + Sync,
{
    let writer = Writer::new(dyntype);
    let reader = writer.as_reader();
    let stream = reflector::reflector(writer, watcher(api, config)).default_backoff().touched_objects();
    tokio::spawn(async move {
        let mut stream = std::pin::pin!(stream);
        while let Some(event) = stream.next().await {
            if let Err(e) = event {
                eprintln!("[gateway-api] watch error: {e}");
            }
            changed.notify_one();
        }
    });
    reader
}

/// Start watching everything (rws's own pods by `services`' namespace and
/// selector); returns the stores and the change signal.
pub fn start_watches(client: &Client, services: &super::services::ServiceSettings) -> (Stores, Arc<Notify>) {
    let pod_labels = services.pod_watch_labels().iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",");
    let managed = format!("app.kubernetes.io/managed-by={}", super::translate::MANAGED_BY);
    let changed = Arc::new(Notify::new());
    let stores = Stores {
        classes: watch(Api::all_with(client.clone(), &resources::gateway_classes()), resources::gateway_classes(), Arc::clone(&changed)),
        gateways: watch(Api::all_with(client.clone(), &resources::gateways()), resources::gateways(), Arc::clone(&changed)),
        http_routes: watch(Api::all_with(client.clone(), &resources::http_routes()), resources::http_routes(), Arc::clone(&changed)),
        grpc_routes: watch(Api::all_with(client.clone(), &resources::grpc_routes()), resources::grpc_routes(), Arc::clone(&changed)),
        tls_routes: watch(Api::all_with(client.clone(), &resources::tls_routes()), resources::tls_routes(), Arc::clone(&changed)),
        tcp_routes: watch(Api::all_with(client.clone(), &resources::tcp_routes()), resources::tcp_routes(), Arc::clone(&changed)),
        udp_routes: watch(Api::all_with(client.clone(), &resources::udp_routes()), resources::udp_routes(), Arc::clone(&changed)),
        listener_sets: watch(Api::all_with(client.clone(), &resources::listener_sets()), resources::listener_sets(), Arc::clone(&changed)),
        backend_tls_policies: watch(Api::all_with(client.clone(), &resources::backend_tls_policies()), resources::backend_tls_policies(), Arc::clone(&changed)),
        reference_grants: watch(Api::all_with(client.clone(), &resources::reference_grants()), resources::reference_grants(), Arc::clone(&changed)),
        services: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
        namespaces: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
        secrets: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
        config_maps: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
        endpoint_slices: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
        pods: watch_with(Api::namespaced(client.clone(), &services.pod_namespace), (), watcher::Config::default().labels(&pod_labels), Arc::clone(&changed)),
        deployments: watch_with(Api::namespaced(client.clone(), &services.pod_namespace), (), watcher::Config::default().labels(&managed), Arc::clone(&changed)),
    };
    (stores, changed)
}

/// Reconcile on every change, when this replica becomes the leader, and
/// every [`RESYNC`], forever.
pub async fn run(client: Client, options: Options, became_leader: Arc<Notify>) {
    let (stores, changed) = start_watches(&client, &options.settings.services);
    loop {
        tokio::select! {
            _ = changed.notified() => {}
            _ = became_leader.notified() => {}
            _ = tokio::time::sleep(RESYNC) => {}
        }
        tokio::time::sleep(DEBOUNCE).await;
        if let Err(e) = reconcile(&client, &stores, &options).await {
            eprintln!("[gateway-api] reconcile: {e}");
            // Try again soon even without a new event.
            let changed = Arc::clone(&changed);
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                changed.notify_one();
            });
        }
    }
}

/// One pass: translate, hand the configuration over, write changed status.
pub async fn reconcile(client: &Client, stores: &Stores, options: &Options) -> Result<(), String> {
    let snapshot = stores.snapshot();
    let now = Time(k8s_openapi::jiff::Timestamp::now());
    let (config, desired) = translate(&snapshot, &options.settings, &now);
    (options.on_config)(config);
    if !options.leader.load(Ordering::SeqCst) {
        return Ok(());
    }
    let mut failures = Vec::new();

    // Each Gateway's Service, applied when it differs from what's there
    // (rws owns these objects outright: server-side apply, forced).
    for ((namespace, name), service) in &desired.services {
        let current = snapshot.services.iter().find(|s| s.namespace().as_deref() == Some(namespace.as_str()) && s.name_any() == *name);
        if current.is_some_and(|c| super::services::same(c, service)) {
            continue;
        }
        let api: Api<Service> = Api::namespaced(client.clone(), namespace);
        let mut applied = service.clone();
        applied.metadata.managed_fields = None;
        let body = serde_json::json!({"apiVersion": "v1", "kind": "Service", "metadata": applied.metadata, "spec": applied.spec});
        if let Err(e) = api.patch(name, &PatchParams::apply(super::FIELD_MANAGER).force(), &Patch::Apply(&body)).await {
            failures.push(format!("Service {namespace}/{name}: {e}"));
        }
    }

    // Provisioned mode: each Gateway's Deployment; those of Gateways gone are deleted.
    if options.settings.provision.is_some() {
        for deployment in desired.deployments.values() {
            let (namespace, name) = (deployment.namespace().unwrap_or_default(), deployment.name_any());
            let current = snapshot.deployments.iter().find(|d| d.namespace().as_deref() == Some(namespace.as_str()) && d.name_any() == name);
            if current.is_some_and(|c| super::provision::same(c, deployment)) {
                continue;
            }
            let api: Api<k8s_openapi::api::apps::v1::Deployment> = Api::namespaced(client.clone(), &namespace);
            let body = serde_json::json!({"apiVersion": "apps/v1", "kind": "Deployment", "metadata": deployment.metadata, "spec": deployment.spec});
            if let Err(e) = api.patch(&name, &PatchParams::apply(super::FIELD_MANAGER).force(), &Patch::Apply(&body)).await {
                failures.push(format!("Deployment {namespace}/{name}: {e}"));
            }
        }
        for stale in super::provision::stale(&snapshot.deployments, &desired.deployments) {
            let api: Api<k8s_openapi::api::apps::v1::Deployment> = Api::namespaced(client.clone(), &stale.namespace().unwrap_or_default());
            if let Err(e) = api.delete(&stale.name_any(), &Default::default()).await {
                failures.push(format!("deleting Deployment {}: {e}", stale.name_any()));
            }
        }
    }

    for ((namespace, name), slice) in &desired.endpoint_slices {
        let current = snapshot.endpoint_slices.iter().find(|s| s.namespace().as_deref() == Some(namespace.as_str()) && s.name_any() == *name);
        if current.is_some_and(|c| super::services::same_slice(c, slice)) {
            continue;
        }
        let api: Api<EndpointSlice> = Api::namespaced(client.clone(), namespace);
        let body = serde_json::json!({
            "apiVersion": "discovery.k8s.io/v1", "kind": "EndpointSlice",
            "metadata": slice.metadata, "addressType": slice.address_type, "endpoints": slice.endpoints, "ports": slice.ports,
        });
        if let Err(e) = api.patch(name, &PatchParams::apply(super::FIELD_MANAGER).force(), &Patch::Apply(&body)).await {
            failures.push(format!("EndpointSlice {namespace}/{name}: {e}"));
        }
    }

    for class in &snapshot.classes {
        let Some(status) = desired.classes.get(&class.name_any()) else { continue };
        if class.status.as_ref() == Some(status) {
            continue;
        }
        let api: Api<GatewayClass> = Api::all_with(client.clone(), &resources::gateway_classes());
        if let Err(e) = patch_status(&api, &class.name_any(), status, None).await {
            failures.push(e);
        }
    }
    for gateway in &snapshot.gateways {
        let namespace = gateway.namespace().unwrap_or_default();
        let Some(status) = desired.gateways.get(&(namespace.clone(), gateway.name_any())) else { continue };
        if gateway.status.as_ref() == Some(status) {
            continue;
        }
        let api: Api<Gateway> = Api::namespaced_with(client.clone(), &namespace, &resources::gateways());
        if let Err(e) = patch_status(&api, &gateway.name_any(), status, None).await {
            failures.push(e);
        }
    }
    for set in &snapshot.listener_sets {
        let namespace = set.namespace().unwrap_or_default();
        let Some(status) = desired.listener_sets.get(&(namespace.clone(), set.name_any())) else { continue };
        if set.status.as_ref() == Some(status) {
            continue;
        }
        let api: Api<ListenerSet> = Api::namespaced_with(client.clone(), &namespace, &resources::listener_sets());
        if let Err(e) = patch_status(&api, &set.name_any(), status, None).await {
            failures.push(e);
        }
    }
    let controller_name = options.settings.controller_name.as_str();
    for policy in &snapshot.backend_tls_policies {
        let namespace = policy.namespace().unwrap_or_default();
        let Some(ours) = desired.backend_tls_policies.get(&(namespace.clone(), policy.name_any())) else { continue };
        let current = policy.status.as_ref().map(|s| s.ancestors.clone()).unwrap_or_default();
        let mut ancestors: Vec<super::types::PolicyAncestorStatus> = current.iter().filter(|a| a.controller_name != controller_name).cloned().collect();
        ancestors.extend(ours.iter().cloned());
        if ancestors == current {
            continue;
        }
        let api: Api<BackendTlsPolicy> = Api::namespaced_with(client.clone(), &namespace, &resources::backend_tls_policies());
        let status = super::types::PolicyStatus { ancestors };
        if let Err(e) = patch_status(&api, &policy.name_any(), &status, policy.resource_version()).await {
            failures.push(e);
        }
    }
    failures.extend(write_route_status(client, &snapshot.http_routes, &desired.http_routes, &resources::http_routes(), controller_name).await);
    failures.extend(write_route_status(client, &snapshot.grpc_routes, &desired.grpc_routes, &resources::grpc_routes(), controller_name).await);
    failures.extend(write_route_status(client, &snapshot.tls_routes, &desired.tls_routes, &resources::tls_routes(), controller_name).await);
    failures.extend(write_route_status(client, &snapshot.tcp_routes, &desired.tcp_routes, &resources::tcp_routes(), controller_name).await);
    failures.extend(write_route_status(client, &snapshot.udp_routes, &desired.udp_routes, &resources::udp_routes(), controller_name).await);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// Write our part of `status.parents` on every route of one kind where it
/// changed; returns the failures.
async fn write_route_status<S>(
    client: &Client,
    routes: &[kube::core::Object<S, super::types::RouteStatus>],
    desired: &std::collections::BTreeMap<(String, String), Vec<RouteParentStatus>>,
    resource: &kube::core::ApiResource,
    controller_name: &str,
) -> Vec<String>
where
    S: Clone + DeserializeOwned + std::fmt::Debug,
{
    let mut failures = Vec::new();
    for route in routes {
        let namespace = route.namespace().unwrap_or_default();
        let Some(ours) = desired.get(&(namespace.clone(), route.name_any())) else { continue };
        let current = route.status.as_ref().map(|s| s.parents.clone()).unwrap_or_default();
        let parents = merge_parents(&current, ours, controller_name);
        if parents == current {
            continue;
        }
        let api: Api<kube::core::Object<S, super::types::RouteStatus>> = Api::namespaced_with(client.clone(), &namespace, resource);
        let status = super::types::RouteStatus { parents };
        if let Err(e) = patch_status(&api, &route.name_any(), &status, route.resource_version()).await {
            failures.push(e);
        }
    }
    failures
}

/// Other controllers' entries as they are, then ours.
pub fn merge_parents(current: &[RouteParentStatus], ours: &[RouteParentStatus], controller_name: &str) -> Vec<RouteParentStatus> {
    let mut parents: Vec<RouteParentStatus> = current.iter().filter(|p| p.controller_name != controller_name).cloned().collect();
    parents.extend(ours.iter().cloned());
    parents
}

async fn patch_status<K, S>(api: &Api<K>, name: &str, status: &S, resource_version: Option<String>) -> Result<(), String>
where
    K: Resource + Clone + DeserializeOwned + std::fmt::Debug,
    S: serde::Serialize,
{
    let mut patch = serde_json::json!({ "status": status });
    if let Some(version) = resource_version {
        patch["metadata"] = serde_json::json!({ "resourceVersion": version });
    }
    api.patch_status(name, &PatchParams::apply(super::FIELD_MANAGER), &Patch::Merge(&patch))
        .await
        .map(|_| ())
        .map_err(|e| format!("status of {name}: {e}"))
}

#[cfg(test)]
mod tests;
