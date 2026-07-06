//! Model Context Protocol (MCP) server — HTTP Streamable HTTP transport.
//!
//! [`McpServer`] implements [`Application`] so it can be passed directly to
//! [`Server::run`]. Unmatched requests fall through to the built-in [`App`]
//! controller chain (static files, health probes, etc.).
//!
//! # Quick start
//!
//! ```rust,no_run
//! use rust_web_server::server::Server;
//! use rust_web_server::mcp::{McpServer, McpContent, PromptMessage};
//! # fn main() {
//! let mcp = McpServer::new("my-server", "1.0")
//!     // A tool: callable by the AI, like a function
//!     .tool(
//!         "echo",
//!         "Echo text back",
//!         r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
//!         |args| {
//!             let text = rust_web_server::mcp::extract_arg(args, "text")
//!                 .unwrap_or_else(|| "(nothing)".to_string());
//!             Ok(McpContent::text(text))
//!         },
//!     )
//!     // A resource: data the AI can read by URI
//!     .resource(
//!         "docs://{topic}",
//!         "Documentation",
//!         "Return documentation for a topic",
//!         |uri| Ok(McpContent::text(format!("Documentation for: {uri}"))),
//!     )
//!     // A prompt template: reusable message structures
//!     .prompt(
//!         "summarize",
//!         "Summarize the given text",
//!         |args| {
//!             let text = rust_web_server::mcp::extract_arg(args, "text")
//!                 .unwrap_or_else(|| "some text".to_string());
//!             Ok(vec![PromptMessage::user(format!("Please summarize: {text}"))])
//!         },
//!     );
//!
//! // let (listener, pool) = Server::setup().unwrap();
//! // Server::run(listener, pool, mcp);
//! # }
//! ```
//!
//! # MCP endpoint
//!
//! All JSON-RPC messages are sent as `POST /mcp` (override with [`.at()`](McpServer::at)).
//! The server implements the [MCP 2024-11-05 specification](https://spec.modelcontextprotocol.io).
//!
//! `GET /mcp` opens a Server-Sent Events stream for server → client push —
//! see [`McpServer::notify`] and the module docs' SSE section below.
//!
//! # SSE streaming transport
//!
//! A client that sends `GET /mcp` (instead of `POST`) gets back a
//! `text/event-stream` response that stays open indefinitely. Call
//! [`McpServer::notify`] from anywhere (a background thread, another request's
//! handler, ...) to push a JSON-RPC notification to every currently-connected
//! SSE client:
//!
//! ```rust,no_run
//! use rust_web_server::mcp::McpServer;
//!
//! let server = McpServer::new("my-server", "1.0");
//! // Elsewhere, e.g. after some background job finishes:
//! server.notify("notifications/message", Some(r#"{"level":"info","data":"job done"}"#));
//! ```
//!
//! Idle connections receive a `: keep-alive` SSE comment every 15 seconds so
//! intermediate proxies don't time them out; this doubles as the mechanism
//! that detects a client has disconnected (the next write attempt fails and
//! the connection is dropped). A client whose event buffer fills up (32
//! pending frames, unconsumed) is treated the same as a disconnected one and
//! dropped from the broadcast list — [`McpServer::notify`] never blocks the
//! calling thread waiting on a slow reader.
//!
//! This transport is only wired up for the plain HTTP/1.1 path
//! (`Server::run`/`Server::process`) — same scope as `Response::stream_pipe`
//! generally, which the HTTP/2 (`h2_handler`) and HTTP/3 (`h3_handler`)
//! handlers don't yet support for *any* response, not just this one.
//!
//! # Environment variables
//!
//! None — configure the server programmatically via the builder.

mod json_rpc;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app::App;
use crate::application::Application;
use crate::core::New;
use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

const PROTOCOL_VERSION: &str = "2024-11-05";

// ── public content types ──────────────────────────────────────────────────────

/// Content returned by tool and resource handlers.
///
/// Create with [`McpContent::text`] (plain text or JSON strings),
/// [`McpContent::json`] (marks MIME type as `application/json`),
/// [`McpContent::image`] (base64-encoded binary image data), or
/// [`McpContent::embedded`] (a resource embedded inline in a tool response).
#[derive(Clone, Debug)]
pub struct McpContent {
    /// `"text"`, `"image"`, or `"resource"`.
    pub kind: &'static str,
    /// The content string — text for `"text"`, base64 data for `"image"`,
    /// or the embedded resource's text for `"resource"`.
    pub text: String,
    /// Optional MIME type override (default `"text/plain"` for `"text"`;
    /// required in practice for `"image"`/`"resource"`, set by their
    /// constructors).
    pub mime_type: Option<String>,
    /// The resource URI — only set (and only serialized) for `"resource"`.
    pub uri: Option<String>,
}

impl McpContent {
    /// Plain-text content.
    pub fn text(s: impl Into<String>) -> Self {
        McpContent { kind: "text", text: s.into(), mime_type: None, uri: None }
    }

    /// JSON content — sets `mimeType` to `application/json`.
    pub fn json(s: impl Into<String>) -> Self {
        McpContent { kind: "text", text: s.into(), mime_type: Some("application/json".to_string()), uri: None }
    }

    /// Image content (screenshot, chart, generated art) — `data` is base64-encoded
    /// binary and `mime_type` is e.g. `"image/png"`.
    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        McpContent { kind: "image", text: data.into(), mime_type: Some(mime_type.into()), uri: None }
    }

    /// A resource embedded inline in a tool response, as opposed to one a
    /// client fetches separately via `resources/read`.
    pub fn embedded(uri: impl Into<String>, text: impl Into<String>, mime_type: impl Into<String>) -> Self {
        McpContent { kind: "resource", text: text.into(), mime_type: Some(mime_type.into()), uri: Some(uri.into()) }
    }

    fn to_content_json(&self) -> String {
        match self.kind {
            "image" => format!(
                r#"{{"type":"image","data":"{}","mimeType":"{}"}}"#,
                json_escape(&self.text),
                json_escape(self.mime_type.as_deref().unwrap_or("application/octet-stream")),
            ),
            "resource" => format!(
                r#"{{"type":"resource","resource":{{"uri":"{}","mimeType":"{}","text":"{}"}}}}"#,
                json_escape(self.uri.as_deref().unwrap_or("")),
                json_escape(self.mime_type.as_deref().unwrap_or("text/plain")),
                json_escape(&self.text),
            ),
            _ => format!(r#"{{"type":"text","text":"{}"}}"#, json_escape(&self.text)),
        }
    }

    fn mime(&self) -> &str {
        self.mime_type.as_deref().unwrap_or("text/plain")
    }
}

/// A single message in a prompt response.
#[derive(Clone, Debug)]
pub struct PromptMessage {
    /// `"user"` or `"assistant"`.
    pub role: &'static str,
    /// The message content.
    pub content: McpContent,
}

