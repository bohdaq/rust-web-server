//! MCP 2026-07-28 ("modern") requests, served beside the earlier versions
//! on the same endpoint. A modern request is self-contained: no
//! `initialize`, no session — its protocol version, client capabilities
//! and identity ride in `params._meta`, mirrored in HTTP headers
//! (`MCP-Protocol-Version`, `Mcp-Method`, `Mcp-Name`) that must agree with
//! the body. A request is modern when its `_meta` names a protocol version
//! or its `MCP-Protocol-Version` header isn't one of the earlier versions;
//! anything else (an `initialize`, a session's requests) keeps the earlier
//! behaviour.
//!
//! What changes on the modern path, beside validation (`-32602` for missing
//! `_meta` fields, `-32020` for headers that disagree, `-32022` for a
//! version this server doesn't speak — all `400`):
//!
//! - `server/discover` answers the versions, capabilities and identity;
//!   `initialize`, `ping`, `logging/setLevel`, `resources/(un)subscribe`
//!   and unknown methods are `404` with `-32601`.
//! - Every result carries `resultType` and `_meta.serverInfo`; cacheable
//!   ones `ttlMs` and `cacheScope` ([`McpServer::cache_hints`]).
//! - Handlers ask the client for input by returning [`InputRequired`]
//!   (multi round-trip requests) instead of sending it requests; the
//!   client retries with `inputResponses` and the `requestState`, which
//!   is HMAC-signed here — bound to the method, the tool or prompt, the
//!   caller and ten minutes — and refused (`-32602`) when altered.
//! - `subscriptions/listen` replaces the `GET` stream: an acknowledgment
//!   first, then only the notification types asked for, each tagged with
//!   the subscription's id.
//! - Notifications about a request (progress, log messages — the latter
//!   only when its `_meta` sets a `logLevel`) go on that request's own
//!   response stream.

use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::Arc;

use super::{
    base64_decode, base64_encode, icons_json, json_escape, json_rpc, LogLevel, McpContext, McpServer,
    PromptMessage, Response, SamplingRequest, SUPPORTED_PROTOCOL_VERSIONS,
};
use crate::core::New;
use crate::header::Header;
use crate::request::Request;
use crate::response::STATUS_CODE_REASON_PHRASE;

/// The protocol versions served statelessly.
pub(crate) const MODERN_VERSIONS: [&str; 1] = ["2026-07-28"];

/// The `_meta` keys of a modern request.
const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
const META_LOG_LEVEL: &str = "io.modelcontextprotocol/logLevel";

const HEADER_MISMATCH: i32 = -32020;
const MISSING_CLIENT_CAPABILITY: i32 = -32021;
const UNSUPPORTED_PROTOCOL_VERSION: i32 = -32022;

/// How long a `requestState` stays valid.
const STATE_TTL_SECS: u64 = 600;

/// What a modern request carries that a handler may read (through
/// [`McpContext`]).
#[derive(Debug, Default)]
pub(crate) struct ModernRequest {
    /// `_meta.io.modelcontextprotocol/clientCapabilities`, raw JSON.
    client_capabilities: String,
    /// `params.inputResponses`, raw JSON.
    input_responses: Option<String>,
    /// The `requestState` this server issued, verified.
    request_state: Option<String>,
    /// `_meta.io.modelcontextprotocol/logLevel`: no log messages without it.
    log_level: Option<LogLevel>,
    /// The client declared the tasks extension.
    tasks_extension: bool,
}

