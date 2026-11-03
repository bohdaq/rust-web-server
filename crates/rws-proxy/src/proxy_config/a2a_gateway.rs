//! `[route.action] type = "a2a"`: one public address for an A2A agent
//! running elsewhere (18.18.0, #65).
//!
//! ```toml
//! [[route]]
//!   [route.match]
//!   path = "/agents/billing/*"
//!
//!   [route.action]
//!   type = "a2a"
//!
//!   [route.action.a2a]
//!   upstream    = "http://10.0.0.7:9000"     # the agent; its card at <upstream>/.well-known/agent-card.json
//!   timeout_ms  = 300000                     # per request (SendMessage may wait for its task)
//!   audit_log   = true
//!   headers_env = { Authorization = "BILLING_AGENT_AUTHORIZATION" }  # sent upstream, from the environment
//! ```
//!
//! `GET <path>/.well-known/agent-card.json` serves the agent's card with its
//! JSON-RPC interface pointing here (the agent's own address stays
//! private); a `POST` to `<path>` goes to the agent's JSON-RPC endpoint,
//! streams included. The route's middleware (auth, rate limits, IP filter)
//! applies as to any route; the caller's `Authorization` isn't passed on,
//! the configured headers are.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::application::Application;
use crate::config_check::ConfigReport;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::service_discovery::json_lite::{self, JsonValue};

use super::parser::{get_str, get_u64, SectionMap};

/// How long the agent's card is kept before it's fetched again.
const CARD_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct A2aGatewayConfig {
    pub upstream: String,
    pub timeout_ms: u64,
    pub audit_log: bool,
    /// `(header, environment variable holding its value)`.
    pub headers_env: Vec<(String, String)>,
}

/// The keys `check_unknown_keys` accepts.
#[doc(hidden)]
pub const KEYS: &[&str] = &["upstream", "timeout_ms", "audit_log"];

/// `section` is the action's, e.g. `route[0].action`.
#[doc(hidden)]
pub fn parse(map: &SectionMap, section: &str) -> A2aGatewayConfig {
    let base = format!("{section}.a2a");
    A2aGatewayConfig {
        upstream: get_str(map, &base, "upstream").trim_end_matches('/').to_string(),
        timeout_ms: get_u64(map, &base, "timeout_ms", 300_000),
        audit_log: get_str(map, &base, "audit_log").eq_ignore_ascii_case("true"),
        headers_env: map.get(&format!("{base}.headers_env")).cloned().unwrap_or_default(),
    }
}

/// Semantic checks for `rws --check` and startup.
#[doc(hidden)]
pub fn validate(config: &A2aGatewayConfig, route_path: Option<&str>, label: &str, report: &mut ConfigReport) {
    if route_path.is_none() {
        report.error(format!("{label}: an a2a route needs [route.match] path, e.g. \"/agents/billing/*\""));
    }
    if config.upstream.is_empty() {
        report.error(format!("{label}: [route.action.a2a] needs upstream, the agent's base URL"));
    } else if super::health::parse_backend_url(&config.upstream).is_none() || !(config.upstream.starts_with("http://") || config.upstream.starts_with("https://")) {
        report.error(format!("{label}: a2a upstream {:?} isn't an http(s) URL", config.upstream));
    }
    for (header, env) in &config.headers_env {
        if std::env::var(env).map(|v| v.is_empty()).unwrap_or(true) && !crate::config_check::offline() {
            report.warn(format!("{label}: a2a header {header} comes from {env}, which is unset or empty"));
        }
    }
}

struct Card {
    /// The agent's card as it sent it.
    json: JsonValue,
    /// Its JSON-RPC endpoint.
    endpoint: String,
    fetched: Instant,
}

/// The route's application.
#[doc(hidden)]
pub struct A2aGateway {
    config: A2aGatewayConfig,
    /// The route's path without a trailing `/*` or `/`.
    prefix: String,
    card: Mutex<Option<Card>>,
}

