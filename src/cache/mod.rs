//! In-memory response cache middleware.
//!
//! [`CacheLayer`] implements [`Middleware`] and short-circuits the inner
//! application for cacheable `GET` responses within their TTL.  Entries are
//! bounded by a configurable capacity; the oldest entry is evicted when the
//! store is full and no expired entries remain.
//!
//! # What is cached
//!
//! - Method: **GET only**; all other methods bypass the cache.
//! - Status: 2xx responses (200, 201, 203, 204, 206, …).
//! - Response `Cache-Control: no-store` or `private` — **not** cached.
//! - Request `Cache-Control: no-cache` — cache is bypassed, handler is called,
//!   but the fresh response **is** stored (revalidation).
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::app::App;
//! use rust_web_server::cache::CacheLayer;
//! use rust_web_server::core::New;
//!
//! let app = App::new()
//!     .wrap(CacheLayer::memory(1000).ttl(60).vary_by_header("Accept"));
//! ```

#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::application::Application;
use crate::header::Header;
use crate::middleware::Middleware;
use crate::request::{METHOD, Request};
use crate::response::Response;
use crate::server::ConnectionInfo;

// ── cache store ───────────────────────────────────────────────────────────────

struct CachedEntry {
    response: Response,
    inserted_at: Instant,
}

struct CacheStore {
    entries: HashMap<String, CachedEntry>,
    /// Insertion order — front is oldest; used for capacity eviction.
    order: VecDeque<String>,
}

impl CacheStore {
    fn new() -> Self {
        CacheStore { entries: HashMap::new(), order: VecDeque::new() }
    }

    fn get(&self, key: &str, ttl: Duration) -> Option<&CachedEntry> {
        self.entries.get(key).filter(|e| e.inserted_at.elapsed() < ttl)
    }

    fn insert(&mut self, key: String, entry: CachedEntry, capacity: usize) {
        // Update in place without disturbing insertion order.
        if self.entries.contains_key(&key) {
            self.entries.insert(key, entry);
            return;
        }
        // `purge_expired` is called by the caller before `insert`, so any
        // remaining entries are still live. Evict the oldest if we're full.
        if self.entries.len() >= capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, entry);
    }

    fn purge_expired(&mut self, ttl: Duration) {
        let expired: Vec<String> = self.entries.iter()
            .filter(|(_, e)| e.inserted_at.elapsed() >= ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.order.retain(|o| o != k);
        }
    }
}

// ── CacheLayer ────────────────────────────────────────────────────────────────

/// An in-memory response cache middleware.
///
/// Construct with [`CacheLayer::memory`] and configure with the builder methods
/// [`ttl`](CacheLayer::ttl) and [`vary_by_header`](CacheLayer::vary_by_header).
///
/// `CacheLayer` is cheaply `Clone`-able — clones share the same underlying
/// store, which lets you keep a handle for cache stats and invalidation while
/// the other clone is used as middleware.
#[derive(Clone)]
pub struct CacheLayer {
    store: Arc<Mutex<CacheStore>>,
    /// `store`'s entry count, kept after every change under its lock: an
    /// empty cache (nothing cacheable seen yet — e.g. static files, sent
    /// `no-cache`) costs a request no key and no lock. The lock was shared
    /// by every worker and taken on every GET.
    stored: Arc<AtomicUsize>,
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
    capacity: usize,
    ttl: Duration,
    vary_headers: Vec<String>,
}

impl CacheLayer {
    /// Create a new in-memory cache bounded to `capacity` entries.
    ///
    /// Default TTL is **60 seconds**. Adjust with [`.ttl()`](CacheLayer::ttl).
    pub fn memory(capacity: usize) -> Self {
        CacheLayer {
            store: Arc::new(Mutex::new(CacheStore::new())),
            stored: Arc::new(AtomicUsize::new(0)),
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            capacity,
            ttl: Duration::from_secs(60),
            vary_headers: vec![],
        }
    }