impl PromptMessage {
    /// Build a user-role message.
    pub fn user(text: impl Into<String>) -> Self {
        PromptMessage { role: "user", content: McpContent::text(text) }
    }

    /// Build an assistant-role message.
    pub fn assistant(text: impl Into<String>) -> Self {
        PromptMessage { role: "assistant", content: McpContent::text(text) }
    }

    fn to_json(&self) -> String {
        format!(
            r#"{{"role":"{}","content":{}}}"#,
            self.role,
            self.content.to_content_json(),
        )
    }
}

/// Argument definition for a prompt template.
#[derive(Clone)]
pub struct PromptArgDef {
    pub name: String,
    pub description: String,
    pub required: bool,
}

impl PromptArgDef {
    pub fn required(name: impl Into<String>, description: impl Into<String>) -> Self {
        PromptArgDef { name: name.into(), description: description.into(), required: true }
    }

    pub fn optional(name: impl Into<String>, description: impl Into<String>) -> Self {
        PromptArgDef { name: name.into(), description: description.into(), required: false }
    }
}

// ── McpContext ────────────────────────────────────────────────────────────────

/// Per-request context passed to tool handlers registered via
/// [`McpServer::tool_with_context`] — caller identity and session info that a
/// plain `Fn(&str) -> ...` tool handler has no way to see.
///
/// Constructed in [`McpServer::execute`] from the current request's headers
/// plus whatever `clientInfo` was recorded for this session at `initialize`
/// time (see [`McpServer::handle_request_with_context`]).
#[derive(Debug, Clone, Default)]
pub struct McpContext {
    /// `clientInfo.name` sent in this session's `initialize` call, if the
    /// client sent one and this request carries a recognized `Mcp-Session-Id`.
    pub client_name: Option<String>,
    /// `clientInfo.version` sent in this session's `initialize` call, under
    /// the same conditions as `client_name`.
    pub client_version: Option<String>,
    /// The `Mcp-Session-Id` header on this request, if present — the value
    /// the server minted and returned in the `initialize` response header
    /// for this session (see the module docs' Sessions section).
    pub session_id: Option<String>,
    /// Verified JWT claims as a JSON string. Not populated by anything in
    /// this crate yet — reserved for a future JWT-auth integration
    /// (MCP_TODO.md TODO-11/TODO-13); always `None` today.
    pub auth_claims: Option<String>,
}

/// `clientInfo` recorded for one session at `initialize` time, looked up by
/// `Mcp-Session-Id` for later requests in the same session. See
/// `McpServer`'s `sessions` field doc comment for the unbounded-growth caveat.
#[derive(Clone, Default)]
struct StoredClientInfo {
    name: Option<String>,
    version: Option<String>,
}

// ── ToolAnnotations ───────────────────────────────────────────────────────────

/// Behavioral hints for a tool, per the MCP 2025-03-26 spec's tool
/// annotations. Clients (Claude Desktop and others) use these to decide
/// whether to warn or ask for confirmation before calling a tool — e.g. skip
/// confirmation for a read-only tool, or warn before a destructive one.
///
/// Every field is a *hint*, not something this server enforces or verifies —
/// nothing stops a handler registered with `read_only_hint: Some(true)` from
/// actually writing to disk. A well-behaved server sets these accurately;
/// a client is free to ignore them or ask for confirmation anyway.
///
/// Register with [`McpServer::tool_annotated`]. Build one with plain struct
/// syntax — every field defaults to `None` (no hint given, the client's own
/// default applies):
///
/// ```rust
/// use rust_web_server::mcp::ToolAnnotations;
///
/// let destructive = ToolAnnotations {
///     destructive_hint: Some(true),
///     read_only_hint: Some(false),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ToolAnnotations {
    /// The tool does not modify its environment.
    pub read_only_hint: Option<bool>,
    /// The tool may perform destructive updates (only meaningful when
    /// `read_only_hint` is not `Some(true)`).
    pub destructive_hint: Option<bool>,
    /// Calling the tool repeatedly with the same arguments has no additional
    /// effect beyond the first call.
    pub idempotent_hint: Option<bool>,
    /// The tool may interact with an open-ended set of external entities
    /// (e.g. web search), as opposed to a fixed, closed set.
    pub open_world_hint: Option<bool>,
}

