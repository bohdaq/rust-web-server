//! A minimal MCP gateway: one endpoint in front of several upstream MCP
//! servers.
//!
//! [`McpGateway::build`] connects to each [`Upstream`], lists its tools and
//! registers each one as `<prefix>__<tool>` on an ordinary [`McpServer`];
//! calling it forwards the call to that upstream. The gateway's caller
//! authenticates to the gateway (put [`McpServer::require_bearer_tokens`] or
//! [`McpServer::require_oauth`] on the server `build` returns); upstreams
//! get only the headers configured for them — the caller's token is never
//! passed on. [`McpGateway::allow`] decides which caller (the `sub` of
//! [`McpContext::auth_claims`](super::McpContext::auth_claims)) sees which tools, and nothing is allowed
//! unless a rule says so.
//!
//! Tools are listed at `build` and again every [`McpGateway::refresh_every`]
//! (5 minutes by default): new upstream tools appear, removed ones go, and
//! the gateway's clients get `notifications/tools/list_changed`. Between
//! refreshes `tools/list` is answered from the gateway's own copy.
//!
//! Each upstream speaks the protocol it supports: the gateway opens a
//! 2025-11-25 session, and when an upstream refuses `initialize` it talks to
//! it the 2026-07-28 way instead, statelessly (`server/discover` first, then
//! every request carrying its version and capabilities in `_meta` and the
//! `Mcp-Method`/`Mcp-Name` headers). Clients of either era reach upstreams of
//! either era.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::{json_escape, json_rpc, McpContent, McpServer, ToolAnnotations};
use crate::http_client::Client;

/// The protocol version the gateway asks upstreams for.
const PROTOCOL_VERSION: &str = "2025-11-25";

/// The stateless protocol, for upstreams that refuse `initialize`.
const MODERN_VERSION: &str = "2026-07-28";

/// Separates an upstream's prefix from its tool names.
pub const SEPARATOR: &str = "__";

/// `tools/list` pages followed per upstream before giving up.
const MAX_LIST_PAGES: usize = 100;

/// An upstream MCP server (Streamable HTTP).
#[derive(Clone, Debug)]
pub struct Upstream {
    prefix: String,
    url: String,
    headers: Vec<(String, String)>,
    timeout_ms: u64,
}

impl Upstream {
    /// The server at `url` (its MCP endpoint, e.g. `https://mcp.example.com/mcp`),
    /// whose tools appear as `<prefix>__<tool>`. `prefix` is letters, digits,
    /// `-` and single `_`s.
    pub fn new(prefix: impl Into<String>, url: impl Into<String>) -> Self {
        Upstream { prefix: prefix.into(), url: url.into(), headers: Vec::new(), timeout_ms: 30_000 }
    }

    /// Send this header to the upstream on every request — its credentials,
    /// e.g. `.header("Authorization", "Bearer <upstream token>")`.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Give up on an upstream request after this long (default 30 s).
    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// Builds a gateway [`McpServer`]; see the [module docs](self).
pub struct McpGateway {
    name: String,
    version: String,
    upstreams: Vec<Upstream>,
    rules: Vec<(String, Vec<String>)>,
    refresh: Option<std::time::Duration>,
    limits: Vec<ToolLimit>,
    audit: bool,
}

/// [`McpGateway::limit`]: calls of the tools matching `pattern`, per caller.
struct ToolLimit {
    pattern: String,
    max_calls: u32,
    window_secs: u64,
    limiter: crate::rate_limit::RateLimiter,
}

/// How often upstream tool lists are re-read by default.
pub const DEFAULT_REFRESH: std::time::Duration = std::time::Duration::from_secs(300);

impl McpGateway {
    /// A gateway announcing itself as `name`/`version`.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        McpGateway { name: name.into(), version: version.into(), upstreams: Vec::new(), rules: Vec::new(), refresh: Some(DEFAULT_REFRESH), limits: Vec::new(), audit: false }
    }

    /// Allow each caller (each `sub`) at most `max_calls` calls per
    /// `window_secs` of the tools matching `tools` (a full name, a prefix
    /// ending in `*`, or `*`), counted per tool. Over the limit a call
    /// answers a tool error saying when to retry. Limits add up: a call
    /// must pass every one that matches. Counted in this process.
    pub fn limit(mut self, tools: impl Into<String>, max_calls: u32, window_secs: u64) -> Self {
        self.limits.push(ToolLimit { pattern: tools.into(), max_calls, window_secs, limiter: crate::rate_limit::RateLimiter::new(max_calls, window_secs) });
        self
    }

