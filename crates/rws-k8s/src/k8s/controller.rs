//! Controllers: watch a kind, reconcile each object toward what it asks
//! for, requeue, clean up through a finalizer — kube-runtime's controller
//! with the parts every operator writes anyway: an [`Operator`] running
//! several controllers under leader election, a [`Context`] with status
//! and event helpers, metrics on `/metrics`, a readiness check, and the
//! RBAC rules the controllers need.
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//! use rws_k8s::k8s::controller::{Action, Context, Controller, Operator};
//! use rws_k8s::k8s::kube::{api::Api, CustomResource};
//! use rws_k8s::k8s::schemars::JsonSchema;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
//! #[kube(group = "example.com", version = "v1", kind = "Backup", namespaced, status = "BackupStatus")]
//! pub struct BackupSpec { pub schedule: String }
//!
//! #[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
//! pub struct BackupStatus { pub last_run: Option<String> }
//!
//! async fn reconcile(backup: Arc<Backup>, ctx: Arc<Context>) -> Result<Action, rws_k8s::k8s::Error> {
//!     // … make the world match backup.spec …
//!     ctx.patch_status(&*backup, serde_json::json!({ "last_run": "now" })).await?;
//!     Ok(Action::requeue(Duration::from_secs(300)))
//! }
//!
//! # async fn example() -> rws_k8s::k8s::Result<()> {
//! let client = rws_k8s::k8s::client::client().await?;
//! Operator::new(client.clone(), "backup-operator")
//!     .leader_election("default", "backup-operator")
//!     .controller(Controller::new(Api::<Backup>::all(client)).reconcile(reconcile).finalizer("example.com/cleanup", |_backup, _ctx| async { Ok(()) }))
//!     .run()
//!     .await;
//! # Ok(()) }
//! ```
//!
//! A reconcile that fails is retried after `on_error`'s delay (5 s by
//! default, doubling up to 5 min for the same object). With a finalizer,
//! the cleanup runs when the object is deleted and the finalizer is removed
//! after it succeeds.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use futures_util::StreamExt;
use kube::api::{Api, Patch, PatchParams};
use kube::runtime::events::{Event as KubeEvent, EventType, Recorder, Reporter};
use kube::runtime::finalizer::{finalizer, Event as FinalizerEvent};
use kube::runtime::watcher;
use kube::{Resource, ResourceExt};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub use kube::runtime::controller::Action;

use super::manifests::Rule;
use super::Error;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// What a reconcile gets besides the object: the client, an event
/// recorder, and the operator's name.
pub struct Context {
    pub client: kube::Client,
    pub operator: String,
    recorder: Recorder,
}

impl Context {
    fn new(client: kube::Client, operator: &str) -> Self {
        let reporter = Reporter { controller: operator.to_string(), instance: Some(super::client::identity()) };
        Context { recorder: Recorder::new(client.clone(), reporter), client, operator: operator.to_string() }
    }

    /// Merge `status` into `obj`'s status subresource.
    pub async fn patch_status<K>(&self, obj: &K, status: serde_json::Value) -> Result<K, Error>
    where
        K: Resource<DynamicType = ()> + Clone + DeserializeOwned + Debug,
        K::Scope: Scope,
    {
        let api = api_for(&self.client, obj);
        let patch = serde_json::json!({ "status": status });
        Ok(api.patch_status(&obj.name_any(), &PatchParams::default(), &Patch::Merge(&patch)).await?)
    }

    /// Server-side apply `child` as owned by `owner` (an owner reference,
    /// so it's garbage-collected with it), with this operator as the field
    /// manager.
    pub async fn apply_owned<O, C>(&self, owner: &O, mut child: C) -> Result<C, Error>
    where
        O: Resource<DynamicType = ()>,
        C: Resource<DynamicType = ()> + Clone + DeserializeOwned + Serialize + Debug,
        C::Scope: Scope,
    {
        let reference = owner.controller_owner_ref(&()).ok_or_else(|| Error::Other("the owner has no uid yet".into()))?;
        child.meta_mut().owner_references = Some(vec![reference]);
        if child.meta().namespace.is_none() {
            child.meta_mut().namespace = owner.meta().namespace.clone();
        }
        let api = api_for(&self.client, &child);
        let name = child.name_any();
        let params = PatchParams::apply(&self.operator).force();
        Ok(api.patch(&name, &params, &Patch::Apply(&child)).await?)
    }

