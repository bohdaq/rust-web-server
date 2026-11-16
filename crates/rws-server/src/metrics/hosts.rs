//! Requests per host (the `Host` header; `:authority` on HTTP/2 and HTTP/3),
//! for the admin API's `GET /breakdown` — "where requests go" (18.38.0).
//!
//! Counted by the server core next to [`super::http`], in one of [`SHARDS`]
//! maps picked per thread. A shard keeps at most [`MAX_HOSTS`] names, so
//! made-up `Host` headers can't grow it: a new name takes the place of the
//! least busy one, whose requests are then counted with "the rest". A host
//! that keeps getting requests so keeps its name, however many made-up ones
//! pass by.
//! Not exported to Prometheus: the names come from clients.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::header::Header;
use crate::request::Request;

/// Names a shard keeps; the requests of a name that lost its place are counted together.
#[doc(hidden)]
pub const MAX_HOSTS: usize = 200;

/// A host name longer than this (DNS allows 253, plus a port) is counted with the rest.
const MAX_NAME: usize = 260;

const SHARDS: usize = 16;

/// What one host got.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[doc(hidden)]
pub struct HostCount {
    #[doc(hidden)]
    pub requests: u64,
    /// Responses with status 500 or above.
    #[doc(hidden)]
    pub errors: u64,
}

impl HostCount {
    fn add(&mut self, status: i16) {
        self.requests += 1;
        if status >= 500 {
            self.errors += 1;
        }
    }

    fn merge(&mut self, other: &HostCount) {
        self.requests += other.requests;
        self.errors += other.errors;
    }
}

#[derive(Default)]
struct Shard {
    hosts: HashMap<String, HostCount>,
    other: HostCount,
}

fn shards() -> &'static [Mutex<Shard>] {
    static STORE: OnceLock<Vec<Mutex<Shard>>> = OnceLock::new();
    STORE.get_or_init(|| (0..SHARDS).map(|_| Mutex::new(Shard::default())).collect())
}

thread_local! {
    static SHARD: usize = {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        NEXT.fetch_add(1, Ordering::Relaxed) % SHARDS
    };
}

/// Count a finished request under the host it asked for.
#[doc(hidden)]
pub fn observe_request(request: &Request, status: i16) {
    if super::thread_uncounted() {
        return;
    }
    record(request.header(Header::_HOST).map(|h| &*h.value), status);
}

/// Count one request for `host` (`None`: the request named no host).
#[doc(hidden)]
pub fn record(host: Option<&str>, status: i16) {
    let shard = SHARD.with(|s| *s);
    shards()[shard].lock().unwrap_or_else(|e| e.into_inner()).record(host, status);
}

impl Shard {
    fn record(&mut self, host: Option<&str>, status: i16) {
        let host = host.unwrap_or("").trim();
        if host.len() > MAX_NAME {
            self.other.add(status);
            return;
        }
        // Names are case-insensitive; almost every client sends lower case.
        let lowered;
        let host = if host.bytes().any(|b| b.is_ascii_uppercase()) {
            lowered = host.to_ascii_lowercase();
            lowered.as_str()
        } else {
            host
        };
        if let Some(count) = self.hosts.get_mut(host) {
            count.add(status);
        } else {
            if self.hosts.len() >= MAX_HOSTS {
                self.drop_least_busy();
            }
            self.hosts.entry(host.to_string()).or_default().add(status);
        }
    }

    /// Make room: the least busy name (of equally busy ones, the last by
    /// name) goes, and what it got is counted with the rest.
    fn drop_least_busy(&mut self) {
        let least = self
            .hosts
            .iter()
            .min_by(|a, b| a.1.requests.cmp(&b.1.requests).then_with(|| b.0.cmp(a.0)))
            .map(|(name, _)| name.clone());
        if let Some(count) = least.and_then(|name| self.hosts.remove(&name)) {
            self.other.merge(&count);
        }
    }
}

/// Every host with its counts, busiest first (then by name), and the
/// requests whose host isn't kept by name. A request without a host is
/// listed under the empty name.
#[doc(hidden)]
pub fn snapshot() -> (Vec<(String, HostCount)>, HostCount) {
    let mut merged = Shard::default();
    for shard in shards() {
        merged.merge(&shard.lock().unwrap_or_else(|e| e.into_inner()));
    }
    merged.listed()
}

impl Shard {
    fn merge(&mut self, shard: &Shard) {
        for (host, count) in shard.hosts.iter() {
            self.hosts.entry(host.clone()).or_default().merge(count);
        }
        self.other.merge(&shard.other);
    }

    fn listed(self) -> (Vec<(String, HostCount)>, HostCount) {
        let mut other = self.other;
        let mut hosts: Vec<(String, HostCount)> = self.hosts.into_iter().collect();
        hosts.sort_by(|a, b| b.1.requests.cmp(&a.1.requests).then_with(|| a.0.cmp(&b.0)));
        // Each shard keeps MAX_HOSTS names, so together they can hold more.
        for (_, count) in hosts.iter().skip(MAX_HOSTS) {
            other.merge(count);
        }
        hosts.truncate(MAX_HOSTS);
        (hosts, other)
    }
}

#[cfg(test)]
mod tests;
