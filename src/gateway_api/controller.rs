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
use k8s_openapi::api::core::v1::{Namespace, Secret, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::api::{Api, Patch, PatchParams};
use kube::runtime::reflector::{self, store::Writer, Store};
use kube::runtime::{watcher, WatchStreamExt};
use kube::{Client, Resource, ResourceExt};
use serde::de::DeserializeOwned;
use tokio::sync::Notify;

use super::resources::{self, Gateway, GatewayClass, GrpcRoute, HttpRoute, ListenerSet, ReferenceGrant, TcpRoute, TlsRoute, UdpRoute};
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
    pub reference_grants: Store<ReferenceGrant>,
    pub services: Store<Service>,
    pub namespaces: Store<Namespace>,
    pub secrets: Store<Secret>,
    pub endpoint_slices: Store<EndpointSlice>,
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
            reference_grants: all(&self.reference_grants),
            services: all(&self.services),
            namespaces: all(&self.namespaces),
            secrets: all(&self.secrets),
            endpoint_slices: all(&self.endpoint_slices),
        }
    }
}

/// Start a watcher for `api` feeding a new store; every change wakes `changed`.
fn watch<K>(api: Api<K>, dyntype: K::DynamicType, changed: Arc<Notify>) -> Store<K>
where
    K: Resource + Clone + DeserializeOwned + std::fmt::Debug + Send + Sync + 'static,
    K::DynamicType: std::hash::Hash + Eq + Clone + Send + Sync,
{
    let writer = Writer::new(dyntype);
    let reader = writer.as_reader();
    let stream = reflector::reflector(writer, watcher(api, watcher::Config::default())).default_backoff().touched_objects();
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

/// Start watching everything; returns the stores and the change signal.
pub fn start_watches(client: &Client) -> (Stores, Arc<Notify>) {
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
        reference_grants: watch(Api::all_with(client.clone(), &resources::reference_grants()), resources::reference_grants(), Arc::clone(&changed)),
        services: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
        namespaces: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
        secrets: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
        endpoint_slices: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
    };
    (stores, changed)
}

/// Reconcile on every change, when this replica becomes the leader, and
/// every [`RESYNC`], forever.
pub async fn run(client: Client, options: Options, became_leader: Arc<Notify>) {
    let (stores, changed) = start_watches(&client);
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
