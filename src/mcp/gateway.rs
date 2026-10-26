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
//! Minimal on purpose: tools are listed once, at `build`; upstream resources,
//! prompts and notifications are not federated; each upstream speaks the
//! protocol version it negotiated with the gateway.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::{json_escape, json_rpc, McpContent, McpServer, ToolAnnotations};
use crate::http_client::Client;

/// The protocol version the gateway asks upstreams for.
const PROTOCOL_VERSION: &str = "2025-11-25";

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
}

impl McpGateway {
    /// A gateway announcing itself as `name`/`version`.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        McpGateway { name: name.into(), version: version.into(), upstreams: Vec::new(), rules: Vec::new() }
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
        for upstream in self.upstreams {
            let connection = Arc::new(Connection::new(upstream));
            for tool in connection.list_tools()? {
                server = register(server, &connection, tool);
            }
        }
        Ok(server)
    }
}

/// One upstream tool, as `tools/list` described it.
struct UpstreamTool {
    name: String,
    description: String,
    title: Option<String>,
    input_schema: String,
    output_schema: Option<String>,
    annotations: Option<ToolAnnotations>,
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
        .any(|pattern| match pattern.strip_suffix('*') {
            Some(prefix) => tool.starts_with(prefix),
            None => pattern == tool,
        })
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
        self.request("tools/call", &params).map_err(|e| format!("{}: {e}", self.upstream.prefix))
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

    fn initialize(&self) -> Result<Session, String> {
        let params = format!(
            r#"{{"protocolVersion":"{PROTOCOL_VERSION}","capabilities":{{}},"clientInfo":{{"name":"rws-mcp-gateway","version":"{}"}}}}"#,
            env!("CARGO_PKG_VERSION"),
        );
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let response = self.post(None, PROTOCOL_VERSION, &envelope(id, "initialize", &params))?;
        if !(200..300).contains(&response.status()) {
            return Err(format!("initialize answered HTTP {}", response.status()));
        }
        let session_id = response.header("mcp-session-id").map(str::to_string);
        let result = parse_reply(response.header("content-type").unwrap_or(""), response.bytes(), id)?;
        let version = top_level_str(&result, "protocolVersion").unwrap_or_else(|| PROTOCOL_VERSION.to_string());
        let session = Session { id: session_id, version };
        let initialized = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        self.post(session.id.as_deref(), &session.version, initialized)?;
        Ok(session)
    }

    fn send(&self, session: &Session, method: &str, params: &str) -> Result<Reply, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let response = self.post(session.id.as_deref(), &session.version, &envelope(id, method, params))?;
        if response.status() == 404 && session.id.is_some() {
            return Ok(Reply::SessionGone);
        }
        if !(200..300).contains(&response.status()) {
            return Err(format!("answered HTTP {}", response.status()));
        }
        parse_reply(response.header("content-type").unwrap_or(""), response.bytes(), id).map(Reply::Result)
    }

    /// One POST to the upstream, with its own headers only.
    fn post(&self, session_id: Option<&str>, version: &str, body: &str) -> Result<crate::http_client::Response, String> {
        let mut request = self
            .client
            .post(&self.upstream.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", version);
        if let Some(id) = session_id {
            request = request.header("Mcp-Session-Id", id);
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
