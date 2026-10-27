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

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use k8s_openapi::api::core::v1::{Namespace, Secret, Service};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::api::{Api, Patch, PatchParams};
use kube::runtime::reflector::{self, store::Writer, Store};
use kube::runtime::{watcher, WatchStreamExt};
use kube::{Client, Resource, ResourceExt};
use serde::de::DeserializeOwned;
use tokio::sync::Notify;

use super::resources::{self, Gateway, GatewayClass, HttpRoute, ReferenceGrant};
use super::translate::{translate, GatewayConfig, Snapshot};
use super::types::RouteParentStatus;

/// How long changes are gathered before a reconcile.
pub const DEBOUNCE: Duration = Duration::from_millis(100);

/// What the controller needs besides the cluster.
pub struct Options {
    pub controller_name: String,
    /// Write status (the leader does; others only serve).
    pub write_status: bool,
    /// Called with every new configuration.
    pub on_config: Arc<dyn Fn(GatewayConfig) + Send + Sync>,
}

/// The watched stores.
#[derive(Clone)]
pub struct Stores {
    pub classes: Store<GatewayClass>,
    pub gateways: Store<Gateway>,
    pub http_routes: Store<HttpRoute>,
    pub reference_grants: Store<ReferenceGrant>,
    pub services: Store<Service>,
    pub namespaces: Store<Namespace>,
    pub secrets: Store<Secret>,
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
            reference_grants: all(&self.reference_grants),
            services: all(&self.services),
            namespaces: all(&self.namespaces),
            secrets: all(&self.secrets),
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
        reference_grants: watch(Api::all_with(client.clone(), &resources::reference_grants()), resources::reference_grants(), Arc::clone(&changed)),
        services: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
        namespaces: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
        secrets: watch(Api::all(client.clone()), (), Arc::clone(&changed)),
    };
    (stores, changed)
}

/// Reconcile on every change, forever.
pub async fn run(client: Client, options: Options) {
    let (stores, changed) = start_watches(&client);
    loop {
        changed.notified().await;
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
    let (config, desired) = translate(&snapshot, &options.controller_name, &now);
    (options.on_config)(config);
    if !options.write_status {
        return Ok(());
    }
    let mut failures = Vec::new();

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
    for route in &snapshot.http_routes {
        let namespace = route.namespace().unwrap_or_default();
        let Some(ours) = desired.http_routes.get(&(namespace.clone(), route.name_any())) else { continue };
        let current = route.status.as_ref().map(|s| s.parents.clone()).unwrap_or_default();
        let parents = merge_parents(&current, ours, &options.controller_name);
        if parents == current {
            continue;
        }
        let api: Api<HttpRoute> = Api::namespaced_with(client.clone(), &namespace, &resources::http_routes());
        let status = super::types::RouteStatus { parents };
        if let Err(e) = patch_status(&api, &route.name_any(), &status, route.resource_version()).await {
            failures.push(e);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
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
