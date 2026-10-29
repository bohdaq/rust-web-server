//! An LLM proxy (18.9.0, #41): one endpoint in front of OpenAI-compatible
//! and Anthropic providers that counts tokens, enforces budgets and fails
//! over between providers.
//!
//! ```rust,no_run
//! use rust_web_server::llm::{Api, LlmProxy, Provider};
//!
//! let proxy = LlmProxy::new()
//!     .provider(Provider::new("openai", Api::OpenAi, "https://api.openai.com").unwrap().api_key("sk-…"))
//!     .provider(Provider::new("azure", Api::OpenAi, "https://example.openai.azure.com").unwrap().api_key("…"))
//!     .provider(Provider::new("anthropic", Api::Anthropic, "https://api.anthropic.com").unwrap().api_key("sk-ant-…"))
//!     .virtual_key("vk-alice", "alice")
//!     .budget("alice", 2_000_000, 86_400);
//! // `proxy` is an `Application`: serve it, or mount it on a route.
//! ```
//!
//! - **Which API.** A request to a path ending in `/messages` is Anthropic's
//!   Messages API; anything else (`/v1/chat/completions`, `/v1/responses`,
//!   `/v1/embeddings`, `/v1/models`, …) is OpenAI's. It goes to the
//!   providers of that API, in the order added; the path is sent as is,
//!   after the provider URL's own path.
//! - **Virtual keys.** Clients authenticate with a key the proxy issued
//!   (`Authorization: Bearer <key>` or `x-api-key: <key>`, as each SDK sends
//!   it); it names an identity and never reaches a provider, which gets its
//!   own key instead (`Authorization: Bearer` for OpenAI, `x-api-key` and a
//!   default `anthropic-version` for Anthropic). Unknown keys get `401`.
//! - **Fallback.** A provider that doesn't answer, or answers `429` or
//!   `5xx`, is skipped for the next one of the same API — before anything
//!   was sent to the client, so a stream never switches provider midway.
//! - **Usage.** Tokens come from the provider's response, JSON or event
//!   stream ([`usage`]). For OpenAI streams the proxy adds
//!   `"stream_options":{"include_usage":true}` when the request has no
//!   `stream_options`, so the stream ends with one extra chunk with empty
//!   `choices` that carries the usage.
//! - **Budgets.** Tokens per identity per fixed window, in memory or shared
//!   between replicas through Redis ([`LlmProxy::redis`]). Checked before a
//!   request and charged after it with what the provider reported, so the
//!   request that crosses the budget completes; the next one gets `429`.
//!   Redis errors are logged and let requests through (fail open).
//! - **Metrics.** `rws_llm_requests_total`, `rws_llm_tokens_total`,
//!   `rws_llm_time_to_first_token_seconds`, `rws_llm_fallbacks_total`,
//!   `rws_llm_budget_rejections_total` on `/metrics`; with
//!   [`LlmProxy::audit_log`] one JSON line per request in the access log.

pub mod usage;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::application::Application;
use crate::header::Header;
use crate::mcp::json_rpc;
use crate::mcp::json_escape;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

pub use usage::{Usage, UsageParser};

/// The API a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Api {
    /// OpenAI's (and every OpenAI-compatible server's): Chat Completions,
    /// Responses, embeddings, models.
    OpenAi,
    /// Anthropic's Messages API.
    Anthropic,
}

impl Api {
    /// `"openai"` or `"anthropic"`, as the config file spells it.
    pub fn parse(name: &str) -> Option<Api> {
        match name.to_ascii_lowercase().as_str() {
            "openai" => Some(Api::OpenAi),
            "anthropic" => Some(Api::Anthropic),
            _ => None,
        }
    }

    /// The API a request path belongs to.
    pub fn of_path(path: &str) -> Api {
        let path = path.split(['?', '#']).next().unwrap_or_default().trim_end_matches('/');
        if path.ends_with("/messages") || path.ends_with("/messages/count_tokens") {
            Api::Anthropic
        } else {
            Api::OpenAi
        }
    }
}

/// One upstream provider.
#[derive(Clone)]
pub struct Provider {
    name: String,
    api: Api,
    tls: bool,
    host: String,
    port: u16,
    base_path: String,
    api_key: String,
}

