//! Unit tests for `CanaryLayer`.

use super::{CanaryLayer, WeightedBackend, WeightedPool};
use crate::service_discovery::BackendPool;

fn hosts_of(candidates: &[(String, u16, bool)]) -> Vec<&str> {
    candidates.iter().map(|(h, _, _)| h.as_str()).collect()
}

// ── basic construction / parsing ──────────────────────────────────────────────

#[test]
fn single_backend_is_always_the_primary_pick() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("backend:9000", 5)]);
    for _ in 0..10 {
        let candidates = layer.next_candidates();
        assert_eq!(1, candidates.len());
        assert_eq!(("backend".to_string(), 9000, false), candidates[0]);
    }
}

#[test]
fn empty_backends_produce_no_candidates() {
    let layer = CanaryLayer::new(vec![]);
    assert!(layer.next_candidates().is_empty());
}

#[test]
fn zero_weight_backend_never_appears() {
    let layer = CanaryLayer::new(vec![
        WeightedBackend::new("a:8080", 2),
        WeightedBackend::new("b:8080", 0),
        WeightedBackend::new("c:8080", 1),
    ]);
    for _ in 0..20 {
        let candidates = layer.next_candidates();
        assert!(!hosts_of(&candidates).contains(&"b"), "zero-weight backend should never be a candidate");
    }
}

#[test]
fn url_parsing_strips_http_prefix() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("http://myhost:1234", 1)]);
    let candidates = layer.next_candidates();
    assert_eq!(("myhost".to_string(), 1234, false), candidates[0]);
}

#[test]
fn url_parsing_defaults_to_port_80() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("myhost", 1)]);
    let candidates = layer.next_candidates();
    assert_eq!(("myhost".to_string(), 80, false), candidates[0]);
}

// ── TLS backend detection ─────────────────────────────────────────────────────

#[test]
fn https_scheme_sets_tls_and_default_port_443() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("https://secure-backend", 1)]);
    assert_eq!(("secure-backend".to_string(), 443, true), layer.next_candidates()[0]);
}

#[test]
fn https_scheme_with_explicit_port_sets_tls() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("https://secure-backend:8443", 1)]);
    assert_eq!(("secure-backend".to_string(), 8443, true), layer.next_candidates()[0]);
}

#[test]
fn h2s_scheme_sets_tls_and_default_port_443() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("h2s://backend", 1)]);
    assert_eq!(("backend".to_string(), 443, true), layer.next_candidates()[0]);
}

#[test]
fn grpcs_scheme_sets_tls_and_default_port_443() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("grpcs://backend", 1)]);
    assert_eq!(("backend".to_string(), 443, true), layer.next_candidates()[0]);
}

#[test]
fn h2_scheme_is_plain_not_tls() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("h2://backend:3000", 1)]);
    assert_eq!(("backend".to_string(), 3000, false), layer.next_candidates()[0]);
}

#[test]
fn grpc_scheme_is_plain_not_tls() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("grpc://backend:3000", 1)]);
    assert_eq!(("backend".to_string(), 3000, false), layer.next_candidates()[0]);
}

#[test]
fn mixed_tls_and_plain_backends_both_appear_as_fallback_candidates() {
    let layer = CanaryLayer::new(vec![
        WeightedBackend::new("https://secure:8443", 1),
        WeightedBackend::new("http://plain:8080", 1),
    ]);
    let candidates = layer.next_candidates();
    assert_eq!(2, candidates.len());
    assert!(candidates.iter().any(|(h, p, tls)| h == "secure" && *p == 8443 && *tls));
    assert!(candidates.iter().any(|(h, p, tls)| h == "plain" && *p == 8080 && !*tls));
}

// ── smooth weighted round-robin ───────────────────────────────────────────────

#[test]
fn weighted_distribution_is_proportional_over_one_full_cycle() {
    let layer = CanaryLayer::new(vec![
        WeightedBackend::new("stable:8080", 3),
        WeightedBackend::new("canary:8080", 1),
    ]);
    // One full SWRR cycle == total_weight ticks; each backend must be the
    // primary pick exactly `weight` times within it.
    let mut stable_primary = 0;
    let mut canary_primary = 0;
    for _ in 0..4 {
        let candidates = layer.next_candidates();
        match candidates[0].0.as_str() {
            "stable" => stable_primary += 1,
            "canary" => canary_primary += 1,
            other => panic!("unexpected primary pick {:?}", other),
        }
    }
    assert_eq!(3, stable_primary);
    assert_eq!(1, canary_primary);
}

#[test]
fn smooth_round_robin_never_bursts_the_high_weight_backend() {
    // Weights 5:1:1 — a flat pre-expanded rotation (the old implementation)
    // would produce "AAAAA" back to back once every 7 picks. SWRR must not.
    let layer = CanaryLayer::new(vec![
        WeightedBackend::new("a:80", 5),
        WeightedBackend::new("b:80", 1),
        WeightedBackend::new("c:80", 1),
    ]);

    let mut sequence = Vec::new();
    for _ in 0..14 {
        // two full cycles
        sequence.push(layer.next_candidates()[0].0.clone());
    }

    let mut max_run = 1;
    let mut current_run = 1;
    for w in sequence.windows(2) {
        if w[0] == w[1] {
            current_run += 1;
            max_run = max_run.max(current_run);
        } else {
            current_run = 1;
        }
    }
    assert!(max_run < 5, "expected no burst of 5 consecutive picks, got a run of {} in {:?}", max_run, sequence);

    let a_count = sequence.iter().filter(|h| h.as_str() == "a").count();
    let b_count = sequence.iter().filter(|h| h.as_str() == "b").count();
    let c_count = sequence.iter().filter(|h| h.as_str() == "c").count();
    assert_eq!(10, a_count);
    assert_eq!(2, b_count);
    assert_eq!(2, c_count);
}

