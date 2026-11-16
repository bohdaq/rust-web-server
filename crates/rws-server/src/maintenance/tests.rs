use std::sync::atomic::Ordering;

use super::*;
use crate::app::App;
use crate::core::New;
use crate::test_client::TestClient;

// Serialize all tests that touch the global MAINTENANCE_MODE static (the
// admin API's tests switch it too).
fn lock() -> std::sync::MutexGuard<'static, ()> {
    crate::test_env::lock()
}

#[test]
fn normal_mode_passes_through() {
    let _g = lock();
    MAINTENANCE_MODE.store(false, Ordering::SeqCst);
    let app = App::new().wrap(MaintenanceLayer);
    let client = TestClient::new(app);
    let res = client.get("/healthz").send();
    MAINTENANCE_MODE.store(false, Ordering::SeqCst);
    assert_eq!(200, res.status());
}

#[test]
fn maintenance_mode_returns_503() {
    let _g = lock();
    MAINTENANCE_MODE.store(true, Ordering::SeqCst);
    let app = App::new().wrap(MaintenanceLayer);
    let client = TestClient::new(app);
    let res = client.get("/api/users").send();
    MAINTENANCE_MODE.store(false, Ordering::SeqCst);
    assert_eq!(503, res.status());
}

#[test]
fn healthz_passes_in_maintenance_mode() {
    let _g = lock();
    MAINTENANCE_MODE.store(true, Ordering::SeqCst);
    let app = App::new().wrap(MaintenanceLayer);
    let client = TestClient::new(app);
    let res = client.get("/healthz").send();
    MAINTENANCE_MODE.store(false, Ordering::SeqCst);
    assert_eq!(200, res.status());
}

#[test]
fn readyz_passes_in_maintenance_mode() {
    let _g = lock();
    MAINTENANCE_MODE.store(true, Ordering::SeqCst);
    let app = App::new().wrap(MaintenanceLayer);
    let client = TestClient::new(app);
    let res = client.get("/readyz").send();
    MAINTENANCE_MODE.store(false, Ordering::SeqCst);
    // readyz returns its own 503 "not ready" — maintenance layer let it through.
    assert_ne!(b"Service Temporarily Unavailable".as_ref(), res.body_bytes());
}

#[test]
fn passes_through_only_while_off() {
    let _g = lock();
    let request = crate::request::Request::parse(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let connection = crate::server::ConnectionInfo::default();
    MAINTENANCE_MODE.store(false, Ordering::SeqCst);
    assert!(MaintenanceLayer.is_passthrough(&request, &connection));
    MAINTENANCE_MODE.store(true, Ordering::SeqCst);
    assert!(!MaintenanceLayer.is_passthrough(&request, &connection));
    MAINTENANCE_MODE.store(false, Ordering::SeqCst);
}

#[test]
fn a_timer_turns_it_off_and_caps_retry_after() {
    let _g = lock();
    turn_on(Some(3600), Some(300));
    let on = status();
    assert!(on.active);
    assert_eq!(on.until, on.since.map(|s| s + 3600));
    assert_eq!(on.retry_after_secs, 300);
    let res = TestClient::new(App::new().wrap(MaintenanceLayer)).get("/api/users").send();
    assert_eq!(503, res.status());
    assert_eq!(res.header("Retry-After"), Some("300"));

    // Ten seconds left: clients aren't told to wait longer than that.
    turn_on(Some(10), Some(300));
    assert!((1..=10).contains(&status().retry_after_secs), "{:?}", status());

    // An hour more, counted from the time it would have ended.
    let before = status().until.unwrap();
    assert!(extend(3600));
    assert_eq!(status().until, Some(before + 3600));

    // The timer ran out: off for whoever asks, timer and choice forgotten.
    UNTIL.store(1, Ordering::SeqCst);
    assert!(!active());
    assert!(!MAINTENANCE_MODE.load(Ordering::SeqCst));
    assert_eq!(status(), Status { active: false, since: None, until: None, retry_after_secs: crate::error::page::maintenance_retry_after() });
    let res = TestClient::new(App::new().wrap(MaintenanceLayer)).get("/healthz").send();
    assert_eq!(200, res.status());
}

#[test]
fn on_without_a_timer_stays_on_and_zero_sends_no_retry_after() {
    let _g = lock();
    turn_on(None, Some(0));
    assert_eq!(status().until, None);
    let res = TestClient::new(App::new().wrap(MaintenanceLayer)).get("/api/users").send();
    assert_eq!(503, res.status());
    assert_eq!(res.header("Retry-After"), None);
    assert!(!extend(60), "nothing to extend without a timer");
    turn_off();
    assert!(!status().active);
    assert!(!extend(60));
}
