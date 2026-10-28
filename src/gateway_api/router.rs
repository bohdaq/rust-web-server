//! Routing one request on a Gateway listener port: pick the listener by
//! hostname, the rule by the Gateway API's precedence, a backend by weight,
//! an endpoint, and forward — keeping the client's `Host`.
//!
//! Precedence (HTTPRoute spec), highest first: the longest non-wildcard
//! matching hostname, the longest matching hostname, an `Exact` path over a
//! `PathPrefix`, the longest path, a method match, the most header matches,
//! the most query-parameter matches; ties go to the oldest route, then the
//! route's `namespace/name`, then rule and match order.

// `Err(Response)` is the answer itself (a redirect, a 404, a 504), moved once.
#![allow(clippy::result_large_err)]

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
pub(crate) fn pick_listener<'a>(listeners: &'a [ListenerConfig], host: &str) -> Option<&'a ListenerConfig> {
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

/// The method of a gRPC path `/<service>/<method>`.
fn grpc_method(path: &str) -> Option<&str> {
    let (service, method) = path.strip_prefix('/')?.split_once('/')?;
    (!service.is_empty() && !method.is_empty() && !method.contains('/')).then_some(method)
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
                super::routes::GRPC_METHOD_MATCH if grpc_method(path) == Some(value) => (0, value.len()),
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

/// A rule's `timeouts` and `retry`, as the forwarding loop applies them:
/// `request` bounds the whole exchange, `backendRequest` each attempt (`0s`
/// disables; passing either is a `504`); `attempts` retries on the listed
/// `codes`, connection errors and backend timeouts, `backoff` apart.
struct Policy<'a> {
    rule: &'a CompiledRule,
    deadline: Option<std::time::Instant>,
    per_attempt: Option<Duration>,
    retries: u32,
    backoff: Duration,
    attempt: u32,
}

impl<'a> Policy<'a> {
    fn new(rule: &'a CompiledRule) -> Policy<'a> {
        let limit = |value: Option<&String>| value.and_then(|v| filters::duration(v)).filter(|d| !d.is_zero());
        let timeouts = rule.timeouts.as_ref();
        Policy {
            rule,
            deadline: limit(timeouts.and_then(|t| t.request.as_ref())).map(|d| std::time::Instant::now() + d),
            per_attempt: limit(timeouts.and_then(|t| t.backend_request.as_ref())),
            retries: rule.retry.as_ref().map(|r| r.attempts.unwrap_or(1).max(0) as u32).unwrap_or(0),
            backoff: rule.retry.as_ref().and_then(|r| r.backoff.as_deref()).and_then(filters::duration).unwrap_or(DEFAULT_BACKOFF),
            attempt: 0,
        }
    }

    /// The next attempt's timeout, or the `504` when the request's time is up.
    fn timeout(&self) -> Result<Duration, Response> {
        let left = self.deadline.map(|d| d.saturating_duration_since(std::time::Instant::now()));
        if left.is_some_and(|l| l.is_zero()) {
            return Err(status(STATUS_CODE_REASON_PHRASE.n504_gateway_timeout));
        }
        Ok([self.per_attempt, left].into_iter().flatten().min().unwrap_or(DEFAULT_TIMEOUT))
    }

    /// The response to send, or how long to wait before retrying.
    fn settle(&mut self, outcome: Attempt) -> Result<Response, Duration> {
        let retry_on = |code: i16| self.rule.retry.as_ref().is_some_and(|r| r.codes.contains(&(code as i32)));
        let retry = self.attempt < self.retries
            && match &outcome {
                Attempt::Response(response) => retry_on(response.status_code),
                Attempt::Failed { .. } => true,
            };
        if !retry {
            return Ok(match outcome {
                Attempt::Response(response) => response,
                Attempt::Failed { timed_out: true } => status(STATUS_CODE_REASON_PHRASE.n504_gateway_timeout),
                Attempt::Failed { timed_out: false } => status(STATUS_CODE_REASON_PHRASE.n502_bad_gateway),
            });
        }
        self.attempt += 1;
        Err(self.deadline.map_or(self.backoff, |d| self.backoff.min(d.saturating_duration_since(std::time::Instant::now()))))
    }
}

/// Where one attempt goes: the backend picked by weight, an endpoint of it,
/// and the request as that backend's filters leave it.
struct Target<'a> {
    backend: &'a super::routes::Backend,
    request: std::borrow::Cow<'a, Request>,
    ip: String,
    port: u16,
    host_header: String,
}