impl A2aGateway {
    #[doc(hidden)]
    pub fn new(config: A2aGatewayConfig, route_path: &str) -> A2aGateway {
        let prefix = route_path.trim_end_matches('*').trim_end_matches('/').to_string();
        A2aGateway { config, prefix, card: Mutex::new(None) }
    }

    fn headers(&self) -> Vec<(String, String)> {
        self.config.headers_env.iter().filter_map(|(h, env)| std::env::var(env).ok().filter(|v| !v.is_empty()).map(|v| (h.clone(), v))).collect()
    }

    /// The agent's card and JSON-RPC endpoint, cached for [`CARD_TTL`].
    fn card(&self) -> Result<(JsonValue, String), String> {
        let mut cached = self.card.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(card) = cached.as_ref().filter(|c| c.fetched.elapsed() < CARD_TTL) {
            return Ok((card.json.clone(), card.endpoint.clone()));
        }
        let url = format!("{}{}", self.config.upstream, crate::a2a::AGENT_CARD_PATH);
        let client = crate::http_client::Client::new().max_redirects(0);
        let mut request = client.get(&url).timeout_ms(10_000);
        for (name, value) in self.headers() {
            request = request.header(&name, &value);
        }
        let response = request.send().map_err(|e| format!("fetching the agent's card: {e}"))?;
        if !response.is_success() {
            return Err(format!("the agent's card answered HTTP {}", response.status()));
        }
        let json = json_lite::parse(&response.text().map_err(|e| e.to_string())?).map_err(|e| format!("the agent's card isn't JSON: {e}"))?;
        let endpoint = json
            .get("supportedInterfaces")
            .and_then(JsonValue::as_array)
            .and_then(|list| list.iter().find(|i| i.get("protocolBinding").and_then(JsonValue::as_str) == Some("JSONRPC")))
            .and_then(|i| i.get("url"))
            .and_then(JsonValue::as_str)
            .map(str::to_string)
            .ok_or("the agent's card lists no JSONRPC interface")?;
        *cached = Some(Card { json: json.clone(), endpoint: endpoint.clone(), fetched: Instant::now() });
        Ok((json, endpoint))
    }