/// A handler's request for input from the client (MCP 2026-07-28 multi
/// round-trip requests): the client answers each request and calls again,
/// the answers then in [`McpContext::input_response`] under the same keys,
/// and [`state`](Self::state) in [`McpContext::request_state`].
///
/// ```rust
/// use rust_web_server::mcp::{InputRequired, McpContent, McpServer};
///
/// let server = McpServer::new("greeter", "1.0").tool_with_context("greet", "Greets you", "{}", |ctx, _args| {
///     match ctx.input_response("name") {
///         Some(answer) => Ok(McpContent::text(format!("Hello! (you said {answer})"))),
///         None => Ok(McpContent::input_required(InputRequired::new().elicit(
///             "name",
///             "What is your name?",
///             r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#,
///         ))),
///     }
/// });
/// ```
#[derive(Debug, Clone, Default)]
pub struct InputRequired {
    /// `(key, method, params JSON)`.
    requests: Vec<(String, &'static str, String)>,
    state: Option<String>,
}

impl InputRequired {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask the user to fill in a form: `requested_schema` is a JSON Schema
    /// object of primitive properties (`elicitation/create`, form mode).
    /// Needs the client's `elicitation` capability.
    pub fn elicit(mut self, key: &str, message: &str, requested_schema: &str) -> Self {
        let params = format!(r#"{{"mode":"form","message":"{}","requestedSchema":{}}}"#, json_escape(message), requested_schema);
        self.requests.push((key.to_string(), "elicitation/create", params));
        self
    }

    /// Ask the client's LLM (`sampling/createMessage`). Needs the client's
    /// `sampling` capability.
    pub fn sample(mut self, key: &str, request: &SamplingRequest) -> Self {
        let messages: Vec<String> = request.messages.iter().map(PromptMessage::to_json).collect();
        let mut params = format!(r#"{{"messages":[{}],"maxTokens":{}"#, messages.join(","), request.max_tokens);
        if let Some(system) = &request.system_prompt {
            params.push_str(&format!(r#","systemPrompt":"{}""#, json_escape(system)));
        }
        params.push('}');
        self.requests.push((key.to_string(), "sampling/createMessage", params));
        self
    }

    /// Ask for the client's roots (`roots/list`). Needs the client's
    /// `roots` capability.
    pub fn list_roots(mut self, key: &str) -> Self {
        self.requests.push((key.to_string(), "roots/list", "{}".to_string()));
        self
    }

    /// Opaque state for the retry, echoed back by the client: signed by
    /// the server, so the handler gets back exactly what it set.
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// The requests as `(key, {"method":...,"params":...})`.
    pub(crate) fn requests_json(&self) -> Vec<(String, String)> {
        self.requests.iter().map(|(key, method, params)| (key.clone(), format!(r#"{{"method":"{method}","params":{params}}}"#))).collect()
    }

    /// `{"inputRequests":{...},"requestState":"<unsigned>"}` — how it rides
    /// inside an [`McpContent`](super::McpContent) to the server.
    pub(crate) fn to_json(&self) -> String {
        let requests: Vec<String> = self
            .requests
            .iter()
            .map(|(key, method, params)| format!(r#""{}":{{"method":"{method}","params":{params}}}"#, json_escape(key)))
            .collect();
        let mut json = format!(r#"{{"inputRequests":{{{}}}"#, requests.join(","));
        if let Some(state) = &self.state {
            json.push_str(&format!(r#","requestState":"{}""#, json_escape(state)));
        }
        json.push('}');
        json
    }
}

/// What a prompt handler registered with
/// [`McpServer::prompt_with_context`] answers.
#[derive(Debug, Clone)]
pub enum PromptReply {
    Messages(Vec<PromptMessage>),
    /// Input from the client first (MCP 2026-07-28).
    InputRequired(InputRequired),
}

impl From<Vec<PromptMessage>> for PromptReply {
    fn from(messages: Vec<PromptMessage>) -> Self {
        PromptReply::Messages(messages)
    }
}

impl McpContext {
    /// The client's answer to the input request `key` (MCP 2026-07-28), as
    /// raw JSON — an `ElicitResult` (`{"action":"accept","content":{...}}`),
    /// a `CreateMessageResult` or a `ListRootsResult`. `None` on the first
    /// call, for a key the client didn't answer, and on earlier versions.
    pub fn input_response(&self, key: &str) -> Option<String> {
        let responses = self.modern.as_ref()?.input_responses.as_deref()?;
        member(&members(responses), key).map(str::to_string)
    }

    /// What the user submitted for the elicitation `key`, as a raw JSON
    /// object — when they accepted it; `None` when they declined,
    /// cancelled, or haven't answered.
    pub fn elicitation_content(&self, key: &str) -> Option<String> {
        let answer = members(&self.input_response(key)?);
        (member_str(&answer, "action").as_deref() == Some("accept")).then(|| member(&answer, "content").unwrap_or("{}").to_string())
    }

    /// The state the handler set on its [`InputRequired`], when the client
    /// retried with it — verified, so exactly what was set.
    pub fn request_state(&self) -> Option<&str> {
        self.modern.as_ref()?.request_state.as_deref()
    }

    /// Whether the client declared `capability` (`"elicitation"`,
    /// `"sampling"`, `"roots"`, ...): from this request's `_meta` (MCP
    /// 2026-07-28), else from the session's `initialize` (`sampling` and
    /// `roots` only).
    pub fn client_supports(&self, capability: &str) -> bool {
        match &self.modern {
            Some(modern) => member(&members(&modern.client_capabilities), capability).is_some(),
            None => match capability {
                "sampling" => self.sampling_supported,
                "roots" => self.roots_supported,
                _ => false,
            },
        }
    }

    /// Whether this request uses MCP 2026-07-28 (stateless; input through
    /// [`InputRequired`]).
    pub fn is_modern(&self) -> bool {
        self.modern.is_some()
    }

    /// Send a `notifications/message` about this request on its response
    /// stream (MCP 2026-07-28): only when the request's `_meta` set a
    /// `logLevel` at or below `level` and the client accepts a stream.
    /// `data_json` is any JSON value. On earlier versions use
    /// [`McpServer::log`].
    pub fn log(&self, level: LogLevel, data_json: &str) {
        let (Some(modern), Some(stream)) = (&self.modern, &self.request_stream) else { return };
        if modern.log_level.is_some_and(|min| level >= min) {
            let params = format!(r#"{{"level":"{}","data":{data_json}}}"#, level.as_str());
            let _ = stream.try_send(format!("data: {}\n\n", super::render_notification("notifications/message", Some(&params))).into_bytes());
        }
    }
}

// ── request parsing ───────────────────────────────────────────────────────────

fn members(json: &str) -> Vec<(String, String)> {
    json_rpc::object_members(json).unwrap_or_default()
}

fn member<'a>(members: &'a [(String, String)], key: &str) -> Option<&'a str> {
    members.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// A JSON string member's value, unescaped.
fn member_str(members: &[(String, String)], key: &str) -> Option<String> {
    member(members, key).filter(|v| v.starts_with('"')).and_then(|v| json_rpc::extract_str(&format!(r#"{{"v":{v}}}"#), "v"))
}

fn header(request: &Request, name: &str) -> Option<String> {
    request.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.to_string())
}

/// `params._meta`'s protocol version, if any.
fn meta_version(body: &str) -> Option<String> {
    let body = members(body);
    let params = members(member(&body, "params")?);
    member_str(&members(member(&params, "_meta")?), META_VERSION)
}

/// Whether `request` is an MCP 2026-07-28 (or later) request: its `_meta`
/// names a protocol version, or its `MCP-Protocol-Version` header is not
/// one of the session-based versions.
pub(crate) fn is_modern(request: &Request, body: &str) -> bool {
    let header_version = header(request, "mcp-protocol-version").map(|v| v.trim().to_string());
    if header_version.as_deref().is_some_and(|v| !SUPPORTED_PROTOCOL_VERSIONS.contains(&v)) {
        return true;
    }
    header_version.is_none() && meta_version(body).is_some()
}

/// A header value as sent, or decoded from `=?base64?...?=`; `None` when
/// it has characters a header value can't, or isn't canonical base64
/// (padding included).
fn header_value(raw: &str) -> Option<String> {
    if raw.bytes().any(|b| !(b == b'\t' || (0x20..=0x7e).contains(&b))) {
        return None;
    }
    match raw.strip_prefix("=?base64?").and_then(|rest| rest.strip_suffix("?=")) {
        Some(encoded) => {
            let decoded = base64_decode(encoded).filter(|bytes| base64_encode(bytes) == encoded)?;
            String::from_utf8(decoded).ok()
        }
        None => Some(raw.to_string()),
    }
}

/// The parameters of a tool's input schema mirrored into headers
/// (`x-mcp-header`): the property path and the header name's suffix. Only
/// properties reached through `properties` keys count.
pub(crate) fn header_params(schema: &str) -> Vec<(Vec<String>, String)> {
    let mut found = Vec::new();
    // (path so far, schema object), depth-bounded.
    let mut pending: Vec<(Vec<String>, String)> = vec![(Vec::new(), schema.to_string())];
    while let Some((path, object)) = pending.pop() {
        if path.len() > 32 {
            continue;
        }
        let Some(properties) = member(&members(&object), "properties").map(members) else { continue };
        for (key, sub) in properties {
            let mut at = path.clone();
            at.push(key);
            let sub_members = members(&sub);
            if let Some(name) = member_str(&sub_members, "x-mcp-header") {
                found.push((at.clone(), name));
            }
            pending.push((at, sub));
        }
    }
    found
}

/// The raw JSON value at `path` in `arguments`.
fn value_at(arguments: &str, path: &[String]) -> Option<String> {
    let mut value = arguments.to_string();
    for key in path {
        value = member(&members(&value), key)?.to_string();
    }
    Some(value)
}

/// Whether the header text `header` stands for the JSON value `raw`:
/// strings as is, booleans as `true`/`false`, numbers compared as numbers.
fn header_matches(raw: &str, header: &str) -> bool {
    if raw.starts_with('"') {
        return json_rpc::extract_str(&format!(r#"{{"v":{raw}}}"#), "v").as_deref() == Some(header);
    }
    if raw == "true" || raw == "false" {
        return raw == header;
    }
    matches!((raw.parse::<f64>(), header.parse::<f64>()), (Ok(a), Ok(b)) if a == b)
}

/// `tools/call`: every `Mcp-Param-{name}` header a tool's schema asks for
/// must be present when its argument has a value, and match it.
fn check_param_headers(request: &Request, schema: &str, arguments: &str) -> Result<(), ModernError> {
    for (path, name) in header_params(schema) {
        let header_name = format!("Mcp-Param-{name}");
        let sent = match header(request, &header_name) {
            Some(raw) => Some(header_value(&raw).ok_or_else(|| {
                ModernError::bad_request(HEADER_MISMATCH, format!("Header mismatch: {header_name} isn't a valid header value (or valid =?base64?...?=)"))
            })?),
            None => None,
        };
        match (value_at(arguments, &path).filter(|v| v != "null"), sent) {
            (Some(_), None) => {
                return Err(ModernError::bad_request(HEADER_MISMATCH, format!("Header mismatch: {header_name} is required, the argument {} has a value", path.join("."))))
            }
            (Some(value), Some(sent)) if !header_matches(&value, &sent) => {
                return Err(ModernError::bad_request(HEADER_MISMATCH, format!("Header mismatch: {header_name} {sent:?} doesn't match the argument {}", path.join("."))))
            }
            _ => {}
        }
    }
    Ok(())
}

/// An error answered on the modern path: a JSON-RPC error with `data`, and
/// the HTTP status the transport prescribes.
struct ModernError {
    code: i32,
    message: String,
    data: Option<String>,
    status: u16,
}

impl ModernError {
    fn new(code: i32, message: impl Into<String>) -> Self {
        ModernError { code, message: message.into(), data: None, status: 200 }
    }

    fn bad_request(code: i32, message: impl Into<String>) -> Self {
        ModernError { status: 400, ..Self::new(code, message) }
    }

    fn data(mut self, data: String) -> Self {
        self.data = Some(data);
        self
    }
}

impl From<(i32, String)> for ModernError {
    fn from((code, message): (i32, String)) -> Self {
        ModernError::new(code, message)
    }
}

/// A modern result: JSON to decorate, and whether it is cacheable.
enum Answer {
    Complete { result: String, cacheable: bool },
    InputRequired(String),
    /// A task was started: its id.
    Task(String),
}

/// `-32021` naming the tasks extension.
fn tasks_extension_required() -> ModernError {
    ModernError::bad_request(MISSING_CLIENT_CAPABILITY, "Missing required client capability: the io.modelcontextprotocol/tasks extension")
        .data(format!(r#"{{"requiredCapabilities":{{"extensions":{{"{}":{{}}}}}}}}"#, super::tasks::EXTENSION))
}

/// One `subscriptions/listen` stream.
pub(crate) struct Listener {
    /// The listen request's JSON-RPC id, raw: its notifications' tag.
    id: String,
    tools: bool,
    prompts: bool,
    resources: bool,
    uris: Vec<String>,
    /// Tasks whose status changes it gets (`notifications/tasks`).
    task_ids: Vec<String>,
    tx: SyncSender<Vec<u8>>,
}

/// Insert `fields` (already rendered, comma-separated) at the start of the
/// JSON object `object`.
fn with_fields(object: &str, fields: &str) -> String {
    let inner = object.trim().strip_prefix('{').unwrap_or("}").trim_start();
    if inner.starts_with('}') {
        format!("{{{fields}}}")
    } else {
        format!("{{{fields},{inner}")
    }
}

fn status_response(status: u16, body: String) -> Response {
    let mut response = super::json_response(&body);
    if status == 400 {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n400_bad_request.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n400_bad_request.reason_phrase.to_string();
    } else if status == 404 {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase.to_string();
    }
    response
}

fn error_json(id: &str, error: &ModernError) -> String {
    let data = error.data.as_deref().map(|d| format!(r#","data":{d}"#)).unwrap_or_default();
    format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":{},"message":"{}"{data}}}}}"#, error.code, json_escape(&error.message))
}

/// 32 random bytes: `/dev/urandom`, else hashed time and counters.
pub(crate) fn random_key() -> Vec<u8> {
    use std::hash::{BuildHasher, Hasher};
    use std::io::Read;
    let mut key = vec![0u8; 32];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut key)).is_ok() {
        return key;
    }
    let keys = std::collections::hash_map::RandomState::new();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    for (i, chunk) in key.chunks_mut(8).enumerate() {
        let mut hasher = keys.build_hasher();
        hasher.write_usize(i);
        hasher.write_u128(nanos);
        chunk.copy_from_slice(&hasher.finish().to_le_bytes());
    }
    key
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl McpServer {
    /// The key that signs `requestState` (MCP 2026-07-28 multi round-trip
    /// requests). Random per process by default; give replicas behind a
    /// load balancer the same key, so a retry may land on any of them.
    pub fn request_state_key(mut self, key: impl AsRef<[u8]>) -> Self {
        self.state_key = Arc::new(key.as_ref().to_vec());
        self
    }

    /// The caching hints on cacheable results (MCP 2026-07-28: discovery,
    /// lists, `resources/read`): fresh for `ttl_ms`, shareable between
    /// users (`public`) or not. Default `0` and private — nothing cached;
    /// raise it when the lists don't vary per caller.
    pub fn cache_hints(mut self, ttl_ms: u64, public: bool) -> Self {
        self.cache_ttl_ms = ttl_ms;
        self.cache_scope = if public { "public" } else { "private" };
        self
    }

    fn server_info_json(&self) -> String {
        format!(
            r#"{{"name":"{}"{}"version":"{}"{}}}"#,
            json_escape(&self.server_name),
            self.server_title.as_ref().map(|t| format!(r#","title":"{}","#, json_escape(t))).unwrap_or_else(|| ",".to_string()),
            json_escape(&self.server_version),
            icons_json(&self.server_icons),
        )
    }

    fn capabilities_json(&self) -> String {
        let logging = if self.logging_enabled { r#","logging":{}"# } else { "" };
        let completions = if self.completions.read().unwrap().is_empty() { "" } else { r#","completions":{}"# };
        format!(
            r#"{{"tools":{{"listChanged":true}},"resources":{{"subscribe":true,"listChanged":true}},"prompts":{{"listChanged":true}}{logging}{completions},"extensions":{{"{}":{{}}}}}}"#,
            super::tasks::EXTENSION
        )
    }

    /// Every version this endpoint serves: modern ones statelessly, the
    /// earlier ones through `initialize`.
    fn supported_versions_json() -> String {
        let all: Vec<String> = MODERN_VERSIONS.iter().chain(SUPPORTED_PROTOCOL_VERSIONS.iter().rev()).map(|v| format!(r#""{v}""#)).collect();
        format!("[{}]", all.join(","))
    }

    /// Sign `state` for the retry of `method` on `name` by `principal`.
    fn sign_state(&self, method: &str, name: &str, principal: &str, state: &str) -> String {
        let payload = format!("{}\n{method}\n{name}\n{principal}\n{state}", unix_now() + STATE_TTL_SECS);
        let mac = crate::core::sha256::hmac_sha256(&self.state_key, payload.as_bytes());
        format!("{}.{}", base64_encode(payload.as_bytes()), base64_encode(&mac))
    }

    /// The state inside `token`, if this server signed it for this method,
    /// name and caller and it hasn't expired.
    fn verify_state(&self, token: &str, method: &str, name: &str, principal: &str) -> Option<String> {
        let (payload, mac) = token.split_once('.')?;
        let payload = base64_decode(payload)?;
        if !crate::core::sha256::verify(&self.state_key, &payload, &base64_decode(mac)?) {
            return None;
        }
        let payload = String::from_utf8(payload).ok()?;
        let mut parts = payload.splitn(5, '\n');
        let expires: u64 = parts.next()?.parse().ok()?;
        let fits = parts.next()? == method && parts.next()? == name && parts.next()? == principal;
        (fits && unix_now() <= expires).then(|| parts.next().unwrap_or("").to_string())
    }

    /// Serve one MCP 2026-07-28 request.
    pub(crate) fn handle_modern(&self, request: &Request, body: &str, mut ctx: McpContext) -> Response {
        let body_members = match json_rpc::object_members(body) {
            Some(members) => members,
            None => {
                let error = ModernError::bad_request(json_rpc::INVALID_REQUEST, "The body must be one JSON-RPC request or notification");
                return status_response(400, error_json("null", &error));
            }
        };
        let id = member(&body_members, "id").map(str::to_string);
        let id_json = id.clone().unwrap_or_else(|| "null".to_string());
        let method = match member_str(&body_members, "method") {
            Some(method) => method,
            None => return status_response(400, error_json(&id_json, &ModernError::bad_request(json_rpc::INVALID_REQUEST, "Missing method"))),
        };
        let params = member(&body_members, "params").unwrap_or("{}").to_string();
        let modern = match self.validate_modern(request, &method, &params) {
            Ok(modern) => modern,
            Err(error) => return status_response(error.status, error_json(&id_json, &error)),
        };
        // A notification: nothing in this revision needs one over HTTP.
        let Some(id) = id else {
            let mut accepted = super::no_content();
            accepted.status_code = 202;
            accepted.reason_phrase = "Accepted".to_string();
            return accepted;
        };

        if method == "subscriptions/listen" {
            return self.listen(&id, &params, modern.tasks_extension);
        }
        if method == "tools/call" {
            if let Err(error) = self.check_tool_headers(request, &params) {
                return status_response(error.status, error_json(&id, &error));
            }
            let tool = member_str(&members(&params), "name").unwrap_or_default();
            if !modern.tasks_extension && self.is_task_required(&tool) {
                return status_response(400, error_json(&id, &tasks_extension_required()));
            }
        }

        let principal = ctx.auth_claims.as_deref().and_then(|claims| json_rpc::extract_str(claims, "sub")).unwrap_or_default();
        let target = {
            let params = members(&params);
            member_str(&params, "name").or_else(|| member_str(&params, "uri")).unwrap_or_default()
        };
        let mut modern = modern;
        if modern.tasks_extension {
            ctx.tasks = Some(super::tasks::TaskHandle { server: self.clone(), principal: principal.clone() });
        }
        if let Some(token) = member_str(&members(&params), "requestState") {
            match self.verify_state(&token, &method, &target, &principal) {
                Some(state) => modern.request_state = Some(state),
                None => {
                    let error = ModernError::new(json_rpc::INVALID_PARAMS, "requestState failed verification (altered, expired, or for another request)");
                    return status_response(200, error_json(&id, &error));
                }
            }
        }
        if let Some(responses) = member(&members(&params), "inputResponses") {
            if !json_rpc::object_members(responses).is_some_and(|answers| answers.iter().all(|(_, v)| v.trim_start().starts_with('{'))) {
                let error = ModernError::new(json_rpc::INVALID_PARAMS, "inputResponses must be an object of result objects");
                return status_response(200, error_json(&id, &error));
            }
            modern.input_responses = Some(responses.to_string());
        }
        let wants_stream = modern.log_level.is_some() || member(&members(&params), "_meta").is_some_and(|meta| member(&members(meta), "progressToken").is_some());
        let accepts_stream = header(request, "accept").is_some_and(|a| a.to_ascii_lowercase().contains("text/event-stream"));
        ctx.modern = Some(Arc::new(modern));
        let body = body.to_string();

        if wants_stream && accepts_stream && matches!(method.as_str(), "tools/call" | "prompts/get" | "resources/read") {
            // Progress and log messages first, then the result, on this
            // request's own stream.
            let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(32);
            ctx.request_stream = Some(tx.clone());
            let server = self.clone();
            std::thread::spawn(move || {
                let (_, json) = server.answer(&id, &method, &body, ctx, &target, &principal);
                let _ = tx.send(format!("data: {json}\n\n").into_bytes());
            });
            let mut response = Response::new();
            response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
            response.headers.push(Header { name: Header::_CONTENT_TYPE.into(), value: "text/event-stream".into() });
            response.headers.push(Header { name: Header::_CACHE_CONTROL.into(), value: "no-cache".into() });
            response.headers.push(Header { name: "X-Accel-Buffering".into(), value: "no".into() });
            response.stream_pipe = Some(Box::new(crate::sse::SseReader::new(rx, super::SSE_KEEPALIVE_INTERVAL)));
            return response;
        }
        let (status, json) = self.answer(&id, &method, &body, ctx, &target, &principal);
        status_response(status, json)
    }

    /// `Mcp-Param-*` headers against the arguments of the tool called.
    fn check_tool_headers(&self, request: &Request, params: &str) -> Result<(), ModernError> {
        let params = members(params);
        let Some(name) = member_str(&params, "name") else { return Ok(()) };
        let schema = self.tools.read().unwrap().iter().find(|t| t.name == name).map(|t| t.input_schema.clone());
        #[cfg(feature = "http2")]
        let schema = schema.or_else(|| self.async_tools.read().unwrap().iter().find(|t| t.name == name).map(|t| t.input_schema.clone()));
        match schema {
            Some(schema) if schema.contains("x-mcp-header") => check_param_headers(request, &schema, member(&params, "arguments").unwrap_or("{}")),
            _ => Ok(()),
        }
    }

    /// The `_meta` and header checks every modern request passes first.
    fn validate_modern(&self, request: &Request, method: &str, params: &str) -> Result<ModernRequest, ModernError> {
        let params = members(params);
        let meta = member(&params, "_meta").map(members).ok_or_else(|| {
            ModernError::bad_request(json_rpc::INVALID_PARAMS, "params._meta is required (protocol version and client capabilities)")
        })?;
        let version = member_str(&meta, META_VERSION)
            .ok_or_else(|| ModernError::bad_request(json_rpc::INVALID_PARAMS, format!("params._meta[\"{META_VERSION}\"] is required")))?;
        let capabilities = member(&meta, META_CAPABILITIES)
            .filter(|c| c.trim_start().starts_with('{'))
            .ok_or_else(|| ModernError::bad_request(json_rpc::INVALID_PARAMS, format!("params._meta[\"{META_CAPABILITIES}\"] is required")))?;
        match header(request, "mcp-protocol-version").map(|v| v.trim().to_string()) {
            Some(header_version) if header_version == version => {}
            Some(header_version) => {
                return Err(ModernError::bad_request(
                    HEADER_MISMATCH,
                    format!("Header mismatch: MCP-Protocol-Version {header_version:?} doesn't match _meta's {version:?}"),
                ))
            }
            None => return Err(ModernError::bad_request(HEADER_MISMATCH, "The MCP-Protocol-Version header is required")),
        }
        if !MODERN_VERSIONS.contains(&version.as_str()) {
            return Err(ModernError::bad_request(UNSUPPORTED_PROTOCOL_VERSION, "Unsupported protocol version")
                .data(format!(r#"{{"supported":{},"requested":"{}"}}"#, Self::supported_versions_json(), json_escape(&version))));
        }
        match header(request, "mcp-method").and_then(|h| header_value(&h)) {
            Some(header_method) if header_method == method => {}
            Some(header_method) => {
                return Err(ModernError::bad_request(HEADER_MISMATCH, format!("Header mismatch: Mcp-Method {header_method:?} doesn't match the body's {method:?}")))
            }
            None => return Err(ModernError::bad_request(HEADER_MISMATCH, "The Mcp-Method header is required (and must be a valid header value)")),
        }
        let named = match method {
            "tools/call" | "prompts/get" => member_str(&params, "name"),
            "resources/read" => member_str(&params, "uri"),
            "tasks/get" | "tasks/update" | "tasks/cancel" => member_str(&params, "taskId"),
            _ => None,
        };
        if let Some(body_name) = named {
            match header(request, "mcp-name").and_then(|h| header_value(&h)) {
                Some(header_name) if header_name == body_name => {}
                Some(header_name) => {
                    return Err(ModernError::bad_request(HEADER_MISMATCH, format!("Header mismatch: Mcp-Name {header_name:?} doesn't match the body's {body_name:?}")))
                }
                None => return Err(ModernError::bad_request(HEADER_MISMATCH, "The Mcp-Name header is required for this method")),
            }
        }
        let tasks_extension = member(&members(capabilities), "extensions").is_some_and(|ext| member(&members(ext), super::tasks::EXTENSION).is_some());
        Ok(ModernRequest {
            client_capabilities: capabilities.to_string(),
            log_level: member_str(&meta, META_LOG_LEVEL).and_then(|level| LogLevel::parse(&level)),
            tasks_extension,
            ..ModernRequest::default()
        })
    }

    /// Dispatch and render: the HTTP status and the JSON-RPC response.
    fn answer(&self, id: &str, method: &str, body: &str, ctx: McpContext, target: &str, principal: &str) -> (u16, String) {
        let capabilities = ctx.modern.as_ref().map(|m| m.client_capabilities.clone()).unwrap_or_default();
        let retry = ctx.modern.as_ref().is_some_and(|m| m.input_responses.is_some() || m.request_state.is_some());
        let server_info = format!(r#""_meta":{{"io.modelcontextprotocol/serverInfo":{}}}"#, self.server_info_json());
        let answer = self.dispatch_modern(method, body, ctx);
        let result = match answer {
            Ok(Answer::Complete { result, cacheable }) => {
                let mut fields = format!(r#""resultType":"complete",{server_info}"#);
                // A retry's result depends on its inputs: never cached.
                if cacheable && !retry {
                    fields.push_str(&format!(r#","ttlMs":{},"cacheScope":"{}""#, self.cache_ttl_ms, self.cache_scope));
                }
                Ok(with_fields(&result, &fields))
            }
            Ok(Answer::InputRequired(input)) => self.input_required_result(&input, &capabilities, method, target, principal, &server_info),
            Ok(Answer::Task(task_id)) => match self.create_task_fields(&task_id, principal) {
                Some(fields) => Ok(format!(r#"{{"resultType":"task",{server_info},{fields}}}"#)),
                None => Err(ModernError::new(json_rpc::INTERNAL_ERROR, "the task disappeared")),
            },
            Err(error) => Err(error),
        };
        match result {
            Ok(result) => (200, format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{result}}}"#)),
            Err(error) => (error.status, error_json(id, &error)),
        }
    }

    /// An `InputRequiredResult` from a handler's [`InputRequired`]: refused
    /// with `-32021` when it asks for something the client didn't declare;
    /// its state signed.
    fn input_required_result(&self, input: &str, capabilities: &str, method: &str, target: &str, principal: &str, server_info: &str) -> Result<String, ModernError> {
        let input = members(input);
        let declared = members(capabilities);
        let requests = member(&input, "inputRequests").unwrap_or("{}");
        let mut missing: Vec<&str> = Vec::new();
        for (_, request) in members(requests) {
            let needs = match member_str(&members(&request), "method").as_deref() {
                Some("elicitation/create") => "elicitation",
                Some("sampling/createMessage") => "sampling",
                Some("roots/list") => "roots",
                _ => continue,
            };
            if member(&declared, needs).is_none() && !missing.contains(&needs) {
                missing.push(needs);
            }
        }
        if !missing.is_empty() {
            let required: Vec<String> = missing.iter().map(|c| format!(r#""{c}":{{}}"#)).collect();
            return Err(ModernError::bad_request(MISSING_CLIENT_CAPABILITY, format!("This request needs the client capabilities: {}", missing.join(", ")))
                .data(format!(r#"{{"requiredCapabilities":{{{}}}}}"#, required.join(","))));
        }
        let mut fields = format!(r#""resultType":"input_required",{server_info}"#);
        if requests.trim() != "{}" {
            fields.push_str(&format!(r#","inputRequests":{requests}"#));
        }
        if let Some(state) = member_str(&input, "requestState") {
            fields.push_str(&format!(r#","requestState":"{}""#, json_escape(&self.sign_state(method, target, principal, &state))));
        }
        Ok(format!("{{{fields}}}"))
    }

    fn dispatch_modern(&self, method: &str, body: &str, ctx: McpContext) -> Result<Answer, ModernError> {
        let complete = |result: String, cacheable: bool| Answer::Complete { result, cacheable };
        Ok(match method {
            "server/discover" => complete(
                format!(r#"{{"supportedVersions":{},"capabilities":{}}}"#, Self::supported_versions_json(), self.capabilities_json()),
                true,
            ),
            "tools/list" => complete(self.do_tools_list(body, &ctx)?, true),
            "tools/call" => match self.call_tool(body, ctx)? {
                Ok(content) if content.kind == "input_required" => Answer::InputRequired(content.text),
                Ok(content) if content.kind == "task" => Answer::Task(content.text),
                other => complete(Self::format_tool_result(other), false),
            },
            "resources/list" => complete(self.do_resources_list(body)?, true),
            "resources/templates/list" => complete(self.do_resource_templates_list(body)?, true),
            "resources/read" => match self.do_resources_read(body) {
                Ok(result) => complete(result, true),
                Err((code, message)) if message.starts_with("Resource not found") => {
                    let uri = json_rpc::extract_raw(body, "params").and_then(|p| json_rpc::extract_str(&p, "uri")).unwrap_or_default();
                    return Err(ModernError::new(code, "Resource not found").data(format!(r#"{{"uri":"{}"}}"#, json_escape(&uri))));
                }
                Err(error) => return Err(error.into()),
            },
            "prompts/list" => complete(self.do_prompts_list(body)?, true),
            "prompts/get" => match self.get_prompt(body, ctx)? {
                (_, super::PromptReply::InputRequired(input)) => Answer::InputRequired(input.to_json()),
                (description, super::PromptReply::Messages(messages)) => {
                    let messages: Vec<String> = messages.iter().map(PromptMessage::to_json).collect();
                    complete(format!(r#"{{"description":"{}","messages":[{}]}}"#, json_escape(&description), messages.join(",")), false)
                }
            },
            "completion/complete" => complete(self.do_completion(body)?, false),
            "tasks/get" | "tasks/update" | "tasks/cancel" => {
                let principal = ctx.tasks.as_ref().map(|handle| handle.principal.clone()).ok_or_else(tasks_extension_required)?;
                let params = members(&json_rpc::object_members(body).and_then(|m| m.into_iter().find(|(k, _)| k == "params").map(|(_, v)| v)).unwrap_or_default());
                let task_id = member_str(&params, "taskId").ok_or_else(|| ModernError::new(json_rpc::INVALID_PARAMS, "taskId is required"))?;
                match method {
                    "tasks/get" => complete(format!("{{{}}}", self.task_get(&task_id, &principal)?), false),
                    "tasks/update" => {
                        self.task_update(&task_id, &principal, member(&params, "inputResponses").unwrap_or("{}"))?;
                        complete("{}".to_string(), false)
                    }
                    _ => {
                        self.task_cancel(&task_id, &principal)?;
                        complete("{}".to_string(), false)
                    }
                }
            }
            _ => {
                let mut error = ModernError::new(json_rpc::METHOD_NOT_FOUND, format!("Method not found: {method}"));
                error.status = 404;
                return Err(error);
            }
        })
    }

    /// `subscriptions/listen`: an SSE stream, the acknowledgment first.
    pub(crate) fn listen(&self, id: &str, params: &str, tasks_extension: bool) -> Response {
        let filter = member(&members(params), "notifications").map(members).unwrap_or_default();
        let flag = |key: &str| member(&filter, key) == Some("true");
        let uris: Vec<String> = member(&filter, "resourceSubscriptions")
            .map(|list| json_rpc::split_array_elements(list).iter().filter_map(|u| json_rpc::extract_str(&format!(r#"{{"v":{u}}}"#), "v")).collect())
            .unwrap_or_default();
        let (tools, prompts, resources) = (flag("toolsListChanged"), flag("promptsListChanged"), flag("resourcesListChanged"));
        let task_ids: Vec<String> = member(&filter, "taskIds")
            .map(|list| json_rpc::split_array_elements(list).iter().filter_map(|t| json_rpc::extract_str(&format!(r#"{{"v":{t}}}"#), "v")).collect())
            .unwrap_or_default();
        if !task_ids.is_empty() && !tasks_extension {
            return status_response(400, error_json(id, &tasks_extension_required()));
        }
        let mut honoured: Vec<String> = Vec::new();
        for (on, key) in [(tools, "toolsListChanged"), (prompts, "promptsListChanged"), (resources, "resourcesListChanged")] {
            if on {
                honoured.push(format!(r#""{key}":true"#));
            }
        }
        if !uris.is_empty() {
            let list: Vec<String> = uris.iter().map(|u| format!(r#""{}""#, json_escape(u))).collect();
            honoured.push(format!(r#""resourceSubscriptions":[{}]"#, list.join(",")));
        }
        if !task_ids.is_empty() {
            let list: Vec<String> = task_ids.iter().map(|t| format!(r#""{}""#, json_escape(t))).collect();
            honoured.push(format!(r#""taskIds":[{}]"#, list.join(",")));
        }
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(super::SSE_CHANNEL_CAPACITY);
        let ack = format!(
            r#"{{"jsonrpc":"2.0","method":"notifications/subscriptions/acknowledged","params":{{"_meta":{{"io.modelcontextprotocol/subscriptionId":{id}}},"notifications":{{{}}}}}}}"#,
            honoured.join(",")
        );
        let _ = tx.try_send(format!("data: {ack}\n\n").into_bytes());
        self.listeners.lock().unwrap().push(Listener { id: id.to_string(), tools, prompts, resources, uris, task_ids, tx });
        let mut response = Response::new();
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.headers.push(Header { name: Header::_CONTENT_TYPE.into(), value: "text/event-stream".into() });
        response.headers.push(Header { name: Header::_CACHE_CONTROL.into(), value: "no-cache".into() });
        response.headers.push(Header { name: "X-Accel-Buffering".into(), value: "no".into() });
        response.stream_pipe = Some(Box::new(crate::sse::SseReader::new(rx, super::SSE_KEEPALIVE_INTERVAL)));
        response
    }

    /// `notifications/tasks` for the task `task_id` to the streams listening
    /// for it.
    pub(crate) fn deliver_task_notification(&self, task_id: &str, params_json: &str) {
        let mut listeners = self.listeners.lock().unwrap();
        listeners.retain(|listener| {
            if !listener.task_ids.iter().any(|t| t == task_id) {
                return true;
            }
            let tag = format!(r#""_meta":{{"io.modelcontextprotocol/subscriptionId":{}}}"#, listener.id);
            let frame = format!("data: {}\n\n", super::render_notification("notifications/tasks", Some(&with_fields(params_json, &tag))));
            !matches!(listener.tx.try_send(frame.into_bytes()), Err(TrySendError::Disconnected(_)))
        });
    }

    /// Send a change notification to the `subscriptions/listen` streams that
    /// asked for its type, tagged with each one's id; drops closed ones.
    pub(crate) fn deliver_to_listeners(&self, method: &str, params_json: Option<&str>, uri: Option<&str>) {
        let mut listeners = self.listeners.lock().unwrap();
        if listeners.is_empty() {
            return;
        }
        listeners.retain(|listener| {
            let wanted = match method {
                "notifications/tools/list_changed" => listener.tools,
                "notifications/prompts/list_changed" => listener.prompts,
                "notifications/resources/list_changed" => listener.resources,
                "notifications/resources/updated" => uri.is_some_and(|uri| listener.uris.iter().any(|u| u == uri)),
                _ => false,
            };
            if !wanted {
                return true;
            }
            let tag = format!(r#""_meta":{{"io.modelcontextprotocol/subscriptionId":{}}}"#, listener.id);
            let params = with_fields(params_json.unwrap_or("{}"), &tag);
            let frame = format!("data: {}\n\n", super::render_notification(method, Some(&params)));
            !matches!(listener.tx.try_send(frame.into_bytes()), Err(TrySendError::Disconnected(_)))
        });
    }
}

#[cfg(test)]
mod tests;