/// The upstream timeouts of an attempt with `timeout` to go.
fn connect_timeout(timeout: Duration) -> Duration {
    timeout.min(Duration::from_secs(10))
}

impl PortRouter {
    #[cfg(test)]
    fn forward(&self, rule: &CompiledRule, request: &Request, connection: &ConnectionInfo) -> Response {
        self.forward_with(rule, request, connection, None)
    }

    /// Forward within the rule's [`Policy`], presenting `client_cert` (the
    /// Gateway's) to TLS backends; blocks for the whole exchange.
    fn forward_with(&self, rule: &CompiledRule, request: &Request, connection: &ConnectionInfo, client_cert: Option<&(Vec<u8>, Vec<u8>)>) -> Response {
        let mut policy = Policy::new(rule);
        loop {
            let timeout = match policy.timeout() {
                Ok(timeout) => timeout,
                Err(response) => return response,
            };
            match policy.settle(self.attempt(rule, request, connection, timeout, client_cert)) {
                Ok(response) => return response,
                Err(pause) => std::thread::sleep(pause),
            }
        }
    }

    /// [`PortRouter::forward_with`] without blocking a thread: plain and
    /// BackendTLSPolicy backends through the async forwarder, h2c ones on
    /// the blocking pool.
    async fn forward_async(&self, rule: &CompiledRule, request: &Request, connection: &ConnectionInfo, client_cert: Option<&(Vec<u8>, Vec<u8>)>) -> Response {
        let mut policy = Policy::new(rule);
        loop {
            let timeout = match policy.timeout() {
                Ok(timeout) => timeout,
                Err(response) => return response,
            };
            match policy.settle(self.attempt_async(rule, request, connection, timeout, client_cert).await) {
                Ok(response) => return response,
                Err(pause) => tokio::time::sleep(pause).await,
            }
        }
    }