#[test]
fn single_tick_only_mutates_state_once_regardless_of_fallback_depth() {
    // Calling next_candidates() (which internally ticks once and returns a
    // *read-only* ranked fallback order) repeatedly must behave exactly like
    // repeatedly picking one backend at a time — i.e. two backends with
    // equal weight strictly alternate as the primary pick.
    let layer = CanaryLayer::new(vec![
        WeightedBackend::new("a:80", 1),
        WeightedBackend::new("b:80", 1),
    ]);
    let mut primaries = Vec::new();
    for _ in 0..6 {
        primaries.push(layer.next_candidates()[0].0.clone());
    }
    for pair in primaries.chunks(2) {
        assert_eq!(2, pair.len());
        assert_ne!(pair[0], pair[1], "equal-weight backends should alternate, got {:?}", primaries);
    }
}

// ── live weight updates ────────────────────────────────────────────────────────

#[test]
fn update_replaces_weights_without_recreating_the_layer() {
    let layer = CanaryLayer::new(vec![
        WeightedBackend::new("a:80", 1),
        WeightedBackend::new("b:80", 1),
    ]);

    layer.update(
        vec![
            WeightedBackend::new("a:80", 0),
            WeightedBackend::new("b:80", 1),
        ],
        vec![],
    );

    for _ in 0..5 {
        let candidates = layer.next_candidates();
        assert_eq!(vec!["b"], hosts_of(&candidates));
    }
}

#[test]
fn update_can_replace_backends_with_pools_and_vice_versa() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("a:80", 1)]);
    let pool = BackendPool::r#static(vec!["10.0.0.1:8080".to_string()]);
    layer.update(vec![], vec![WeightedPool::new(pool, 1)]);

    let candidates = layer.next_candidates();
    assert_eq!(vec![("10.0.0.1".to_string(), 8080, false)], candidates);
}

#[test]
fn clones_share_state() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("a:80", 1)]);
    let handle = layer.clone();

    handle.update(vec![WeightedBackend::new("only-b:80", 1)], vec![]);

    let candidates = layer.next_candidates();
    assert_eq!(vec!["only-b"], hosts_of(&candidates));
}

// ── BackendPool integration ────────────────────────────────────────────────────

#[test]
fn add_pool_mixes_static_backend_with_dynamic_pool() {
    let pool = BackendPool::r#static(vec!["10.0.0.5:9090".to_string()]);
    let layer = CanaryLayer::new(vec![WeightedBackend::new("stable:8080", 1)]).add_pool(pool, 1);

    let mut saw_stable = false;
    let mut saw_pool_member = false;
    for _ in 0..10 {
        let candidates = layer.next_candidates();
        if hosts_of(&candidates).contains(&"stable") {
            saw_stable = true;
        }
        if hosts_of(&candidates).contains(&"10.0.0.5") {
            saw_pool_member = true;
        }
    }
    assert!(saw_stable, "static backend should still be selected sometimes");
    assert!(saw_pool_member, "pool member should be selected sometimes");
}

#[test]
fn with_pools_builds_a_layer_purely_from_dynamic_groups() {
    let pool = BackendPool::r#static(vec!["10.0.0.9:80".to_string()]);
    let layer = CanaryLayer::with_pools(vec![WeightedPool::new(pool, 1)]);
    let candidates = layer.next_candidates();
    assert_eq!(vec![("10.0.0.9".to_string(), 80, false)], candidates);
}

#[test]
fn pool_group_round_robins_its_own_members_across_selections() {
    let pool = BackendPool::r#static(vec!["10.0.0.1:80".to_string(), "10.0.0.2:80".to_string()]);
    let layer = CanaryLayer::with_pools(vec![WeightedPool::new(pool, 1)]);

    let first = layer.next_candidates()[0].0.clone();
    let second = layer.next_candidates()[0].0.clone();
    assert_ne!(first, second, "the pool's own round-robin cursor should alternate members");

    let third = layer.next_candidates()[0].0.clone();
    assert_eq!(first, third, "cursor should cycle back after both members are seen");
}

#[test]
fn empty_pool_is_skipped_falling_through_to_the_next_group() {
    let empty_pool = BackendPool::r#static(vec![]);
    let layer = CanaryLayer::new(vec![WeightedBackend::new("stable:8080", 1)]).add_pool(empty_pool, 1);

    // The pool contributes nothing while empty — every candidate list must
    // fall back to the static backend regardless of which entry SWRR picked
    // as primary.
    for _ in 0..6 {
        let candidates = layer.next_candidates();
        assert_eq!(vec![("stable".to_string(), 8080, false)], candidates);
    }
}

#[test]
fn zero_weight_pool_is_never_a_candidate() {
    let pool = BackendPool::r#static(vec!["10.0.0.1:80".to_string()]);
    let layer = CanaryLayer::new(vec![WeightedBackend::new("stable:8080", 1)]).add_pool(pool, 0);

    for _ in 0..5 {
        let candidates = layer.next_candidates();
        assert!(!hosts_of(&candidates).contains(&"10.0.0.1"));
    }
}

// ── path_prefix / timeouts still configurable (builder API unchanged) ────────

#[test]
fn builder_methods_are_chainable() {
    let layer = CanaryLayer::new(vec![WeightedBackend::new("a:80", 1)])
        .path_prefix("/api")
        .connect_timeout_ms(1234)
        .read_timeout_ms(5678);
    assert_eq!(1, layer.next_candidates().len());
}
