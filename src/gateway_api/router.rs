//! Routing one request on a Gateway listener port: pick the listener by
//! hostname, the rule by the Gateway API's precedence, a backend by weight,
//! an endpoint, and forward — keeping the client's `Host`.
//!
//! Precedence (HTTPRoute spec), highest first: the longest non-wildcard
//! matching hostname, the longest matching hostname, an `Exact` path over a
//! `PathPrefix`, the longest path, a method match, the most header matches,
//! the most query-parameter matches; ties go to the oldest route, then the
//! route's `namespace/name`, then rule and match order.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::application::Application;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

use super::filters::{self, status, Cors};
use crate::server::ConnectionInfo;

use super::routes::{CompiledHttpRoute, CompiledRule, Mirror};
use super::translate::ListenerConfig;
use super::types::{HttpRouteFilter, HttpRouteMatch};

/// The listeners served on one container port (one Gateway's, one port;
/// several when they differ by hostname).
pub type PortListeners = Arc<RwLock<Arc<Vec<ListenerConfig>>>>;

#[derive(Clone)]
pub struct PortRouter {
    pub listeners: PortListeners,
    counter: Arc<AtomicUsize>,
}

impl PortRouter {
    pub fn new(listeners: PortListeners) -> PortRouter {
        PortRouter { listeners, counter: Arc::new(AtomicUsize::new(0)) }
    }
}

/// The request's host, lower-cased, without a port.
pub(crate) fn request_host(request: &Request) -> String {
    let host = request.header("host").map(|h| h.value.to_string()).unwrap_or_default();
    let host = if host.starts_with('[') {
        host.split(']').next().map(|h| format!("{h}]")).unwrap_or(host)
    } else {
        host.rsplit_once(':').map(|(h, _)| h.to_string()).unwrap_or(host)
    };
    host.to_ascii_lowercase()
}

/// How specifically `pattern` (a hostname, maybe `*.`-wildcard) matches
/// `host`: `None` for no match; non-wildcard matches score above wildcards,
/// longer above shorter.
pub(crate) fn host_score(pattern: &str, host: &str) -> Option<(usize, usize)> {
    let pattern = pattern.to_ascii_lowercase();
    match pattern.strip_prefix("*.") {
        None => (pattern == host).then_some((pattern.len(), pattern.len())),
        Some(suffix) => (host.len() > suffix.len() + 1 && host.ends_with(&format!(".{suffix}"))).then_some((0, pattern.len())),
    }
}

/// The listener a request on this port belongs to: the most specific
/// matching hostname; a listener without one matches any host.
fn pick_listener<'a>(listeners: &'a [ListenerConfig], host: &str) -> Option<&'a ListenerConfig> {
    listeners
        .iter()
        .filter_map(|l| match l.hostname.as_deref().filter(|h| !h.is_empty()) {
            None => Some(((0, 0), l)),
            Some(pattern) => host_score(pattern, host).map(|s| (s, l)),
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, l)| l)
}

fn path_of(request: &Request) -> &str {
    let uri = request.request_uri.as_str();
    uri.split(['?', '#']).next().unwrap_or(uri)
}

fn query_value(request: &Request, name: &str) -> Option<String> {
    let query = request.request_uri.split_once('?')?.1;
    let query = query.split('#').next().unwrap_or(query);
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (crate::url::URL::percent_decode(k) == name).then(|| crate::url::URL::percent_decode(v))
    })
}

/// A path prefix matches whole segments: `/foo` matches `/foo` and
/// `/foo/bar`, not `/foobar`; a trailing `/` on the prefix is ignored.
pub(crate) fn prefix_matches(prefix: &str, path: &str) -> bool {
    let prefix = if prefix.len() > 1 { prefix.trim_end_matches('/') } else { prefix };
    prefix == "/" || path == prefix || (path.starts_with(prefix) && path.as_bytes().get(prefix.len()) == Some(&b'/'))
}

