use std::thread;
use std::time::Duration;

use crate::header::Header;
use crate::http::VERSION;
use crate::request::{METHOD, Request};
use crate::session::{self, SessionStore};

fn empty_get() -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn get_with_cookie(cookie_header: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![Header {
            name: "Cookie".to_string(),
            value: cookie_header.to_string(),
        }],
        body: vec![],
    }
}

// ── Session get / set / remove / contains ────────────────────────────────────

#[test]
fn session_get_returns_none_for_missing_key() {
    let store = SessionStore::new(3600);
    let session = store.create();
    assert!(session.get("missing").is_none());
}

#[test]
fn session_set_and_get() {
    let store = SessionStore::new(3600);
    let mut session = store.create();
    session.set("user_id", "42");
    assert_eq!(Some("42"), session.get("user_id"));
}

#[test]
fn session_contains() {
    let store = SessionStore::new(3600);
    let mut session = store.create();
    assert!(!session.contains("k"));
    session.set("k", "v");
    assert!(session.contains("k"));
}

#[test]
fn session_remove() {
    let store = SessionStore::new(3600);
    let mut session = store.create();
    session.set("x", "1");
    session.remove("x");
    assert!(session.get("x").is_none());
}

// ── SessionStore create / load / save ────────────────────────────────────────

#[test]
fn create_generates_non_empty_id() {
    let store = SessionStore::new(3600);
    let session = store.create();
    assert!(!session.id.is_empty());
}

#[test]
fn create_with_id_uses_provided_id() {
    let store = SessionStore::new(3600);
    let session = store.create_with_id("my-custom-id".to_string());
    assert_eq!("my-custom-id", session.id);
}

#[test]
fn load_returns_created_session() {
    let store = SessionStore::new(3600);
    let session = store.create();
    let id = session.id.clone();
    let loaded = store.load(&id);
    assert!(loaded.is_some());
    assert_eq!(id, loaded.unwrap().id);
}

#[test]
fn load_unknown_id_returns_none() {
    let store = SessionStore::new(3600);
    assert!(store.load("no-such-id").is_none());
}

#[test]
fn save_persists_changes() {
    let store = SessionStore::new(3600);
    let mut session = store.create();
    let id = session.id.clone();
    session.set("role", "admin");
    store.save(&session);

    let loaded = store.load(&id).unwrap();
    assert_eq!(Some("admin"), loaded.get("role"));
}

#[test]
fn unsaved_changes_are_not_visible() {
    let store = SessionStore::new(3600);
    let mut session = store.create();
    let id = session.id.clone();
    session.set("role", "admin");
    // no save()
    let loaded = store.load(&id).unwrap();
    assert!(loaded.get("role").is_none());
}

// ── destroy ───────────────────────────────────────────────────────────────────

#[test]
fn destroy_removes_session() {
    let store = SessionStore::new(3600);
    let session = store.create();
    let id = session.id.clone();
    store.destroy(&id);
    assert!(store.load(&id).is_none());
}

// ── expiry and purge ──────────────────────────────────────────────────────────

#[test]
fn expired_session_not_loadable() {
    let store = SessionStore::new(0); // 0-second TTL — expires immediately
    let session = store.create();
    let id = session.id.clone();
    thread::sleep(Duration::from_millis(5));
    assert!(store.load(&id).is_none());
}

#[test]
fn purge_expired_removes_expired_entries() {
    let store = SessionStore::new(0);
    store.create();
    store.create();
    assert_eq!(2, store.len());
    thread::sleep(Duration::from_millis(5));
    store.purge_expired();
    assert_eq!(0, store.len());
}

#[test]
fn purge_expired_keeps_live_sessions() {
    let store = SessionStore::new(3600);
    store.create();
    store.create();
    store.purge_expired();
    assert_eq!(2, store.len());
}

#[test]
fn is_empty_reflects_store_state() {
    let store = SessionStore::new(3600);
    assert!(store.is_empty());
    let s = store.create();
    assert!(!store.is_empty());
    store.destroy(&s.id);
    assert!(store.is_empty());
}

// ── clone shares backing store ────────────────────────────────────────────────

#[test]
fn cloned_store_shares_data() {
    let store = SessionStore::new(3600);
    let clone = store.clone();
    let mut session = store.create();
    session.set("k", "v");
    store.save(&session);

    let loaded = clone.load(&session.id).unwrap();
    assert_eq!(Some("v"), loaded.get("k"));
}

// ── cookie helpers ────────────────────────────────────────────────────────────

#[test]
fn session_id_from_request_reads_named_cookie() {
    let req = get_with_cookie("sid=abc123; other=xyz");
    let id = session::session_id_from_request(&req, "sid");
    assert_eq!(Some("abc123".to_string()), id);
}

#[test]
fn session_id_from_request_returns_none_when_missing() {
    let req = get_with_cookie("other=xyz");
    assert!(session::session_id_from_request(&req, "sid").is_none());
}

#[test]
fn session_id_from_request_returns_none_without_cookie_header() {
    let req = empty_get();
    assert!(session::session_id_from_request(&req, "sid").is_none());
}

#[test]
fn session_cookie_contains_id_and_name() {
    let value = session::session_cookie("tok123", "sid", 3600);
    assert!(value.contains("sid=tok123"), "got: {}", value);
    assert!(value.contains("Max-Age=3600"), "got: {}", value);
    assert!(value.contains("HttpOnly"), "got: {}", value);
    assert!(value.contains("SameSite=Lax"), "got: {}", value);
}

#[test]
fn destroy_cookie_sets_max_age_zero() {
    let value = session::destroy_cookie("sid");
    assert!(value.contains("sid="), "got: {}", value);
    assert!(value.contains("Max-Age=0"), "got: {}", value);
}