    /// Pick a backend by weight and an endpoint; `Err` is the response
    /// when there is nothing to send to.
    fn target<'a>(&self, rule: &'a CompiledRule, request: &'a Request) -> Result<Target<'a>, Response> {
        let total: u32 = rule.backends.iter().map(|b| b.weight).sum();
        if total == 0 {
            // No backends, or all weighted 0: the spec's 500.
            return Err(status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error));
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
        let request = if backend.filters.is_empty() {
            std::borrow::Cow::Borrowed(request)
        } else {
            let mut copy = request.clone();
            for filter in &backend.filters {
                if let Some(f) = filter.request_header_modifier.as_ref().filter(|_| filter.filter_type == "RequestHeaderModifier") {
                    filters::modify_headers(&mut copy.headers, f);
                }
            }
            std::borrow::Cow::Owned(copy)
        };
        let endpoints = match &backend.endpoints {
            Err(_) => return Err(status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error)),
            Ok(e) if e.is_empty() => return Err(status(STATUS_CODE_REASON_PHRASE.n503_service_unavailable)),
            Ok(e) => e,
        };
        let endpoint = &endpoints[self.counter.fetch_add(1, Ordering::Relaxed) % endpoints.len()];
        let (ip, port) = endpoint
            .rsplit_once(':')
            .and_then(|(ip, port)| Some((ip.trim_start_matches('[').trim_end_matches(']').to_string(), port.parse::<u16>().ok()?)))
            .ok_or_else(|| status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error))?;
        let host_header = request.header("host").map(|h| h.value.to_string()).unwrap_or_else(|| endpoint.clone());
        Ok(Target { backend, request, ip, port, host_header })
    }

    /// The BackendTLSPolicy configuration for a target, with the Gateway's
    /// client certificate when it has one; `None` for a plain backend.
    fn backend_tls(target: &Target, client_cert: Option<&(Vec<u8>, Vec<u8>)>) -> Result<Option<super::backend_tls::BackendTls>, Response> {
        match &target.backend.tls {
            None => Ok(None),
            // A policy covers the backend but can't be used: never send it in the clear.
            Some(Err(_)) => Err(status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error)),
            Some(Ok(tls)) => match client_cert {
                None => Ok(Some(tls.clone())),
                Some((chain, key)) => tls.with_client_cert(chain, key).map(Some).map_err(|_| status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error)),
            },
        }
    }

    /// Forward once, blocking.
    fn attempt(&self, rule: &CompiledRule, request: &Request, connection: &ConnectionInfo, timeout: Duration, client_cert: Option<&(Vec<u8>, Vec<u8>)>) -> Attempt {
        let target = match self.target(rule, request) {
            Ok(target) => target,
            Err(response) => return Attempt::Response(response),
        };
        let tls = match Self::backend_tls(&target, client_cert) {
            Ok(tls) => tls,
            Err(response) => return Attempt::Response(response),
        };
        let (request, ip, port, host_header) = (target.request.as_ref(), target.ip.as_str(), target.port, target.host_header.as_str());
        let started = std::time::Instant::now();
        let forwarded = if let Some(tls) = &tls {
            crate::proxy::forward_tls_with_host(&tls.config, &tls.label, request, &connection.client.ip, ip, port, &tls.server_name, host_header, connect_timeout(timeout), timeout)
        } else if super::upgrade::is_upgrade(request) {
            super::upgrade::forward(request, &connection.client.ip, ip, port, host_header, timeout)
        } else if target.backend.app_protocol.as_deref() == Some("kubernetes.io/h2c") {
            crate::proxy::forward_h2c_with_host(request, &connection.client.ip, ip, port, host_header, connect_timeout(timeout), timeout)
        } else {
            crate::proxy::forward_pooled_with_host(crate::proxy::shared_pool(), request, &connection.client.ip, ip, port, host_header, connect_timeout(timeout), timeout)
        };
        match forwarded {
            Ok(response) => Attempt::Response(response),
            Err(_) => Attempt::Failed { timed_out: started.elapsed() >= timeout },
        }
    }

    /// Forward once without blocking (upgrades never get here: they take
    /// the blocking path, [`Application::execute`]).
    async fn attempt_async(&self, rule: &CompiledRule, request: &Request, connection: &ConnectionInfo, timeout: Duration, client_cert: Option<&(Vec<u8>, Vec<u8>)>) -> Attempt {
        let target = match self.target(rule, request) {
            Ok(target) => target,
            Err(response) => return Attempt::Response(response),
        };
        let tls = match Self::backend_tls(&target, client_cert) {
            Ok(tls) => tls,
            Err(response) => return Attempt::Response(response),
        };
        let started = std::time::Instant::now();
        let forwarded = if tls.is_none() && target.backend.app_protocol.as_deref() == Some("kubernetes.io/h2c") {
            // HTTP/2 upstreams have their own runtime and a blocking API.
            let (request, client_ip, ip, port, host) = (target.request.into_owned(), connection.client.ip.clone(), target.ip, target.port, target.host_header);
            tokio::task::spawn_blocking(move || crate::proxy::forward_h2c_with_host(&request, &client_ip, &ip, port, &host, connect_timeout(timeout), timeout))
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
        } else {
            let tls = tls.as_ref().map(|t| (t.server_name.as_str(), crate::proxy::async_upstream::AsyncTls { config: std::sync::Arc::clone(&t.config), pool: crate::proxy::async_upstream::gateway_tls_pool(&t.label) }));
            crate::proxy::async_upstream::forward_gateway(&target.request, &connection.client.ip, &target.ip, target.port, &target.host_header, tls, connect_timeout(timeout), timeout).await
        };
        match forwarded {
            Ok(response) => Attempt::Response(response),
            Err(_) => Attempt::Failed { timed_out: started.elapsed() >= timeout },
        }
    }
}

/// What the rule's filters made of a request, before it's forwarded.
struct Prepared<'a> {
    /// The request as the filters left it (`None`: unchanged).
    request: Option<Request>,
    response_filters: Vec<&'a HttpRouteFilter>,
    cors_headers: Vec<crate::header::Header>,
}

