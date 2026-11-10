//! A Kubernetes client: in a pod its service account (token and CA under
//! `/var/run/secrets/kubernetes.io/serviceaccount`), elsewhere the current
//! context of `KUBECONFIG` or `~/.kube/config` — what kubectl uses.

use super::Result;

/// The client kube's `Client::try_default` makes (in-cluster first).
pub async fn client() -> Result<kube::Client> {
    Ok(kube::Client::try_default().await?)
}

/// A client for the kubeconfig file at `path` (its current context).
pub async fn from_kubeconfig(path: &str) -> Result<kube::Client> {
    let kubeconfig = kube::config::Kubeconfig::read_from(path).map_err(|e| super::Error::Other(format!("{path}: {e}")))?;
    let config = kube::Config::from_custom_kubeconfig(kubeconfig, &Default::default()).await.map_err(|e| super::Error::Other(format!("{path}: {e}")))?;
    Ok(kube::Client::try_from(config)?)
}

/// A client that is never used to connect: for code that needs one to
/// describe controllers (their RBAC rules) without a cluster, such as an
/// operator's `--manifests`. Any request it makes fails.
pub fn offline() -> kube::Client {
    let make = || kube::Client::try_from(kube::Config::new("https://127.0.0.1:9".parse().expect("a valid URL"))).expect("a client for a local URL");
    // The client spawns a task when it's built; outside a runtime, on one
    // of its own (kept for the process: the task must not be dropped).
    if tokio::runtime::Handle::try_current().is_ok() {
        return make();
    }
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| tokio::runtime::Builder::new_current_thread().build().expect("a runtime for an offline client"));
    let _entered = runtime.enter();
    make()
}

/// The namespace this pod runs in (the service account's), else the
/// kubeconfig's current one, else `default`.
pub fn current_namespace() -> String {
    if let Ok(ns) = std::env::var("POD_NAMESPACE") {
        if !ns.is_empty() {
            return ns;
        }
    }
    if let Ok(ns) = std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/namespace") {
        let ns = ns.trim();
        if !ns.is_empty() {
            return ns.to_string();
        }
    }
    kube::config::Kubeconfig::read()
        .ok()
        .and_then(|k| {
            let current = k.current_context?;
            k.contexts.into_iter().find(|c| c.name == current)?.context?.namespace
        })
        .unwrap_or_else(|| "default".to_string())
}

/// This replica's identity: the pod's name (`POD_NAME`, else `HOSTNAME`),
/// else the host name, else a random one.
pub fn identity() -> String {
    for var in ["POD_NAME", "HOSTNAME"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| format!("rws-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)))
}
