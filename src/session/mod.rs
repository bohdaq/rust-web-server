//! Server-side session management.
//!
//! Three session store implementations are provided:
//!
//! * [`SessionStore`] — in-process `HashMap`; fast, zero-config, but sessions
//!   are lost on restart. Good for single-instance deployments.
//! * [`DbSessionStore`] — backed by the model-layer [`DbPool`]; sessions
//!   survive restarts and are shared across multiple processes that use the
//!   same database. Requires a `model-sqlite`, `model-postgres`, or
//!   `model-mysql` feature.
//! * [`RedisSessionStore`] — backed by a Redis server via a hand-rolled RESP
//!   client; scales horizontally with no shared database needed. Requires a
//!   running Redis server.
//!
//! All three expose the same public API: `create`, `create_with_id`, `load`,
//! `save`, `destroy`, `purge_expired`, `len`, `is_empty`.
//!
//! [`Session`] holds the key/value data for one session. Retrieve it with
//! the store's `load`, mutate it, then persist changes with `save`.
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

// ── DbSessionStore ────────────────────────────────────────────────────────────

/// Session store backed by a relational database via the model-layer
/// [`DbPool`][crate::model::DbPool].
///
/// Sessions survive process restarts and are visible to every process that
/// connects to the same database, making this suitable for multi-instance
/// deployments and zero-downtime restarts.
///
/// The first call to [`DbSessionStore::new`] creates the `rws_sessions` table
/// if it does not already exist:
///
/// ```sql
/// CREATE TABLE IF NOT EXISTS rws_sessions (
///     id         TEXT    PRIMARY KEY,
///     data       TEXT    NOT NULL DEFAULT '',
///     expires_at INTEGER NOT NULL
/// )
/// ```
///
/// Session data is serialized as a URL-encoded string
/// (`key1=val1&key2=val2`). Expired sessions are **not** removed
/// automatically — call [`purge_expired`][DbSessionStore::purge_expired]
/// periodically (e.g. from a background thread).
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
/// # {
/// use rust_web_server::model::DbPool;
/// use rust_web_server::session::DbSessionStore;
///
/// let pool = DbPool::memory().unwrap();
/// let store = DbSessionStore::new(pool, 3600).unwrap();
///
/// let mut sess = store.create().unwrap();
/// sess.set("user_id", "42");
/// store.save(&sess).unwrap();
///
/// let loaded = store.load(&sess.id).unwrap().unwrap();
/// assert_eq!(Some("42"), loaded.get("user_id"));
/// # }
/// ```
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub struct DbSessionStore {
    pool: Arc<crate::model::DbPool>,
    ttl: Duration,
}

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
impl Clone for DbSessionStore {
    fn clone(&self) -> Self {
        DbSessionStore { pool: Arc::clone(&self.pool), ttl: self.ttl }
    }
}

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
impl DbSessionStore {
    /// Open (or reuse) a `DbSessionStore` backed by `pool`.
    ///
    /// Creates the `rws_sessions` table on the first call if it is absent.
    /// Returns `Err` if the DDL fails.
    pub fn new(pool: crate::model::DbPool, ttl_secs: u64) -> Result<Self, crate::model::DbError> {
        let store = DbSessionStore {
            pool: Arc::new(pool),
            ttl: Duration::from_secs(ttl_secs),
        };
        store.ensure_table()?;
        Ok(store)
    }