impl PortRouter {
    /// Run the rule's filters in order: a redirect or a CORS preflight is
    /// the answer (`Err`); header modifiers, rewrites and mirrors act on the
    /// request now; response modifiers and CORS headers wait for
    /// [`PortRouter::finish`].
    fn prepare<'a>(&self, listener: &ListenerConfig, rule: &'a CompiledRule, matched: &HttpRouteMatch, request: &Request, host: &str, connection: &ConnectionInfo) -> Result<Prepared<'a>, Response> {
        let mut prepared = Prepared { request: None, response_filters: Vec::new(), cors_headers: Vec::new() };
        if rule.filters.is_empty() {
            return Ok(prepared);
        }
        let mut request = request.clone();
        let mut mirrored = 0;
        for filter in &rule.filters {
            match filter.filter_type.as_str() {
                "RequestHeaderModifier" => {
                    if let Some(f) = &filter.request_header_modifier {
                        filters::modify_headers(&mut request.headers, f);
                    }
                }
                "ResponseHeaderModifier" => prepared.response_filters.push(filter),
                "RequestRedirect" => {
                    if let Some(f) = &filter.request_redirect {
                        return Err(filters::redirect(&request, host, listener.protocol == "HTTPS", listener.port as u16, matched, f));
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
                                return Err(response);
                            }
                            Cors::Actual(headers) => prepared.cors_headers = headers,
                        }
                    }
                }
                "RequestMirror" => mirrored += filter.request_mirror.is_some() as usize,
                // ExtensionRef and anything unknown: the spec's 500.
                _ => return Err(status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error)),
            }
        }
        // Mirrors get the request as the filters left it.
        for mirror in rule.mirrors.iter().take(mirrored) {
            if sample(mirror.share) {
                send_mirror(mirror, &request, &connection.client.ip);
            }
        }
        prepared.request = Some(request);
        Ok(prepared)
    }

    /// The response modifiers and CORS headers, on the backend's response.
    fn finish(prepared: Prepared, mut response: Response) -> Response {
        for filter in prepared.response_filters {
            if let Some(f) = &filter.response_header_modifier {
                filters::modify_headers(&mut response.headers, f);
            }
        }
        for header in prepared.cors_headers {
            response.headers.retain(|h| !h.name.eq_ignore_ascii_case(&header.name));
            response.headers.push(header);
        }
        response
    }

    /// Filters, forwarding (blocking) and the response filters.
    fn route(&self, listener: &ListenerConfig, rule: &CompiledRule, matched: &HttpRouteMatch, request: &Request, host: &str, connection: &ConnectionInfo) -> Response {
        match self.prepare(listener, rule, matched, request, host, connection) {
            Err(response) => response,
            Ok(prepared) => {
                let response = self.forward_with(rule, prepared.request.as_ref().unwrap_or(request), connection, listener.backend_client_cert.as_ref());
                Self::finish(prepared, response)
            }
        }
    }
}

/// Over TLS, a request whose `Host` belongs to another listener than the
/// one its SNI chose (and whose certificate it got) is misdirected: `421`.
fn misdirected(listeners: &[ListenerConfig], by_host: &ListenerConfig, connection: &ConnectionInfo) -> bool {
    let Some(sni) = connection.sni_hostname.as_deref() else { return false };
    pick_listener(listeners, &sni.to_ascii_lowercase()).is_some_and(|by_sni| !std::ptr::eq(by_sni, by_host))
}

impl PortRouter {
    /// The listener and rule a request goes to, or the answer when none
    /// does (`404`, `421`).
    fn resolve<'l>(listeners: &'l [ListenerConfig], request: &Request, host: &str, connection: &ConnectionInfo) -> Result<(&'l ListenerConfig, &'l CompiledRule, &'l HttpRouteMatch), Response> {
        let Some(listener) = pick_listener(listeners, host) else {
            return Err(status(STATUS_CODE_REASON_PHRASE.n404_not_found));
        };
        if misdirected(listeners, listener, connection) {
            return Err(status(STATUS_CODE_REASON_PHRASE.n421_misdirected_request));
        }
        match select(listener, request, host) {
            Some((_, rule, matched)) => Ok((listener, rule, matched)),
            None => Err(status(STATUS_CODE_REASON_PHRASE.n404_not_found)),
        }
    }
}

impl Application for PortRouter {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let listeners = self.listeners.read().unwrap_or_else(|e| e.into_inner()).clone();
        let host = request_host(request);
        Ok(match Self::resolve(&listeners, request, &host, connection) {
            Ok((listener, rule, matched)) => self.route(listener, rule, matched, request, &host, connection),
            Err(response) => response,
        })
    }

    /// Everything but upgrades runs here, on the connection's task: the
    /// forwarding loop waits on tokio sockets instead of holding a
    /// blocking-pool thread per request in flight.
    #[cfg(feature = "http2")]
    fn execute_async<'a>(&'a self, request: &'a Request, connection: &'a ConnectionInfo) -> Option<crate::application::AsyncResponse<'a>> {
        if super::upgrade::is_upgrade(request) {
            return None;
        }
        Some(Box::pin(async move {
            let listeners = self.listeners.read().unwrap_or_else(|e| e.into_inner()).clone();
            let host = request_host(request);
            let (listener, rule, matched) = match Self::resolve(&listeners, request, &host, connection) {
                Ok(found) => found,
                Err(response) => return Ok(response),
            };
            let prepared = match self.prepare(listener, rule, matched, request, &host, connection) {
                Ok(prepared) => prepared,
                Err(response) => return Ok(response),
            };
            let response = self.forward_async(rule, prepared.request.as_ref().unwrap_or(request), connection, listener.backend_client_cert.as_ref()).await;
            Ok(Self::finish(prepared, response))
        }))
    }
}

#[cfg(test)]
mod tests;
