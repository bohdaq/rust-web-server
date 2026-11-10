use std::time::Duration;

use k8s_openapi::api::coordination::v1::{Lease, LeaseSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::MicroTime;
use k8s_openapi::jiff::Timestamp;

use super::*;

const D: Duration = Duration::from_secs(15);

fn lease(holder: &str, renewed: i64) -> Lease {
    Lease {
        metadata: Default::default(),
        spec: Some(LeaseSpec {
            holder_identity: Some(holder.to_string()),
            lease_duration_seconds: Some(15),
            renew_time: Some(MicroTime(Timestamp::from_second(renewed).unwrap())),
            ..Default::default()
        }),
    }
}

#[test]
fn replicas_create_renew_follow_and_take_over() {
    let now = Timestamp::from_second(1_000).unwrap();
    assert_eq!(Decision::Create, decide(None, "a", now, D));
    assert_eq!(Decision::Take, decide(Some(&lease("a", 999)), "a", now, D), "the holder renews");
    assert_eq!(Decision::Follow, decide(Some(&lease("b", 990)), "a", now, D), "a live holder keeps it");
    assert_eq!(Decision::Take, decide(Some(&lease("b", 980)), "a", now, D), "an expired one is taken over");
    assert_eq!(Decision::Take, decide(Some(&lease("", 999)), "a", now, D), "nobody holds it");
    assert_eq!(Decision::Take, decide(Some(&Lease::default()), "a", now, D), "a Lease without a spec");
}

#[test]
fn taking_over_counts_a_transition_and_keeps_the_resource_version() {
    let mut current = lease("b", 0);
    current.metadata.resource_version = Some("42".to_string());
    let now = Timestamp::from_second(100).unwrap();
    let taken = held("ns", "l", "a", D, Some(&current), now);
    assert_eq!(Some("42".to_string()), taken.metadata.resource_version);
    let spec = taken.spec.unwrap();
    assert_eq!((Some("a".to_string()), Some(1), Some(15)), (spec.holder_identity.clone(), spec.lease_transitions, spec.lease_duration_seconds));
    let renewed = held("ns", "l", "a", D, Some(&Lease { spec: Some(spec.clone()), ..current }), now);
    assert_eq!(Some(1), renewed.spec.unwrap().lease_transitions, "renewing isn't a transition");
}