impl ToolAnnotations {
    /// Render as a JSON object containing only the hints that are `Some`,
    /// using the spec's camelCase key names. Returns `"{}"` if every field
    /// is `None`.
    fn to_json(self) -> String {
        let mut fields = Vec::with_capacity(4);
        if let Some(v) = self.read_only_hint {
            fields.push(format!(r#""readOnlyHint":{v}"#));
        }
        if let Some(v) = self.destructive_hint {
            fields.push(format!(r#""destructiveHint":{v}"#));
        }
        if let Some(v) = self.idempotent_hint {
            fields.push(format!(r#""idempotentHint":{v}"#));
        }
        if let Some(v) = self.open_world_hint {
            fields.push(format!(r#""openWorldHint":{v}"#));
        }
        format!("{{{}}}", fields.join(","))
    }
}

// ── LogLevel ──────────────────────────────────────────────────────────────────

/// RFC 5424 syslog severity levels, as used by the MCP `logging/setLevel`
/// request and `notifications/message` log entries — ordered from most to
/// least verbose so `level < min_level` comparisons work directly (this
/// relies on declaration order matching severity order; don't reorder the
/// variants).
///
/// ```rust
/// use rust_web_server::mcp::LogLevel;
///
/// assert!(LogLevel::Debug < LogLevel::Warning);
/// assert!(LogLevel::Emergency > LogLevel::Error);
/// assert_eq!(LogLevel::parse("warning"), Some(LogLevel::Warning));
/// assert_eq!(LogLevel::Warning.as_str(), "warning");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

impl LogLevel {
    /// Parse the MCP spec's lowercase level name. Returns `None` for
    /// anything that isn't one of the eight recognized levels.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "debug"     => Some(LogLevel::Debug),
            "info"      => Some(LogLevel::Info),
            "notice"    => Some(LogLevel::Notice),
            "warning"   => Some(LogLevel::Warning),
            "error"     => Some(LogLevel::Error),
            "critical"  => Some(LogLevel::Critical),
            "alert"     => Some(LogLevel::Alert),
            "emergency" => Some(LogLevel::Emergency),
            _           => None,
        }
    }

    /// The MCP spec's lowercase level name, e.g. `"warning"`.
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Debug     => "debug",
            LogLevel::Info      => "info",
            LogLevel::Notice    => "notice",
            LogLevel::Warning   => "warning",
            LogLevel::Error     => "error",
            LogLevel::Critical  => "critical",
            LogLevel::Alert     => "alert",
            LogLevel::Emergency => "emergency",
        }
    }
}

// ── internal handler registrations ───────────────────────────────────────────

type ToolFn     = Arc<dyn Fn(McpContext, &str) -> Result<McpContent, String>    + Send + Sync>;
type ResourceFn = Arc<dyn Fn(&str) -> Result<McpContent, String>    + Send + Sync>;
type PromptFn   = Arc<dyn Fn(&str) -> Result<Vec<PromptMessage>, String> + Send + Sync>;

#[derive(Clone)]
struct ToolDef {
    name: String,
    description: String,
    input_schema: String,
    annotations: Option<ToolAnnotations>,
    handler: ToolFn,
}

#[derive(Clone)]
struct ResourceDef {
    uri_template: String,
    name: String,
    description: String,
    handler: ResourceFn,
}

#[derive(Clone)]
struct PromptDef {
    name: String,
    description: String,
    arguments: Vec<PromptArgDef>,
    handler: PromptFn,
}

// ── McpServer ─────────────────────────────────────────────────────────────────

/// An HTTP server that implements the MCP 2024-11-05 protocol.
///
/// Register tools, resources, and prompts with the builder methods, then pass
/// the server to [`Server::run`] (or [`Server::run_tls`]) as an [`Application`].
/// Requests that do not match the MCP endpoint fall through to the built-in
/// [`App`] controller chain.
#[derive(Clone)]
pub struct McpServer {
    server_name: String,
    server_version: String,
    path: String,
    tools: Vec<ToolDef>,
    resources: Vec<ResourceDef>,
    prompts: Vec<PromptDef>,
    fallback: Option<Arc<dyn Application + Send + Sync>>,
    auth_token: Option<String>,
    /// Max items per page for `tools/list`/`resources/list`/`prompts/list`,
    /// set via [`Self::page_size`]. `None` (the default) means no pagination
    /// — every item comes back in one response, same as before pagination
    /// existed.
    page_size: Option<usize>,
    /// `clientInfo` recorded per `Mcp-Session-Id`, minted at `initialize` time.
    /// `Arc<Mutex<_>>` so every clone of this `McpServer` shares one map.
    ///
    /// This map only grows — nothing ever removes an entry, since there's no
    /// session-termination signal in the MCP Streamable HTTP transport to key
    /// eviction off of. Fine for the expected usage (a modest, roughly-stable
    /// set of long-lived AI-agent clients); a public-internet-facing server
    /// churning through unbounded distinct clients would leak memory here
    /// with no built-in reaping mechanism.
    sessions: Arc<Mutex<HashMap<String, StoredClientInfo>>>,
    /// Senders for every currently-connected `GET /mcp` SSE client, pushed to
    /// by [`Self::notify`]. `Arc<Mutex<_>>` so every clone of this `McpServer`
    /// broadcasts to the same set of listeners.
    ///
    /// Entries for clients that disconnected (or whose buffer filled up) are
    /// only pruned lazily, the next time [`Self::notify`] is called and its
    /// `try_send` fails — not proactively, since nothing else observes the
    /// underlying `Receiver` closing. A server that never calls `notify`
    /// after clients disconnect will accumulate dead entries here.
    sse_clients: Arc<Mutex<Vec<SseSender>>>,
    /// Whether `initialize`'s advertised `capabilities` includes `"logging":{}`.
    /// Set via [`Self::logging_enabled`]. This only controls what's
    /// advertised — [`Self::log`] works regardless, same as [`Self::notify`]
    /// does; a spec-honest client just wouldn't call `logging/setLevel` in
    /// the first place if the capability was never advertised.
    logging_enabled: bool,
    /// The minimum [`LogLevel`] that [`Self::log`] will actually push,
    /// settable at runtime by a client's `logging/setLevel` request. Starts
    /// at [`LogLevel::Debug`] (the least restrictive level, i.e. nothing is
    /// filtered) until a client requests otherwise.
    min_log_level: Arc<Mutex<LogLevel>>,
}

/// One `GET /mcp` SSE client's outbound channel. Bounded so a slow or stuck
/// client can't grow memory without limit; [`McpServer::notify`] uses
/// `try_send` (never blocks) and drops any client whose buffer is full.
type SseSender = SyncSender<Vec<u8>>;

/// Max buffered-but-unread SSE frames per client before it's treated as dead.
const SSE_CHANNEL_CAPACITY: usize = 32;

/// How often an idle SSE connection gets a `: keep-alive` comment.
const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);

impl McpServer {
    /// Create a new `McpServer`.  The default MCP endpoint is `POST /mcp`.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        McpServer {
            server_name: name.into(),
            server_version: version.into(),
            path: "/mcp".to_string(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            fallback: None,
            auth_token: None,
            page_size: None,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            sse_clients: Arc::new(Mutex::new(Vec::new())),
            logging_enabled: false,
            min_log_level: Arc::new(Mutex::new(LogLevel::Debug)),
        }
    }

    /// Cap `tools/list`, `resources/list`, and `prompts/list` to at most `n`
    /// items per response, enabling cursor-based pagination: a response with
    /// more items remaining includes `"nextCursor"`, an opaque string the
    /// client echoes back as `params.cursor` on its next call to get the next
    /// page. `n` is clamped to a minimum of `1`.
    ///
    /// Without calling this, every registered tool/resource/prompt is
    /// returned in a single response — the default, and the only behavior
    /// before pagination existed.
    ///
    /// ```rust
    /// use rust_web_server::mcp::McpServer;
    ///
    /// let server = McpServer::new("my-server", "1.0").page_size(50);
    /// ```
    pub fn page_size(mut self, n: usize) -> Self {
        self.page_size = Some(n.max(1));
        self
    }

    /// Push a JSON-RPC notification (no `id` — fire-and-forget, per the
    /// spec) to every client currently connected to the `GET /mcp` SSE
    /// stream, framed as an SSE `data:` event.
    ///
    /// `params_json`, if given, must already be a valid JSON value (usually
    /// an object) — it's spliced in verbatim, not escaped or re-serialized.
    ///
    /// Never blocks: a client whose event buffer is full (not reading fast
    /// enough) is treated the same as a disconnected one and dropped from
    /// the broadcast list, same as `notify` would drop it anyway.
    ///
    /// ```rust
    /// use rust_web_server::mcp::McpServer;
    ///
    /// let server = McpServer::new("my-server", "1.0");
    /// server.notify("notifications/message", Some(r#"{"level":"info","data":"hello"}"#));
    /// ```
    pub fn notify(&self, method: &str, params_json: Option<&str>) {
        let params_field = match params_json {
            Some(p) => format!(r#","params":{p}"#),
            None => String::new(),
        };
        let json = format!(r#"{{"jsonrpc":"2.0","method":"{}"{}}}"#, json_escape(method), params_field);
        self.broadcast_sse(&json);
    }

    /// Send a raw pre-built JSON-RPC message to every connected SSE client,
    /// pruning any whose channel is full or disconnected.
    fn broadcast_sse(&self, json: &str) {
        let frame = format!("data: {json}\n\n").into_bytes();
        let mut clients = self.sse_clients.lock().unwrap();
        clients.retain(|tx| tx.try_send(frame.clone()).is_ok());
    }

    /// Handle `GET /mcp`: register a new SSE client and return a
    /// `text/event-stream` response that streams from its channel until the
    /// connection closes. See the module docs' SSE section for the wire
    /// details (keep-alive interval, backpressure behavior).
    fn start_sse_stream(&self) -> Response {
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(SSE_CHANNEL_CAPACITY);
        self.sse_clients.lock().unwrap().push(tx);

        let mut response = Response::new();
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.headers.push(Header {
            name: Header::_CONTENT_TYPE.to_string(),
            value: "text/event-stream".to_string(),
        });
        response.headers.push(Header {
            name: Header::_CACHE_CONTROL.to_string(),
            value: "no-cache".to_string(),
        });
        response.headers.push(Header {
            name: "X-Accel-Buffering".to_string(),
            value: "no".to_string(),
        });
        response.stream_pipe = Some(Box::new(SseChannelReader::new(rx)));
        response
    }

    /// Advertise the `logging` capability (`"logging":{}`) in `initialize`'s
    /// response, so clients know they can call `logging/setLevel` and expect
    /// `notifications/message` log entries over the `GET /mcp` SSE stream.
    ///
    /// This only changes what's *advertised* — [`Self::log`] pushes log
    /// entries regardless of whether this was called, same as [`Self::notify`]
    /// works unconditionally. A spec-honest client simply wouldn't send
    /// `logging/setLevel` in the first place without seeing the capability.
    ///
    /// ```rust
    /// use rust_web_server::mcp::McpServer;
    ///
    /// let server = McpServer::new("my-server", "1.0").logging_enabled();
    /// ```
    pub fn logging_enabled(mut self) -> Self {
        self.logging_enabled = true;
        self
    }

    /// Push a `notifications/message` log entry to every client connected to
    /// the `GET /mcp` SSE stream, if `level` is at or above the level most
    /// recently set via a client's `logging/setLevel` request (or
    /// [`LogLevel::Debug`] — i.e. every level — if none has been set yet).
    ///
    /// `data_json` must already be a valid JSON value (the spec allows any
    /// type here, not just an object — a plain string is fine) — it's
    /// spliced in verbatim, not escaped or re-serialized. `logger`, if
    /// given, identifies the log's source (e.g. a module or subsystem name)
    /// and is escaped automatically.
    ///
    /// ```rust
    /// use rust_web_server::mcp::{LogLevel, McpServer};
    ///
    /// let server = McpServer::new("my-server", "1.0").logging_enabled();
    /// server.log(LogLevel::Warning, Some("database"), r#""connection pool exhausted""#);
    /// ```
    pub fn log(&self, level: LogLevel, logger: Option<&str>, data_json: &str) {
        if level < *self.min_log_level.lock().unwrap() {
            return;
        }
        let logger_field = match logger {
            Some(l) => format!(r#","logger":"{}""#, json_escape(l)),
            None => String::new(),
        };
        let params = format!(r#"{{"level":"{}"{logger_field},"data":{data_json}}}"#, level.as_str());
        self.notify("notifications/message", Some(&params));
    }

    /// Handle `logging/setLevel`: store the requested minimum level so
    /// subsequent [`Self::log`] calls filter against it. Returns
    /// `INVALID_PARAMS` for a missing or unrecognized `params.level`.
    fn do_set_log_level(&self, body: &str) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let level_str = json_rpc::extract_str(&params, "level")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing level".to_string()))?;
        let level = LogLevel::parse(&level_str)
            .ok_or_else(|| (json_rpc::INVALID_PARAMS, format!("Unknown log level: {level_str}")))?;
        *self.min_log_level.lock().unwrap() = level;
        Ok("{}".to_string())
    }

    /// Require a bearer token on every request to the MCP endpoint.
    ///
    /// The client must send `Authorization: Bearer <token>`. Requests with a
    /// missing or wrong token receive `401 Unauthorized` before any JSON-RPC
    /// processing occurs.
    ///
    /// Store the token in an environment variable — never hard-code it:
    ///
    /// ```rust,no_run
    /// use rust_web_server::app::App;
    /// use rust_web_server::core::New;
    ///
    /// let app = App::new()
    ///     .mcp("my-server", "1.0")
    ///     .require_bearer(std::env::var("MCP_TOKEN").expect("MCP_TOKEN not set"));
    /// ```
    ///
    /// Claude Desktop config:
    /// ```json
    /// { "mcpServers": { "my-server": {
    ///     "url": "http://localhost:7878/mcp",
    ///     "headers": { "Authorization": "Bearer <token>" }
    /// }}}
    /// ```
    pub fn require_bearer(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Wrap an existing [`Application`] so that non-MCP requests are forwarded
    /// to it instead of the built-in [`App`].
    ///
    /// Use this when your existing server has custom routes, state, or
    /// middleware that you want to keep alongside the MCP endpoint:
    ///
    /// ```rust,no_run
    /// use rust_web_server::app::App;
    /// use rust_web_server::mcp::{McpServer, McpContent};
    /// use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
    /// use rust_web_server::test_client::TestClient;
    ///
    /// let existing_app = App::with_state(42u32)
    ///     .get("/api/hello", |_req, _params, _conn, _state| {
    ///         Response::get_response(&STATUS_CODE_REASON_PHRASE.n200_ok, None, None)
    ///     });
    ///
    /// let server = McpServer::new("my-app", "1.0")
    ///     .tool("ping", "Ping", "{}", |_| Ok(McpContent::text("pong")))
    ///     .wrap(existing_app);
    ///
    /// // Both /mcp and /api/hello are now handled by the same server.
    /// let client = TestClient::new(server);
    /// ```
    pub fn wrap(mut self, app: impl Application + Send + Sync + 'static) -> Self {
        self.fallback = Some(Arc::new(app));
        self
    }

    /// Override the HTTP path for the MCP endpoint (default `"/mcp"`).
    pub fn at(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Register a callable tool.
    ///
    /// - `name` — tool identifier (snake_case recommended)
    /// - `description` — human-readable description shown to the AI
    /// - `input_schema` — JSON Schema object for the tool's arguments
    /// - `handler` — closure receiving the raw `arguments` JSON string
    ///
    /// The handler returns [`McpContent`] on success or an error string.  An
    /// error is returned to the client as `isError: true` (not a protocol error).
    ///
    /// Use [`Self::tool_with_context`] instead if the handler needs the
    /// caller's identity, session, or headers.
    pub fn tool<F>(mut self, name: &str, description: &str, input_schema: &str, handler: F) -> Self
    where
        F: Fn(&str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.tools.push(ToolDef {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema.to_string(),
            annotations: None,
            handler: Arc::new(move |_ctx: McpContext, args: &str| handler(args)),
        });
        self
    }

    /// Register a callable tool with [`ToolAnnotations`] — behavioral hints
    /// (read-only, destructive, idempotent, open-world) that MCP clients use
    /// to decide whether to warn or confirm before calling it. Otherwise
    /// identical to [`Self::tool`] — the handler still only receives
    /// `arguments`, not [`McpContext`] (there is currently no single builder
    /// combining annotations with per-request context; call [`Self::tool_with_context`]
    /// instead if you need context and don't need annotations).
    ///
    /// ```rust,no_run
    /// use rust_web_server::mcp::{McpContent, McpServer, ToolAnnotations};
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .tool_annotated(
    ///         "delete_file",
    ///         "Delete a file from disk",
    ///         r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#,
    ///         ToolAnnotations {
    ///             destructive_hint: Some(true),
    ///             read_only_hint: Some(false),
    ///             idempotent_hint: Some(true), // deleting twice = deleting once
    ///             ..Default::default()
    ///         },
    ///         |_args| Ok(McpContent::text("deleted")),
    ///     );
    /// ```
    pub fn tool_annotated<F>(
        mut self,
        name: &str,
        description: &str,
        input_schema: &str,
        annotations: ToolAnnotations,
        handler: F,
    ) -> Self
    where
        F: Fn(&str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.tools.push(ToolDef {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema.to_string(),
            annotations: Some(annotations),
            handler: Arc::new(move |_ctx: McpContext, args: &str| handler(args)),
        });
        self
    }

    /// Register a callable tool whose handler also receives [`McpContext`] —
    /// caller identity/session info derived from this request's headers and
    /// whatever `clientInfo` this session sent at `initialize` time.
    ///
    /// Same `name`/`description`/`input_schema` semantics as [`Self::tool`];
    /// the only difference is the handler's first parameter.
    ///
    /// ```rust,no_run
    /// use rust_web_server::mcp::{McpContent, McpServer};
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .tool_with_context(
    ///         "whoami",
    ///         "Report the caller's client info",
    ///         "{}",
    ///         |ctx, _args| {
    ///             let name = ctx.client_name.as_deref().unwrap_or("unknown client");
    ///             Ok(McpContent::text(format!("Called by {name}")))
    ///         },
    ///     );
    /// ```
    pub fn tool_with_context<F>(mut self, name: &str, description: &str, input_schema: &str, handler: F) -> Self
    where
        F: Fn(McpContext, &str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.tools.push(ToolDef {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema.to_string(),
            annotations: None,
            handler: Arc::new(handler),
        });
        self
    }

    /// Register a readable resource.
    ///
    /// `uri_template` uses `{param}` placeholders, e.g. `"user://{id}"`.
    /// The handler receives the full concrete URI string.
    pub fn resource<F>(mut self, uri_template: &str, name: &str, description: &str, handler: F) -> Self
    where
        F: Fn(&str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.resources.push(ResourceDef {
            uri_template: uri_template.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            handler: Arc::new(handler),
        });
        self
    }

    /// Register a prompt template.
    ///
    /// The handler receives the raw `arguments` JSON string and returns a
    /// list of [`PromptMessage`] values.
    pub fn prompt<F>(mut self, name: &str, description: &str, handler: F) -> Self
    where
        F: Fn(&str) -> Result<Vec<PromptMessage>, String> + Send + Sync + 'static,
    {
        self.prompts.push(PromptDef {
            name: name.to_string(),
            description: description.to_string(),
            arguments: vec![],
            handler: Arc::new(handler),
        });
        self
    }

    /// Register a prompt template with explicit argument definitions.
    pub fn prompt_with_args<F>(
        mut self,
        name: &str,
        description: &str,
        args: Vec<PromptArgDef>,
        handler: F,
    ) -> Self
    where
        F: Fn(&str) -> Result<Vec<PromptMessage>, String> + Send + Sync + 'static,
    {
        self.prompts.push(PromptDef {
            name: name.to_string(),
            description: description.to_string(),
            arguments: args,
            handler: Arc::new(handler),
        });
        self
    }

    // ── request dispatch ──────────────────────────────────────────────────────

    /// Process a raw JSON-RPC body and return an HTTP response.
    ///
    /// Equivalent to [`Self::handle_request_with_context`] with an empty
    /// [`McpContext`] — tool handlers registered via
    /// [`Self::tool_with_context`] will see every field as `None`. Prefer
    /// calling through [`Application::execute`] (i.e. actually serving HTTP
    /// requests) when you need real per-request context; this method exists
    /// for calling the JSON-RPC layer directly, e.g. in tests.
    pub fn handle_request(&self, body: &str) -> Response {
        self.handle_request_with_context(body, McpContext::default())
    }

    /// Process a raw JSON-RPC body with an explicit [`McpContext`] and return
    /// an HTTP response. [`Self::execute`] calls this with a context built
    /// from the request's headers and this session's stored `clientInfo`;
    /// [`Self::handle_request`] calls this with an empty context.
    ///
    /// On a successful `initialize`, this mints a new session id (reusing
    /// [`crate::request_id::generate_request_id`]'s ID generator), records
    /// `params.clientInfo` under it, and returns the id in an
    /// `Mcp-Session-Id` response header — the client is expected to echo that
    /// header back on subsequent requests so later `tools/call`s in the same
    /// session can look their `clientInfo` back up.
    pub fn handle_request_with_context(&self, body: &str, ctx: McpContext) -> Response {
        let trimmed = body.trim_start();
        if trimmed.starts_with('[') {
            return self.handle_batch(trimmed, ctx);
        }

        let method = match json_rpc::extract_str(body, "method") {
            Some(m) => m,
            None => return rpc_error(None, json_rpc::INVALID_REQUEST, "Missing method"),
        };

        let id = json_rpc::extract_id(body);

        // Notifications have no `id` — acknowledge with 202 and no body.
        if method == "notifications/initialized" || (id.is_none() && method != "ping") {
            return no_content();
        }

        let result = self.dispatch(&method, body, ctx);
        let id_str = id.as_deref().unwrap_or("null");
        let is_ok = result.is_ok();

        let mut response = json_response(&Self::format_result(id_str, &result));

        if method == "initialize" && is_ok {
            self.start_session(body, &mut response);
        }

        response
    }

    /// Process a JSON-RPC 2.0 batch request — a top-level JSON array of
    /// request objects sent in a single `POST /mcp` body, per the JSON-RPC
    /// batch spec that MCP inherits. Each element is dispatched exactly as
    /// [`Self::handle_request_with_context`] would dispatch it standalone;
    /// notifications (no `id`) contribute no entry to the response array,
    /// same as they'd get no response body outside a batch.
    ///
    /// An empty array (`[]`) is itself an invalid request per the JSON-RPC
    /// spec, so it gets one `Invalid Request` error object back rather than
    /// an empty array. A batch made up entirely of notifications produces no
    /// response body at all (`202 Accepted`), matching a single notification.
    ///
    /// If the batch contains a successful `initialize` call, the *first* one
    /// mints a session and attaches `Mcp-Session-Id` to the overall response,
    /// same as a standalone `initialize` would — sending more than one
    /// `initialize` in a batch is unusual and only the first is honored for
    /// session purposes, since one HTTP response can only carry one session id.
    fn handle_batch(&self, array_body: &str, ctx: McpContext) -> Response {
        let elements = json_rpc::split_array_elements(array_body);
        if elements.is_empty() {
            return rpc_error(None, json_rpc::INVALID_REQUEST, "Invalid Request");
        }

        let mut parts: Vec<String> = Vec::new();
        let mut session_init_body: Option<String> = None;

        for elem in &elements {
            let method = match json_rpc::extract_str(elem, "method") {
                Some(m) => m,
                None => {
                    parts.push(Self::format_result(
                        "null",
                        &Err((json_rpc::INVALID_REQUEST, "Missing method".to_string())),
                    ));
                    continue;
                }
            };

            let id = json_rpc::extract_id(elem);

            if method == "notifications/initialized" || (id.is_none() && method != "ping") {
                continue;
            }

            let result = self.dispatch(&method, elem, ctx.clone());
            let id_str = id.as_deref().unwrap_or("null");
            let is_ok = result.is_ok();

            if method == "initialize" && is_ok && session_init_body.is_none() {
                session_init_body = Some(elem.clone());
            }

            parts.push(Self::format_result(id_str, &result));
        }

        if parts.is_empty() {
            // Every element was a notification — no response body, same as a
            // single standalone notification.
            return no_content();
        }

        let mut response = json_response(&format!("[{}]", parts.join(",")));
        if let Some(init_body) = session_init_body {
            self.start_session(&init_body, &mut response);
        }
        response
    }

    /// Dispatch one already-parsed JSON-RPC `method` against `body` (the raw
    /// single-object message, whether it arrived standalone or as one element
    /// of a batch) and return the JSON-RPC `result` payload or an error.
    /// Shared by [`Self::handle_request_with_context`] and [`Self::handle_batch`]
    /// so the method table exists in exactly one place.
    fn dispatch(&self, method: &str, body: &str, ctx: McpContext) -> Result<String, (i32, String)> {
        match method {
            "initialize"     => self.do_initialize(body),
            "ping"           => Ok("{}".to_string()),
            "tools/list"     => self.do_tools_list(body),
            "tools/call"     => self.do_tools_call(body, ctx),
            "resources/list" => self.do_resources_list(body),
            "resources/read" => self.do_resources_read(body),
            "prompts/list"   => self.do_prompts_list(body),
            "prompts/get"    => self.do_prompts_get(body),
            "logging/setLevel" => self.do_set_log_level(body),
            _                => Err((json_rpc::METHOD_NOT_FOUND, format!("Unknown method: {method}"))),
        }
    }

    /// Render one JSON-RPC 2.0 response object — `{"jsonrpc":"2.0","result":...,"id":...}`
    /// or the `error` shape — from a dispatch result and its request's `id` (already
    /// rendered as a raw JSON token, e.g. `"1"`, `"\"abc\""`, or `"null"`).
    fn format_result(id_str: &str, result: &Result<String, (i32, String)>) -> String {
        match result {
            Ok(result_json) => format!(
                r#"{{"jsonrpc":"2.0","result":{result_json},"id":{id_str}}}"#
            ),
            Err((code, msg)) => {
                let escaped = json_escape(msg);
                format!(
                    r#"{{"jsonrpc":"2.0","error":{{"code":{code},"message":"{escaped}"}},"id":{id_str}}}"#
                )
            }
        }
    }

    /// Mint a new session id, record `body`'s `params.clientInfo` under it
    /// (logging the caller's identity), and attach the id to `response` as
    /// an `Mcp-Session-Id` header. Called once, from
    /// [`Self::handle_request_with_context`], right after a successful
    /// `initialize`.
    fn start_session(&self, body: &str, response: &mut Response) {
        let client_info = json_rpc::extract_raw(body, "params")
            .and_then(|p| json_rpc::extract_raw(&p, "clientInfo"));
        let (name, version) = match &client_info {
            Some(info) => (
                json_rpc::extract_str(info, "name"),
                json_rpc::extract_str(info, "version"),
            ),
            None => (None, None),
        };

        eprintln!(
            "[mcp] initialize from client {} v{}",
            name.as_deref().unwrap_or("unknown"),
            version.as_deref().unwrap_or("unknown"),
        );

        let session_id = crate::request_id::generate_request_id();
        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.clone(), StoredClientInfo { name, version });

        response.headers.push(Header {
            name: "Mcp-Session-Id".to_string(),
            value: session_id,
        });
    }

    // ── method handlers ───────────────────────────────────────────────────────

    /// Handle `initialize`. Per spec, the server must inspect the client's
    /// requested `protocolVersion` and respond with the version it actually
    /// supports — allowing the client to abort the session if incompatible —
    /// rather than blindly echoing `PROTOCOL_VERSION` regardless of what was
    /// asked for.
    ///
    /// This server implements exactly one protocol version, so "negotiation"
    /// here means: if the client asked for that same version, confirm it;
    /// otherwise tell the client the version we actually speak (older *or*
    /// newer than what was requested), which is always the lower of the two
    /// — version strings are `YYYY-MM-DD` dates, so a plain string comparison
    /// already orders them correctly with no date parsing needed.
    ///
    /// `clientInfo` is *not* handled here — [`Self::handle_request_with_context`]
    /// extracts and stores it (under a freshly minted session id) after this
    /// returns, so it's only ever parsed out of `body` once per call.
    fn do_initialize(&self, body: &str) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params");

        let client_version = params.as_deref().and_then(|p| json_rpc::extract_str(p, "protocolVersion"));

        let negotiated_version: &str = match client_version.as_deref() {
            Some(v) if v < PROTOCOL_VERSION => v,
            _ => PROTOCOL_VERSION,
        };

        let logging_cap = if self.logging_enabled { r#","logging":{}"# } else { "" };
        let caps = format!(
            r#"{{"tools":{{"listChanged":false}},"resources":{{"subscribe":false,"listChanged":false}},"prompts":{{"listChanged":false}}{logging_cap}}}"#
        );
        Ok(format!(
            r#"{{"protocolVersion":"{}","capabilities":{caps},"serverInfo":{{"name":"{}","version":"{}"}}}}"#,
            json_escape(negotiated_version),
            json_escape(&self.server_name),
            json_escape(&self.server_version),
        ))
    }

    fn do_tools_list(&self, body: &str) -> Result<String, (i32, String)> {
        let items: Vec<String> = self.tools.iter().map(|t| {
            let annotations = match t.annotations {
                Some(a) => format!(r#","annotations":{}"#, a.to_json()),
                None => String::new(),
            };
            format!(
                r#"{{"name":"{}","description":"{}","inputSchema":{}{}}}"#,
                json_escape(&t.name),
                json_escape(&t.description),
                t.input_schema,
                annotations,
            )
        }).collect();
        let (page, next_cursor) = self.paginate(&items, body)?;
        Ok(format!(r#"{{"tools":[{}]{}}}"#, page.join(","), next_cursor_json(&next_cursor)))
    }

    fn do_tools_call(&self, body: &str, ctx: McpContext) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let name = json_rpc::extract_str(&params, "name")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing tool name".to_string()))?;
        let args = json_rpc::extract_raw(&params, "arguments")
            .unwrap_or_else(|| "{}".to_string());

        let tool = self.tools.iter().find(|t| t.name == name)
            .ok_or_else(|| (json_rpc::INVALID_PARAMS, format!("Unknown tool: {name}")))?;

        match (tool.handler)(ctx, &args) {
            Ok(c) => Ok(format!(
                r#"{{"content":[{}],"isError":false}}"#,
                c.to_content_json(),
            )),
            Err(e) => {
                let escaped = json_escape(&e);
                Ok(format!(
                    r#"{{"content":[{{"type":"text","text":"{escaped}"}}],"isError":true}}"#
                ))
            }
        }
    }

    fn do_resources_list(&self, body: &str) -> Result<String, (i32, String)> {
        let items: Vec<String> = self.resources.iter().map(|r| {
            format!(
                r#"{{"uri":"{}","name":"{}","description":"{}","mimeType":"text/plain"}}"#,
                json_escape(&r.uri_template),
                json_escape(&r.name),
                json_escape(&r.description),
            )
        }).collect();
        let (page, next_cursor) = self.paginate(&items, body)?;
        Ok(format!(r#"{{"resources":[{}]{}}}"#, page.join(","), next_cursor_json(&next_cursor)))
    }

    fn do_resources_read(&self, body: &str) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let uri = json_rpc::extract_str(&params, "uri")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing uri".to_string()))?;

        let resource = self.resources.iter().find(|r| uri_matches(&r.uri_template, &uri))
            .ok_or_else(|| (json_rpc::INVALID_PARAMS, format!("Resource not found: {uri}")))?;

        match (resource.handler)(&uri) {
            Ok(c) => {
                let text_esc = json_escape(&c.text);
                let uri_esc  = json_escape(&uri);
                Ok(format!(
                    r#"{{"contents":[{{"uri":"{uri_esc}","mimeType":"{}","text":"{text_esc}"}}]}}"#,
                    c.mime(),
                ))
            }
            Err(e) => Err((json_rpc::INVALID_PARAMS, e)),
        }
    }

    fn do_prompts_list(&self, body: &str) -> Result<String, (i32, String)> {
        let items: Vec<String> = self.prompts.iter().map(|p| {
            let arg_defs: Vec<String> = p.arguments.iter().map(|a| {
                format!(
                    r#"{{"name":"{}","description":"{}","required":{}}}"#,
                    json_escape(&a.name),
                    json_escape(&a.description),
                    a.required,
                )
            }).collect();
            format!(
                r#"{{"name":"{}","description":"{}","arguments":[{}]}}"#,
                json_escape(&p.name),
                json_escape(&p.description),
                arg_defs.join(","),
            )
        }).collect();
        let (page, next_cursor) = self.paginate(&items, body)?;
        Ok(format!(r#"{{"prompts":[{}]{}}}"#, page.join(","), next_cursor_json(&next_cursor)))
    }

    /// Slice `items` (already-rendered JSON object strings for one
    /// `*/list` response) according to [`Self::page_size`] and this
    /// request's `params.cursor`, returning the page and — if more items
    /// remain — the opaque `nextCursor` to embed in the response.
    ///
    /// Without a configured `page_size`, always returns every item and no
    /// cursor, i.e. pagination is fully opt-in.
    fn paginate<'a>(&self, items: &'a [String], body: &str) -> Result<(&'a [String], Option<String>), (i32, String)> {
        let page_size = match self.page_size {
            Some(n) => n,
            None => return Ok((items, None)),
        };

        let cursor = json_rpc::extract_raw(body, "params")
            .and_then(|p| json_rpc::extract_str(&p, "cursor"));

        let offset = match cursor {
            Some(c) => decode_cursor(&c)
                .ok_or((json_rpc::INVALID_PARAMS, "Invalid cursor".to_string()))?,
            None => 0,
        };

        if offset >= items.len() {
            return Ok((&[], None));
        }

        let end = (offset + page_size).min(items.len());
        let next_cursor = if end < items.len() { Some(encode_cursor(end)) } else { None };
        Ok((&items[offset..end], next_cursor))
    }

    fn do_prompts_get(&self, body: &str) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let name = json_rpc::extract_str(&params, "name")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing prompt name".to_string()))?;
        let args = json_rpc::extract_raw(&params, "arguments")
            .unwrap_or_else(|| "{}".to_string());

        let prompt = self.prompts.iter().find(|p| p.name == name)
            .ok_or_else(|| (json_rpc::INVALID_PARAMS, format!("Unknown prompt: {name}")))?;

        match (prompt.handler)(&args) {
            Ok(msgs) => {
                let msg_jsons: Vec<String> = msgs.iter().map(|m| m.to_json()).collect();
                Ok(format!(
                    r#"{{"description":"{}","messages":[{}]}}"#,
                    json_escape(&prompt.description),
                    msg_jsons.join(","),
                ))
            }
            Err(e) => Err((json_rpc::INVALID_PARAMS, e)),
        }
    }

    /// Build the [`McpContext`] for an incoming request: the `Mcp-Session-Id`
    /// header, if present, plus whatever `clientInfo` was recorded for that
    /// session at `initialize` time (if this session is recognized).
    fn context_for(&self, request: &Request) -> McpContext {
        let session_id = request
            .get_header("Mcp-Session-Id".to_string())
            .map(|h| h.value.clone());

        let (client_name, client_version) = match &session_id {
            Some(sid) => match self.sessions.lock().unwrap().get(sid) {
                Some(info) => (info.name.clone(), info.version.clone()),
                None => (None, None),
            },
            None => (None, None),
        };

        McpContext { client_name, client_version, session_id, auth_claims: None }
    }
}

// ── SSE channel reader ────────────────────────────────────────────────────────

/// Adapts an `mpsc::Receiver<Vec<u8>>` of pre-framed SSE bytes into a
/// blocking [`std::io::Read`], so `Server::pipe_stream` (already written for
/// proxy passthrough streaming) can drive a `GET /mcp` SSE connection with no
/// changes to the server's write loop.
///
/// Blocks in [`Self::read`] until either a frame arrives, the sender side is
/// dropped (all `McpServer` clones gone — EOF, closing the connection), or
/// [`SSE_KEEPALIVE_INTERVAL`] elapses with nothing to send (writes a `:
/// keep-alive` comment instead, both to satisfy proxies that time out
/// silent connections and to surface a dead peer on the next write attempt).
struct SseChannelReader {
    rx: mpsc::Receiver<Vec<u8>>,
    leftover: Vec<u8>,
}

impl SseChannelReader {
    fn new(rx: mpsc::Receiver<Vec<u8>>) -> Self {
        SseChannelReader { rx, leftover: Vec::new() }
    }
}

impl std::io::Read for SseChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.leftover.is_empty() {
            loop {
                match self.rx.recv_timeout(SSE_KEEPALIVE_INTERVAL) {
                    Ok(frame) => { self.leftover = frame; break; }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        self.leftover = b": keep-alive\n\n".to_vec();
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
                }
            }
        }

        let n = self.leftover.len().min(buf.len());
        buf[..n].copy_from_slice(&self.leftover[..n]);
        self.leftover.drain(..n);
        Ok(n)
    }
}

// ── Application ───────────────────────────────────────────────────────────────

impl Application for McpServer {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        if request.request_uri == self.path {
            // Check bearer token before processing any MCP request.
            if let Some(expected) = &self.auth_token {
                let provided = request.headers.iter()
                    .find(|h| h.name.eq_ignore_ascii_case("authorization"))
                    .map(|h| h.value.as_str())
                    .unwrap_or("");
                let bearer = provided.strip_prefix("Bearer ").unwrap_or("");
                if bearer != expected.as_str() {
                    return Ok(unauthorized());
                }
            }

            return Ok(match request.method.as_str() {
                "POST" => {
                    let body = std::str::from_utf8(&request.body).unwrap_or("");
                    let ctx = self.context_for(request);
                    self.handle_request_with_context(body, ctx)
                }
                "GET" => self.start_sse_stream(),
                "OPTIONS" => {
                    // CORS preflight for browser-based MCP clients
                    let mut r = Response::new();
                    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
                    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
                    r.headers.push(Header {
                        name: "Allow".to_string(),
                        value: "GET, POST, OPTIONS".to_string(),
                    });
                    r
                }
                _ => {
                    let mut r = Response::new();
                    r.status_code = *STATUS_CODE_REASON_PHRASE.n405_method_not_allowed.status_code;
                    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n405_method_not_allowed.reason_phrase.to_string();
                    r.headers.push(Header {
                        name: "Allow".to_string(),
                        value: "GET, POST, OPTIONS".to_string(),
                    });
                    r.content_range_list = vec![Range::get_content_range(
                        b"MCP endpoint only accepts GET (SSE) or POST".to_vec(),
                        MimeType::TEXT_PLAIN.to_string(),
                    )];
                    r
                }
            });
        }

        // Not an MCP path — fall through to the wrapped app (or built-in App).
        match &self.fallback {
            Some(app) => app.execute(request, connection),
            None      => App::new().execute(request, connection),
        }
    }
}

// ── public helper ─────────────────────────────────────────────────────────────

/// Extract a string argument from a tool/prompt `arguments` JSON object.
///
/// ```rust
/// use rust_web_server::mcp::extract_arg;
/// assert_eq!(extract_arg(r#"{"text":"hello"}"#, "text").as_deref(), Some("hello"));
/// assert_eq!(extract_arg(r#"{}"#, "missing"), None);
/// ```
pub fn extract_arg(arguments: &str, name: &str) -> Option<String> {
    json_rpc::extract_str(arguments, name)
}

// ── internal helpers ──────────────────────────────────────────────────────────

fn json_response(body: &str) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        body.as_bytes().to_vec(),
        MimeType::APPLICATION_JSON.to_string(),
    )];
    r
}

fn no_content() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n202_accepted.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n202_accepted.reason_phrase.to_string();
    r
}

fn unauthorized() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase.to_string();
    r.headers.push(Header {
        name: "WWW-Authenticate".to_string(),
        value: "Bearer".to_string(),
    });
    r.content_range_list = vec![Range::get_content_range(
        b"Unauthorized".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    )];
    r
}

fn rpc_error(id: Option<&str>, code: i32, message: &str) -> Response {
    let id_str  = id.unwrap_or("null");
    let escaped = json_escape(message);
    json_response(&format!(
        r#"{{"jsonrpc":"2.0","error":{{"code":{code},"message":"{escaped}"}},"id":{id_str}}}"#
    ))
}

pub(crate) fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '"'  => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => { let _ = std::fmt::Write::write_fmt(&mut out, format_args!("\\u{:04x}", c as u32)); }
            c    => out.push(c),
        }
    }
    out
}