    fn ensure_table(&self) -> Result<(), crate::model::DbError> {
        let mut conn = self.pool.get()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS rws_sessions \
             (id TEXT PRIMARY KEY, data TEXT NOT NULL DEFAULT '', expires_at INTEGER NOT NULL)",
            &[],
        )?;
        Ok(())
    }

    fn now_epoch() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    fn serialize(data: &HashMap<String, String>) -> String {
        crate::url::URL::build_query(data.clone())
    }

    fn deserialize(s: &str) -> HashMap<String, String> {
        crate::url::URL::parse_query(s)
    }

    /// Create a new empty session and persist it immediately.
    pub fn create(&self) -> Result<Session, crate::model::DbError> {
        self.create_with_id(generate_id())
    }

    /// Create a new empty session with a caller-supplied ID and persist it.
    pub fn create_with_id(&self, id: String) -> Result<Session, crate::model::DbError> {
        let expires_at = Self::now_epoch() + self.ttl.as_secs() as i64;
        let mut conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO rws_sessions (id, data, expires_at) VALUES (?1, ?2, ?3)",
            &[
                crate::model::Value::Text(id.clone()),
                crate::model::Value::Text(String::new()),
                crate::model::Value::Int(expires_at),
            ],
        )?;
        Ok(Session { id, data: HashMap::new() })
    }

    /// Load a session by ID. Returns `None` if unknown or expired.
    pub fn load(&self, id: &str) -> Result<Option<Session>, crate::model::DbError> {
        let now = Self::now_epoch();
        let mut conn = self.pool.get()?;
        let rows = conn.query_rows(
            "SELECT data FROM rws_sessions WHERE id = ?1 AND expires_at > ?2",
            &[
                crate::model::Value::Text(id.to_string()),
                crate::model::Value::Int(now),
            ],
        )?;
        if rows.is_empty() {
            return Ok(None);
        }
        let raw: String = rows[0].get("data")?;
        Ok(Some(Session { id: id.to_string(), data: Self::deserialize(&raw) }))
    }

    /// Persist a session's data back to the store.
    pub fn save(&self, session: &Session) -> Result<(), crate::model::DbError> {
        let mut conn = self.pool.get()?;
        conn.execute(
            "UPDATE rws_sessions SET data = ?1 WHERE id = ?2",
            &[
                crate::model::Value::Text(Self::serialize(&session.data)),
                crate::model::Value::Text(session.id.clone()),
            ],
        )?;
        Ok(())
    }

    /// Delete a session immediately.
    pub fn destroy(&self, id: &str) -> Result<(), crate::model::DbError> {
        let mut conn = self.pool.get()?;
        conn.execute(
            "DELETE FROM rws_sessions WHERE id = ?1",
            &[crate::model::Value::Text(id.to_string())],
        )?;
        Ok(())
    }

    /// Delete all sessions whose TTL has elapsed.
    pub fn purge_expired(&self) -> Result<(), crate::model::DbError> {
        let now = Self::now_epoch();
        let mut conn = self.pool.get()?;
        conn.execute(
            "DELETE FROM rws_sessions WHERE expires_at <= ?1",
            &[crate::model::Value::Int(now)],
        )?;
        Ok(())
    }

    /// Total number of sessions in the store, including expired ones not yet purged.
    pub fn len(&self) -> Result<usize, crate::model::DbError> {
        let mut conn = self.pool.get()?;
        let rows = conn.query_rows("SELECT COUNT(*) AS n FROM rws_sessions", &[])?;
        if rows.is_empty() {
            return Ok(0);
        }
        let n: i64 = rows[0].get("n")?;
        Ok(n as usize)
    }

    /// `true` if the store contains no sessions.
    pub fn is_empty(&self) -> Result<bool, crate::model::DbError> {
        Ok(self.len()? == 0)
    }
}

// ── RedisSessionStore ─────────────────────────────────────────────────────────

/// A minimal RESP v2 client for issuing Redis commands.
///
/// Reconnects automatically when the connection is dropped.
pub struct RespConn {
    addr: String,
    password: Option<String>,
    stream: Mutex<Option<std::net::TcpStream>>,
}

impl RespConn {
    fn new(addr: impl Into<String>, password: Option<String>) -> Self {
        RespConn { addr: addr.into(), password, stream: Mutex::new(None) }
    }

