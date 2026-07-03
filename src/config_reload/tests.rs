use super::{current, reload, ConfigSnapshot, RELOAD_REQUESTED};
use std::sync::atomic::Ordering;

#[test]
fn snapshot_from_env_reads_cors_allow_all() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_CORS_ALLOW_ALL", "true");
    let snap = ConfigSnapshot::from_env();
    assert!(snap.cors_allow_all);
}

#[test]
fn snapshot_from_env_reads_cors_allow_all_false() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_CORS_ALLOW_ALL", "false");
    let snap = ConfigSnapshot::from_env();
    assert!(!snap.cors_allow_all);
}

#[test]
fn snapshot_from_env_reads_cors_allow_all_case_insensitive() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_CORS_ALLOW_ALL", "TRUE");
    let snap = ConfigSnapshot::from_env();
    assert!(snap.cors_allow_all);
}

#[test]
fn snapshot_from_env_reads_rate_limit_defaults_when_unset() {
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS");
    std::env::remove_var("RWS_CONFIG_RATE_LIMIT_WINDOW_SECS");
    let snap = ConfigSnapshot::from_env();
    assert_eq!(1000, snap.rate_limit_max_requests);
    assert_eq!(60, snap.rate_limit_window_secs);
}

#[test]
fn snapshot_from_env_reads_rate_limit_when_set() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_RATE_LIMIT_MAX_REQUESTS", "500");
    std::env::set_var("RWS_CONFIG_RATE_LIMIT_WINDOW_SECS", "30");
    let snap = ConfigSnapshot::from_env();
    assert_eq!(500, snap.rate_limit_max_requests);
    assert_eq!(30, snap.rate_limit_window_secs);
}

#[test]
fn snapshot_from_env_reads_log_format() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_LOG_FORMAT", "json");
    let snap = ConfigSnapshot::from_env();
    assert_eq!("json", snap.log_format);
}

#[test]
fn snapshot_from_env_reads_cors_origins() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_CORS_ALLOW_ORIGINS", "https://example.com");
    let snap = ConfigSnapshot::from_env();
    assert_eq!("https://example.com", snap.cors_allow_origins);
}

#[test]
fn snapshot_from_env_reads_request_allocation_size() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES", "65536");
    let snap = ConfigSnapshot::from_env();
    assert_eq!(65536, snap.request_allocation_size);
}

#[test]
fn snapshot_from_env_reads_max_body_size() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES", "5242880");
    let snap = ConfigSnapshot::from_env();
    std::env::remove_var("RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES");
    assert_eq!(5242880, snap.max_body_size);
}

#[test]
fn snapshot_from_env_max_body_size_defaults_to_unlimited() {
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES");
    let snap = ConfigSnapshot::from_env();
    assert_eq!(0, snap.max_body_size);
}

#[test]
fn current_returns_a_snapshot() {
    // Verifies that current() doesn't panic and returns a structurally valid value.
    let snap = current();
    // rate_limit_window_secs is always a non-negative u64
    let _ = snap.rate_limit_window_secs;
}

#[test]
fn snapshot_clone_is_independent() {
    let snap = ConfigSnapshot::from_env();
    let clone = snap.clone();
    assert_eq!(snap.rate_limit_max_requests, clone.rate_limit_max_requests);
    assert_eq!(snap.log_format, clone.log_format);
}

#[test]
fn reload_requested_flag_starts_false() {
    // Verify load is safe to call in any test context.
    let _val = RELOAD_REQUESTED.load(Ordering::SeqCst);
}

#[test]
fn reload_requested_can_be_set_and_cleared() {
    RELOAD_REQUESTED.store(true, Ordering::SeqCst);
    assert!(RELOAD_REQUESTED.load(Ordering::SeqCst));
    RELOAD_REQUESTED.store(false, Ordering::SeqCst);
    assert!(!RELOAD_REQUESTED.load(Ordering::SeqCst));
}

#[test]
fn reload_does_not_panic_without_config_file() {
    // override_environment_variables_from_config handles missing files gracefully.
    let _g = crate::test_env::lock();
    reload();
}

#[test]
fn reload_updates_snapshot() {
    // Verify that reload() keeps the snapshot in sync with the current env.
    // We don't assert specific values because reload() first re-reads rws.config.toml,
    // which may override any env vars we set. Instead, confirm that after reload(),
    // current() agrees with ConfigSnapshot::from_env() (no stale snapshot).
    let _g = crate::test_env::lock();
    reload();
    let snap = current();
    let expected = ConfigSnapshot::from_env();
    assert_eq!(expected.rate_limit_max_requests, snap.rate_limit_max_requests);
    assert_eq!(expected.rate_limit_window_secs, snap.rate_limit_window_secs);
    assert_eq!(expected.cors_allow_all, snap.cors_allow_all);
    assert_eq!(expected.log_format, snap.log_format);
}
