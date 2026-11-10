//! ConfigMaps and Secrets from the Kubernetes API, kept current by a watch:
//! no volume mount, no restart, and a change reaches the app within a
//! second or two.
//!
//! ```rust,no_run
//! # async fn example() -> rws_k8s::k8s::Result<()> {
//! use rws_k8s::k8s::{client, config::ConfigWatch};
//!
//! let client = client::client().await?;
//! let settings = ConfigWatch::config_map(client.clone(), "default", "app-settings").await?;
//! let page_size: usize = settings.get("page_size").and_then(|v| v.parse().ok()).unwrap_or(20);
//! settings.on_change(|data| println!("settings now: {data:?}"));
//!
//! // `RWS_CONFIG_*` keys straight into rws's own settings, re-read on change:
//! settings.apply_to_env("RWS_CONFIG_");
//!
//! let password = rws_k8s::k8s::config::resolve(&client, "k8s://default/db-credentials#password").await?;
//! # Ok(()) }
//! ```
//!
//! The pod needs `get`, `list` and `watch` on the object's kind in its
//! namespace ([`manifests::Rbac`](super::manifests::Rbac)).

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use futures_util::StreamExt;
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use kube::api::Api;
use kube::runtime::watcher;

use super::{Error, Result};

type Listener = Arc<dyn Fn(&BTreeMap<String, String>) + Send + Sync>;

struct Shared {
    data: RwLock<BTreeMap<String, String>>,
    listeners: RwLock<Vec<Listener>>,
    present: std::sync::atomic::AtomicBool,
}

/// One ConfigMap's or Secret's data, kept current. Cheap to clone; clones
/// share it. The watch stops when the last clone is dropped.
#[derive(Clone)]
pub struct ConfigWatch {
    shared: Arc<Shared>,
    _task: Arc<AbortOnDrop>,
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// A ConfigMap's data and binary data (as UTF-8 where it is).
pub fn config_map_data(cm: &ConfigMap) -> BTreeMap<String, String> {
    let mut out = cm.data.clone().unwrap_or_default();
    for (k, v) in cm.binary_data.clone().unwrap_or_default() {
        if let Ok(text) = String::from_utf8(v.0) {
            out.insert(k, text);
        }
    }
    out
}

/// A Secret's data (decoded) and string data, as UTF-8 where it is.
pub fn secret_data(secret: &Secret) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (k, v) in secret.data.clone().unwrap_or_default() {
        if let Ok(text) = String::from_utf8(v.0) {
            out.insert(k, text);
        }
    }
    out.extend(secret.string_data.clone().unwrap_or_default());
    out
}

impl ConfigWatch {
    /// Watch the ConfigMap `name` in `namespace`. Fails when it can't be
    /// read now (missing, or no permission).
    pub async fn config_map(client: kube::Client, namespace: &str, name: &str) -> Result<Self> {
        Self::start(Api::<ConfigMap>::namespaced(client, namespace), name, config_map_data).await
    }

    /// Watch the Secret `name` in `namespace`.
    pub async fn secret(client: kube::Client, namespace: &str, name: &str) -> Result<Self> {
        Self::start(Api::<Secret>::namespaced(client, namespace), name, secret_data).await
    }

    async fn start<K>(api: Api<K>, name: &str, read: fn(&K) -> BTreeMap<String, String>) -> Result<Self>
    where
        K: kube::Resource + Clone + std::fmt::Debug + serde::de::DeserializeOwned + Send + Sync + 'static,
    {
        let initial = api.get(name).await?;
        let shared = Arc::new(Shared {
            data: RwLock::new(read(&initial)),
            listeners: RwLock::new(Vec::new()),
            present: std::sync::atomic::AtomicBool::new(true),
        });
        let config = watcher::Config::default().fields(&format!("metadata.name={name}"));
        let task_shared = Arc::clone(&shared);
        let task = tokio::spawn(async move {
            let mut stream = watcher::watcher(api, config).boxed();
            while let Some(event) = stream.next().await {
                let next = match event {
                    Ok(watcher::Event::Apply(obj)) | Ok(watcher::Event::InitApply(obj)) => Some(read(&obj)),
                    Ok(watcher::Event::Delete(_)) => {
                        task_shared.present.store(false, std::sync::atomic::Ordering::SeqCst);
                        None
                    }
                    Ok(_) => None,
                    Err(e) => {
                        eprintln!("[k8s] config watch: {e}");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        None
                    }
                };
                if let Some(data) = next {
                    task_shared.present.store(true, std::sync::atomic::Ordering::SeqCst);
                    let changed = {
                        let mut current = task_shared.data.write().unwrap_or_else(|e| e.into_inner());
                        let changed = *current != data;
                        *current = data.clone();
                        changed
                    };
                    if changed {
                        let listeners = task_shared.listeners.read().unwrap_or_else(|e| e.into_inner()).clone();
                        for listener in listeners {
                            listener(&data);
                        }
                    }
                }
            }
        });
        Ok(ConfigWatch { shared, _task: Arc::new(AbortOnDrop(task)) })
    }