/// Whether a match accepts the request, and its precedence score:
/// (exact path, path length, method, headers, query params).
fn match_score(m: &HttpRouteMatch, request: &Request) -> Option<(u8, usize, u8, usize, usize)> {
    let path = path_of(request);
    let (exact, length) = match &m.path {
        None => (0, 1),
        Some(p) => {
            let value = p.value.as_deref().unwrap_or("/");
            match p.match_type.as_deref().unwrap_or("PathPrefix") {
                "Exact" if path == value => (1, value.len()),
                "PathPrefix" if prefix_matches(value, path) => (0, value.len()),
                _ => return None,
            }
        }
    };
    if let Some(method) = &m.method {
        if &request.method != method {
            return None;
        }
    }
    for header in &m.headers {
        if header.match_type.as_deref().unwrap_or("Exact") != "Exact" {
            return None;
        }
        if request.header(&header.name).map(|h| h.value.as_ref()) != Some(header.value.as_str()) {
            return None;
        }
    }
    for param in &m.query_params {
        if param.match_type.as_deref().unwrap_or("Exact") != "Exact" {
            return None;
        }
        if query_value(request, &param.name).as_deref() != Some(param.value.as_str()) {
            return None;
        }
    }
    Some((exact, length, m.method.is_some() as u8, m.headers.len(), m.query_params.len()))
}

/// The rule a request goes to on a listener, and the match that chose it.
pub(crate) fn select<'a>(listener: &'a ListenerConfig, request: &Request, host: &str) -> Option<(&'a CompiledHttpRoute, &'a CompiledRule, &'a HttpRouteMatch)> {
    type Key<'k> = ((usize, usize), (u8, usize, u8, usize, usize), std::cmp::Reverse<(&'k str, &'k (String, String), usize, usize)>);
    let mut best: Option<(Key, &CompiledHttpRoute, &CompiledRule, &HttpRouteMatch)> = None;
    for route in &listener.http_routes {
        let host_match = if route.hostnames.is_empty() {
            Some((0, 0))
        } else {
            route.hostnames.iter().filter_map(|h| host_score(h, host)).max()
        };
        let Some(host_match) = host_match else { continue };
        for (rule_index, rule) in route.rules.iter().enumerate() {
            for (match_index, m) in rule.matches.iter().enumerate() {
                let Some(score) = match_score(m, request) else { continue };
                let key = (host_match, score, std::cmp::Reverse((route.created.as_str(), &route.key, rule_index, match_index)));
                if best.as_ref().is_none_or(|(b, ..)| key > *b) {
                    best = Some((key, route, rule, m));
                }
            }
        }
    }
    best.map(|(_, route, rule, m)| (route, rule, m))
}

/// True for about `share` (0.0–1.0) of calls.
fn sample(share: f64) -> bool {
    use std::hash::{BuildHasher, Hasher};
    static KEYS: std::sync::OnceLock<std::collections::hash_map::RandomState> = std::sync::OnceLock::new();
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    if share >= 1.0 {
        return true;
    }
    let mut hasher = KEYS.get_or_init(Default::default).build_hasher();
    hasher.write_usize(COUNTER.fetch_add(1, Ordering::Relaxed));
    ((hasher.finish() >> 11) as f64 / (1u64 << 53) as f64) < share
}

/// Send a copy of `request` to one of the mirror's endpoints on a thread
/// of its own; the response is dropped.
fn send_mirror(mirror: &Mirror, request: &Request, client_ip: &str) {
    let Ok(endpoints) = &mirror.endpoints else { return };
    let Some(endpoint) = endpoints.get(MIRROR_COUNTER.fetch_add(1, Ordering::Relaxed) % endpoints.len().max(1)).cloned() else { return };
    let request = request.clone();
    let client_ip = client_ip.to_string();
    std::thread::spawn(move || {
        let Some((ip, port)) = endpoint.rsplit_once(':') else { return };
        let ip = ip.trim_start_matches('[').trim_end_matches(']');
        let Ok(port) = port.parse::<u16>() else { return };
        let host = request.header("host").map(|h| h.value.to_string()).unwrap_or_else(|| endpoint.clone());
        let _ = crate::proxy::forward_pooled_with_host(crate::proxy::shared_pool(), &request, &client_ip, ip, port, &host, Duration::from_secs(2), Duration::from_secs(10));
    });
}

static MIRROR_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// How one attempt ended.
enum Attempt {
    /// A response to send (or to retry on its status).
    Response(Response),
    /// No response: the connection failed, or the attempt's timeout passed.
    Failed { timed_out: bool },
}

/// Read timeout of an attempt without a route timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// Wait between retries without `retry.backoff`.
const DEFAULT_BACKOFF: Duration = Duration::from_millis(25);