impl std::fmt::Debug for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Provider")
            .field("name", &self.name)
            .field("api", &self.api)
            .field("url", &format!("{}://{}:{}{}", if self.tls { "https" } else { "http" }, self.host, self.port, self.base_path))
            .finish_non_exhaustive()
    }
}

impl Provider {
    /// `url` is `http(s)://host[:port][/path]`; request paths go after its
    /// path (`https://api.openai.com` + `/v1/chat/completions`).
    pub fn new(name: impl Into<String>, api: Api, url: &str) -> Result<Provider, String> {
        let (tls, rest) = if let Some(rest) = url.strip_prefix("https://") {
            (true, rest)
        } else if let Some(rest) = url.strip_prefix("http://") {
            (false, rest)
        } else {
            return Err(format!("provider url {url:?} must start with http:// or https://"));
        };
        let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if !host.ends_with(']') || authority.starts_with('[') => {
                (host, port.parse::<u16>().map_err(|_| format!("provider url {url:?}: bad port"))?)
            }
            _ => (authority, if tls { 443 } else { 80 }),
        };
        let host = host.trim_start_matches('[').trim_end_matches(']');
        if host.is_empty() {
            return Err(format!("provider url {url:?} has no host"));
        }
        Ok(Provider {
            name: name.into(),
            api,
            tls,
            host: host.to_string(),
            port,
            base_path: path.trim_end_matches('/').to_string(),
            api_key: String::new(),
        })
    }

    /// The provider's own API key.
    pub fn api_key(mut self, key: impl Into<String>) -> Provider {
        self.api_key = key.into();
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Copy)]
struct Budget {
    tokens: u64,
    window_secs: u64,
}

