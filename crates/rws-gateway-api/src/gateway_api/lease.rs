//! Leader election over a `coordination.k8s.io/v1` Lease: every replica
//! serves traffic, only the holder writes status and Services. The holder
//! renews every [`RENEW`]; another replica takes over once a renewal is older
//! than the lease's duration. Updates carry the Lease's `resourceVersion`,
//! so two replicas can't both win. Since 18.36.0 the work is done by
//! `rws_k8s`'s `LeaderElector`, shared with operators.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use k8s_openapi::api::coordination::v1::Lease;
use kube::Client;
use tokio::sync::Notify;

/// How long a holder's renewal counts.
pub const DURATION: Duration = Duration::from_secs(15);
/// How often the holder renews (and others check).
pub const RENEW: Duration = Duration::from_secs(5);

pub struct LeaseSettings {
    pub namespace: String,
    pub name: String,
    /// This replica (the pod's name).
    pub identity: String,
}

/// What a replica should do with the Lease as it is now.
#[derive(Debug, PartialEq)]
pub enum Decision {
    /// Create it, holding it.
    Create,
    /// Write it back holding it (renewing, or taking over an expired one).
    Take,
    /// Someone else holds it.
    Follow,
}

pub fn decide(lease: Option<&Lease>, identity: &str, now: k8s_openapi::jiff::Timestamp) -> Decision {
    match rws_k8s::k8s::leader::decide(lease, identity, now, DURATION) {
        rws_k8s::k8s::leader::Decision::Create => Decision::Create,
        rws_k8s::k8s::leader::Decision::Take => Decision::Take,
        rws_k8s::k8s::leader::Decision::Follow => Decision::Follow,
    }
}

/// Hold or follow the Lease forever (through `rws_k8s`'s `LeaderElector`,
/// 18.36.0); `leader` says which, `gained` fires when this replica becomes
/// the leader (so it can reconcile at once).
pub async fn run(client: Client, settings: LeaseSettings, leader: Arc<AtomicBool>, gained: Arc<Notify>) {
    let elector = rws_k8s::k8s::leader::LeaderElector::new(client, &settings.namespace, &settings.name).identity(&settings.identity).timing(DURATION, RENEW);
    loop {
        let holding = elector.step().await;
        let was = leader.swap(holding, Ordering::SeqCst);
        if holding && !was {
            gained.notify_one();
        }
        tokio::time::sleep(RENEW).await;
    }
}

/// The Lease held by this replica, based on `current` (keeping its
/// `resourceVersion` and counting a change of holder).
#[cfg(test)]
fn held(settings: &LeaseSettings, current: Option<&Lease>, now: k8s_openapi::jiff::Timestamp) -> Lease {
    rws_k8s::k8s::leader::held(&settings.namespace, &settings.name, &settings.identity, DURATION, current, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::coordination::v1::LeaseSpec;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::MicroTime;

    fn lease(holder: &str, renewed: i64) -> Lease {
        Lease {
            metadata: Default::default(),
            spec: Some(LeaseSpec {
                holder_identity: Some(holder.to_string()),
                lease_duration_seconds: Some(15),
                renew_time: Some(MicroTime(k8s_openapi::jiff::Timestamp::from_second(renewed).unwrap())),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn replicas_create_renew_follow_and_take_over() {
        let now = k8s_openapi::jiff::Timestamp::from_second(1_000).unwrap();
        assert_eq!(Decision::Create, decide(None, "a", now));
        assert_eq!(Decision::Take, decide(Some(&lease("a", 999)), "a", now), "the holder renews");
        assert_eq!(Decision::Follow, decide(Some(&lease("b", 990)), "a", now), "a live holder keeps it");
        assert_eq!(Decision::Take, decide(Some(&lease("b", 980)), "a", now), "an expired one is taken over");
        assert_eq!(Decision::Take, decide(Some(&lease("", 999)), "a", now), "nobody holds it");
    }

    #[test]
    fn taking_over_counts_a_transition_and_keeps_the_resource_version() {
        let settings = LeaseSettings { namespace: "ns".to_string(), name: "l".to_string(), identity: "a".to_string() };
        let mut current = lease("b", 0);
        current.metadata.resource_version = Some("42".to_string());
        let now = k8s_openapi::jiff::Timestamp::from_second(100).unwrap();
        let taken = held(&settings, Some(&current), now);
        assert_eq!(Some("42".to_string()), taken.metadata.resource_version);
        let spec = taken.spec.unwrap();
        assert_eq!((Some("a".to_string()), Some(1)), (spec.holder_identity.clone(), spec.lease_transitions));
        let renewed = held(&settings, Some(&Lease { spec: Some(spec.clone()), ..current }), now);
        assert_eq!(Some(1), renewed.spec.unwrap().lease_transitions, "renewing isn't a transition");
    }
}
