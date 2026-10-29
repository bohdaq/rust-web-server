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