// ── pagination cursors ─────────────────────────────────────────────────────────

/// Render `,"nextCursor":"..."` for a `*/list` response, or `""` if there's
/// no next page — spliced directly after the closing `]` of the items array.
fn next_cursor_json(next_cursor: &Option<String>) -> String {
    match next_cursor {
        Some(c) => format!(r#","nextCursor":"{}""#, json_escape(c)),
        None => String::new(),
    }
}

/// Encode a `tools/list`/`resources/list`/`prompts/list` offset as the
/// opaque `nextCursor`/`params.cursor` string the MCP spec expects — just
/// base64 of the decimal offset, e.g. `50` → `"NTA="`. Callers only ever
/// treat this as opaque; the encoding is a private implementation detail of
/// this module, not a client-facing contract.
fn encode_cursor(offset: usize) -> String {
    base64_encode(offset.to_string().as_bytes())
}

/// Decode a cursor produced by [`encode_cursor`]. Returns `None` for
/// anything that isn't valid base64 of a decimal `usize` — a malformed or
/// tampered cursor, not a crash.
fn decode_cursor(cursor: &str) -> Option<usize> {
    String::from_utf8(base64_decode(cursor)?).ok()?.parse().ok()
}

const BASE64_TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(BASE64_TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { BASE64_TABLE[((n >> 6) & 0x3F) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { BASE64_TABLE[(n & 0x3F) as usize] as char } else { '=' });
    }
    out
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn sextet(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let trimmed = s.trim_end_matches('=');
    let bytes = trimmed.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4 + 3);
    for chunk in bytes.chunks(4) {
        if chunk.len() == 1 {
            return None; // not a valid base64 length
        }
        let vals: Vec<u32> = chunk.iter().map(|&b| sextet(b)).collect::<Option<Vec<_>>>()?;
        let n = vals.iter().enumerate().fold(0u32, |acc, (i, &v)| acc | (v << (18 - 6 * i)));
        out.push(((n >> 16) & 0xFF) as u8);
        if vals.len() > 2 { out.push(((n >> 8) & 0xFF) as u8); }
        if vals.len() > 3 { out.push((n & 0xFF) as u8); }
    }
    Some(out)
}

fn uri_matches(template: &str, uri: &str) -> bool {
    // Template `"user://{id}"` matches any URI starting with `"user://"`.
    match template.find('{') {
        Some(pos) => uri.starts_with(&template[..pos]),
        None      => template == uri,
    }
}
