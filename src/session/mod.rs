//! Server-side session management.
//!
//! [`SessionStore`] is a thread-safe, TTL-aware in-memory store. Store it
//! inside your application state (`AppWithState<S>`) so every handler shares
//! the same session map automatically.
//!
//! [`Session`] holds the key/value data for one session. Retrieve it with
//! [`SessionStore::load`], mutate it, then persist changes with
//! [`SessionStore::save`].
//!
//! Helper functions [`session_id_from_request`], [`session_cookie`], and
//! [`destroy_cookie`] translate between the HTTP cookie layer and the store.
//!
//! # Security note
//!
//! Session IDs are generated from a non-cryptographic hash of the system
//! clock and an atomic counter. Sufficient for most internal applications.
//! For public-facing services requiring unpredictable IDs, supply your own
//! CSPRNG via [`SessionStore::create_with_id`].
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::session::{self, SessionStore};
//! use rust_web_server::header::Header;
//! use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
//!
//! struct State { sessions: SessionStore }
//!
//! let app = App::with_state(State { sessions: SessionStore::new(3600) })
//!     .post("/login", |req, _params, _conn, state| {
//!         // verify credentials …
//!         let mut sess = state.sessions.create();
//!         sess.set("user_id", "42");
//!         state.sessions.save(&sess);
//!
//!         let mut r = Response::new();
//!         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
//!         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
//!         r.headers.push(Header {
//!             name: "Set-Cookie".to_string(),
//!             value: session::session_cookie(&sess.id, "sid", 3600),
//!         });
//!         r
//!     })
//!     .get("/profile", |req, _params, _conn, state| {
//!         let mut r = Response::new();
//!         let sid = match session::session_id_from_request(&req, "sid") {
//!             Some(id) => id,
//!             None => {
//!                 r.status_code = *STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code;
//!                 r.reason_phrase = STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase.to_string();
//!                 return r;
//!             }
//!         };
//!         let sess = match state.sessions.load(&sid) {
//!             Some(s) => s,
//!             None => {
//!                 r.status_code = *STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code;
//!                 r.reason_phrase = STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase.to_string();
//!                 return r;
//!             }
//!         };
//!         let user_id = sess.get("user_id").unwrap_or("guest");
//!         r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
//!         r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
//!         r
//!     });
//! ```

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::cookie::{CookieJar, SetCookie};
use crate::request::Request;

// ── ID generation ─────────────────────────────────────────────────────────────

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn generate_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let count = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    // splitmix64 finalizer applied to two independent seeds
    let mut x = nanos ^ count.wrapping_mul(0x9e3779b97f4a7c15);
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^= x >> 31;
    let mut y = count ^ nanos.wrapping_mul(0x517cc1b727220a95);
    y ^= y >> 30;
    y = y.wrapping_mul(0xbf58476d1ce4e5b9);
    y ^= y >> 27;
    y = y.wrapping_mul(0x94d049bb133111eb);
    y ^= y >> 31;
    format!("{:016x}{:016x}", x, y)
}

// ── Session ───────────────────────────────────────────────────────────────────

/// Data for a single session, keyed by [`Session::id`].
///
/// After mutating the session, call [`SessionStore::save`] to persist the
/// changes back to the store.
pub struct Session {
    /// Opaque session identifier — store this in the client cookie.
    pub id: String,
    pub(crate) data: HashMap<String, String>,
}

impl Session {
    /// Return the value for `key`, or `None` if absent.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(String::as_str)
    }

    /// Insert or update `key` with `value`.
    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        self.data.insert(key.to_string(), value.into());
    }

    /// Remove `key`. No-op if absent.
    pub fn remove(&mut self, key: &str) {
        self.data.remove(key);
    }

    /// Return `true` if `key` is present.
    pub fn contains(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }
}

// ── SessionStore ──────────────────────────────────────────────────────────────

struct Entry {
    data: HashMap<String, String>,
    expires_at: Instant,
}

struct Inner {
    sessions: HashMap<String, Entry>,
}

