//! Service discovery from the Kubernetes API: the ready endpoints of a
//! Service (its EndpointSlices), kept current in a
//! [`BackendPool`](rws_server::service_discovery::BackendPool) — for a
//! `ReverseProxy`, or to talk to each pod of a headless Service.
//!
//! ```rust,no_run
//! # async fn example() -> rws_k8s::k8s::Result<()> {
//! use rws_k8s::k8s::{client, discovery};
//!
//! let pool = discovery::service_endpoints(client::client().await?, "default", "orders", Some("http")).await?;
//! println!("{:?}", pool.backends());   // ["10.42.0.17:8080", "10.42.1.9:8080"]
//! # Ok(()) }
//! ```
//!
//! Only endpoints whose `ready` condition isn't `false` count. The pod
//! needs `list` and `watch` on `endpointslices` (`discovery.k8s.io`).

use std::collections::BTreeMap;

use futures_util::StreamExt;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::api::Api;
use kube::runtime::watcher;
use rws_server::service_discovery::BackendPool;

use super::Result;

/// `ip:port` of every ready endpoint in `slices`, sorted and deduplicated.
/// `port_name`: the Service port's name, else the slice's first port.
pub fn ready_addresses<'a>(slices: impl IntoIterator<Item = &'a EndpointSlice>, port_name: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    for slice in slices {
        let ports = slice.ports.clone().unwrap_or_default();
        let port = match port_name {
            Some(name) => ports.iter().find(|p| p.name.as_deref() == Some(name)).and_then(|p| p.port),
            None => ports.first().and_then(|p| p.port),
        };
        let Some(port) = port else { continue };
        let ipv6 = slice.address_type == "IPv6";
        for endpoint in &slice.endpoints {
            let ready = endpoint.conditions.as_ref().and_then(|c| c.ready).unwrap_or(true);
            if !ready {
                continue;
            }
            for address in &endpoint.addresses {
                out.push(if ipv6 { format!("[{address}]:{port}") } else { format!("{address}:{port}") });
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// A pool holding the ready endpoints of Service `service` in `namespace`,
/// kept current by a watch of its EndpointSlices (the task ends with the
/// runtime). Waits for the first list.
pub async fn service_endpoints(client: kube::Client, namespace: &str, service: &str, port_name: Option<&str>) -> Result<BackendPool> {
    let pool = BackendPool::r#static(Vec::new());
    let api: Api<EndpointSlice> = Api::namespaced(client, namespace);
    let selector = format!("kubernetes.io/service-name={service}");
    // The first list now: errors (no permission) surface to the caller.
    let initial = api.list(&kube::api::ListParams::default().labels(&selector)).await?;
    let mut slices: BTreeMap<String, EndpointSlice> = initial.items.into_iter().map(|s| (s.metadata.name.clone().unwrap_or_default(), s)).collect();
    let port = port_name.map(str::to_string);
    pool.update(ready_addresses(slices.values(), port.as_deref()));
    let task_pool = pool.clone();
    tokio::spawn(async move {
        let mut stream = watcher::watcher(api, watcher::Config::default().labels(&selector)).boxed();
        let mut init: BTreeMap<String, EndpointSlice> = BTreeMap::new();
        while let Some(event) = stream.next().await {
            match event {
                Ok(watcher::Event::Apply(s)) => {
                    slices.insert(s.metadata.name.clone().unwrap_or_default(), s);
                }
                Ok(watcher::Event::Delete(s)) => {
                    slices.remove(&s.metadata.name.clone().unwrap_or_default());
                }
                Ok(watcher::Event::Init) => init.clear(),
                Ok(watcher::Event::InitApply(s)) => {
                    init.insert(s.metadata.name.clone().unwrap_or_default(), s);
                    continue;
                }
                Ok(watcher::Event::InitDone) => slices = std::mem::take(&mut init),
                Err(e) => {
                    eprintln!("[k8s] endpoints of {selector}: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            }
            task_pool.update(ready_addresses(slices.values(), port.as_deref()));
        }
    });
    Ok(pool)
}

#[cfg(test)]
mod tests;