impl PortRouter {
    /// Forward within the rule's `timeouts` (`request` for the whole
    /// exchange, `backendRequest` per attempt; `0s` disables — passing either
    /// is a `504`) and `retry` (`attempts` retries on the listed `codes`,
    /// connection errors and backend timeouts, `backoff` apart).
    fn forward(&self, rule: &CompiledRule, request: &Request, connection: &ConnectionInfo) -> Response {
        let limit = |value: Option<&String>| value.and_then(|v| filters::duration(v)).filter(|d| !d.is_zero());
        let timeouts = rule.timeouts.as_ref();
        let deadline = limit(timeouts.and_then(|t| t.request.as_ref())).map(|d| std::time::Instant::now() + d);
        let per_attempt = limit(timeouts.and_then(|t| t.backend_request.as_ref()));
        let retries = rule.retry.as_ref().map(|r| r.attempts.unwrap_or(1).max(0) as u32).unwrap_or(0);
        let backoff = rule.retry.as_ref().and_then(|r| r.backoff.as_deref()).and_then(filters::duration).unwrap_or(DEFAULT_BACKOFF);
        let retry_on = |code: i16| rule.retry.as_ref().is_some_and(|r| r.codes.contains(&(code as i32)));
        let mut attempt = 0;
        loop {
            let left = deadline.map(|d| d.saturating_duration_since(std::time::Instant::now()));
            if left.is_some_and(|l| l.is_zero()) {
                return status(STATUS_CODE_REASON_PHRASE.n504_gateway_timeout);
            }
            let timeout = [per_attempt, left].into_iter().flatten().min().unwrap_or(DEFAULT_TIMEOUT);
            let outcome = self.attempt(rule, request, connection, timeout);
            let retry = attempt < retries
                && match &outcome {
                    Attempt::Response(response) => retry_on(response.status_code),
                    Attempt::Failed { .. } => true,
                };
            if !retry {
                return match outcome {
                    Attempt::Response(response) => response,
                    Attempt::Failed { timed_out: true } => status(STATUS_CODE_REASON_PHRASE.n504_gateway_timeout),
                    Attempt::Failed { timed_out: false } => status(STATUS_CODE_REASON_PHRASE.n502_bad_gateway),
                };
            }
            attempt += 1;
            let pause = deadline.map_or(backoff, |d| backoff.min(d.saturating_duration_since(std::time::Instant::now())));
            std::thread::sleep(pause);
        }
    }

    /// Pick a backend by weight, then an endpoint, and forward once.
    fn attempt(&self, rule: &CompiledRule, request: &Request, connection: &ConnectionInfo, timeout: Duration) -> Attempt {
        let total: u32 = rule.backends.iter().map(|b| b.weight).sum();
        if total == 0 {
            // No backends, or all weighted 0: the spec's 500.
            return Attempt::Response(status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error));
        }
        let roll = (self.counter.fetch_add(1, Ordering::Relaxed) as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32;
        let mut pick = (roll % total as u64) as u32;
        let backend = rule
            .backends
            .iter()
            .find(|b| {
                if pick < b.weight {
                    true
                } else {
                    pick -= b.weight;
                    false
                }
            })
            .expect("weights sum to total");
        let modified;
        let request = if backend.filters.is_empty() {
            request
        } else {
            let mut copy = request.clone();
            for filter in &backend.filters {
                if let Some(f) = filter.request_header_modifier.as_ref().filter(|_| filter.filter_type == "RequestHeaderModifier") {
                    filters::modify_headers(&mut copy.headers, f);
                }
            }
            modified = copy;
            &modified
        };
        let endpoints = match &backend.endpoints {
            Err(_) => return Attempt::Response(status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error)),
            Ok(e) if e.is_empty() => return Attempt::Response(status(STATUS_CODE_REASON_PHRASE.n503_service_unavailable)),
            Ok(e) => e,
        };
        let endpoint = &endpoints[self.counter.fetch_add(1, Ordering::Relaxed) % endpoints.len()];
        let Some((ip, port)) = endpoint.rsplit_once(':') else {
            return Attempt::Response(status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error));
        };
        let ip = ip.trim_start_matches('[').trim_end_matches(']');
        let Ok(port) = port.parse::<u16>() else {
            return Attempt::Response(status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error));
        };
        let host_header = request.header("host").map(|h| h.value.to_string()).unwrap_or_else(|| endpoint.clone());
        let started = std::time::Instant::now();
        let forwarded = if super::upgrade::is_upgrade(request) {
            super::upgrade::forward(request, &connection.client.ip, ip, port, &host_header, timeout)
        } else if backend.app_protocol.as_deref() == Some("kubernetes.io/h2c") {
            crate::proxy::forward_h2c_with_host(request, &connection.client.ip, ip, port, &host_header, timeout.min(Duration::from_secs(10)), timeout)
        } else {
            crate::proxy::forward_pooled_with_host(
            crate::proxy::shared_pool(),
            request,
            &connection.client.ip,
            ip,
            port,
            &host_header,
                timeout.min(Duration::from_secs(10)),
                timeout,
            )
        };
        match forwarded {
            Ok(response) => Attempt::Response(response),
            Err(_) => Attempt::Failed { timed_out: started.elapsed() >= timeout },
        }
    }
}