    fn connect(&self) -> std::io::Result<std::net::TcpStream> {
        let stream = std::net::TcpStream::connect(&self.addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(stream)
    }

    /// Send a Redis command (array of byte slices) and return the raw reply.
    fn cmd(&self, args: &[&[u8]]) -> std::io::Result<RespReply> {
        use std::io::{Read, Write};
        let mut guard = self.stream.lock().unwrap();
        // Lazy connect / reconnect
        if guard.is_none() {
            let mut s = self.connect()?;
            if let Some(ref pw) = self.password {
                let auth_frame = resp_array(&[b"AUTH", pw.as_bytes()]);
                s.write_all(&auth_frame)?;
                read_reply(&mut s)?; // consume +OK
            }
            *guard = Some(s);
        }
        let frame = resp_array(args);
        let stream = guard.as_mut().unwrap();
        if stream.write_all(&frame).is_err() {
            // Connection broke — drop and retry once
            *guard = None;
            drop(guard);
            return self.cmd(args);
        }
        read_reply(stream)
    }
}

/// A decoded RESP reply.
enum RespReply {
    Ok,
    Int(i64),
    Bulk(Option<Vec<u8>>),
    Error(String),
}

fn resp_array(args: &[&[u8]]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", args.len()).into_bytes();
    for arg in args {
        out.extend_from_slice(format!("${}\r\n", arg.len()).as_bytes());
        out.extend_from_slice(arg);
        out.extend_from_slice(b"\r\n");
    }
    out
}

fn read_reply(stream: &mut std::net::TcpStream) -> std::io::Result<RespReply> {
    use std::io::{BufRead, BufReader, Read};
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let line = line.trim_end_matches("\r\n");
    match line.chars().next() {
        Some('+') => Ok(RespReply::Ok),
        Some(':') => {
            let n = line[1..].parse::<i64>().unwrap_or(0);
            Ok(RespReply::Int(n))
        }
        Some('-') => Ok(RespReply::Error(line[1..].to_string())),
        Some('$') => {
            let len = line[1..].parse::<i64>().unwrap_or(-1);
            if len < 0 {
                return Ok(RespReply::Bulk(None));
            }
            let mut buf = vec![0u8; len as usize + 2]; // +2 for \r\n
            reader.read_exact(&mut buf)?;
            buf.truncate(len as usize);
            Ok(RespReply::Bulk(Some(buf)))
        }
        _ => Ok(RespReply::Ok), // ignore arrays etc.
    }
}

/// Session store backed by a Redis server.
///
/// Sessions are stored as Redis strings keyed by `rws:sess:{id}` and given
/// a Redis TTL via `SET … EX`. Expired sessions are removed automatically by
/// Redis — no `purge_expired` sweep is needed.
///
/// Cloning is cheap — all clones share the same underlying TCP connection
/// (one persistent connection per `RedisSessionStore` instance).
///
/// # Connection
///
/// Specify the server address as `host:port` (e.g. `"127.0.0.1:6379"`).
/// Pass `Some("password")` for Redis servers that require AUTH.
/// Use [`RedisSessionStore::from_env`] to read connection details from
/// `RWS_REDIS_HOST`, `RWS_REDIS_PORT`, and `RWS_REDIS_PASSWORD`.
///
/// # Example
///
/// ```rust,no_run
/// use rust_web_server::session::RedisSessionStore;
///
/// let store = RedisSessionStore::new("127.0.0.1:6379", None, 3600);
///
/// let mut sess = store.create().expect("create session");
/// sess.set("user_id", "42");
/// store.save(&sess).expect("save session");
///
/// let loaded = store.load(&sess.id).expect("load session").unwrap();
/// assert_eq!(Some("42"), loaded.get("user_id"));
/// ```
pub struct RedisSessionStore {
    conn: Arc<RespConn>,
    ttl: u64,
}

impl Clone for RedisSessionStore {
    fn clone(&self) -> Self {
        RedisSessionStore { conn: Arc::clone(&self.conn), ttl: self.ttl }
    }
}

impl RedisSessionStore {
    /// Create a store that connects to `addr` (e.g. `"127.0.0.1:6379"`).
    /// `password` is passed to Redis `AUTH` if `Some`.
    pub fn new(addr: impl Into<String>, password: Option<String>, ttl_secs: u64) -> Self {
        RedisSessionStore {
            conn: Arc::new(RespConn::new(addr, password)),
            ttl: ttl_secs,
        }
    }