    /// The card, with only a JSON-RPC interface: this route.
    fn public_card(&self, request: &Request, connection: &ConnectionInfo) -> Response {
        let (card, _) = match self.card() {
            Ok(card) => card,
            Err(e) => return unavailable(&e),
        };
        let host = request.header(crate::header::Header::_HOST).map(|h| h.value.to_string()).unwrap_or_else(|| format!("{}:{}", connection.server.ip, connection.server.port));
        let public = format!("{}://{host}{}", if connection.tls { "https" } else { "http" }, if self.prefix.is_empty() { "/" } else { &self.prefix });
        let JsonValue::Object(mut fields) = card else { return unavailable("the agent's card isn't an object") };
        for (key, value) in fields.iter_mut() {
            if key == "supportedInterfaces" {
                let version = value
                    .as_array()
                    .and_then(|l| l.iter().find(|i| i.get("protocolBinding").and_then(JsonValue::as_str) == Some("JSONRPC")))
                    .and_then(|i| i.get("protocolVersion"))
                    .and_then(JsonValue::as_str)
                    .unwrap_or(crate::a2a::PROTOCOL_VERSION)
                    .to_string();
                *value = JsonValue::Array(vec![JsonValue::Object(vec![
                    ("url".to_string(), JsonValue::String(public.clone())),
                    ("protocolBinding".to_string(), JsonValue::String("JSONRPC".to_string())),
                    ("protocolVersion".to_string(), JsonValue::String(version)),
                ])]);
            }
        }
        let mut response = Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, crate::a2a::json_text(&JsonValue::Object(fields)).into_bytes());
        response.headers.push(crate::header::Header::new("Cache-Control", "public, max-age=60".to_string()));
        response
    }

    /// The JSON-RPC request, forwarded to the agent (streams pass through).
    fn forward(&self, request: &Request, connection: &ConnectionInfo) -> Response {
        let started = Instant::now();
        let (_, endpoint) = match self.card() {
            Ok(card) => card,
            Err(e) => return unavailable(&e),
        };
        let Some((host, port, tls)) = super::health::parse_backend_url(&endpoint) else {
            return unavailable(&format!("the agent's JSON-RPC URL {endpoint:?} can't be used"));
        };
        let path = endpoint.splitn(4, '/').nth(3).map(|p| format!("/{p}")).unwrap_or_else(|| "/".to_string());
        let mut upstream = request.clone();
        upstream.request_uri = path;
        upstream.headers.retain(|h| !h.name.eq_ignore_ascii_case("authorization") && !h.name.eq_ignore_ascii_case("host"));
        upstream.headers.push(crate::header::Header::new("Host", if (tls && port == 443) || (!tls && port == 80) { host.clone() } else { format!("{host}:{port}") }));
        for (name, value) in self.headers() {
            upstream.headers.retain(|h| !h.name.eq_ignore_ascii_case(&name));
            upstream.headers.push(crate::header::Header::new(name, value));
        }
        let timeout = Duration::from_millis(self.config.timeout_ms.max(1));
        let result = crate::proxy::forward_upstream_trusting(crate::proxy::shared_pool(), &upstream, crate::proxy::Peer::of(connection), &host, port, tls, None, timeout.min(Duration::from_secs(10)), timeout);
        let response = match result {
            Ok(mut response) => {
                crate::proxy::status::received(&mut response, Some(&endpoint));
                response
            }
            Err(e) => {
                let mut response = crate::error::problem(STATUS_CODE_REASON_PHRASE.n502_bad_gateway, "");
                crate::proxy::status::failed_with(&mut response, &e, Some(&endpoint));
                response
            }
        };
        if self.config.audit_log {
            let method = std::str::from_utf8(&request.body).ok().and_then(|b| json_lite::parse(b).ok()).and_then(|v| v.get("method").and_then(JsonValue::as_str).map(str::to_string)).unwrap_or_default();
            let line = audit_line(&crate::ip_filter::client_ip(request, &connection.client.ip), &method, &self.config.upstream, response.status_code, started.elapsed());
            crate::log::access_sink::write_line(|out| out.extend_from_slice(line.as_bytes()));
        }
        response
    }
}

fn unavailable(reason: &str) -> Response {
    let mut response = crate::error::problem(STATUS_CODE_REASON_PHRASE.n503_service_unavailable, reason);
    response.headers.push(crate::header::Header::new("Retry-After", "10".to_string()));
    response
}

/// One audit log line (without the newline).
fn audit_line(client: &str, method: &str, upstream: &str, status: i16, duration: Duration) -> String {
    let mut time = String::new();
    crate::log::iso8601_now(|iso| time.push_str(iso));
    let q = crate::a2a::quote_json;
    format!(
        r#"{{"time":"{time}","event":"a2a_call","client":{},"method":{},"upstream":{},"status":{status},"duration_ms":{:.3}}}"#,
        q(client),
        q(method),
        q(upstream),
        duration.as_secs_f64() * 1000.0
    )
}

impl Application for A2aGateway {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let path = crate::url::URL::request_target_path(&request.request_uri).unwrap_or("");
        let rest = path.strip_prefix(self.prefix.as_str()).unwrap_or(path);
        if rest == crate::a2a::AGENT_CARD_PATH && (request.method == "GET" || request.method == "HEAD") {
            return Ok(self.public_card(request, connection));
        }
        if rest.is_empty() || rest == "/" {
            if request.method == "POST" {
                return Ok(self.forward(request, connection));
            }
            let mut response = crate::error::problem(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed, "A2A requests are JSON-RPC POSTs");
            response.headers.push(crate::header::Header::new("Allow", "POST".to_string()));
            return Ok(response);
        }
        Ok(crate::error::problem(STATUS_CODE_REASON_PHRASE.n404_not_found, ""))
    }
}

#[cfg(test)]
mod tests;