    /// Write one JSON line per `tools/call` to the access log (stdout, or
    /// `RWS_CONFIG_ACCESS_LOG`): time, caller (`sub`), tool, upstream,
    /// outcome (`ok`, `tool_error`, `refused` by a limit, `denied` by the
    /// ACL) and duration. Arguments and results are not logged.
    pub fn audit_log(mut self, on: bool) -> Self {
        self.audit = on;
        self
    }

    /// Re-read every upstream's tools this often (default
    /// [`DEFAULT_REFRESH`]); `None` lists them only at `build`. An upstream
    /// that can't be reached keeps the tools it had.
    pub fn refresh_every(mut self, every: Option<std::time::Duration>) -> Self {
        self.refresh = every;
        self
    }

    /// Federate `upstream`'s tools.
    pub fn upstream(mut self, upstream: Upstream) -> Self {
        self.upstreams.push(upstream);
        self
    }

    /// Let `identity` see and call the tools matching `tools`: a full name
    /// (`github__search_issues`), a prefix ending in `*` (`github__*`), or
    /// `*` for all. `identity` is the caller's `sub` claim; `*` means any
    /// identified caller. Rules add up; a caller no rule names gets no tools.
    pub fn allow<I, S>(mut self, identity: impl Into<String>, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.rules.push((identity.into(), tools.into_iter().map(Into::into).collect()));
        self
    }

    /// Connect to every upstream, list its tools and return the gateway.
    /// Fails, naming the upstream, if one can't be reached or answers
    /// something that isn't MCP. Add authentication to the result.
    pub fn build(self) -> Result<McpServer, String> {
        let mut prefixes: Vec<&str> = Vec::new();
        for upstream in &self.upstreams {
            if !valid_prefix(&upstream.prefix) {
                return Err(format!("upstream prefix \"{}\": use letters, digits, '-' and single '_'", upstream.prefix));
            }
            if prefixes.contains(&upstream.prefix.as_str()) {
                return Err(format!("upstream prefix \"{}\" is used twice", upstream.prefix));
            }
            prefixes.push(&upstream.prefix);
        }

        let rules = Arc::new(self.rules);
        let mut server = McpServer::new(self.name, self.version).tool_filter(move |ctx, tool| {
            let identity = ctx.auth_claims.as_deref().and_then(|claims| top_level_str(claims, "sub"));
            identity.is_some_and(|identity| allowed(&rules, &identity, tool))
        });
        if self.audit {
            server = server.after_tool_call(|ctx, call| {
                let sub = ctx.auth_claims.as_deref().and_then(|claims| top_level_str(claims, "sub")).unwrap_or_default();
                let upstream = call.tool.split_once(SEPARATOR).map_or("", |(prefix, _)| prefix);
                let outcome = match call.outcome {
                    crate::mcp::ToolCallOutcome::Ok => "ok",
                    crate::mcp::ToolCallOutcome::ToolError => "tool_error",
                    crate::mcp::ToolCallOutcome::Refused => "refused",
                    crate::mcp::ToolCallOutcome::Denied => "denied",
                };
                let line = audit_line(&sub, call.tool, upstream, outcome, call.duration);
                crate::log::access_sink::write_line(|out| out.extend_from_slice(line.as_bytes()));
            });
        }
        if !self.limits.is_empty() {
            let limits = self.limits;
            server = server.before_tool_call(move |ctx, tool| {
                let identity = ctx.auth_claims.as_deref().and_then(|claims| top_level_str(claims, "sub")).unwrap_or_default();
                for limit in limits.iter().filter(|l| matches_pattern(&l.pattern, tool)) {
                    let key = format!("{identity}\u{0}{tool}");
                    if !limit.limiter.check(&key) {
                        return Err(format!("Rate limited: {tool} allows {} calls per {} s; retry in {} s", limit.max_calls, limit.window_secs, limit.limiter.retry_after(&key).max(1)));
                    }
                }
                Ok(())
            });
        }
        let mut federated = Vec::new();
        for upstream in self.upstreams {
            let connection = Arc::new(Connection::new(upstream));
            let mut known = std::collections::HashMap::new();
            for tool in connection.list_tools()? {
                known.insert(tool.name.clone(), tool.fingerprint());
                server = register(server, &connection, tool);
            }
            federated.push((connection, known));
        }
        if let Some(every) = self.refresh {
            let live = server.clone();
            let spawned = std::thread::Builder::new().name("rws-mcp-gateway-refresh".to_string()).spawn(move || loop {
                std::thread::sleep(every);
                for (connection, known) in federated.iter_mut() {
                    refresh(&live, connection, known);
                }
            });
            if let Err(e) = spawned {
                eprintln!("mcp gateway: tool list refresh not started: {e}");
            }
        }
        Ok(server)
    }
}