    /// Build a store from environment variables:
    /// - `RWS_REDIS_HOST` (default `127.0.0.1`)
    /// - `RWS_REDIS_PORT` (default `6379`)
    /// - `RWS_REDIS_PASSWORD` (optional)
    /// - `RWS_REDIS_TTL_SECS` (default `3600`)
    pub fn from_env() -> Self {
        let host = std::env::var("RWS_REDIS_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port = std::env::var("RWS_REDIS_PORT").unwrap_or_else(|_| "6379".into());
        let addr = format!("{}:{}", host, port);
        let password = std::env::var("RWS_REDIS_PASSWORD").ok();
        let ttl = std::env::var("RWS_REDIS_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600u64);
        Self::new(addr, password, ttl)
    }

    fn key(id: &str) -> Vec<u8> {
        format!("rws:sess:{}", id).into_bytes()
    }

    fn serialize(data: &HashMap<String, String>) -> Vec<u8> {
        crate::url::URL::build_query(data.clone()).into_bytes()
    }

    fn deserialize(bytes: Vec<u8>) -> HashMap<String, String> {
        let s = String::from_utf8(bytes).unwrap_or_default();
        crate::url::URL::parse_query(&s)
    }

    /// Create a new empty session and persist it to Redis.
    pub fn create(&self) -> std::io::Result<Session> {
        self.create_with_id(generate_id())
    }

    /// Create a new empty session with a caller-supplied ID and persist it.
    pub fn create_with_id(&self, id: String) -> std::io::Result<Session> {
        let ttl_str = self.ttl.to_string();
        self.conn.cmd(&[
            b"SET",
            &Self::key(&id),
            b"",
            b"EX",
            ttl_str.as_bytes(),
        ])?;
        Ok(Session { id, data: HashMap::new() })
    }

    /// Load a session by ID. Returns `None` if unknown or expired.
    pub fn load(&self, id: &str) -> std::io::Result<Option<Session>> {
        match self.conn.cmd(&[b"GET", &Self::key(id)])? {
            RespReply::Bulk(Some(bytes)) => {
                Ok(Some(Session { id: id.to_string(), data: Self::deserialize(bytes) }))
            }
            _ => Ok(None),
        }
    }

    /// Persist a session's data back to Redis, resetting the TTL.
    pub fn save(&self, session: &Session) -> std::io::Result<()> {
        let ttl_str = self.ttl.to_string();
        let data = Self::serialize(&session.data);
        self.conn.cmd(&[
            b"SET",
            &Self::key(&session.id),
            &data,
            b"EX",
            ttl_str.as_bytes(),
        ])?;
        Ok(())
    }

    /// Delete a session immediately.
    pub fn destroy(&self, id: &str) -> std::io::Result<()> {
        self.conn.cmd(&[b"DEL", &Self::key(id)])?;
        Ok(())
    }

    /// No-op — Redis expiry removes sessions automatically.
    pub fn purge_expired(&self) {}

    /// Total number of keys in the Redis database.
    ///
    /// Uses `DBSIZE`, which counts *all* keys, not just session keys.
    /// Useful as a rough indicator; not exact for mixed-use Redis instances.
    pub fn len(&self) -> std::io::Result<usize> {
        match self.conn.cmd(&[b"DBSIZE"])? {
            RespReply::Int(n) => Ok(n as usize),
            _ => Ok(0),
        }
    }

    /// `true` if `DBSIZE` returns 0.
    pub fn is_empty(&self) -> std::io::Result<bool> {
        Ok(self.len()? == 0)
    }
}