/// Thread-safe in-memory session store with TTL-based expiry.
///
/// Cloning is cheap — all clones share the same backing map via `Arc`.
/// Place one instance in your application state and share it across handlers.
pub struct SessionStore {
    inner: Arc<Mutex<Inner>>,
    ttl: Duration,
}

impl Clone for SessionStore {
    fn clone(&self) -> Self {
        SessionStore { inner: Arc::clone(&self.inner), ttl: self.ttl }
    }
}

impl SessionStore {
    /// Create a new store where sessions expire `ttl_secs` seconds after
    /// creation.
    pub fn new(ttl_secs: u64) -> Self {
        SessionStore {
            inner: Arc::new(Mutex::new(Inner { sessions: HashMap::new() })),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    /// Create a new empty session with a generated ID, insert it into the
    /// store, and return it. Mutate the session then call [`save`][Self::save].
    pub fn create(&self) -> Session {
        self.create_with_id(generate_id())
    }

    /// Create a new empty session using `id` (caller supplies the value,
    /// e.g. from a CSPRNG). Inserts the session and returns it.
    pub fn create_with_id(&self, id: String) -> Session {
        let entry = Entry {
            data: HashMap::new(),
            expires_at: Instant::now() + self.ttl,
        };
        self.inner.lock().unwrap().sessions.insert(id.clone(), entry);
        Session { id, data: HashMap::new() }
    }

    /// Load a session by ID. Returns `None` if unknown or expired.
    pub fn load(&self, id: &str) -> Option<Session> {
        let inner = self.inner.lock().unwrap();
        let entry = inner.sessions.get(id)?;
        if Instant::now() > entry.expires_at {
            return None;
        }
        Some(Session { id: id.to_string(), data: entry.data.clone() })
    }

    /// Persist a session's data back to the store. No-op if the session ID
    /// is no longer present (e.g. already destroyed or expired and purged).
    pub fn save(&self, session: &Session) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(entry) = inner.sessions.get_mut(&session.id) {
            entry.data = session.data.clone();
        }
    }

    /// Delete a session immediately. Also clear the client cookie using
    /// [`destroy_cookie`].
    pub fn destroy(&self, id: &str) {
        self.inner.lock().unwrap().sessions.remove(id);
    }

    /// Remove all sessions whose TTL has elapsed. Call periodically to
    /// reclaim memory (e.g. once per minute from a background thread).
    pub fn purge_expired(&self) {
        let now = Instant::now();
        self.inner.lock().unwrap().sessions.retain(|_, e| e.expires_at > now);
    }

    /// Number of sessions in the store, including expired but not yet purged.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().sessions.len()
    }

    /// `true` if the store contains no sessions.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ── Cookie helpers ────────────────────────────────────────────────────────────

/// Extract the session ID from the named cookie in a request's `Cookie`
/// header. Returns `None` if the header is absent or the cookie is missing.
pub fn session_id_from_request(request: &Request, cookie_name: &str) -> Option<String> {
    let header = request.get_header("Cookie".to_string())?;
    let jar = CookieJar::parse(&header.value);
    jar.get(cookie_name).map(|c| c.value.clone())
}

/// Build a `Set-Cookie` header value that stores `session_id` in
/// `cookie_name` with `HttpOnly`, `SameSite=Lax`, `Path=/`, and `Max-Age`.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::{session, header::Header};
///
/// let value = session::session_cookie("abc123", "sid", 3600);
/// // response.headers.push(Header { name: "Set-Cookie".to_string(), value });
/// ```
pub fn session_cookie(session_id: &str, cookie_name: &str, ttl_secs: u64) -> String {
    SetCookie::new(cookie_name, session_id)
        .path("/")
        .http_only()
        .same_site("Lax")
        .max_age(ttl_secs as i64)
        .build()
}

/// Build a `Set-Cookie` header value that clears `cookie_name` in the
/// browser (`Max-Age=0`). Use after calling [`SessionStore::destroy`].
pub fn destroy_cookie(cookie_name: &str) -> String {
    SetCookie::new(cookie_name, "").path("/").max_age(0).build()
}
