//! [`ValueCache`]: an in-memory cache of computed values with a
//! `fetch_or` — Rails' `Rails.cache.fetch`, Spring's `@Cacheable` as a
//! closure (18.35.0, #95).

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Entry<V> {
    value: V,
    expires: Instant,
    seq: u64,
}

struct Inner<V> {
    entries: HashMap<String, Entry<V>>,
    /// Insertion order, for evicting the oldest when full.
    order: BTreeMap<u64, String>,
    next_seq: u64,
    hits: u64,
    misses: u64,
}

/// Values by key, each with its own time to live, at most `capacity` of
/// them (the oldest goes first when full, after anything expired). Cheap
/// to clone; clones share the entries.
///
/// ```rust
/// # extern crate rws_server as rust_web_server;
/// use std::time::Duration;
/// use rust_web_server::cache::ValueCache;
///
/// let cache: ValueCache<String> = ValueCache::new(1000);
/// let a = cache.fetch_or("greeting:en", Duration::from_secs(60), || "Hello".to_string());
/// let b = cache.fetch_or("greeting:en", Duration::from_secs(60), || unreachable!("cached"));
/// assert_eq!(a, b);
/// ```
///
/// The value is computed outside the lock: two threads missing the same key
/// at once both compute it (the last one stored wins). Errors from
/// [`try_fetch_or`](Self::try_fetch_or) aren't cached.
pub struct ValueCache<V> {
    inner: Arc<Mutex<Inner<V>>>,
    capacity: usize,
}

impl<V> Clone for ValueCache<V> {
    fn clone(&self) -> Self {
        ValueCache { inner: Arc::clone(&self.inner), capacity: self.capacity }
    }
}

impl<V: Clone> ValueCache<V> {
    /// At most `capacity` entries (at least 1).
    pub fn new(capacity: usize) -> Self {
        ValueCache {
            inner: Arc::new(Mutex::new(Inner { entries: HashMap::new(), order: BTreeMap::new(), next_seq: 0, hits: 0, misses: 0 })),
            capacity: capacity.max(1),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner<V>> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The value under `key`, unless it's missing or expired.
    pub fn get(&self, key: &str) -> Option<V> {
        let mut inner = self.lock();
        let now = Instant::now();
        let found = match inner.entries.get(key) {
            Some(entry) if entry.expires > now => Some(entry.value.clone()),
            Some(_) => {
                if let Some(entry) = inner.entries.remove(key) {
                    inner.order.remove(&entry.seq);
                }
                None
            }
            None => None,
        };
        match found {
            Some(value) => {
                inner.hits += 1;
                Some(value)
            }
            None => {
                inner.misses += 1;
                None
            }
        }
    }

    /// Store `value` under `key` for `ttl`.
    pub fn put(&self, key: &str, value: V, ttl: Duration) {
        let mut inner = self.lock();
        let now = Instant::now();
        if let Some(old) = inner.entries.remove(key) {
            inner.order.remove(&old.seq);
        }
        if inner.entries.len() >= self.capacity {
            let expired: Vec<String> = inner.entries.iter().filter(|(_, e)| e.expires <= now).map(|(k, _)| k.clone()).collect();
            for k in expired {
                if let Some(e) = inner.entries.remove(&k) {
                    inner.order.remove(&e.seq);
                }
            }
        }
        while inner.entries.len() >= self.capacity {
            let Some((&seq, _)) = inner.order.iter().next() else { break };
            if let Some(oldest) = inner.order.remove(&seq) {
                inner.entries.remove(&oldest);
            }
        }
        let seq = inner.next_seq;
        inner.next_seq += 1;
        inner.order.insert(seq, key.to_string());
        inner.entries.insert(key.to_string(), Entry { value, expires: now + ttl, seq });
    }

    /// The cached value, or `compute()` stored for `ttl` and returned.
    pub fn fetch_or(&self, key: &str, ttl: Duration, compute: impl FnOnce() -> V) -> V {
        if let Some(value) = self.get(key) {
            return value;
        }
        let value = compute();
        self.put(key, value.clone(), ttl);
        value
    }

    /// [`fetch_or`](Self::fetch_or) for a computation that can fail: an
    /// `Err` is returned and nothing is stored.
    pub fn try_fetch_or<E>(&self, key: &str, ttl: Duration, compute: impl FnOnce() -> Result<V, E>) -> Result<V, E> {
        if let Some(value) = self.get(key) {
            return Ok(value);
        }
        let value = compute()?;
        self.put(key, value.clone(), ttl);
        Ok(value)
    }

    /// [`fetch_or`](Self::fetch_or) with an async computation (a query).
    pub async fn fetch_or_async<F, Fut>(&self, key: &str, ttl: Duration, compute: F) -> V
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V>,
    {
        if let Some(value) = self.get(key) {
            return value;
        }
        let value = compute().await;
        self.put(key, value.clone(), ttl);
        value
    }

    /// [`try_fetch_or`](Self::try_fetch_or) with an async computation.
    pub async fn try_fetch_or_async<E, F, Fut>(&self, key: &str, ttl: Duration, compute: F) -> Result<V, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>>,
    {
        if let Some(value) = self.get(key) {
            return Ok(value);
        }
        let value = compute().await?;
        self.put(key, value.clone(), ttl);
        Ok(value)
    }

    /// Remove `key`; whether it was there.
    pub fn invalidate(&self, key: &str) -> bool {
        let mut inner = self.lock();
        match inner.entries.remove(key) {
            Some(entry) => {
                inner.order.remove(&entry.seq);
                true
            }
            None => false,
        }
    }

    /// Remove every key starting with `prefix` (`"user:42:"`); how many.
    pub fn invalidate_prefix(&self, prefix: &str) -> usize {
        let mut inner = self.lock();
        let keys: Vec<String> = inner.entries.keys().filter(|k| k.starts_with(prefix)).cloned().collect();
        for k in &keys {
            if let Some(entry) = inner.entries.remove(k) {
                inner.order.remove(&entry.seq);
            }
        }
        keys.len()
    }

    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.entries.clear();
        inner.order.clear();
    }

    /// Entries held (expired ones until they're noticed).
    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `(hits, misses)` of [`get`](Self::get) (and so of the `fetch_or`s).
    pub fn stats(&self) -> (u64, u64) {
        let inner = self.lock();
        (inner.hits, inner.misses)
    }
}

#[cfg(test)]
mod tests;