impl PortRouter {
    /// Run the rule's filters in order around forwarding: a redirect or a
    /// CORS preflight answers here; header modifiers, rewrites and CORS
    /// headers change the request or the response.
    fn route(&self, listener: &ListenerConfig, rule: &CompiledRule, matched: &HttpRouteMatch, request: &Request, host: &str, connection: &ConnectionInfo) -> Response {
        if rule.filters.is_empty() {
            return self.forward(rule, request, connection);
        }
        let mut request = request.clone();
        let mut response_filters: Vec<&HttpRouteFilter> = Vec::new();
        let mut cors_headers = Vec::new();
        let mut mirrored = 0;
        for filter in &rule.filters {
            match filter.filter_type.as_str() {
                "RequestHeaderModifier" => {
                    if let Some(f) = &filter.request_header_modifier {
                        filters::modify_headers(&mut request.headers, f);
                    }
                }
                "ResponseHeaderModifier" => response_filters.push(filter),
                "RequestRedirect" => {
                    if let Some(f) = &filter.request_redirect {
                        return filters::redirect(&request, host, listener.protocol == "HTTPS", listener.port as u16, matched, f);
                    }
                }
                "URLRewrite" => {
                    if let Some(f) = &filter.url_rewrite {
                        if let Some(hostname) = &f.hostname {
                            request.headers.retain(|h| !h.name.eq_ignore_ascii_case("host"));
                            request.headers.push(crate::header::Header::new("Host", hostname.clone()));
                        }
                        if let Some(modifier) = &f.path {
                            let (path, query) = filters::split_target(&request.request_uri);
                            request.request_uri = format!("{}{query}", filters::modified_path(path, matched, modifier));
                        }
                    }
                }
                "CORS" => {
                    if let Some(f) = &filter.cors {
                        match filters::cors(&request, f) {
                            Cors::Pass => {}
                            Cors::Preflight(headers) => {
                                let mut response = status(STATUS_CODE_REASON_PHRASE.n204_no_content);
                                response.headers.extend(headers);
                                return response;
                            }
                            Cors::Actual(headers) => cors_headers = headers,
                        }
                    }
                }
                "RequestMirror" => mirrored += filter.request_mirror.is_some() as usize,
                // ExtensionRef and anything unknown: the spec's 500.
                _ => return status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error),
            }
        }
        // Mirrors get the request as the filters left it.
        for mirror in rule.mirrors.iter().take(mirrored) {
            if sample(mirror.share) {
                send_mirror(mirror, &request, &connection.client.ip);
            }
        }
        let mut response = self.forward(rule, &request, connection);
        for filter in response_filters {
            if let Some(f) = &filter.response_header_modifier {
                filters::modify_headers(&mut response.headers, f);
            }
        }
        for header in cors_headers {
            response.headers.retain(|h| !h.name.eq_ignore_ascii_case(&header.name));
            response.headers.push(header);
        }
        response
    }
}

impl Application for PortRouter {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let listeners = self.listeners.read().unwrap_or_else(|e| e.into_inner()).clone();
        let host = request_host(request);
        let Some(listener) = pick_listener(&listeners, &host) else {
            return Ok(status(STATUS_CODE_REASON_PHRASE.n404_not_found));
        };
        match select(listener, request, &host) {
            Some((_, rule, matched)) => Ok(self.route(listener, rule, matched, request, &host, connection)),
            None => Ok(status(STATUS_CODE_REASON_PHRASE.n404_not_found)),
        }
    }
}

#[cfg(test)]
mod tests;
