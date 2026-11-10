use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

#[test]
fn readiness_passes_and_fails_with_its_checks() {
    let _g = crate::test_env::lock();
    crate::metrics::SERVER_READY.store(true, Ordering::SeqCst);
    let r = Registry::new();
    let up = Arc::new(AtomicBool::new(true));
    let u = Arc::clone(&up);
    r.register(Probe::Readiness, "db", move || if u.load(Ordering::SeqCst) { Ok(()) } else { Err("connection refused".into()) });
    r.register(Probe::Readiness, "cache", || Ok(()));
    assert!(r.report(Probe::Readiness).ok);
    assert_eq!("OK", r.report(Probe::Readiness).body(false));
    up.store(false, Ordering::SeqCst);
    let failed = r.report(Probe::Readiness);
    assert!(!failed.ok);
    assert_eq!("[-]db failed: connection refused", failed.body(false));
    assert_eq!("[-]db failed: connection refused\n[+]cache ok\nreadyz check failed", failed.body(true));
    // Not ready while the server isn't.
    up.store(true, Ordering::SeqCst);
    crate::metrics::SERVER_READY.store(false, Ordering::SeqCst);
    let starting = r.report(Probe::Readiness);
    assert!(!starting.ok && starting.body(false).contains("server not ready"));
    crate::metrics::SERVER_READY.store(true, Ordering::SeqCst);
    assert!(r.unregister(Probe::Readiness, "db") && !r.unregister(Probe::Readiness, "db"));
    assert!(!r.has_checks(Probe::Liveness) && r.has_checks(Probe::Readiness));
}

#[test]
fn slow_and_panicking_checks_fail() {
    let r = Registry::new();
    r.set_timeout(Duration::from_millis(50));
    r.register(Probe::Liveness, "slow", || {
        std::thread::sleep(Duration::from_millis(500));
        Ok(())
    });
    r.register(Probe::Liveness, "boom", || panic!("bad"));
    let report = r.report(Probe::Liveness);
    assert!(!report.ok);
    let body = report.body(false);
    assert!(body.contains("slow failed: timed out after 50 ms") && body.contains("boom failed: the check panicked"), "{body}");
    // Replaced by name.
    r.register(Probe::Liveness, "slow", || Ok(()));
    r.register(Probe::Liveness, "boom", || Ok(()));
    assert!(r.report(Probe::Liveness).ok);
}

#[test]
fn startup_stays_passed_once_passed() {
    let r = Registry::new();
    let ready = Arc::new(AtomicBool::new(false));
    let x = Arc::clone(&ready);
    r.register(Probe::Startup, "index", move || if x.load(Ordering::SeqCst) { Ok(()) } else { Err("loading".into()) });
    assert!(!r.report(Probe::Startup).ok);
    ready.store(true, Ordering::SeqCst);
    assert!(r.report(Probe::Startup).ok);
    ready.store(false, Ordering::SeqCst);
    assert!(r.report(Probe::Startup).ok, "startup doesn't fail again");
}

#[test]
fn probe_paths() {
    assert_eq!(Some(Probe::Liveness), probe_for_path("/healthz"));
    assert_eq!(Some(Probe::Liveness), probe_for_path("/livez"));
    assert_eq!(Some(Probe::Readiness), probe_for_path("/readyz"));
    assert_eq!(Some(Probe::Startup), probe_for_path("/startupz"));
    assert_eq!(None, probe_for_path("/ready"));
    assert_eq!("/startupz", Probe::Startup.path());
}

#[test]
fn endpoints_answer_through_the_app() {
    let _g = crate::test_env::lock();
    crate::metrics::SERVER_READY.store(true, std::sync::atomic::Ordering::SeqCst);
    use crate::core::New;
    let client = crate::test_client::TestClient::new(crate::app::App::new());
    // Without checks: the old answers.
    assert_eq!((200, "OK"), (client.get("/healthz").send().status(), client.get("/healthz").send().body_text()));
    assert_eq!(200, client.get("/livez").send().status());
    assert_eq!("livez check passed", client.get("/livez?verbose").send().body_text());
    // A startup check, through the process-wide registry (only this test uses /startupz).
    startup("app-test-warmup", || Err("warming up".into()));
    let starting = client.get("/startupz").send();
    assert_eq!((503, "[-]app-test-warmup failed: warming up"), (starting.status(), starting.body_text()));
    assert_eq!(Some("no-store"), starting.header("cache-control"));
    let request = crate::request::Request { method: "GET".into(), request_uri: "/startupz".into(), http_version: "HTTP/1.1".into(), headers: vec![], body: Vec::new().into() };
    assert!(!crate::app::App::is_inline_request(&request), "a probe with checks runs off the event loop");
    startup("app-test-warmup", || Ok(()));
    assert_eq!(200, client.get("/startupz").send().status());
    assert!(unregister(Probe::Startup, "app-test-warmup"));
}