/// Re-list `connection`'s tools and apply the difference to `server`:
/// register new or changed tools, remove gone ones, then tell clients.
fn refresh(server: &McpServer, connection: &Arc<Connection>, known: &mut std::collections::HashMap<String, String>) {
    let tools = match connection.list_tools() {
        Ok(tools) => tools,
        Err(e) => {
            eprintln!("mcp gateway: keeping {}'s tools: {e}", connection.name());
            return;
        }
    };
    let prefix = &connection.upstream.prefix;
    let mut changed = false;
    let mut seen = std::collections::HashSet::new();
    for tool in tools {
        seen.insert(tool.name.clone());
        let fingerprint = tool.fingerprint();
        if known.get(&tool.name) == Some(&fingerprint) {
            continue;
        }
        let exposed = format!("{prefix}{SEPARATOR}{}", tool.name);
        if known.contains_key(&tool.name) {
            server.remove_tool(&exposed);
        }
        known.insert(tool.name.clone(), fingerprint);
        let _ = register(server.clone(), connection, tool);
        changed = true;
    }
    let gone: Vec<String> = known.keys().filter(|name| !seen.contains(*name)).cloned().collect();
    for name in gone {
        known.remove(&name);
        server.remove_tool(&format!("{prefix}{SEPARATOR}{name}"));
        changed = true;
    }
    if changed {
        server.notify("notifications/tools/list_changed", None);
    }
}

/// One upstream tool, as `tools/list` described it.
#[derive(Clone)]
struct UpstreamTool {
    name: String,
    description: String,
    title: Option<String>,
    input_schema: String,
    output_schema: Option<String>,
    annotations: Option<ToolAnnotations>,
}

impl UpstreamTool {
    /// Changes when anything the gateway republishes changes.
    fn fingerprint(&self) -> String {
        format!("{}\u{0}{}\u{0}{:?}\u{0}{}\u{0}{:?}\u{0}{:?}", self.name, self.description, self.title, self.input_schema, self.output_schema, self.annotations)
    }
}

fn register(server: McpServer, connection: &Arc<Connection>, tool: UpstreamTool) -> McpServer {
    let exposed = format!("{}{SEPARATOR}{}", connection.upstream.prefix, tool.name);
    let forward = Arc::clone(connection);
    let upstream_name = tool.name.clone();
    let mut server = server.tool_with_context(&exposed, &tool.description, &tool.input_schema, move |_ctx, args| {
        forward.call_tool(&upstream_name, args).map(McpContent::raw_result)
    });
    if let Some(title) = tool.title {
        server = server.tool_title(&exposed, title);
    }
    if let Some(schema) = tool.output_schema {
        server = server.tool_output_schema(&exposed, &schema);
    }
    if let Some(annotations) = tool.annotations {
        server = server.tool_annotations(&exposed, annotations);
    }
    server
}

fn valid_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && !prefix.contains(SEPARATOR)
        && !prefix.ends_with('_')
        && prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Does a rule for `identity` (or `*`) match `tool`?
fn allowed(rules: &[(String, Vec<String>)], identity: &str, tool: &str) -> bool {
    rules
        .iter()
        .filter(|(who, _)| who == identity || who == "*")
        .flat_map(|(_, patterns)| patterns)
        .any(|pattern| matches_pattern(pattern, tool))
}

/// One audit log line (without the newline).
fn audit_line(sub: &str, tool: &str, upstream: &str, outcome: &str, duration: std::time::Duration) -> String {
    let mut time = String::new();
    crate::log::iso8601_now(|iso| time.push_str(iso));
    format!(
        r#"{{"time":"{time}","event":"mcp_tool_call","sub":"{}","tool":"{}","upstream":"{}","outcome":"{outcome}","duration_ms":{:.3}}}"#,
        json_escape(sub),
        json_escape(tool),
        json_escape(upstream),
        duration.as_secs_f64() * 1000.0
    )
}