    /// The value of `key` now.
    pub fn get(&self, key: &str) -> Option<String> {
        self.shared.data.read().unwrap_or_else(|e| e.into_inner()).get(key).cloned()
    }

    /// Every key now.
    pub fn snapshot(&self) -> BTreeMap<String, String> {
        self.shared.data.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Whether the object exists (it may be deleted after the watch started;
    /// the last data is kept).
    pub fn exists(&self) -> bool {
        self.shared.present.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Call `f` with the new data after each change.
    pub fn on_change(&self, f: impl Fn(&BTreeMap<String, String>) + Send + Sync + 'static) {
        self.shared.listeners.write().unwrap_or_else(|e| e.into_inner()).push(Arc::new(f));
    }

    /// Set every key starting with `prefix` (`RWS_CONFIG_`) as an
    /// environment variable now and after each change, and have rws re-read
    /// its settings (`server_config::invalidate`). Keys under the prefix
    /// that disappear are left as they were.
    pub fn apply_to_env(&self, prefix: &str) {
        apply(&self.snapshot(), prefix);
        let prefix = prefix.to_string();
        self.on_change(move |data| apply(data, &prefix));
    }
}

fn apply(data: &BTreeMap<String, String>, prefix: &str) {
    let mut any = false;
    for (k, v) in data.iter().filter(|(k, _)| k.starts_with(prefix)) {
        // SAFETY of set_var: rws reads its settings through ServerConfig
        // snapshots, rebuilt after `invalidate`.
        unsafe { std::env::set_var(k, v) };
        any = true;
    }
    if any {
        rws_server::server_config::invalidate();
    }
}

/// A `k8s://<namespace>/<secret>#<key>` reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    pub namespace: String,
    pub name: String,
    pub key: String,
}

impl SecretRef {
    /// Parse `k8s://namespace/name#key` (`k8s://name#key`: the current
    /// namespace).
    pub fn parse(reference: &str) -> Result<SecretRef> {
        let rest = reference.strip_prefix("k8s://").ok_or_else(|| Error::Other(format!("{reference}: not a k8s:// reference")))?;
        let (path, key) = rest.split_once('#').ok_or_else(|| Error::Other(format!("{reference}: no #key")))?;
        let (namespace, name) = match path.split_once('/') {
            Some((ns, name)) => (ns.to_string(), name.to_string()),
            None => (super::client::current_namespace(), path.to_string()),
        };
        if namespace.is_empty() || name.is_empty() || key.is_empty() || name.contains('/') {
            return Err(Error::Other(format!("{reference}: expected k8s://namespace/name#key")));
        }
        Ok(SecretRef { namespace, name, key: key.to_string() })
    }
}

/// The value a `k8s://namespace/name#key` reference names: one key of a
/// Secret, read now.
pub async fn resolve(client: &kube::Client, reference: &str) -> Result<String> {
    let r = SecretRef::parse(reference)?;
    let secret = Api::<Secret>::namespaced(client.clone(), &r.namespace).get(&r.name).await?;
    secret_data(&secret)
        .remove(&r.key)
        .ok_or_else(|| Error::Other(format!("secret {}/{} has no key {}", r.namespace, r.name, r.key)))
}

#[cfg(test)]
mod tests;