    /// Record a Kubernetes Event about `obj` (`kubectl describe` shows it).
    pub async fn event<K: Resource<DynamicType = ()>>(&self, obj: &K, warning: bool, reason: &str, note: &str) -> Result<(), Error> {
        let event = KubeEvent {
            type_: if warning { EventType::Warning } else { EventType::Normal },
            reason: reason.to_string(),
            note: Some(note.to_string()),
            action: reason.to_string(),
            secondary: None,
        };
        Ok(self.recorder.publish(&event, &obj.object_ref(&())).await?)
    }
}

use super::{api_for, Scope};

type Reconcile<K> = Arc<dyn Fn(Arc<K>, Arc<Context>) -> BoxFuture<Result<Action, Error>> + Send + Sync>;
type Cleanup<K> = Arc<dyn Fn(Arc<K>, Arc<Context>) -> BoxFuture<Result<(), Error>> + Send + Sync>;
type Owns<K> = Box<dyn FnOnce(kube::runtime::Controller<K>, &kube::Client) -> kube::runtime::Controller<K> + Send>;

/// One controller: a kind, what it owns, how it reconciles. See the
/// [module docs](self).
pub struct Controller<K: Resource<DynamicType = ()> + Clone + Debug + 'static> {
    api: Api<K>,
    config: watcher::Config,
    owns: Vec<Owns<K>>,
    reconcile: Option<Reconcile<K>>,
    cleanup: Option<(String, Cleanup<K>)>,
    error_delay: Duration,
    rules: Vec<Rule>,
}

/// The API group and plural of `K` (`("apps", "deployments")`).
pub fn group_and_plural<K: Resource<DynamicType = ()>>() -> (String, String) {
    (K::group(&()).to_string(), K::plural(&()).to_string())
}

impl<K> Controller<K>
where
    K: Resource<DynamicType = ()> + Clone + DeserializeOwned + Serialize + Debug + Send + Sync + 'static,
    K::Scope: Scope,
{
    /// Watch `api`'s objects (`Api::all` or `Api::namespaced`).
    pub fn new(api: Api<K>) -> Self {
        let (group, plural) = group_and_plural::<K>();
        let rules = vec![
            Rule::new(&group, &[&plural], &["get", "list", "watch", "patch", "update"]),
            Rule::new(&group, &[&format!("{plural}/status")], &["get", "patch", "update"]),
            Rule::new("events.k8s.io", &["events"], &["create", "patch"]),
        ];
        Controller { api, config: watcher::Config::default(), owns: Vec::new(), reconcile: None, cleanup: None, error_delay: Duration::from_secs(5), rules }
    }

    /// Only the objects `config` selects (labels, fields).
    pub fn watching(mut self, config: watcher::Config) -> Self {
        self.config = config;
        self
    }

    /// Also reconcile an object when a `C` it owns (an owner reference)
    /// changes — the Deployment of a custom resource, say.
    pub fn owns<C>(mut self, api: Api<C>) -> Self
    where
        C: Resource<DynamicType = ()> + Clone + DeserializeOwned + Debug + Send + Sync + 'static,
    {
        let (group, plural) = group_and_plural::<C>();
        self.rules.push(Rule::new(&group, &[&plural], &["get", "list", "watch", "create", "update", "patch", "delete"]));
        self.owns.push(Box::new(move |c, _| c.owns(api, watcher::Config::default())));
        self
    }

    /// The reconcile function: make the world match the object, answer
    /// when to look again.
    pub fn reconcile<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(Arc<K>, Arc<Context>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Action, Error>> + Send + 'static,
    {
        self.reconcile = Some(Arc::new(move |obj, ctx| Box::pin(f(obj, ctx))));
        self
    }

    /// Add the finalizer `name` to every object and run `cleanup` when one
    /// is deleted; the finalizer is removed (letting the deletion finish)
    /// once `cleanup` succeeds.
    pub fn finalizer<F, Fut>(mut self, name: &str, cleanup: F) -> Self
    where
        F: Fn(Arc<K>, Arc<Context>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), Error>> + Send + 'static,
    {
        self.cleanup = Some((name.to_string(), Arc::new(move |obj, ctx| Box::pin(cleanup(obj, ctx)))));
        self
    }

    /// The first delay before retrying a failed reconcile (doubling per
    /// failure of the same object, up to 5 min). Default 5 s.
    pub fn on_error(mut self, first_delay: Duration) -> Self {
        self.error_delay = first_delay.max(Duration::from_millis(10));
        self
    }

    /// Extra RBAC rules the reconcile needs (reading Secrets, say).
    pub fn rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }
}

/// Per-controller counters for `/metrics`.
#[derive(Default)]
struct Counters {
    reconciles: AtomicU64,
    errors: AtomicU64,
}

