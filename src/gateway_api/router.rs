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
use crate::core::New;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

use super::routes::{CompiledHttpRoute, CompiledRule};
use super::translate::ListenerConfig;
use super::types::HttpRouteMatch;

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

fn query_value<'a>(request: &'a Request, name: &str) -> Option<String> {
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

/// The rule a request goes to on a listener.
pub(crate) fn select<'a>(listener: &'a ListenerConfig, request: &Request, host: &str) -> Option<(&'a CompiledHttpRoute, &'a CompiledRule)> {
    type Key<'k> = ((usize, usize), (u8, usize, u8, usize, usize), std::cmp::Reverse<(&'k str, &'k (String, String), usize, usize)>);
    let mut best: Option<(Key, &CompiledHttpRoute, &CompiledRule)> = None;
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
                if best.as_ref().is_none_or(|(b, _, _)| key > *b) {
                    best = Some((key, route, rule));
                }
            }
        }
    }
    best.map(|(_, route, rule)| (route, rule))
}

fn status(code: &'static crate::response::StatusCodeReasonPhrase) -> Response {
    let mut response = Response::new();
    response.status_code = *code.status_code;
    response.reason_phrase = code.reason_phrase.to_string();
    response
}

impl PortRouter {
    /// Pick a backend by weight, then an endpoint, and forward.
    fn forward(&self, rule: &CompiledRule, request: &Request, connection: &ConnectionInfo) -> Response {
        let total: u32 = rule.backends.iter().map(|b| b.weight).sum();
        if total == 0 {
            // No backends, or all weighted 0: the spec's 500.
            return status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error);
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
        let endpoints = match &backend.endpoints {
            Err(_) => return status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error),
            Ok(e) if e.is_empty() => return status(STATUS_CODE_REASON_PHRASE.n503_service_unavailable),
            Ok(e) => e,
        };
        let endpoint = &endpoints[self.counter.fetch_add(1, Ordering::Relaxed) % endpoints.len()];
        let Some((ip, port)) = endpoint.rsplit_once(':') else {
            return status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error);
        };
        let ip = ip.trim_start_matches('[').trim_end_matches(']');
        let Ok(port) = port.parse::<u16>() else {
            return status(STATUS_CODE_REASON_PHRASE.n500_internal_server_error);
        };
        let host_header = request.header("host").map(|h| h.value.to_string()).unwrap_or_else(|| endpoint.clone());
        match crate::proxy::forward_pooled_with_host(
            crate::proxy::shared_pool(),
            request,
            &connection.client.ip,
            ip,
            port,
            &host_header,
            Duration::from_secs(10),
            Duration::from_secs(60),
        ) {
            Ok(response) => response,
            Err(_) => status(STATUS_CODE_REASON_PHRASE.n502_bad_gateway),
        }
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
            Some((_, rule)) => Ok(self.forward(rule, request, connection)),
            None => Ok(status(STATUS_CODE_REASON_PHRASE.n404_not_found)),
        }
    }
}

#[cfg(test)]
mod tests;