/// A full tool name, a prefix ending in `*`, or `*`.
fn matches_pattern(pattern: &str, tool: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => tool.starts_with(prefix),
        None => pattern == tool,
    }
}

/// The string member `key` of the top level of JSON object `json`.
fn top_level_str(json: &str, key: &str) -> Option<String> {
    let raw = top_level_raw(json, key)?;
    raw.starts_with('"').then(|| json_rpc::extract_str(&format!(r#"{{"v":{raw}}}"#), "v")).flatten()
}

/// The raw JSON of member `key` of the top level of JSON object `json`.
fn top_level_raw(json: &str, key: &str) -> Option<String> {
    json_rpc::object_members(json)?.into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// The gateway's session with one upstream.
struct Connection {
    upstream: Upstream,
    client: Client,
    /// `(Mcp-Session-Id, negotiated protocol version)`, once initialized.
    session: Mutex<Option<Session>>,
    next_id: AtomicU64,
}

#[derive(Clone)]
struct Session {
    id: Option<String>,
    version: String,
    /// 2026-07-28: no session; `_meta` and headers on every request.
    modern: bool,
}

impl Connection {
    fn new(upstream: Upstream) -> Self {
        let client = Client::new().timeout_ms(upstream.timeout_ms).max_redirects(0);
        Connection { upstream, client, session: Mutex::new(None), next_id: AtomicU64::new(1) }
    }

    fn name(&self) -> String {
        format!("upstream {} ({})", self.upstream.prefix, self.upstream.url)
    }

    /// Every tool, following `nextCursor`.
    fn list_tools(&self) -> Result<Vec<UpstreamTool>, String> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_LIST_PAGES {
            let params = match &cursor {
                Some(cursor) => format!(r#"{{"cursor":"{}"}}"#, json_escape(cursor)),
                None => "{}".to_string(),
            };
            let result = self.request("tools/list", &params).map_err(|e| format!("{}: {e}", self.name()))?;
            let list = top_level_raw(&result, "tools").ok_or_else(|| format!("{}: tools/list without tools", self.name()))?;
            for tool in json_rpc::split_array_elements(&list) {
                tools.push(parse_tool(&tool).ok_or_else(|| format!("{}: malformed tool {tool}", self.name()))?);
            }
            cursor = top_level_str(&result, "nextCursor");
            if cursor.is_none() {
                return Ok(tools);
            }
        }
        Err(format!("{}: more than {MAX_LIST_PAGES} pages of tools", self.name()))
    }

    /// Calls `name` upstream: the raw `tools/call` result, or why it failed.
    fn call_tool(&self, name: &str, arguments: &str) -> Result<String, String> {
        let params = format!(r#"{{"name":"{}","arguments":{arguments}}}"#, json_escape(name));
        let result = self.request("tools/call", &params).map_err(|e| format!("{}: {e}", self.upstream.prefix))?;
        if top_level_str(&result, "resultType").as_deref() == Some("input_required") {
            return Err(format!("{}: the tool asked for more input (2026-07-28 multi round-trip), which the gateway doesn't relay", self.upstream.prefix));
        }
        Ok(without_modern_fields(&result))
    }

    /// Sends one request in the current session (initializing first if
    /// needed, and once more if the upstream has forgotten the session).
    fn request(&self, method: &str, params: &str) -> Result<String, String> {
        let session = self.session()?;
        match self.send(&session, method, params)? {
            Reply::Result(result) => Ok(result),
            Reply::SessionGone => {
                self.forget(&session);
                let session = self.session()?;
                match self.send(&session, method, params)? {
                    Reply::Result(result) => Ok(result),
                    Reply::SessionGone => Err("session not found twice in a row".to_string()),
                }
            }
        }
    }

    fn session(&self) -> Result<Session, String> {
        let mut guard = self.session.lock().map_err(|_| "session lock poisoned".to_string())?;
        if let Some(session) = guard.as_ref() {
            return Ok(session.clone());
        }
        let session = self.initialize()?;
        *guard = Some(session.clone());
        Ok(session)
    }

    fn forget(&self, stale: &Session) {
        if let Ok(mut guard) = self.session.lock() {
            if guard.as_ref().is_some_and(|current| current.id == stale.id) {
                *guard = None;
            }
        }
    }

    /// A 2025-11-25 session, or, when the upstream refuses that, the
    /// stateless 2026-07-28 protocol after a `server/discover`.
    fn initialize(&self) -> Result<Session, String> {
        match self.initialize_legacy() {
            Ok(session) => Ok(session),
            Err(legacy) => self.discover().map_err(|modern| format!("{legacy}; as {MODERN_VERSION}: {modern}")),
        }
    }

    fn discover(&self) -> Result<Session, String> {
        let session = Session { id: None, version: MODERN_VERSION.to_string(), modern: true };
        match self.send(&session, "server/discover", "{}")? {
            Reply::Result(result) if result.contains(MODERN_VERSION) => Ok(session),
            Reply::Result(_) => Err(format!("server/discover doesn't list {MODERN_VERSION}")),
            Reply::SessionGone => Err("server/discover answered 404".to_string()),
        }
    }

    fn initialize_legacy(&self) -> Result<Session, String> {
        let params = format!(
            r#"{{"protocolVersion":"{PROTOCOL_VERSION}","capabilities":{{}},"clientInfo":{{"name":"rws-mcp-gateway","version":"{}"}}}}"#,
            env!("CARGO_PKG_VERSION"),
        );
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let response = self.post(None, PROTOCOL_VERSION, &[], &envelope(id, "initialize", &params))?;
        if !(200..300).contains(&response.status()) {
            return Err(format!("initialize answered HTTP {}", response.status()));
        }
        let session_id = response.header("mcp-session-id").map(str::to_string);
        let result = parse_reply(response.header("content-type").unwrap_or(""), response.bytes(), id)?;
        let version = top_level_str(&result, "protocolVersion").unwrap_or_else(|| PROTOCOL_VERSION.to_string());
        let session = Session { id: session_id, version, modern: false };
        let initialized = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        self.post(session.id.as_deref(), &session.version, &[], initialized)?;
        Ok(session)
    }

    fn send(&self, session: &Session, method: &str, params: &str) -> Result<Reply, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let response = if session.modern {
            let mut headers = vec![("Mcp-Method", method.to_string())];
            let named = match method {
                "tools/call" | "prompts/get" => top_level_str(params, "name"),
                "resources/read" => top_level_str(params, "uri"),
                _ => None,
            };
            if let Some(name) = named {
                headers.push(("Mcp-Name", name));
            }
            self.post(None, &session.version, &headers, &envelope(id, method, &with_modern_meta(params)))?
        } else {
            self.post(session.id.as_deref(), &session.version, &[], &envelope(id, method, params))?
        };
        if response.status() == 404 && session.id.is_some() {
            return Ok(Reply::SessionGone);
        }
        if !(200..300).contains(&response.status()) {
            return Err(format!("answered HTTP {}", response.status()));
        }
        parse_reply(response.header("content-type").unwrap_or(""), response.bytes(), id).map(Reply::Result)
    }

    /// One POST to the upstream, with its own headers only.
    fn post(&self, session_id: Option<&str>, version: &str, extra: &[(&str, String)], body: &str) -> Result<crate::http_client::Response, String> {
        let mut request = self
            .client
            .post(&self.upstream.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", version);
        if let Some(id) = session_id {
            request = request.header("Mcp-Session-Id", id);
        }
        for (name, value) in extra {
            request = request.header(*name, value);
        }
        for (name, value) in &self.upstream.headers {
            request = request.header(name, value);
        }
        request.body(body.as_bytes().to_vec()).send().map_err(|e| format!("unreachable: {e}"))
    }
}

enum Reply {
    Result(String),
    SessionGone,
}

/// A 2026-07-28 result without the fields the gateway's own server adds for
/// its client (`resultType`, cache hints): passed through, they would
/// appear twice, or reach a 2025 client that doesn't know them.
fn without_modern_fields(result: &str) -> String {
    let Some(members) = json_rpc::object_members(result) else { return result.to_string() };
    if !members.iter().any(|(k, _)| matches!(k.as_str(), "resultType" | "ttlMs" | "cacheScope")) {
        return result.to_string();
    }
    let kept: Vec<String> = members
        .into_iter()
        .filter(|(k, _)| !matches!(k.as_str(), "resultType" | "ttlMs" | "cacheScope"))
        .map(|(k, v)| format!(r#""{}":{v}"#, json_escape(&k)))
        .collect();
    format!("{{{}}}", kept.join(","))
}

/// `params` with the 2026-07-28 `_meta` (version, client info, no client
/// capabilities: the gateway answers no server-to-client requests).
fn with_modern_meta(params: &str) -> String {
    let meta = format!(
        r#""_meta":{{"io.modelcontextprotocol/protocolVersion":"{MODERN_VERSION}","io.modelcontextprotocol/clientInfo":{{"name":"rws-mcp-gateway","version":"{}"}},"io.modelcontextprotocol/clientCapabilities":{{}}}}"#,
        env!("CARGO_PKG_VERSION"),
    );
    let inner = params.trim().strip_prefix('{').and_then(|p| p.strip_suffix('}')).unwrap_or("").trim();
    // After the request's own members: some JSON readers take the first
    // `name` at any depth, which would be clientInfo's.
    if inner.is_empty() { format!("{{{meta}}}") } else { format!("{{{inner},{meta}}}") }
}

fn envelope(id: u64, method: &str, params: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#)
}

fn parse_tool(json: &str) -> Option<UpstreamTool> {
    let members = json_rpc::object_members(json)?;
    let raw = |key: &str| members.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let string = |key: &str| top_level_str(json, key);
    let name = string("name")?;
    let input_schema = raw("inputSchema").filter(|schema| schema.starts_with('{'))?;
    let annotations = raw("annotations").filter(|a| a.starts_with('{')).map(|a| {
        let hint = |key: &str| top_level_raw(&a, key).and_then(|v| v.parse::<bool>().ok());
        ToolAnnotations {
            read_only_hint: hint("readOnlyHint"),
            destructive_hint: hint("destructiveHint"),
            idempotent_hint: hint("idempotentHint"),
            open_world_hint: hint("openWorldHint"),
        }
    });
    Some(UpstreamTool {
        name,
        description: string("description").unwrap_or_default(),
        title: string("title"),
        input_schema,
        output_schema: raw("outputSchema").filter(|schema| schema.starts_with('{')),
        annotations,
    })
}

/// The `result` of the JSON-RPC response with `id` in an upstream reply —
/// a JSON body, or an SSE stream whose events may include notifications and
/// requests before the response. A JSON-RPC `error` becomes its message.
#[doc(hidden)]
pub fn parse_reply(content_type: &str, body: &[u8], id: u64) -> Result<String, String> {
    let body = std::str::from_utf8(body).map_err(|_| "reply is not UTF-8".to_string())?;
    let media_type = content_type.split(';').next().unwrap_or("").trim();
    let id = id.to_string();
    if media_type.eq_ignore_ascii_case("text/event-stream") {
        let mut data = String::new();
        for line in body.lines().chain(std::iter::once("")) {
            if let Some(value) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value.strip_prefix(' ').unwrap_or(value));
            } else if line.is_empty() && !data.is_empty() {
                let event = std::mem::take(&mut data);
                if top_level_raw(&event, "id").as_deref() == Some(id.as_str()) && top_level_raw(&event, "method").is_none() {
                    return response_result(&event);
                }
            }
        }
        Err("the event stream ended without a response".to_string())
    } else {
        if top_level_raw(body, "id").as_deref() != Some(id.as_str()) {
            return Err("reply is not the response to this request".to_string());
        }
        response_result(body)
    }
}

fn response_result(response: &str) -> Result<String, String> {
    if let Some(result) = top_level_raw(response, "result") {
        return Ok(result);
    }
    let error = top_level_raw(response, "error").ok_or_else(|| "response has neither result nor error".to_string())?;
    Err(top_level_str(&error, "message").unwrap_or(error))
}

/// Entry points for `fuzz_api`.
#[doc(hidden)]
pub mod fuzzing {
    /// The tools a `tools/list` result describes.
    pub fn tools(result: &str) -> usize {
        let Some(list) = super::top_level_raw(result, "tools") else { return 0 };
        super::json_rpc::split_array_elements(&list).iter().filter_map(|tool| super::parse_tool(tool)).count()
    }
}

#[cfg(test)]
mod tests;