fn counters() -> &'static Mutex<BTreeMap<String, Arc<Counters>>> {
    static C: OnceLock<Mutex<BTreeMap<String, Arc<Counters>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn counters_for(kind: &str) -> Arc<Counters> {
    rws_server::metrics::register_section(rws_server::metrics::section::CONTROLLERS, metrics_text);
    Arc::clone(counters().lock().unwrap_or_else(|e| e.into_inner()).entry(kind.to_string()).or_default())
}

/// `rws_controller_reconciles_total` / `rws_controller_reconcile_errors_total`
/// by controller kind.
pub fn metrics_text() -> String {
    let all = counters().lock().unwrap_or_else(|e| e.into_inner());
    if all.is_empty() {
        return String::new();
    }
    let mut out = String::from("# HELP rws_controller_reconciles_total Reconciles run, by controller.\n# TYPE rws_controller_reconciles_total counter\n");
    for (kind, c) in all.iter() {
        out.push_str(&format!("rws_controller_reconciles_total{{controller=\"{kind}\"}} {}\n", c.reconciles.load(Ordering::Relaxed)));
    }
    out.push_str("# HELP rws_controller_reconcile_errors_total Reconciles that failed, by controller.\n# TYPE rws_controller_reconcile_errors_total counter\n");
    for (kind, c) in all.iter() {
        out.push_str(&format!("rws_controller_reconcile_errors_total{{controller=\"{kind}\"}} {}\n", c.errors.load(Ordering::Relaxed)));
    }
    out
}

/// The delay before retry number `failures` (1-based): `first` doubled per
/// failure, at most 5 minutes.
pub fn backoff(first: Duration, failures: u32) -> Duration {
    let factor = 1u64 << failures.saturating_sub(1).min(16);
    first.saturating_mul(factor as u32).min(Duration::from_secs(300))
}

/// A controller, its kind erased, for the [`Operator`].
pub trait Runnable: Send {
    fn kind(&self) -> String;
    fn rules(&self) -> Vec<Rule>;
    fn start(self: Box<Self>, client: kube::Client, ctx: Arc<Context>) -> BoxFuture<()>;
}

#[derive(Debug)]
struct Failure(Error);

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Failure {}

impl<K> Runnable for Controller<K>
where
    K: Resource<DynamicType = ()> + Clone + DeserializeOwned + Serialize + Debug + Send + Sync + 'static,
    K::Scope: Scope,
{
    fn kind(&self) -> String {
        K::kind(&()).to_string()
    }

    fn rules(&self) -> Vec<Rule> {
        self.rules.clone()
    }

    fn start(self: Box<Self>, client: kube::Client, ctx: Arc<Context>) -> BoxFuture<()> {
        let this = *self;
        let kind = K::kind(&()).to_string();
        let counters = counters_for(&kind);
        let Some(reconcile) = this.reconcile else {
            eprintln!("[k8s] controller {kind}: no reconcile function; not started");
            return Box::pin(async {});
        };
        let cleanup = this.cleanup;
        let error_delay = this.error_delay;
        let failures: Arc<Mutex<BTreeMap<String, u32>>> = Arc::new(Mutex::new(BTreeMap::new()));
        let mut controller = kube::runtime::Controller::new(this.api, this.config);
        for owns in this.owns {
            controller = owns(controller, &client);
        }
        let reconcile_counters = Arc::clone(&counters);
        let reconcile_failures = Arc::clone(&failures);
        let reconciler = move |obj: Arc<K>, ctx: Arc<Context>| {
            let (reconcile, cleanup) = (Arc::clone(&reconcile), cleanup.clone());
            let (counters, failures) = (Arc::clone(&reconcile_counters), Arc::clone(&reconcile_failures));
            async move {
                counters.reconciles.fetch_add(1, Ordering::Relaxed);
                let key = format!("{}/{}", obj.namespace().unwrap_or_default(), obj.name_any());
                let result = match cleanup {
                    None => reconcile(obj, ctx).await,
                    Some((name, cleanup)) => {
                        let object_api = api_for(&ctx.client, &*obj);
                        finalizer(&object_api, &name, obj, |event| async {
                            match event {
                                FinalizerEvent::Apply(o) => reconcile(o, Arc::clone(&ctx)).await.map_err(Failure),
                                FinalizerEvent::Cleanup(o) => cleanup(o, Arc::clone(&ctx)).await.map(|_| Action::await_change()).map_err(Failure),
                            }
                        })
                        .await
                        .map_err(|e| Error::Other(e.to_string()))
                    }
                };
                if result.is_ok() {
                    failures.lock().unwrap_or_else(|e| e.into_inner()).remove(&key);
                }
                result.map_err(Failure)
            }
        };
        let error_counters = Arc::clone(&counters);
        let error_policy = move |obj: Arc<K>, error: &Failure, _ctx: Arc<Context>| {
            error_counters.errors.fetch_add(1, Ordering::Relaxed);
            let key = format!("{}/{}", obj.namespace().unwrap_or_default(), obj.name_any());
            let n = {
                let mut f = failures.lock().unwrap_or_else(|e| e.into_inner());
                let n = f.entry(key.clone()).or_insert(0);
                *n += 1;
                *n
            };
            let delay = backoff(error_delay, n);
            eprintln!("[k8s] reconcile {} {key} failed ({error}); retry in {delay:?}", K::kind(&()));
            Action::requeue(delay)
        };
        Box::pin(async move {
            controller
                .run(reconciler, error_policy, ctx)
                .for_each(|result| async move {
                    if let Err(e) = result {
                        // Watch errors (no permission, a missing CRD) and
                        // objects gone before their reconcile.
                        eprintln!("[k8s] controller: {e}");
                    }
                })
                .await;
        })
    }
}

