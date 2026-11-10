//! Tests against a real API server: a client for a test cluster (kind,
//! k3d, envtest's `kube-apiserver`) and a namespace of its own per test,
//! deleted afterwards — what controllers and webhooks need, since a fake
//! API server doesn't run admission, garbage collection or `status`
//! subresources.
//!
//! ```rust,no_run
//! use rws_k8s::k8s::testing::TestCluster;
//!
//! #[tokio::test]
//! async fn my_controller_creates_the_deployment() {
//!     let Some(cluster) = TestCluster::from_env().await else { return };   // skipped without a cluster
//!     let ns = cluster.namespace("backup").await.unwrap();
//!     // … create objects in ns.name(), run the reconcile, assert …
//!     ns.delete().await;
//! }
//! ```
//!
//! The cluster comes from `RWS_TEST_KUBECONFIG` (a kubeconfig file). Without
//! it the test is skipped, unless `RWS_TEST_REQUIRE_KUBERNETES` is set — then
//! it fails, so CI can't pass by skipping. Never point it at a cluster that
//! matters: the tests create and delete namespaces.

use std::time::Duration;

use k8s_openapi::api::core::v1::Namespace;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, DeleteParams, PostParams};

use super::Result;

/// A test cluster's client.
#[derive(Clone)]
pub struct TestCluster {
    pub client: kube::Client,
}

impl TestCluster {
    /// The cluster in `RWS_TEST_KUBECONFIG`, or `None` (skip the test).
    ///
    /// # Panics
    ///
    /// With `RWS_TEST_REQUIRE_KUBERNETES` set and no usable cluster.
    pub async fn from_env() -> Option<TestCluster> {
        let required = std::env::var("RWS_TEST_REQUIRE_KUBERNETES").is_ok_and(|v| !v.is_empty());
        let Some(path) = std::env::var("RWS_TEST_KUBECONFIG").ok().filter(|p| !p.is_empty()) else {
            assert!(!required, "RWS_TEST_REQUIRE_KUBERNETES is set but RWS_TEST_KUBECONFIG isn't");
            eprintln!("skipped: no RWS_TEST_KUBECONFIG");
            return None;
        };
        match super::client::from_kubeconfig(&path).await {
            Ok(client) => Some(TestCluster { client }),
            Err(e) => {
                assert!(!required, "RWS_TEST_KUBECONFIG={path}: {e}");
                eprintln!("skipped: RWS_TEST_KUBECONFIG={path}: {e}");
                None
            }
        }
    }

    /// A new namespace `<prefix>-<random>`, labelled `rws-test=true`.
    pub async fn namespace(&self, prefix: &str) -> Result<TestNamespace> {
        let suffix = format!("{:08x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() ^ (d.as_secs() as u32).rotate_left(7)).unwrap_or(0) ^ std::process::id());
        let name = format!("{}-{suffix}", prefix.trim_matches('-').to_lowercase());
        let ns = Namespace {
            metadata: ObjectMeta { name: Some(name.clone()), labels: Some([("rws-test".to_string(), "true".to_string())].into()), ..Default::default() },
            ..Default::default()
        };
        Api::<Namespace>::all(self.client.clone()).create(&PostParams::default(), &ns).await?;
        Ok(TestNamespace { client: self.client.clone(), name })
    }

    /// Apply a CRD (server-side) and wait until the API server serves it.
    pub async fn install_crd<K: kube::CustomResourceExt>(&self) -> Result<()> {
        use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
        use kube::api::{Patch, PatchParams};
        let crd = K::crd();
        let name = crd.metadata.name.clone().unwrap_or_default();
        let api: Api<CustomResourceDefinition> = Api::all(self.client.clone());
        api.patch(&name, &PatchParams::apply("rws-test").force(), &Patch::Apply(&crd)).await?;
        let established = kube::runtime::wait::conditions::is_crd_established();
        tokio::time::timeout(Duration::from_secs(30), kube::runtime::wait::await_condition(api, &name, established))
            .await
            .map_err(|_| super::Error::Other(format!("CRD {name} not established within 30 s")))?
            .map_err(|e| super::Error::Other(e.to_string()))?;
        Ok(())
    }
}

/// A namespace of one test. [`delete`](Self::delete) it at the end (a
/// failed test leaves it for inspection; they're labelled `rws-test=true`).
pub struct TestNamespace {
    client: kube::Client,
    name: String,
}

impl TestNamespace {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// An `Api` for `K` in this namespace.
    pub fn api<K>(&self) -> Api<K>
    where
        K: kube::Resource<Scope = k8s_openapi::NamespaceResourceScope, DynamicType = ()>,
    {
        Api::namespaced(self.client.clone(), &self.name)
    }

    /// Delete it (in the background on the API server's side).
    pub async fn delete(self) {
        let _ = Api::<Namespace>::all(self.client.clone()).delete(&self.name, &DeleteParams::background()).await;
    }
}

/// Wait until `check` answers `Some` (polled every 100 ms), at most
/// `timeout`; for asserting what a controller eventually does.
pub async fn eventually<T, F, Fut>(timeout: Duration, mut check: F) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(v) = check().await {
            return Some(v);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
