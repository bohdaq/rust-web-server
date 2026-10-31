//! What the admin API and the `rws` binary's MCP tools both do, as JSON.
//! One implementation, so the two can't drift apart.
//!
//! State that belongs to an application rather than the process (its
//! response cache, its route table) is registered here by whoever builds
//! the application ([`register_cache`], [`register_routes`]).

use std::sync::atomic::Ordering;
use std::sync::{OnceLock, RwLock};

use crate::cache::CacheLayer;
use crate::mcp::json_escape;

fn quoted(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

/// Every `RWS_CONFIG_*` setting in effect, sorted; secrets (names with
/// `SECRET`, `PASSWORD` or `TOKEN`) shown as `(hidden)`.
pub fn config_json() -> String {
    let mut pairs: Vec<(String, String)> = std::env::vars().filter(|(k, _)| k.starts_with("RWS_CONFIG_")).collect();
    pairs.sort();
    let members: Vec<String> =
        pairs.iter().map(|(k, v)| format!("{}:{}", quoted(k), quoted(&crate::entry_point::shown(k, v)))).collect();
    format!("{{{}}}", members.join(","))
}

/// Compile-time features as `{"name":bool}`.
pub fn build_features_json() -> String {
    macro_rules! features {
        ($($name:literal),*) => { vec![$( ($name, cfg!(feature = $name)) ),*] };
    }
    let all: Vec<(&str, bool)> = features!(
        "http1", "http2", "http3", "acme", "auth", "serde", "macros", "sso", "sso-saml", "sso-server", "gateway-api",
        "tower", "openapi", "mimalloc", "brotli", "zstd", "wasm-plugins"
    );
    let members: Vec<String> = all.iter().map(|(name, on)| format!("\"{name}\":{on}")).collect();
    format!("{{{}}}", members.join(","))
}

/// The server's counters.
pub fn metrics_json() -> String {
    use crate::metrics;
    format!(
        r#"{{"requests_total":{},"errors_total":{},"active_connections":{},"thread_pool_queued":{},"ready":{}}}"#,
        metrics::REQUESTS_TOTAL.load(Ordering::Relaxed),
        metrics::ERRORS_TOTAL.load(Ordering::Relaxed),
        metrics::ACTIVE_CONNECTIONS.load(Ordering::Relaxed),
        metrics::THREAD_POOL_QUEUED.load(Ordering::Relaxed),
        metrics::SERVER_READY.load(Ordering::Relaxed),
    )
}

pub fn rate_limit_json() -> String {
    let cfg = crate::config_reload::current();
    format!(
        r#"{{"configured":{},"max_requests":{},"window_secs":{}}}"#,
        crate::rate_limit::is_configured(),
        cfg.rate_limit_max_requests,
        cfg.rate_limit_window_secs
    )
}

/// `ip`'s remaining quota under the global limit; counts as one request.
pub fn check_rate_limit_json(ip: &str) -> String {
    let limiter = crate::rate_limit::global();
    let remaining = limiter.remaining(ip);
    let allowed = limiter.check(ip);
    format!(r#"{{"ip":{},"remaining":{remaining},"allowed":{allowed}}}"#, quoted(ip))
}

pub fn cors_json() -> String {
    let cfg = crate::config_reload::current();
    format!(
        r#"{{"allow_all":{},"origins":{},"methods":{},"headers":{},"credentials":{},"expose_headers":{},"max_age":{}}}"#,
        cfg.cors_allow_all,
        quoted(&cfg.cors_allow_origins),
        quoted(&cfg.cors_allow_methods),
        quoted(&cfg.cors_allow_headers),
        quoted(&cfg.cors_allow_credentials.to_string()),
        quoted(&cfg.cors_expose_headers),
        quoted(&cfg.cors_max_age.to_string()),
    )
}

pub fn maintenance_json() -> String {
    format!(r#"{{"active":{}}}"#, crate::maintenance::MAINTENANCE_MODE.load(Ordering::Relaxed))
}

pub fn set_maintenance(active: bool) -> String {
    crate::maintenance::MAINTENANCE_MODE.store(active, Ordering::SeqCst);
    maintenance_json()
}

pub fn blocklist_json() -> String {
    let list: Vec<String> = crate::blocklist::global().list().iter().map(|ip| quoted(ip)).collect();
    format!("[{}]", list.join(","))
}

pub fn block(ip: &str) -> String {
    crate::blocklist::global().block(ip);
    format!(r#"{{"blocked":{}}}"#, quoted(ip))
}

pub fn unblock(ip: &str) -> String {
    crate::blocklist::global().unblock(ip);
    format!(r#"{{"unblocked":{}}}"#, quoted(ip))
}

/// Runtime feature flags ([`crate::feature`]).
pub fn flags_json() -> String {
    let flags: Vec<String> =
        crate::feature::global().list().iter().map(|(name, on)| format!(r#"{{"name":{},"enabled":{on}}}"#, quoted(name))).collect();
    format!("[{}]", flags.join(","))
}

pub fn set_flag(name: &str, enabled: bool) -> String {
    crate::feature::global().set(name, enabled);
    format!(r#"{{"name":{},"enabled":{enabled}}}"#, quoted(name))
}

/// The last `n` requests (at most 100), or the last `n` with a 4xx/5xx
/// status.
pub fn recent_json(n: usize, errors_only: bool) -> String {
    let n = n.min(100);
    let log = crate::request_log::global();
    let entries = if errors_only { log.recent_errors(n) } else { log.recent(n) };
    let entries: Vec<String> = entries
        .iter()
        .map(|e| {
            format!(
                r#"{{"ts":{},"method":{},"path":{},"status":{},"ip":{},"latency_ms":{}}}"#,
                e.timestamp,
                quoted(&e.method),
                quoted(&e.path),
                e.status,
                quoted(&e.client_ip),
                e.latency_ms
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

fn cache_slot() -> &'static RwLock<Option<CacheLayer>> {
    static SLOT: OnceLock<RwLock<Option<CacheLayer>>> = OnceLock::new();
    SLOT.get_or_init(RwLock::default)
}

fn routes_slot() -> &'static RwLock<Vec<(String, String)>> {
    static SLOT: OnceLock<RwLock<Vec<(String, String)>>> = OnceLock::new();
    SLOT.get_or_init(RwLock::default)
}

/// The response cache the admin API reports on and clears (a clone shares
/// the store).
pub fn register_cache(cache: CacheLayer) {
    crate::cache::registry::register("app", cache.clone());
    *cache_slot().write().unwrap_or_else(|e| e.into_inner()) = Some(cache);
}

/// The application's routes as `(method, pattern)`, for listing.
pub fn register_routes(routes: Vec<(String, String)>) {
    *routes_slot().write().unwrap_or_else(|e| e.into_inner()) = routes;
}

pub fn cache_stats_json(cache: &CacheLayer) -> String {
    format!(r#"{{"hits":{},"misses":{},"size":{}}}"#, cache.hits(), cache.misses(), cache.size())
}

/// Stats of the registered cache; `None` without one.
pub fn registered_cache_json() -> Option<String> {
    cache_slot().read().unwrap_or_else(|e| e.into_inner()).as_ref().map(cache_stats_json)
}

/// Clear the registered cache; `false` without one.
pub fn clear_registered_cache() -> bool {
    match cache_slot().read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        Some(cache) => {
            cache.clear();
            true
        }
        None => false,
    }
}

pub fn routes_json(routes: &[(String, String)]) -> String {
    let entries: Vec<String> =
        routes.iter().map(|(m, p)| format!(r#"{{"method":{},"pattern":{}}}"#, quoted(m), quoted(p))).collect();
    format!("[{}]", entries.join(","))
}

pub fn registered_routes_json() -> String {
    routes_json(&routes_slot().read().unwrap_or_else(|e| e.into_inner()))
}

/// The config file's upstreams: each backend's weight, health, breaker
/// state and whether it is drained.
pub fn upstreams_json() -> String {
    let upstreams: Vec<String> = crate::proxy_config::upstream::snapshot()
        .iter()
        .map(|(name, backends)| {
            let backends: Vec<String> = backends
                .iter()
                .map(|b| {
                    format!(
                        r#"{{"url":{},"weight":{},"healthy":{},"breaker":{},"drained":{}}}"#,
                        quoted(&b.url),
                        b.weight,
                        b.healthy,
                        b.breaker.map_or("null".to_string(), quoted),
                        b.drained
                    )
                })
                .collect();
            format!(r#"{{"name":{},"backends":[{}]}}"#, quoted(name), backends.join(","))
        })
        .collect();
    format!("[{}]", upstreams.join(","))
}

/// Drain or undrain `backend` of `upstream`; `Err` names what doesn't exist.
pub fn set_drained(upstream: &str, backend: &str, drained: bool) -> Result<String, String> {
    let snapshot = crate::proxy_config::upstream::snapshot();
    let Some((_, backends)) = snapshot.iter().find(|(name, _)| name == upstream) else {
        return Err(format!("no upstream {upstream:?}"));
    };
    if !backends.iter().any(|b| b.url == backend) {
        return Err(format!("upstream {upstream:?} has no backend {backend:?}"));
    }
    crate::proxy_config::upstream::set_drained(upstream, backend, drained);
    Ok(format!(r#"{{"upstream":{},"backend":{},"drained":{drained}}}"#, quoted(upstream), quoted(backend)))
}

/// The certificates served, soonest expiry first, and the ACME renewal
/// loop's state.
pub fn certificates_json() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut certificates = crate::metrics::tls::certificates();
    certificates.sort_by_key(|(_, not_after, _)| *not_after);
    let list: Vec<String> = certificates
        .iter()
        .map(|(name, not_after, source)| {
            format!(
                r#"{{"name":{},"not_after":{},"expires":{},"days_left":{},"source":"{source}"}}"#,
                quoted(name),
                not_after,
                quoted(&crate::http::date::format((*not_after).max(0) as u64)),
                (not_after - now).div_euclid(86_400)
            )
        })
        .collect();
    format!(r#"{{"certificates":[{}],"acme":{}}}"#, list.join(","), acme_json())
}

#[cfg(feature = "acme")]
fn acme_json() -> String {
    let (running, last) = crate::acme::renewal_status();
    let last = last.map_or("null".to_string(), |(at, outcome)| format!(r#"{{"at":{at},"outcome":{}}}"#, quoted(&outcome)));
    format!(r#"{{"running":{running},"last_check":{last}}}"#)
}

#[cfg(not(feature = "acme"))]
fn acme_json() -> String {
    r#"{"running":false,"last_check":null}"#.to_string()
}

/// Ask ACME to renew now; `false` when it isn't running.
pub fn request_renewal() -> bool {
    #[cfg(feature = "acme")]
    return crate::acme::request_renewal();
    #[cfg(not(feature = "acme"))]
    false
}

/// The last `n` access-log lines (at most [`crate::log::tail::CAPACITY`]),
/// oldest first, each as a string.
pub fn logs_json(n: usize) -> String {
    let lines: Vec<String> = crate::log::tail::recent(n).iter().map(|l| quoted(l)).collect();
    format!("[{}]", lines.join(","))
}

/// Every named cache (the binary's `app`, `route:<name>` for config-file
/// routes): kind, entries, bytes on disk, hits, misses.
pub fn caches_json() -> String {
    let caches: Vec<String> = crate::cache::registry::list()
        .iter()
        .map(|(name, c)| {
            format!(
                r#"{{"name":{},"kind":"{}","entries":{},"bytes":{},"hits":{},"misses":{}}}"#,
                quoted(name),
                c.kind(),
                c.size(),
                c.stored_bytes().map_or("null".to_string(), |b| b.to_string()),
                c.hits(),
                c.misses()
            )
        })
        .collect();
    format!("[{}]", caches.join(","))
}

/// Purge `name`: everything, one target (`path`) or a `prefix`; `None` for
/// an unknown cache.
pub fn purge_cache(name: &str, path: Option<&str>, prefix: Option<&str>) -> Option<String> {
    let cache = crate::cache::registry::get(name)?;
    let target = match (path, prefix) {
        (Some(p), _) if !p.is_empty() => crate::cache::Purge::Exact(p.to_string()),
        (_, Some(p)) if !p.is_empty() => crate::cache::Purge::Prefix(p.to_string()),
        _ => crate::cache::Purge::All,
    };
    let n = cache.purge(&target);
    Some(format!(r#"{{"name":{},"purged":{n}}}"#, quoted(name)))
}
