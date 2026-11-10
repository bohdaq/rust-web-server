//! Leader election over a `coordination.k8s.io/v1` Lease: every replica
//! runs, one at a time holds the Lease and does the singleton work (a
//! controller, a scheduler). The holder renews every `renew`; another
//! replica takes over once a renewal is older than the lease's duration.
//! Writes carry the Lease's `resourceVersion`, so two replicas can't both
//! win.
//!
//! ```rust,no_run
//! # async fn example() -> rws_k8s::k8s::Result<()> {
//! use rws_k8s::k8s::{client, leader::LeaderElector};
//!
//! let elector = LeaderElector::new(client::client().await?, "default", "reports-scheduler");
//! let is_leader = elector.handle();
//! tokio::spawn(elector.run());
//! // Anywhere: do the singleton work only while leading.
//! if is_leader.is_leader() { /* … */ }
//! # Ok(()) }
//! ```
//!
//! The pod needs `get`, `create` and `update` on `leases` in the namespace
//! ([`manifests::Rbac::leader_election`](super::manifests::Rbac::leader_election)).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use k8s_openapi::api::coordination::v1::{Lease, LeaseSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta};
use k8s_openapi::jiff::Timestamp;
use kube::api::{Api, PostParams};
use tokio::sync::watch;

/// What a replica does with the Lease as it is now.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Create it, holding it.
    Create,
    /// Write it back holding it (renewing, or taking over an expired one).
    Take,
    /// Someone else holds it.
    Follow,
}

/// The decision for `identity` given the Lease (`None`: there is none).
pub fn decide(lease: Option<&Lease>, identity: &str, now: Timestamp, default_duration: Duration) -> Decision {
    let Some(spec) = lease.and_then(|l| l.spec.as_ref()) else {
        return if lease.is_some() { Decision::Take } else { Decision::Create };
    };
    if spec.holder_identity.as_deref() == Some(identity) {
        return Decision::Take;
    }
    let duration = spec.lease_duration_seconds.map(i64::from).unwrap_or(default_duration.as_secs() as i64);
    let renewed = spec.renew_time.as_ref().map(|t| t.0.as_second()).unwrap_or(0);
    if spec.holder_identity.as_deref().unwrap_or("").is_empty() || renewed + duration < now.as_second() {
        Decision::Take
    } else {
        Decision::Follow
    }
}

/// Whether this replica leads, and a way to wait for a change.
#[derive(Clone)]
pub struct LeaderHandle {
    leading: Arc<AtomicBool>,
    changes: watch::Receiver<bool>,
}

impl LeaderHandle {
    pub fn is_leader(&self) -> bool {
        self.leading.load(Ordering::SeqCst)
    }

    /// Wait until this replica leads (at once when it does).
    pub async fn wait_until_leader(&mut self) {
        let _ = self.changes.wait_for(|leading| *leading).await;
    }

    /// Wait for the next change of leadership.
    pub async fn changed(&mut self) -> bool {
        let _ = self.changes.changed().await;
        *self.changes.borrow()
    }
}

/// Holds or follows one Lease. See the [module docs](self).
pub struct LeaderElector {
    api: Api<Lease>,
    namespace: String,
    name: String,
    identity: String,
    duration: Duration,
    renew: Duration,
    leading: Arc<AtomicBool>,
    tx: watch::Sender<bool>,
    rx: watch::Receiver<bool>,
}

impl LeaderElector {
    /// The Lease `name` in `namespace`; this replica's identity from
    /// [`client::identity`](super::client::identity). 15 s duration,
    /// renewed every 5 s.
    pub fn new(client: kube::Client, namespace: &str, name: &str) -> Self {
        let (tx, rx) = watch::channel(false);
        LeaderElector {
            api: Api::namespaced(client, namespace),
            namespace: namespace.to_string(),
            name: name.to_string(),
            identity: super::client::identity(),
            duration: Duration::from_secs(15),
            renew: Duration::from_secs(5),
            leading: Arc::new(AtomicBool::new(false)),
            tx,
            rx,
        }
    }

    /// This replica's identity in the Lease.
    pub fn identity(mut self, identity: &str) -> Self {
        self.identity = identity.to_string();
        self
    }

    /// How long a renewal counts, and how often to renew (renew < duration).
    pub fn timing(mut self, duration: Duration, renew: Duration) -> Self {
        self.duration = duration.max(Duration::from_secs(1));
        self.renew = renew.min(self.duration / 2).max(Duration::from_millis(100));
        self
    }

    /// A handle to read and wait on leadership.
    pub fn handle(&self) -> LeaderHandle {
        LeaderHandle { leading: Arc::clone(&self.leading), changes: self.rx.clone() }
    }

    /// One round: read the Lease, create, renew, take over or follow.
    /// Returns whether this replica leads now.
    pub async fn step(&self) -> bool {
        let now = Timestamp::now();
        let holding = match self.api.get_opt(&self.name).await {
            Err(e) => {
                eprintln!("[k8s] lease {}/{}: {e}", self.namespace, self.name);
                false
            }
            Ok(current) => match decide(current.as_ref(), &self.identity, now, self.duration) {
                Decision::Follow => false,
                Decision::Create => self.api.create(&PostParams::default(), &self.held(None, now)).await.is_ok(),
                Decision::Take => self.api.replace(&self.name, &PostParams::default(), &self.held(current.as_ref(), now)).await.is_ok(),
            },
        };
        if self.leading.swap(holding, Ordering::SeqCst) != holding {
            let _ = self.tx.send(holding);
        }
        holding
    }

    /// Hold or follow the Lease until the task is dropped.
    pub async fn run(self) {
        loop {
            self.step().await;
            tokio::time::sleep(self.renew).await;
        }
    }

    /// Give the Lease up (on shutdown), so another replica takes over at
    /// once instead of after the duration.
    pub async fn release(&self) -> super::Result<()> {
        if let Some(mut lease) = self.api.get_opt(&self.name).await? {
            if lease.spec.as_ref().and_then(|s| s.holder_identity.as_deref()) == Some(self.identity.as_str()) {
                if let Some(spec) = lease.spec.as_mut() {
                    spec.holder_identity = None;
                    spec.renew_time = None;
                }
                self.api.replace(&self.name, &PostParams::default(), &lease).await?;
            }
        }
        self.leading.store(false, Ordering::SeqCst);
        let _ = self.tx.send(false);
        Ok(())
    }

    /// The Lease held by this replica, based on `current`.
    fn held(&self, current: Option<&Lease>, now: Timestamp) -> Lease {
        held(&self.namespace, &self.name, &self.identity, self.duration, current, now)
    }
}

/// The Lease `name` held by `identity`, based on `current` (keeping its
/// `resourceVersion` and counting a change of holder).
pub fn held(namespace: &str, name: &str, identity: &str, duration: Duration, current: Option<&Lease>, now: Timestamp) -> Lease {
    let previous = current.and_then(|l| l.spec.clone()).unwrap_or_default();
    let changed = previous.holder_identity.as_deref() != Some(identity);
    Lease {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            resource_version: current.and_then(|l| l.metadata.resource_version.clone()),
            ..Default::default()
        },
        spec: Some(LeaseSpec {
            holder_identity: Some(identity.to_string()),
            lease_duration_seconds: Some(duration.as_secs() as i32),
            acquire_time: if changed { Some(MicroTime(now)) } else { previous.acquire_time.clone() },
            renew_time: Some(MicroTime(now)),
            lease_transitions: Some(previous.lease_transitions.unwrap_or(0) + changed as i32),
            ..Default::default()
        }),
    }
}

#[cfg(test)]
mod tests;