/// Several controllers, optionally under leader election, with a readiness
/// check. See the [module docs](self).
pub struct Operator {
    client: kube::Client,
    name: String,
    controllers: Vec<Box<dyn Runnable>>,
    lease: Option<(String, String)>,
}

impl Operator {
    /// `name` is the field manager and event reporter.
    pub fn new(client: kube::Client, name: &str) -> Self {
        Operator { client, name: name.to_string(), controllers: Vec::new(), lease: None }
    }

    pub fn controller(mut self, controller: impl Runnable + 'static) -> Self {
        self.controllers.push(Box::new(controller));
        self
    }

    /// Run the controllers only on the replica holding the Lease `name` in
    /// `namespace`; the others wait, ready to take over.
    pub fn leader_election(mut self, namespace: &str, name: &str) -> Self {
        self.lease = Some((namespace.to_string(), name.to_string()));
        self
    }

    /// The RBAC rules the controllers (and the Lease) need.
    pub fn rules(&self) -> Vec<Rule> {
        let mut rules: Vec<Rule> = self.controllers.iter().flat_map(|c| c.rules()).collect();
        if self.lease.is_some() {
            rules.push(Rule::new("coordination.k8s.io", &["leases"], &["get", "create", "update"]));
        }
        super::manifests::merge_rules(rules)
    }

    /// The kinds the controllers reconcile.
    pub fn kinds(&self) -> Vec<String> {
        self.controllers.iter().map(|c| c.kind()).collect()
    }

    /// Run until the task is dropped. Registers a readiness check named
    /// after the operator: ready once this replica runs its controllers or
    /// follows a leader.
    pub async fn run(self) {
        let ctx = Arc::new(Context::new(self.client.clone(), &self.name));
        rws_server::health::readiness(&format!("operator {}", self.name), || Ok(()));
        match self.lease {
            None => run_all(self.controllers, self.client, ctx).await,
            Some((namespace, lease)) => {
                let elector = super::leader::LeaderElector::new(self.client.clone(), &namespace, &lease);
                let mut handle = elector.handle();
                tokio::spawn(elector.run());
                let mut controllers = Some(self.controllers);
                loop {
                    handle.wait_until_leader().await;
                    // kube's controllers can't be restarted from their parts: a
                    // replica that loses the Lease exits, and Kubernetes starts
                    // it again as a follower.
                    let Some(all) = controllers.take() else {
                        eprintln!("[k8s] operator {}: leadership lost; exiting so the pod restarts as a follower", self.name);
                        std::process::exit(1);
                    };
                    println!("[k8s] operator {}: leading; starting {} controller(s)", self.name, all.len());
                    let running = run_all(all, self.client.clone(), Arc::clone(&ctx));
                    tokio::pin!(running);
                    loop {
                        tokio::select! {
                            _ = &mut running => return,
                            leading = handle.changed() => if !leading { break },
                        }
                    }
                }
            }
        }
    }
}

async fn run_all(controllers: Vec<Box<dyn Runnable>>, client: kube::Client, ctx: Arc<Context>) {
    let futures: Vec<BoxFuture<()>> = controllers.into_iter().map(|c| c.start(client.clone(), Arc::clone(&ctx))).collect();
    futures_util::future::join_all(futures).await;
}

#[cfg(test)]
mod tests;
