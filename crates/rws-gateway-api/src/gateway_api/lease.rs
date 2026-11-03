//! Leader election over a `coordination.k8s.io/v1` Lease: every replica
//! serves traffic, only the holder writes status and Services. The holder
//! renews every [`RENEW`]; another replica takes over once a renewal is older
//! than the lease's duration. Updates carry the Lease's `resourceVersion`,
//! so two replicas can't both win.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use k8s_openapi::api::coordination::v1::{Lease, LeaseSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta};
use kube::api::{Api, PostParams};
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
    let Some(spec) = lease.and_then(|l| l.spec.as_ref()) else {
        return if lease.is_some() { Decision::Take } else { Decision::Create };
    };
    if spec.holder_identity.as_deref() == Some(identity) {
        return Decision::Take;
    }
    let duration = spec.lease_duration_seconds.unwrap_or(DURATION.as_secs() as i32) as i64;
    let renewed = spec.renew_time.as_ref().map(|t| t.0.as_second()).unwrap_or(0);
    if spec.holder_identity.as_deref().unwrap_or("").is_empty() || renewed + duration < now.as_second() {
        Decision::Take
    } else {
        Decision::Follow
    }
}

/// Hold or follow the Lease forever; `leader` says which, `gained` fires
/// when this replica becomes the leader (so it can reconcile at once).
pub async fn run(client: Client, settings: LeaseSettings, leader: Arc<AtomicBool>, gained: Arc<Notify>) {
    let api: Api<Lease> = Api::namespaced(client, &settings.namespace);
    loop {
        let now = k8s_openapi::jiff::Timestamp::now();
        let current = api.get_opt(&settings.name).await;
        let holding = match current {
            Err(e) => {
                eprintln!("[gateway-api] lease: {e}");
                false
            }
            Ok(current) => match decide(current.as_ref(), &settings.identity, now) {
                Decision::Follow => false,
                Decision::Create => {
                    let lease = held(&settings, None, now);
                    api.create(&PostParams::default(), &lease).await.is_ok()
                }
                Decision::Take => {
                    let lease = held(&settings, current.as_ref(), now);
                    api.replace(&settings.name, &PostParams::default(), &lease).await.is_ok()
                }
            },
        };
        let was = leader.swap(holding, Ordering::SeqCst);
        if holding && !was {
            gained.notify_one();
        }
        tokio::time::sleep(RENEW).await;
    }
}

/// The Lease held by this replica, based on `current` (keeping its
/// `resourceVersion` and counting a change of holder).
fn held(settings: &LeaseSettings, current: Option<&Lease>, now: k8s_openapi::jiff::Timestamp) -> Lease {
    let previous = current.and_then(|l| l.spec.clone()).unwrap_or_default();
    let changed = previous.holder_identity.as_deref() != Some(settings.identity.as_str());
    Lease {
        metadata: ObjectMeta {
            name: Some(settings.name.clone()),
            namespace: Some(settings.namespace.clone()),
            resource_version: current.and_then(|l| l.metadata.resource_version.clone()),
            ..Default::default()
        },
        spec: Some(LeaseSpec {
            holder_identity: Some(settings.identity.clone()),
            lease_duration_seconds: Some(DURATION.as_secs() as i32),
            acquire_time: if changed { Some(MicroTime(now)) } else { previous.acquire_time.clone() },
            renew_time: Some(MicroTime(now)),
            lease_transitions: Some(previous.lease_transitions.unwrap_or(0) + changed as i32),
            ..Default::default()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
