//! Unit tests for `BackendPool` and `DiscoverySource`.

use super::BackendPool;

const SCRATCHPAD: &str = "/private/tmp/claude-501/-Users-bohdantsap-git-rust-web-server/fca82dfb-17f6-4b5e-8147-6129313f132a/scratchpad";

// ── Static source ─────────────────────────────────────────────────────────────

#[test]
fn static_source_returns_backends() {
    let pool = BackendPool::r#static(vec!["a:8080".into()]);
    assert_eq!(vec!["a:8080".to_string()], pool.backends());
}

#[test]
fn static_start_is_noop() {
    // Calling start() on a static pool should not panic and the backends
    // should remain unchanged.
    let pool = BackendPool::r#static(vec!["x:1234".into()]);
    pool.start();
    assert_eq!(vec!["x:1234".to_string()], pool.backends());
}

// ── EnvPrefix source ──────────────────────────────────────────────────────────

#[test]
fn env_prefix_reads_env_vars() {
    std::env::set_var("TEST_DISC_BACKEND_0", "a:8080");
    std::env::set_var("TEST_DISC_BACKEND_1", "b:8080");
    // Make sure _2 is absent so the scan stops.
    std::env::remove_var("TEST_DISC_BACKEND_2");

    let pool = BackendPool::env_prefix("TEST_DISC_BACKEND");
    pool.refresh();
    let backends = pool.backends();
    assert!(backends.contains(&"a:8080".to_string()), "should contain a:8080, got {:?}", backends);
    assert!(backends.contains(&"b:8080".to_string()), "should contain b:8080, got {:?}", backends);
    assert_eq!(2, backends.len());
}

#[test]
fn env_prefix_empty_when_no_vars() {
    std::env::remove_var("NO_SUCH_PREFIX_0");
    let pool = BackendPool::env_prefix("NO_SUCH_PREFIX");
    pool.refresh();
    assert!(pool.backends().is_empty());
}

// ── File source ───────────────────────────────────────────────────────────────

#[test]
fn file_source_reads_file() {
    let path = format!("{}/test_backends.txt", SCRATCHPAD);
    std::fs::create_dir_all(SCRATCHPAD).unwrap();
    std::fs::write(&path, "10.0.0.1:8080\n10.0.0.2:8080\n").unwrap();

    let pool = BackendPool::file(&path);
    pool.refresh();
    let backends = pool.backends();
    assert_eq!(2, backends.len(), "got {:?}", backends);
    assert!(backends.contains(&"10.0.0.1:8080".to_string()));
    assert!(backends.contains(&"10.0.0.2:8080".to_string()));
}

#[test]
fn file_source_ignores_comments_and_blank_lines() {
    let path = format!("{}/test_backends_comments.txt", SCRATCHPAD);
    std::fs::create_dir_all(SCRATCHPAD).unwrap();
    std::fs::write(
        &path,
        "# primary backends\n10.0.0.1:8080\n\n# secondary\n10.0.0.2:9090\n",
    )
    .unwrap();

    let pool = BackendPool::file(&path);
    pool.refresh();
    let backends = pool.backends();
    assert_eq!(2, backends.len(), "got {:?}", backends);
    for b in &backends {
        assert!(!b.starts_with('#'), "comment line should be filtered: {}", b);
        assert!(!b.is_empty(), "blank line should be filtered");
    }
}

#[test]
fn file_source_returns_empty_on_missing_file() {
    let pool = BackendPool::file("/this/path/does/not/exist.txt");
    pool.refresh();
    assert!(pool.backends().is_empty());
}

// ── Manual update ─────────────────────────────────────────────────────────────

#[test]
fn update_replaces_backends() {
    let pool = BackendPool::r#static(vec!["old:1234".into()]);
    assert_eq!(vec!["old:1234".to_string()], pool.backends());
    pool.update(vec!["new:5678".into(), "new2:5679".into()]);
    let backends = pool.backends();
    assert_eq!(2, backends.len());
    assert!(backends.contains(&"new:5678".to_string()));
    assert!(backends.contains(&"new2:5679".to_string()));
}

#[test]
fn update_to_empty_clears_list() {
    let pool = BackendPool::r#static(vec!["a:80".into()]);
    pool.update(vec![]);
    assert!(pool.backends().is_empty());
}

// ── Clone sharing ─────────────────────────────────────────────────────────────

#[test]
fn clones_share_backend_list() {
    let pool = BackendPool::r#static(vec!["a:80".into()]);
    let clone = pool.clone();
    pool.update(vec!["b:81".into()]);
    // The clone should see the updated list because they share the Arc<RwLock<>>.
    assert_eq!(vec!["b:81".to_string()], clone.backends());
}