    /// Number of cache hits since the cache was created.
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Number of cache misses since the cache was created.
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Current number of entries in the store (including potentially expired ones
    /// that haven't been purged yet).
    pub fn size(&self) -> usize {
        self.store.lock().unwrap().entries.len()
    }

    /// Remove all entries from the cache.
    pub fn clear(&self) {
        let mut guard = self.store.lock().unwrap();
        guard.entries.clear();
        guard.order.clear();
        self.stored.store(0, Ordering::Relaxed);
    }

    /// Set the time-to-live for cached entries.
    pub fn ttl(mut self, secs: u64) -> Self {
        self.ttl = Duration::from_secs(secs);
        self
    }

    /// Include a request header in the cache key so that different values of
    /// that header produce separate cache entries.
    ///
    /// Header name matching is case-insensitive. Call multiple times to vary
    /// by more than one header.
    ///
    /// ```rust,no_run
    /// use rust_web_server::cache::CacheLayer;
    ///
    /// let layer = CacheLayer::memory(500)
    ///     .vary_by_header("Accept")
    ///     .vary_by_header("Accept-Language");
    /// ```
    pub fn vary_by_header(mut self, name: &str) -> Self {
        self.vary_headers.push(name.to_ascii_lowercase());
        self
    }

    fn store(&self) -> &Mutex<CacheStore> {
        &self.store
    }

    /// Build a cache key from the request URI and any configured vary headers.
    fn cache_key(&self, request: &Request) -> String {
        let mut key = request.request_uri.clone();
        for vh in &self.vary_headers {
            let val = request.headers.iter()
                .find(|h| h.name.eq_ignore_ascii_case(vh))
                .map(|h| h.value.as_ref())
                .unwrap_or("");
            key.push('\x00');
            key.push_str(val);
        }
        key
    }

    /// `true` when the request carries `Cache-Control: no-cache`, meaning the
    /// client wants a fresh response (but we may still store the result).
    fn request_bypasses_cache(request: &Request) -> bool {
        request.headers.iter().any(|h| {
            h.name.eq_ignore_ascii_case(Header::_CACHE_CONTROL)
                && h.value.to_ascii_lowercase().contains("no-cache")
        })
    }

    /// `true` when the response may be stored and replayed as is:
    /// - a 2xx to a request without `Range` (a 206 is one slice, not the
    ///   resource);
    /// - no `Cache-Control: no-store`, `private` or `no-cache` (`no-cache`
    ///   allows storing only to revalidate before every reuse, which this
    ///   cache can't do — static files are sent with it by default);
    /// - every request header its `Vary` names is part of the cache key, or
    ///   didn't change this response: `Origin` without an
    ///   `Access-Control-Allow-Origin` in it, `Accept-Encoding` with an
    ///   unencoded body (valid for every client). Otherwise e.g. a gzipped
    ///   body would be replayed to a client that didn't ask for gzip.
    ///   `Vary: *` is never cacheable.
    fn response_is_cacheable(&self, request: &Request, response: &Response) -> bool {
        if response.status_code < 200 || response.status_code >= 300 {
            return false;
        }
        if request.header(Header::_RANGE).is_some() {
            return false;
        }
        let has = |name: &str| response.headers.iter().any(|h| h.name.eq_ignore_ascii_case(name));
        let varies_harmlessly = |name: &str| {
            (name.eq_ignore_ascii_case(Header::_ORIGIN) && !has(Header::_ACCESS_CONTROL_ALLOW_ORIGIN))
                || (name.eq_ignore_ascii_case(Header::_ACCEPT_ENCODING) && !has(Header::_CONTENT_ENCODING))
        };
        response.headers.iter().all(|h| {
            if h.name.eq_ignore_ascii_case(Header::_CACHE_CONTROL) {
                let v = h.value.to_ascii_lowercase();
                !(v.contains("no-store") || v.contains("private") || v.contains("no-cache"))
            } else if h.name.eq_ignore_ascii_case(Header::_VARY) {
                h.value.split(',').map(str::trim).filter(|name| !name.is_empty()).all(|name| {
                    name != "*" && (self.vary_headers.iter().any(|vh| vh.eq_ignore_ascii_case(name)) || varies_harmlessly(name))
                })
            } else {
                true
            }
        })
    }

    /// Age of the entry in whole seconds, capped at u64::MAX.
    fn age_secs(entry: &CachedEntry) -> u64 {
        entry.inserted_at.elapsed().as_secs()
    }

    /// Build a response from a cache hit, injecting an `Age` header.
    fn cached_response(entry: &CachedEntry) -> Response {
        let mut response = entry.response.clone();
        let age = Self::age_secs(entry);
        // Replace or add the Age header.
        if let Some(h) = response.headers.iter_mut().find(|h| h.name.eq_ignore_ascii_case("Age")) {
            h.value = age.to_string().into();
        } else {
            response.headers.push(Header { name: "Age".into(), value: age.to_string().into() });
        }
        response
    }
}

impl Middleware for CacheLayer {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        // Only cache GET requests.
        if request.method != METHOD.get {
            return next.execute(request, connection);
        }

        let bypass = Self::request_bypasses_cache(request);

        if !bypass && self.stored.load(Ordering::Relaxed) > 0 {
            // Check for a valid cache hit.
            let key = self.cache_key(request);
            let guard = self.store().lock().unwrap();
            if let Some(entry) = guard.get(&key, self.ttl) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(Self::cached_response(entry));
            }
        }
        self.misses.fetch_add(1, Ordering::Relaxed);

        // Cache miss (or bypass): call the handler.
        let response = next.execute(request, connection)?;

        if self.response_is_cacheable(request, &response) {
            let key = self.cache_key(request);
            let mut guard = self.store().lock().unwrap();
            guard.purge_expired(self.ttl);
            guard.insert(
                key,
                CachedEntry { response: response.clone(), inserted_at: Instant::now() },
                self.capacity,
            );
            self.stored.store(guard.entries.len(), Ordering::Relaxed);
        }

        Ok(response)
    }

    /// In-memory only: safe on the event loop.
    fn runs_inline(&self, _request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }
}