/// Where spent tokens are counted.
enum Store {
    Memory(Mutex<HashMap<String, (u64, u64)>>),
    #[cfg(not(target_arch = "wasm32"))]
    Redis { conn: crate::redis_protocol::RespConn, prefix: String },
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Store {
    /// Tokens `identity` spent in the current window.
    fn spent(&self, identity: &str, window_secs: u64) -> u64 {
        let window = now_secs() / window_secs;
        match self {
            Store::Memory(map) => match map.lock().unwrap_or_else(|e| e.into_inner()).get(identity) {
                Some((w, spent)) if *w == window => *spent,
                _ => 0,
            },
            #[cfg(not(target_arch = "wasm32"))]
            Store::Redis { conn, prefix } => {
                let key = format!("{prefix}{identity}:{window_secs}:{window}");
                match conn.cmd(&[b"GET", key.as_bytes()]) {
                    Ok(crate::redis_protocol::RespReply::Bulk(Some(v))) => std::str::from_utf8(&v).ok().and_then(|s| s.parse().ok()).unwrap_or(0),
                    Ok(_) => 0,
                    Err(e) => {
                        eprintln!("llm proxy: budget read from Redis failed ({e}); letting the request through");
                        0
                    }
                }
            }
        }
    }

    fn charge(&self, identity: &str, window_secs: u64, tokens: u64) {
        if tokens == 0 {
            return;
        }
        let window = now_secs() / window_secs;
        match self {
            Store::Memory(map) => {
                let mut map = map.lock().unwrap_or_else(|e| e.into_inner());
                let entry = map.entry(identity.to_string()).or_insert((window, 0));
                if entry.0 != window {
                    *entry = (window, 0);
                }
                entry.1 = entry.1.saturating_add(tokens);
            }
            #[cfg(not(target_arch = "wasm32"))]
            Store::Redis { conn, prefix } => {
                let key = format!("{prefix}{identity}:{window_secs}:{window}");
                let n = tokens.to_string();
                match conn.cmd(&[b"INCRBY", key.as_bytes(), n.as_bytes()]) {
                    // The first charge in this window: let the key expire with it.
                    Ok(crate::redis_protocol::RespReply::Int(total)) if total as u64 == tokens => {
                        let ttl = (window_secs + 60).to_string();
                        if let Err(e) = conn.cmd(&[b"EXPIRE", key.as_bytes(), ttl.as_bytes()]) {
                            eprintln!("llm proxy: setting the budget key's expiry in Redis failed: {e}");
                        }
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("llm proxy: charging {tokens} tokens to {identity} in Redis failed: {e}"),
                }
            }
        }
    }
}

struct Inner {
    providers: Vec<Provider>,
    /// (virtual key, identity)
    keys: Vec<(String, String)>,
    anonymous: bool,
    budgets: HashMap<String, Budget>,
    store: Store,
    audit_log: bool,
    connect_timeout: Duration,
    read_timeout: Duration,
}

/// The LLM proxy, an [`Application`]. See the [module docs](self).
#[derive(Clone)]
pub struct LlmProxy {
    inner: Arc<Inner>,
}

impl Default for LlmProxy {
    fn default() -> Self {
        LlmProxy::new()
    }
}

impl LlmProxy {
    pub fn new() -> LlmProxy {
        LlmProxy {
            inner: Arc::new(Inner {
                providers: Vec::new(),
                keys: Vec::new(),
                anonymous: false,
                budgets: HashMap::new(),
                store: Store::Memory(Mutex::default()),
                audit_log: false,
                connect_timeout: Duration::from_secs(10),
                read_timeout: Duration::from_secs(300),
            }),
        }
    }

    fn edit(mut self, f: impl FnOnce(&mut Inner)) -> LlmProxy {
        match Arc::get_mut(&mut self.inner) {
            Some(inner) => f(inner),
            None => panic!("LlmProxy: configure it before cloning"),
        }
        self
    }

    /// A provider; tried in the order added, per API.
    pub fn provider(self, provider: Provider) -> LlmProxy {
        self.edit(|i| i.providers.push(provider))
    }

    /// A key clients send, and the identity it stands for.
    pub fn virtual_key(self, key: impl Into<String>, identity: impl Into<String>) -> LlmProxy {
        self.edit(|i| i.keys.push((key.into(), identity.into())))
    }

    /// Let requests without a known key through, as identity `anonymous`
    /// (a proxy behind its own authentication).
    pub fn allow_anonymous(self) -> LlmProxy {
        self.edit(|i| i.anonymous = true)
    }

    /// At most `tokens` (input + output) per `window_secs` for `identity`.
    pub fn budget(self, identity: impl Into<String>, tokens: u64, window_secs: u64) -> LlmProxy {
        let budget = Budget { tokens, window_secs: window_secs.max(1) };
        self.edit(|i| {
            i.budgets.insert(identity.into(), budget);
        })
    }

    /// Count budgets in Redis at `addr` (`host:port`), under keys starting
    /// with `prefix`, so every replica sees the same spend.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn redis(self, addr: impl Into<String>, password: Option<String>, prefix: impl Into<String>) -> LlmProxy {
        let store = Store::Redis { conn: crate::redis_protocol::RespConn::new(addr, password), prefix: prefix.into() };
        self.edit(|i| i.store = store)
    }

    /// One JSON line per request (identity, provider, model, status, tokens,
    /// time to first byte, duration) through the access log.
    pub fn audit_log(self, on: bool) -> LlmProxy {
        self.edit(|i| i.audit_log = on)
    }

    /// How long to wait for a provider's connection, and for each read of
    /// its response (default 10 s and 300 s).
    pub fn timeouts(self, connect: Duration, read: Duration) -> LlmProxy {
        self.edit(|i| {
            i.connect_timeout = connect;
            i.read_timeout = read;
        })
    }

    fn identity(&self, request: &Request) -> Option<String> {
        let sent = request
            .header("Authorization")
            .and_then(|h| h.value.strip_prefix("Bearer ").map(|t| t.trim().to_string()))
            .or_else(|| request.header("x-api-key").map(|h| h.value.trim().to_string()));
        if let Some(sent) = sent.filter(|s| !s.is_empty()) {
            let mut found = None;
            for (key, identity) in &self.inner.keys {
                if crate::proxy_config::constant_time_eq(key.as_bytes(), sent.as_bytes()) {
                    found = Some(identity.clone());
                }
            }
            if found.is_some() {
                return found;
            }
        }
        self.inner.anonymous.then(|| "anonymous".to_string())
    }
}

fn error(status: &crate::response::StatusCodeReasonPhrase, kind: &str, message: &str) -> Response {
    let body = format!(r#"{{"type":"error","error":{{"type":"{kind}","message":"{}"}}}}"#, json_escape(message));
    Response::json(status, body.into_bytes())
}

/// What the request asked for, read at the top level of its JSON body.
struct Asked {
    model: String,
    stream: bool,
    has_stream_options: bool,
}

fn asked(body: &[u8]) -> Asked {
    let members = std::str::from_utf8(body).ok().and_then(json_rpc::object_members).unwrap_or_default();
    let mut asked = Asked { model: String::new(), stream: false, has_stream_options: false };
    for (key, value) in members {
        match key.as_str() {
            "model" if value.starts_with('"') => {
                asked.model = json_rpc::extract_str(&format!(r#"{{"v":{value}}}"#), "v").unwrap_or_default();
            }
            "stream" => asked.stream = value == "true",
            "stream_options" => asked.has_stream_options = true,
            _ => {}
        }
    }
    asked
}

/// `body` with `"stream_options":{"include_usage":true}` as its first member.
fn with_usage_option(body: &[u8]) -> Option<Vec<u8>> {
    let start = body.iter().position(|b| !b.is_ascii_whitespace())?;
    if body[start] != b'{' {
        return None;
    }
    let rest = &body[start + 1..];
    let empty = rest.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'}');
    let mut out = Vec::with_capacity(body.len() + 40);
    out.extend_from_slice(b"{\"stream_options\":{\"include_usage\":true}");
    if !empty {
        out.push(b',');
    }
    out.extend_from_slice(rest);
    Some(out)
}

/// A request done: what to record once its body has been read.
struct Done {
    inner: Arc<Inner>,
    provider: String,
    model: String,
    identity: String,
    status: u16,
    started: Instant,
}

impl Done {
    fn record(self, usage: Option<Usage>, ttft: Option<Duration>) {
        let ok = (200..300).contains(&self.status);
        let usage = usage.filter(|_| ok);
        crate::metrics::llm::observe(&self.provider, &self.model, &self.identity, &self.status.to_string(), usage, ttft);
        if let (Some(usage), Some(budget)) = (usage, self.inner.budgets.get(&self.identity)) {
            self.inner.store.charge(&self.identity, budget.window_secs, usage.total());
        }
        if self.inner.audit_log {
            let mut time = String::new();
            #[cfg(not(target_arch = "wasm32"))]
            crate::log::iso8601_now(|iso| time.push_str(iso));
            let usage = usage.unwrap_or_default();
            let line = format!(
                r#"{{"time":"{time}","event":"llm_request","sub":"{}","provider":"{}","model":"{}","status":{},"input_tokens":{},"output_tokens":{},"ttft_ms":{},"duration_ms":{:.3}}}"#,
                json_escape(&self.identity),
                json_escape(&self.provider),
                json_escape(&self.model),
                self.status,
                usage.input_tokens,
                usage.output_tokens,
                ttft.map_or("null".to_string(), |t| format!("{:.3}", t.as_secs_f64() * 1000.0)),
                self.started.elapsed().as_secs_f64() * 1000.0,
            );
            #[cfg(not(target_arch = "wasm32"))]
            crate::log::access_sink::write_line(|out| out.extend_from_slice(line.as_bytes()));
            #[cfg(target_arch = "wasm32")]
            let _ = line;
        }
    }
}

/// Headers of the client's request that never reach a provider.
fn hidden(name: &str) -> bool {
    ["authorization", "x-api-key", "accept-encoding", "content-length"].iter().any(|h| name.eq_ignore_ascii_case(h))
}

fn upstream_request(request: &Request, provider: &Provider, body: &[u8]) -> Request {
    let mut out = Request {
        method: request.method.clone(),
        request_uri: format!("{}{}", provider.base_path, request.request_uri),
        http_version: request.http_version.clone(),
        headers: request.headers.iter().filter(|h| !hidden(&h.name)).cloned().collect(),
        body: body.to_vec().into(),
    };
    match provider.api {
        Api::OpenAi => out.headers.push(Header::new("Authorization", format!("Bearer {}", provider.api_key))),
        Api::Anthropic => {
            out.headers.push(Header::new("x-api-key", provider.api_key.clone()));
            if request.header("anthropic-version").is_none() {
                out.headers.push(Header::new("anthropic-version", "2023-06-01"));
            }
        }
    }
    if !body.is_empty() || request.method.eq_ignore_ascii_case("POST") || request.method.eq_ignore_ascii_case("PUT") {
        out.headers.push(Header::new("Content-Length", body.len().to_string()));
    }
    out
}

fn content_type(response: &Response) -> String {
    response
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("content-type"))
        .map(|h| h.value.to_string())
        .or_else(|| response.content_range_list.first().map(|p| p.content_type.clone()))
        .unwrap_or_default()
}

impl Application for LlmProxy {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let Some(identity) = self.identity(request) else {
            return Ok(error(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "authentication_error", "invalid or missing API key"));
        };
        if let Some(budget) = self.inner.budgets.get(&identity) {
            if self.inner.store.spent(&identity, budget.window_secs) >= budget.tokens {
                crate::metrics::llm::observe_budget_rejection(&identity);
                let mut response = error(STATUS_CODE_REASON_PHRASE.n429_too_many_requests, "rate_limit_error", "token budget spent for this window");
                let retry = budget.window_secs - now_secs() % budget.window_secs;
                response.headers.push(Header::new("Retry-After", retry.to_string()));
                return Ok(response);
            }
        }
        let api = Api::of_path(&request.request_uri);
        let providers: Vec<&Provider> = self.inner.providers.iter().filter(|p| p.api == api).collect();
        if providers.is_empty() {
            return Ok(error(STATUS_CODE_REASON_PHRASE.n404_not_found, "not_found_error", "no provider serves this API"));
        }

        let asked = asked(&request.body);
        let rewritten = (api == Api::OpenAi && asked.stream && !asked.has_stream_options).then(|| with_usage_option(&request.body)).flatten();
        let body: &[u8] = rewritten.as_deref().unwrap_or(&request.body);

        let mut last_error = String::new();
        for (i, provider) in providers.iter().enumerate() {
            let last = i + 1 == providers.len();
            let started = Instant::now();
            let upstream = upstream_request(request, provider, body);
            let result = crate::proxy::forward_upstream_trusting(
                crate::proxy::shared_pool(),
                &upstream,
                crate::proxy::Peer::of(connection),
                &provider.host,
                provider.port,
                provider.tls,
                None,
                self.inner.connect_timeout,
                self.inner.read_timeout,
            );
            let response = match result {
                Ok(response) if last || !(response.status_code == 429 || response.status_code >= 500) => response,
                Ok(response) => {
                    last_error = format!("{} answered {}", provider.name, response.status_code);
                    crate::metrics::llm::observe(&provider.name, &asked.model, &identity, &response.status_code.to_string(), None, None);
                    crate::metrics::llm::observe_fallback(&provider.name);
                    continue;
                }
                Err(e) => {
                    last_error = format!("{}: {e}", provider.name);
                    crate::metrics::llm::observe(&provider.name, &asked.model, &identity, "error", None, None);
                    if !last {
                        crate::metrics::llm::observe_fallback(&provider.name);
                    }
                    continue;
                }
            };
            return Ok(self.finish(response, provider, &asked.model, &identity, started));
        }
        eprintln!("llm proxy: no provider answered ({last_error})");
        Ok(error(STATUS_CODE_REASON_PHRASE.n502_bad_gateway, "api_error", "no provider answered"))
    }
}

impl LlmProxy {
    /// Count the response's usage: now for a buffered body, as it streams
    /// for a piped one.
    fn finish(&self, mut response: Response, provider: &Provider, model: &str, identity: &str, started: Instant) -> Response {
        let done = Done {
            inner: Arc::clone(&self.inner),
            provider: provider.name.clone(),
            model: model.to_string(),
            identity: identity.to_string(),
            status: response.status_code as u16,
            started,
        };
        let content_type = content_type(&response);
        match response.stream_pipe.take() {
            Some(pipe) => {
                let chunked = response
                    .headers
                    .iter()
                    .any(|h| h.name.eq_ignore_ascii_case("transfer-encoding") && h.value.to_ascii_lowercase().contains("chunked"));
                // The reader removes the framing; the server frames it again.
                response.headers.retain(|h| !h.name.eq_ignore_ascii_case("transfer-encoding"));
                let reader = usage::UsageReader::new(pipe, &content_type, chunked, started, Box::new(move |usage, ttft| done.record(usage, ttft)));
                response.stream_pipe = Some(Box::new(reader));
            }
            None => {
                let ttft = started.elapsed();
                let mut parser = UsageParser::new(&content_type);
                for part in &response.content_range_list {
                    parser.feed(&part.body);
                }
                done.record(parser.finish(), Some(ttft));
            }
        }
        response
    }
}

#[cfg(test)]
mod tests;
