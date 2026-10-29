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
//! The server implements the [MCP specification](https://modelcontextprotocol.io/specification/2025-11-25)
//! up to 2025-11-25, negotiating 2024-11-05, 2025-03-26, 2025-06-18 or 2025-11-25.
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
//! generally, which the HTTP/2 (`h2_server`) and HTTP/3 (`h3_handler`)
//! handlers don't yet support for *any* response, not just this one.
//!
//! # Environment variables
//!
//! None — configure the server programmatically via the builder.

pub(crate) mod json_rpc;
pub mod gateway;
mod modern;
mod tasks;

pub use modern::{InputRequired, PromptReply};
pub use tasks::{TaskContext, TaskError};

#[cfg(test)]
mod tests;

use std::collections::HashMap;
#[cfg(feature = "http2")]
use std::future::Future;
#[cfg(feature = "http2")]
use std::pin::Pin;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, RwLock};
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

/// The newest MCP protocol version this server speaks (#3).
pub(crate) const PROTOCOL_VERSION: &str = "2025-11-25";

/// Every protocol version this server speaks, oldest first.
pub(crate) const SUPPORTED_PROTOCOL_VERSIONS: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

/// The version a request without `MCP-Protocol-Version` is taken to use
/// (Streamable HTTP transport, 2025-06-18).
const ASSUMED_PROTOCOL_VERSION: &str = "2025-03-26";

/// The version to answer an `initialize` asking for `requested` with: that
/// version when this server speaks it, else the newest (the client then
/// decides whether it can continue).
pub(crate) fn negotiate_version(requested: Option<&str>) -> &'static str {
    requested
        .and_then(|v| SUPPORTED_PROTOCOL_VERSIONS.iter().find(|s| **s == v).copied())
        .unwrap_or(PROTOCOL_VERSION)
}

// ── public content types ──────────────────────────────────────────────────────

/// Content returned by tool and resource handlers.
///
/// Create with [`McpContent::text`] (plain text or JSON strings),
/// [`McpContent::json`] (marks MIME type as `application/json`),
/// [`McpContent::image`] (base64-encoded binary image data), or
/// [`McpContent::embedded`] (a resource embedded inline in a tool response).
#[derive(Clone, Debug)]
pub struct McpContent {
    /// `"text"`, `"image"`, `"audio"`, `"resource"`, `"blob"`, `"blocks"`
    /// or `"structured"`.
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

    /// Structured content (MCP 2025-06-18): `json` is a JSON value — usually an
    /// object matching the tool's [`McpServer::tool_output_schema`] — sent as
    /// the result's `structuredContent`, and as text for older clients.
    pub fn structured(json: impl Into<String>) -> Self {
        McpContent { kind: "structured", text: json.into(), mime_type: Some("application/json".to_string()), uri: None }
    }

    /// Audio content — `data` is base64-encoded audio and `mime_type` e.g.
    /// `"audio/wav"` (MCP 2025-03-26).
    pub fn audio(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        McpContent { kind: "audio", text: data.into(), mime_type: Some(mime_type.into()), uri: None }
    }

    /// A binary resource for `resources/read` — `data` is base64-encoded and
    /// sent as the contents' `blob` (with `mime_type`) instead of `text`.
    pub fn blob(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        McpContent { kind: "blob", text: data.into(), mime_type: Some(mime_type.into()), uri: None }
    }

    /// Several content items in one tool result — e.g. a text summary, an
    /// image and an embedded resource. Each item renders as it would alone.
    pub fn blocks(items: impl IntoIterator<Item = McpContent>) -> Self {
        let rendered: Vec<String> = items.into_iter().map(|item| item.to_content_json()).collect();
        McpContent { kind: "blocks", text: rendered.join(","), mime_type: None, uri: None }
    }

    /// The tool needs input from the client before it can finish (MCP
    /// 2026-07-28 multi round-trip requests): the client answers
    /// `input`'s requests and calls the tool again, with the answers in
    /// [`McpContext::input_response`] and the state in
    /// [`McpContext::request_state`]. A client on an earlier protocol
    /// version gets a tool error instead.
    pub fn input_required(input: InputRequired) -> Self {
        McpContent { kind: "input_required", text: input.to_json(), mime_type: None, uri: None }
    }

    /// A complete `tools/call` result (`{"content":[...],...}`), sent as is —
    /// how the gateway passes an upstream server's result through.
    pub(crate) fn raw_result(json: impl Into<String>) -> Self {
        McpContent { kind: "result", text: json.into(), mime_type: Some("application/json".to_string()), uri: None }
    }

    /// One content block (`{"type":…}`) sent as is — how the gateway passes
    /// an upstream prompt's message content through.
    pub(crate) fn raw_block(json: impl Into<String>) -> Self {
        McpContent { kind: "raw_block", text: json.into(), mime_type: None, uri: None }
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
            "image" | "audio" => format!(
                r#"{{"type":"{}","data":"{}","mimeType":"{}"}}"#,
                self.kind,
                json_escape(&self.text),
                json_escape(self.mime_type.as_deref().unwrap_or("application/octet-stream")),
            ),
            "blob" => format!(
                r#"{{"type":"resource","resource":{{"uri":"{}","mimeType":"{}","blob":"{}"}}}}"#,
                json_escape(self.uri.as_deref().unwrap_or("")),
                json_escape(self.mime()),
                json_escape(&self.text),
            ),
            "blocks" | "raw_block" => self.text.clone(),
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
    /// The verified caller's claims as a JSON string: the serialized
    /// [`crate::sso::OidcClaims`] when [`McpServer::require_oauth`] (`sso`
    /// feature) is configured and this request's bearer token verified, or
    /// `{"sub":"<identity>"}` for a token from
    /// [`McpServer::require_bearer_tokens`]. `None` if OAuth isn't configured, or for a
    /// hand-built context (e.g. `handle_request` in tests) that bypassed
    /// `McpServer::execute`'s auth check entirely.
    pub auth_claims: Option<String>,
    /// The raw JSON value of `params._meta.progressToken` from the
    /// triggering `tools/call` request, if the client sent one — a spec
    /// `string | number`, so this is stored pre-rendered (already correctly
    /// quoted if it's a string) rather than decoded, and spliced back
    /// verbatim by [`Self::report_progress`]. `None` for anything other than
    /// a `tools/call` whose caller asked for progress updates.
    pub progress_token: Option<String>,
    /// Shared handle back to the owning [`McpServer`]'s SSE broadcast list,
    /// used by [`Self::report_progress`]. Not `pub` — this is plumbing, not
    /// part of the context data a handler reads. `None` for a context built
    /// by hand (e.g. via [`McpServer::handle_request_with_context`] in a
    /// test) rather than through [`McpServer::execute`], in which case
    /// `report_progress` silently no-ops — there's no live server to
    /// broadcast through.
    sse_clients: Option<Arc<Mutex<Vec<SseClient>>>>,
    /// Shared flag flipped by a `notifications/cancelled` referencing this
    /// `tools/call`'s request id, checked by [`Self::is_cancelled`]. Not
    /// `pub` — see that method. `None` for anything other than a live
    /// `tools/call` dispatched through [`McpServer::execute`]/`handle_request_with_context`.
    cancellation: Option<Arc<std::sync::atomic::AtomicBool>>,
    /// Whether this session's `initialize` call declared
    /// `params.capabilities.sampling`, checked by [`Self::sample`] before
    /// sending a request the client never said it could answer.
    sampling_supported: bool,
    /// Shared handle to the owning [`McpServer`]'s in-flight
    /// server-initiated-request registry (`sampling/createMessage`,
    /// `roots/list`), used by [`Self::sample`]/[`Self::list_roots`]. Not
    /// `pub` — see `sse_clients` for the same "plumbing, not context data"
    /// reasoning; `None` under the same conditions.
    pending_replies: Option<Arc<Mutex<HashMap<String, mpsc::Sender<Result<String, String>>>>>>,
    /// Whether this session's `initialize` call declared
    /// `params.capabilities.roots`, checked by [`Self::list_roots`] before
    /// sending a request the client never said it could answer.
    roots_supported: bool,
    /// Shared handle to the owning [`McpServer`]'s session registry, used
    /// by [`Self::list_roots`] to read/write this session's cached roots
    /// list. Not `pub` — plumbing, not context data.
    sessions: Option<Arc<Mutex<HashMap<String, StoredClientInfo>>>>,
    /// The SSE response of the `tools/call` POST this context serves, when
    /// the client asked for progress and accepts `text/event-stream`:
    /// progress notifications go there, ahead of the result.
    request_stream: Option<SseSender>,
    /// What an MCP 2026-07-28 request carries per request: the client's
    /// capabilities, `inputResponses`, the verified `requestState` and the
    /// log level. `None` on earlier protocol versions.
    modern: Option<Arc<modern::ModernRequest>>,
    /// Where [`Self::start_task`] puts tasks — set when the client declared
    /// the tasks extension on this request (MCP 2026-07-28).
    tasks: Option<tasks::TaskHandle>,
}

impl McpContext {
    /// Push a `notifications/progress` event over the SSE channel for this
    /// request's `progressToken`, if the client asked for progress updates
    /// (`params._meta.progressToken` on the triggering `tools/call`) and
    /// this context was built through a live [`McpServer`] (via `execute()`,
    /// not a bare `McpContext { .. }` — see the `sse_clients` field doc).
    ///
    /// Silently does nothing in either case — a handler doesn't need to
    /// branch on whether progress reporting is actually wired up before
    /// calling this; it's always safe to call.
    ///
    /// `total` and `message` are both optional, matching the spec's
    /// `notifications/progress` shape: `{"progressToken":...,"progress":...,
    /// "total":...,"message":"..."}` (with `total`/`message` omitted when not
    /// given here).
    ///
    /// ```rust,no_run
    /// use rust_web_server::mcp::{McpContent, McpServer};
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .tool_with_context("long_job", "Do something slow", "{}", |ctx, _args| {
    ///         ctx.report_progress(0.0, Some(100.0), Some("starting"));
    ///         // ... do work ...
    ///         ctx.report_progress(100.0, Some(100.0), Some("done"));
    ///         Ok(McpContent::text("done"))
    ///     });
    /// ```
    pub fn report_progress(&self, progress: f64, total: Option<f64>, message: Option<&str>) {
        let (Some(token), Some(sse_clients)) = (&self.progress_token, &self.sse_clients) else {
            return;
        };

        let total_field = match total {
            Some(t) => format!(r#","total":{t}"#),
            None => String::new(),
        };
        let message_field = match message {
            Some(m) => format!(r#","message":"{}""#, json_escape(m)),
            None => String::new(),
        };
        let params = format!(
            r#"{{"progressToken":{token},"progress":{progress}{total_field}{message_field}}}"#
        );
        let json = render_notification("notifications/progress", Some(&params));
        if let Some(stream) = &self.request_stream {
            let _ = stream.send(format!("data: {json}\n\n").into_bytes());
        } else if let Some(session) = &self.session_id {
            send_sse_to_sessions(sse_clients, std::slice::from_ref(session), &json);
        } else {
            broadcast_sse_to(sse_clients, &json);
        }
    }

    /// `true` if the client sent `notifications/cancelled` referencing this
    /// `tools/call`'s request id.
    ///
    /// Rust cannot forcibly interrupt a running synchronous closure — same
    /// limitation [`crate::timeout::with_timeout`] documents — so this is
    /// *cooperative* cancellation: nothing happens automatically. A handler
    /// that does its own iterative work (processing a batch of items, say)
    /// can check this between steps and return early; a handler that never
    /// checks it runs to completion regardless, exactly as before this
    /// existed.
    ///
    /// Always `false` if the client never sent a cancellation, if this
    /// wasn't dispatched as a `tools/call` (the only method cancellation
    /// applies to), or if `ctx` was built without a live server behind it.
    ///
    /// ```rust,no_run
    /// use rust_web_server::mcp::{McpContent, McpServer};
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .tool_with_context("process_batch", "Process many items", "{}", |ctx, _args| {
    ///         for i in 0..1_000_000 {
    ///             if ctx.is_cancelled() {
    ///                 return Err("cancelled by client".to_string());
    ///             }
    ///             // ... process item i ...
    ///         }
    ///         Ok(McpContent::text("done"))
    ///     });
    /// ```
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.as_ref().is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// **Deprecated by MCP 2026-07-28** (SEP-2577): Sampling keeps working with
    /// clients that use it, for at least twelve months, but new servers shouldn't
    /// rely on it.
    ///
    /// Ask the connected client to run LLM inference ("sampling") and block
    /// until it replies or `timeout` elapses — the server-initiated half of
    /// `sampling/createMessage`. This reverses the usual request direction:
    /// the server sends a JSON-RPC *request* to the client over the `GET
    /// /mcp` SSE stream, and the client answers with its own `POST /mcp`
    /// carrying a JSON-RPC *response* (no `method`) that
    /// [`McpServer::handle_request_with_context`] recognizes and routes
    /// back here instead of treating it as an invalid request.
    ///
    /// Blocking (not `async fn`) is deliberate: tool handlers in this crate
    /// are plain synchronous closures — there is no async tool handler
    /// support yet (MCP_TODO.md's TODO-17) for this to `.await` inside of.
    /// The calling thread parks on the response channel for up to
    /// `timeout`; on a thread-pool server this ties up one worker for that
    /// long, same tradeoff `crate::timeout::with_timeout` already accepts
    /// for bounding a slow handler's *caller*.
    ///
    /// Fails fast, before sending anything, if: the client's `initialize`
    /// call never declared `capabilities.sampling` (it wouldn't know what
    /// to do with the request); this request has no session id (nothing to
    /// address the request to); or `ctx` has no live server behind it.
    /// Otherwise the request goes out on the session's `GET /mcp` SSE stream
    /// — waited for, within `timeout`, if the client hasn't opened it yet
    /// (clients often call a tool right after `initialize`, before their
    /// stream is up) — and it fails with a timeout error if the stream never
    /// opens or the client never responds.
    ///
    /// ```rust,no_run
    /// use rust_web_server::mcp::{McpServer, PromptMessage, SamplingRequest};
    /// use std::time::Duration;
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .tool_with_context("ask_llm", "Ask the connected client's model a question", "{}", |ctx, _args| {
    ///         let response = ctx.sample(
    ///             SamplingRequest {
    ///                 messages: vec![PromptMessage::user("What is 2+2?")],
    ///                 max_tokens: 100,
    ///                 system_prompt: None,
    ///             },
    ///             Duration::from_secs(30),
    ///         )?;
    ///         Ok(response.content)
    ///     });
    /// ```
    pub fn sample(&self, request: SamplingRequest, timeout: Duration) -> Result<SamplingResponse, String> {
        if !self.sampling_supported {
            return Err("the connected client did not declare sampling support in its initialize capabilities".to_string());
        }

        let messages_json: Vec<String> = request.messages.iter().map(|m| m.to_json()).collect();
        let system_prompt_field = match &request.system_prompt {
            Some(s) => format!(r#","systemPrompt":"{}""#, json_escape(s)),
            None => String::new(),
        };
        let params = format!(
            r#"{{"messages":[{}],"maxTokens":{}{system_prompt_field}}}"#,
            messages_json.join(","), request.max_tokens,
        );

        let result_json = self.send_and_wait("sampling/createMessage", Some(&params), timeout)?;
        parse_sampling_response(&result_json)
    }

    /// **Deprecated by MCP 2026-07-28** (SEP-2577): Roots keeps working with
    /// clients that use it, for at least twelve months, but new servers shouldn't
    /// rely on it.
    ///
    /// Ask the connected client which filesystem roots (workspace
    /// directories, mounted volumes, ...) it has access to — useful for a
    /// file-system-aware tool that should only operate within the client's
    /// declared workspace rather than the whole filesystem.
    ///
    /// Like [`Self::sample`], this reverses the usual request direction (a
    /// server-initiated `roots/list` request over SSE, answered by the
    /// client's own `POST /mcp`) and blocks the calling thread for up to
    /// `timeout` — see `sample`'s docs for why blocking is deliberate here.
    ///
    /// The result is cached per session: the first call after `initialize`
    /// (or after the client sends `notifications/roots/list_changed`, which
    /// invalidates the cache) does a live round trip; later calls in the
    /// same session return the cached list without sending anything. A tool
    /// handler can call this on every invocation without worrying about
    /// spamming the client with redundant `roots/list` requests.
    ///
    /// Fails fast, before sending anything, under the same conditions as
    /// [`Self::sample`] (translated to roots): the client never declared
    /// `capabilities.roots`, this request has no session id, or `ctx` has
    /// no live server behind it.
    ///
    /// ```rust,no_run
    /// use rust_web_server::mcp::McpServer;
    /// use std::time::Duration;
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .tool_with_context("list_workspace_files", "List files in the client's workspace", "{}", |ctx, _args| {
    ///         let roots = ctx.list_roots(Duration::from_secs(10))?;
    ///         let names: Vec<String> = roots.iter().map(|r| r.uri.clone()).collect();
    ///         Ok(rust_web_server::mcp::McpContent::text(names.join(", ")))
    ///     });
    /// ```
    pub fn list_roots(&self, timeout: Duration) -> Result<Vec<McpRoot>, String> {
        if !self.roots_supported {
            return Err("the connected client did not declare roots support in its initialize capabilities".to_string());
        }

        if let (Some(session_id), Some(sessions)) = (&self.session_id, &self.sessions) {
            if let Some(cached) = sessions.lock().unwrap().get(session_id).and_then(|info| info.roots.clone()) {
                return Ok(cached);
            }
        }

        let result_json = self.send_and_wait("roots/list", None, timeout)?;
        let roots = parse_roots_response(&result_json)?;

        if let (Some(session_id), Some(sessions)) = (&self.session_id, &self.sessions) {
            if let Some(info) = sessions.lock().unwrap().get_mut(session_id) {
                info.roots = Some(roots.clone());
            }
        }

        Ok(roots)
    }

    /// Send a JSON-RPC request to the client for this context's session and
    /// block for the reply — the shared send-and-correlate mechanic behind
    /// [`Self::sample`] and [`Self::list_roots`]. Returns the raw `result`
    /// JSON string (not yet parsed into either method's own response type)
    /// on success.
    fn send_and_wait(&self, method: &str, params_json: Option<&str>, timeout: Duration) -> Result<String, String> {
        let session_id = self.session_id.clone()
            .ok_or_else(|| format!("{method} requires a session (Mcp-Session-Id)"))?;
        let sse_clients = self.sse_clients.as_ref()
            .ok_or_else(|| format!("{method} requires a live server (this context has none)"))?;
        let pending = self.pending_replies.as_ref()
            .ok_or_else(|| format!("{method} requires a live server (this context has none)"))?;

        let request_id = format!("\"{}\"", crate::request_id::generate_request_id());
        let (tx, rx) = mpsc::channel::<Result<String, String>>();
        pending.lock().unwrap().insert(request_id.clone(), tx);

        let params_field = match params_json {
            Some(p) => format!(r#","params":{p}"#),
            None => String::new(),
        };
        let rpc = format!(r#"{{"jsonrpc":"2.0","id":{request_id},"method":"{method}"{params_field}}}"#);

        // A client may call a tool before its `GET /mcp` stream is open (the
        // SDK clients open it in the background after `initialize`): wait for
        // the stream, within the same timeout, instead of sending into nothing.
        let deadline = std::time::Instant::now() + timeout;
        let outcome = loop {
            if send_sse_to_sessions(sse_clients, std::slice::from_ref(&session_id), &rpc) > 0 {
                break rx.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                    .map_err(|_| format!("{method} timed out waiting for the client's response"));
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                break Err(format!("{method} timed out: the client has no GET /mcp stream open for this session"));
            }
            std::thread::sleep(left.min(SSE_STREAM_WAIT_POLL));
        };
        pending.lock().unwrap().remove(&request_id);

        outcome.and_then(|reply| reply)
    }
}

/// A request to ask the connected MCP client to run LLM inference
/// ("sampling") — build one and pass it to [`McpContext::sample`].
///
/// `messages` reuses [`PromptMessage`] rather than introducing a near-identical
/// type — the spec's sampling message shape (`{"role":...,"content":{"type":"text",...}}`)
/// is exactly what `PromptMessage` already models.
pub struct SamplingRequest {
    pub messages: Vec<PromptMessage>,
    pub max_tokens: u32,
    /// An optional system prompt to steer the client's model, per the
    /// spec's `params.systemPrompt`.
    pub system_prompt: Option<String>,
}

/// The client's answer to a [`SamplingRequest`], returned by
/// [`McpContext::sample`].
pub struct SamplingResponse {
    /// Almost always `"assistant"`, per spec.
    pub role: String,
    pub content: McpContent,
    /// The model the client actually used to generate this response, e.g.
    /// `"claude-3-sonnet-20240307"`.
    pub model: String,
    /// Why the model stopped, e.g. `"endTurn"`, `"maxTokens"`, `"stopSequence"`.
    /// `None` if the client didn't report one.
    pub stop_reason: Option<String>,
}

/// Parse a `sampling/createMessage` response's `result` object (already
/// extracted from the enclosing JSON-RPC envelope) into a [`SamplingResponse`].
fn parse_sampling_response(result_json: &str) -> Result<SamplingResponse, String> {
    let role = json_rpc::extract_str(result_json, "role").unwrap_or_else(|| "assistant".to_string());
    let content_json = json_rpc::extract_raw(result_json, "content")
        .ok_or_else(|| "sampling response missing content".to_string())?;
    let kind = json_rpc::extract_str(&content_json, "type").unwrap_or_else(|| "text".to_string());
    let content = match kind.as_str() {
        "image" => McpContent::image(
            json_rpc::extract_str(&content_json, "data").unwrap_or_default(),
            json_rpc::extract_str(&content_json, "mimeType").unwrap_or_default(),
        ),
        _ => McpContent::text(json_rpc::extract_str(&content_json, "text").unwrap_or_default()),
    };
    let model = json_rpc::extract_str(result_json, "model").unwrap_or_default();
    let stop_reason = json_rpc::extract_str(result_json, "stopReason");
    Ok(SamplingResponse { role, content, model, stop_reason })
}

/// One filesystem root a client has access to, returned by
/// [`McpContext::list_roots`] — a workspace directory, a mounted volume,
/// or similar. `uri` is typically a `file://` URI per spec.
#[derive(Debug, Clone)]
pub struct McpRoot {
    pub uri: String,
    /// A human-readable label for the root, if the client provided one.
    pub name: Option<String>,
}

/// Parse a `roots/list` response's `result` object (already extracted from
/// the enclosing JSON-RPC envelope) into a `Vec<McpRoot>`.
fn parse_roots_response(result_json: &str) -> Result<Vec<McpRoot>, String> {
    let roots_array = json_rpc::extract_raw(result_json, "roots")
        .ok_or_else(|| "roots/list response missing roots".to_string())?;
    let roots = json_rpc::split_array_elements(&roots_array).iter().map(|elem| {
        McpRoot {
            uri: json_rpc::extract_str(elem, "uri").unwrap_or_default(),
            name: json_rpc::extract_str(elem, "name"),
        }
    }).collect();
    Ok(roots)
}

/// Configuration installed by [`McpServer::require_oauth`] (`sso` feature).
/// Reuses [`crate::sso::jwks::JwksCache`] and [`crate::sso::OidcProvider`]
/// directly rather than re-implementing JWKS fetch/verify — see
/// `require_oauth`'s doc comment for the full rationale.
#[cfg(feature = "sso")]
struct OAuthConfig {
    jwks: crate::sso::jwks::JwksCache,
    provider: crate::sso::OidcProvider,
    audience: String,
}

/// `clientInfo` recorded for one session at `initialize` time, looked up by
/// `Mcp-Session-Id` for later requests in the same session. See
/// `McpServer`'s `sessions` field doc comment for the unbounded-growth caveat.
#[derive(Debug, Clone)]
struct StoredClientInfo {
    /// Last request in this session; idle past the server's session TTL, the
    /// session is evicted (#4).
    last_seen: std::time::Instant,
    /// The protocol version `initialize` settled on.
    protocol_version: &'static str,
    name: Option<String>,
    version: Option<String>,
    /// Whether this session's `initialize` call declared `params.capabilities.sampling`
    /// (spec: the client, not the server, declares sampling support — the
    /// presence of the key is what matters, not its contents, which are
    /// normally an empty object). Checked by [`McpContext::sample`] before
    /// sending a `sampling/createMessage` request that the client would
    /// have no idea how to answer.
    supports_sampling: bool,
    /// Whether this session's `initialize` call declared `params.capabilities.roots`,
    /// checked by [`McpContext::list_roots`] the same way `supports_sampling` gates `sample`.
    supports_roots: bool,
    /// Cached result of this session's last `roots/list` round trip, read
    /// and written by [`McpContext::list_roots`]. `None` means "never
    /// fetched, or invalidated" — set back to `None` when this session
    /// sends `notifications/roots/list_changed`, so the next `list_roots`
    /// call does a fresh round trip instead of serving stale data.
    roots: Option<Vec<McpRoot>>,
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
type PromptFn   = Arc<dyn Fn(McpContext, &str) -> Result<PromptReply, String> + Send + Sync>;
/// `Fn(argument_name, partial_value) -> candidate completion strings`.
type CompletionFn = Arc<dyn Fn(&str, &str) -> Result<Vec<String>, String> + Send + Sync>;
type ToolFilter = Arc<dyn Fn(&McpContext, &str) -> bool + Send + Sync>;
/// [`McpServer::resource_filter`] (on the URI template) and [`McpServer::prompt_filter`] (on the name).
type NameFilter = Arc<dyn Fn(&McpContext, &str) -> bool + Send + Sync>;
type ToolCallHook = Arc<dyn Fn(&McpContext, &str) -> Result<(), String> + Send + Sync>;
type ToolCallObserver = Arc<dyn Fn(&McpContext, &ToolCallRecord<'_>) + Send + Sync>;

/// How a `tools/call` ended, for [`McpServer::after_tool_call`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallOutcome {
    /// The tool ran and returned a result.
    Ok,
    /// The tool ran and returned an error (`isError: true`), or its
    /// arguments didn't match its schema.
    ToolError,
    /// A [`McpServer::before_tool_call`] hook refused it.
    Refused,
    /// [`McpServer::tool_filter`] hides the tool from this caller, or no
    /// such tool exists.
    Denied,
}

/// One finished `tools/call`, for [`McpServer::after_tool_call`].
#[derive(Debug, Clone)]
pub struct ToolCallRecord<'a> {
    pub tool: &'a str,
    pub outcome: ToolCallOutcome,
    pub duration: Duration,
}
/// An async tool handler, boxed so `AsyncToolDef` can store handlers with
/// different concrete `Future` types uniformly. The `Future` itself doesn't
/// need `Send` for [`crate::async_bridge::block_on_isolated`] (which drives
/// it to completion without ever moving it across a thread boundary), but
/// it's required here anyway for consistency with [`crate::async_state`]'s
/// equivalent handler type — the common case in practice, and one less
/// surprise for anyone switching between the two async handler styles.
#[cfg(feature = "http2")]
type AsyncToolFn = Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<McpContent, String>> + Send>> + Send + Sync>;

#[derive(Clone)]
struct ToolDef {
    name: String,
    description: String,
    input_schema: String,
    annotations: Option<ToolAnnotations>,
    meta: ToolMeta,
    handler: ToolFn,
}

/// Optional `tools/list` fields set after registration, by tool name:
/// [`McpServer::tool_title`], [`McpServer::tool_icon`],
/// [`McpServer::tool_output_schema`].
#[derive(Clone, Default)]
struct ToolMeta {
    title: Option<String>,
    icons: Vec<McpIcon>,
    output_schema: Option<String>,
}

/// An icon for the server, a tool, a resource or a prompt (MCP 2025-11-25):
/// `src` is an `https:` or `data:` URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpIcon {
    pub src: String,
    pub mime_type: Option<String>,
    /// e.g. `["48x48"]`, `["any"]` for SVG.
    pub sizes: Vec<String>,
}

impl McpIcon {
    /// An icon at `src`, with no MIME type or sizes.
    pub fn new(src: impl Into<String>) -> Self {
        McpIcon { src: src.into(), mime_type: None, sizes: Vec::new() }
    }

    /// Set the MIME type (`image/png`, `image/svg+xml`, …).
    pub fn mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Add a size (`48x48`, or `any` for scalable formats).
    pub fn size(mut self, size: impl Into<String>) -> Self {
        self.sizes.push(size.into());
        self
    }

    fn to_json(&self) -> String {
        let mut out = format!(r#"{{"src":"{}""#, json_escape(&self.src));
        if let Some(mime) = &self.mime_type {
            out.push_str(&format!(r#","mimeType":"{}""#, json_escape(mime)));
        }
        if !self.sizes.is_empty() {
            let sizes: Vec<String> = self.sizes.iter().map(|s| format!(r#""{}""#, json_escape(s))).collect();
            out.push_str(&format!(r#","sizes":[{}]"#, sizes.join(",")));
        }
        out.push('}');
        out
    }
}

fn icons_json(icons: &[McpIcon]) -> String {
    if icons.is_empty() {
        return String::new();
    }
    let icons: Vec<String> = icons.iter().map(McpIcon::to_json).collect();
    format!(r#","icons":[{}]"#, icons.join(","))
}

/// A tool registered via [`McpServer::async_tool`]/[`McpServer::register_async_tool`]
/// — stored separately from [`ToolDef`] rather than unifying the two behind
/// a handler enum, since sync and async tools are otherwise identical and
/// keeping them apart touches far less of the existing (already
/// substantial) tool-handling code. `tools/list` and `tools/call` merge
/// both collections transparently; from the client's perspective there is
/// no distinction.
#[cfg(feature = "http2")]
#[derive(Clone)]
struct AsyncToolDef {
    name: String,
    description: String,
    input_schema: String,
    annotations: Option<ToolAnnotations>,
    meta: ToolMeta,
    handler: AsyncToolFn,
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

/// One `.completion()` registration — completion candidates for a single
/// named argument of a single tool or prompt. `ref_type` is the short form
/// passed to `.completion()` (e.g. `"tool"`, `"prompt"`), matched against the
/// request's `ref.type` (e.g. `"ref/tool"`) with the `"ref/"` prefix
/// stripped, not the raw wire value.
#[derive(Clone)]
struct CompletionDef {
    ref_type: String,
    ref_name: String,
    handler: CompletionFn,
}

// ── McpServer ─────────────────────────────────────────────────────────────────

/// An HTTP server that implements the MCP protocol (2024-11-05 to 2025-11-25).
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
    /// `Arc<RwLock<_>>` (not a plain `Vec`) so a running server's tool list
    /// can be mutated at runtime — see [`Self::register_tool`]/[`Self::remove_tool`]
    /// — and every clone of this `McpServer` (each connection thread gets
    /// one) sees the same live list.
    tools: Arc<RwLock<Vec<ToolDef>>>,
    /// Tools registered via [`Self::async_tool`]/[`Self::register_async_tool`],
    /// bridged into synchronous [`Application::execute`] via
    /// [`crate::async_bridge::block_on_isolated`] at call time. Kept
    /// separate from `tools` rather than merged into one collection — see
    /// [`AsyncToolDef`]'s doc comment.
    #[cfg(feature = "http2")]
    async_tools: Arc<RwLock<Vec<AsyncToolDef>>>,
    resources: Arc<RwLock<Vec<ResourceDef>>>,
    prompts: Arc<RwLock<Vec<PromptDef>>>,
    /// Argument completion providers registered via [`Self::completion`].
    /// `initialize` advertises the `completions` capability iff this is
    /// non-empty at that moment.
    completions: Arc<RwLock<Vec<CompletionDef>>>,
    fallback: Option<Arc<dyn Application + Send + Sync>>,
    auth_token: Option<String>,
    /// `(token, identity)` pairs from [`Self::require_bearer_tokens`].
    bearer_tokens: Vec<(String, String)>,
    /// Which tools a caller may see and call ([`Self::tool_filter`]).
    tool_filter: Option<ToolFilter>,
    /// Which resources and prompts a caller may see and use.
    resource_filter: Option<NameFilter>,
    prompt_filter: Option<NameFilter>,
    /// Runs before every tool call ([`Self::before_tool_call`]).
    before_call: Vec<ToolCallHook>,
    /// Told about every finished tool call ([`Self::after_tool_call`]).
    after_call: Vec<ToolCallObserver>,
    /// Origins a browser may call the endpoint from (see
    /// [`Self::allowed_origins`]); a request with any other `Origin` is
    /// refused.
    allowed_origins: Vec<String>,
    /// Installed by [`Self::require_oauth`] (`sso` feature) — an alternative
    /// to `auth_token` that verifies a client-supplied OAuth 2.0 / OIDC
    /// bearer JWT via JWKS instead of comparing against one static shared
    /// secret. If both are somehow configured, OAuth verification takes
    /// precedence and `auth_token` is ignored (see [`Self::require_oauth`]).
    #[cfg(feature = "sso")]
    oauth: Option<Arc<OAuthConfig>>,
    /// Max items per page for `tools/list`/`resources/list`/`prompts/list`,
    /// set via [`Self::page_size`]. `None` (the default) means no pagination
    /// — every item comes back in one response, same as before pagination
    /// existed.
    page_size: Option<usize>,
    /// `clientInfo` recorded per `Mcp-Session-Id`, minted at `initialize` time.
    /// `Arc<Mutex<_>>` so every clone of this `McpServer` shares one map.
    /// Bounded (#4): a session idle for `session_ttl` is evicted when next
    /// seen or when a new one is created, `max_sessions` caps the map
    /// (oldest evicted first), and `DELETE /mcp` ends one. An evicted
    /// session's subscriptions and SSE streams go with it.
    sessions: Arc<Mutex<HashMap<String, StoredClientInfo>>>,
    /// Idle time after which a session is evicted (default 1 hour).
    session_ttl: Duration,
    /// Most sessions kept (default 10 000).
    max_sessions: usize,
    /// Most open `GET /mcp` SSE streams (default 1000); past it `503`.
    max_sse_clients: usize,
    /// `serverInfo.title` and `serverInfo.icons`.
    server_title: Option<String>,
    server_icons: Vec<McpIcon>,
    /// Senders for every currently-connected `GET /mcp` SSE client, pushed to
    /// by [`Self::notify`]. `Arc<Mutex<_>>` so every clone of this `McpServer`
    /// broadcasts to the same set of listeners.
    ///
    /// Entries for clients that disconnected (or whose buffer filled up) are
    /// only pruned lazily, the next time [`Self::notify`] is called and its
    /// `try_send` fails — not proactively, since nothing else observes the
    /// underlying `Receiver` closing. A server that never calls `notify`
    /// after clients disconnect will accumulate dead entries here.
    sse_clients: Arc<Mutex<Vec<SseClient>>>,
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
    /// In-flight `tools/call` cancellation flags, keyed by the request's raw
    /// `id` JSON token. Populated just before dispatching a `tools/call` and
    /// removed again once it returns (success, error, or the flag was never
    /// checked) — unlike `sessions`/`sse_clients`, this map's entries are
    /// always short-lived by construction, not merely "not evicted."
    /// `notifications/cancelled` looks up `params.requestId` here and flips
    /// the flag; see [`McpContext::is_cancelled`] for how (and whether) a
    /// handler ever observes it.
    cancellations: Arc<Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>>>,
    /// Resource URI -> session ids subscribed to it via
    /// `resources/subscribe`, consulted by [`Self::notify_resource_updated`].
    /// A session id is only meaningful if the client also opened `GET /mcp`
    /// with the same `Mcp-Session-Id` header (see [`SseClient`]) — a session
    /// that only ever calls `resources/subscribe` over POST with no matching
    /// SSE connection is subscribed in name only, since there's no channel
    /// to push the update over. Entries are removed by explicit
    /// `resources/unsubscribe` calls (URIs with zero subscribers are pruned
    /// entirely); a session that disconnects without unsubscribing leaves a
    /// harmless stale session id behind, same "no proactive eviction"
    /// tradeoff already documented for `sessions`.
    subscriptions: Arc<Mutex<HashMap<String, Vec<String>>>>,
    /// In-flight `sampling/createMessage` requests this server has sent to
    /// a client, keyed by the raw (already-quoted) request id token this
    /// server minted. A JSON-RPC *response* has no `method` field, so
    /// [`Self::handle_request_with_context`]/[`Self::handle_batch`]
    /// special-case a method-less body with a recognized `id` as a
    /// sampling reply (via [`Self::try_deliver_sampling_response`]) rather
    /// than an invalid request. See [`McpContext::sample`] for the
    /// send-and-block side.
    pending_replies: Arc<Mutex<HashMap<String, mpsc::Sender<Result<String, String>>>>>,
    /// Open `subscriptions/listen` streams (MCP 2026-07-28).
    listeners: Arc<Mutex<Vec<modern::Listener>>>,
    /// The HMAC key protecting `requestState` ([`Self::request_state_key`]).
    state_key: Arc<Vec<u8>>,
    /// `ttlMs` / `cacheScope` on cacheable results ([`Self::cache_hints`]).
    cache_ttl_ms: u64,
    cache_scope: &'static str,
    /// Tasks started by tools (MCP Tasks extension).
    task_store: Arc<tasks::TaskStore>,
    task_ttl_ms: u64,
    /// Tasks whose work is still running, and the cap
    /// ([`Self::max_running_tasks`]).
    running_tasks: Arc<std::sync::atomic::AtomicUsize>,
    max_running_tasks: usize,
    /// Tools usable only as tasks ([`Self::task_required`]).
    task_required_tools: Arc<RwLock<Vec<String>>>,
}

/// One `GET /mcp` SSE client: its outbound channel plus the
/// `Mcp-Session-Id` it connected with, if any. Broadcast notifications
/// (`.notify()`, `.log()`, `list_changed`) go to every client regardless of
/// `session_id`; [`McpServer::notify_resource_updated`] targets only
/// clients whose `session_id` is subscribed to the changed URI.
#[derive(Debug)]
struct SseClient {
    session_id: Option<String>,
    sender: SseSender,
}

/// One `GET /mcp` SSE client's outbound channel. Bounded so a slow or stuck
/// client can't grow memory without limit; [`McpServer::notify`] uses
/// `try_send` (never blocks) and drops any client whose buffer is full.
type SseSender = SyncSender<Vec<u8>>;

/// Max buffered-but-unread SSE frames per client before it's treated as dead.
const SSE_CHANNEL_CAPACITY: usize = 32;

/// Default idle time after which an MCP session is evicted.
pub const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(3600);
/// Default cap on MCP sessions.
pub const DEFAULT_MAX_SESSIONS: usize = 10_000;
/// Default cap on open `GET /mcp` SSE streams.
pub const DEFAULT_MAX_SSE_CLIENTS: usize = 1000;

/// How often an idle SSE connection gets a `: keep-alive` comment.
const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// Max `completion/complete` values returned in one response, per spec
/// guidance that servers SHOULD NOT return more than 100. A handler
/// returning more has the rest reported via `hasMore`/`total` rather than
/// silently included.
const MAX_COMPLETION_VALUES: usize = 100;

impl McpServer {
    /// Create a new `McpServer`.  The default MCP endpoint is `POST /mcp`.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        McpServer {
            server_name: name.into(),
            server_version: version.into(),
            path: "/mcp".to_string(),
            tools: Arc::new(RwLock::new(Vec::new())),
            #[cfg(feature = "http2")]
            async_tools: Arc::new(RwLock::new(Vec::new())),
            resources: Arc::new(RwLock::new(Vec::new())),
            prompts: Arc::new(RwLock::new(Vec::new())),
            completions: Arc::new(RwLock::new(Vec::new())),
            fallback: None,
            auth_token: None,
            bearer_tokens: Vec::new(),
            tool_filter: None,
            resource_filter: None,
            prompt_filter: None,
            before_call: Vec::new(),
            after_call: Vec::new(),
            allowed_origins: Vec::new(),
            #[cfg(feature = "sso")]
            oauth: None,
            page_size: None,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            session_ttl: DEFAULT_SESSION_TTL,
            max_sessions: DEFAULT_MAX_SESSIONS,
            max_sse_clients: DEFAULT_MAX_SSE_CLIENTS,
            server_title: None,
            server_icons: Vec::new(),
            sse_clients: Arc::new(Mutex::new(Vec::new())),
            logging_enabled: false,
            min_log_level: Arc::new(Mutex::new(LogLevel::Debug)),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            pending_replies: Arc::new(Mutex::new(HashMap::new())),
            listeners: Arc::new(Mutex::new(Vec::new())),
            state_key: Arc::new(modern::random_key()),
            cache_ttl_ms: 0,
            cache_scope: "private",
            task_store: Arc::new(tasks::TaskStore::default()),
            task_ttl_ms: 3_600_000,
            running_tasks: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            max_running_tasks: tasks::DEFAULT_MAX_RUNNING_TASKS,
            task_required_tools: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Evict a session after it has been idle this long (default 1 hour,
    /// [`DEFAULT_SESSION_TTL`]). A request with an evicted session's
    /// `Mcp-Session-Id` gets `404`, which tells the client to initialize a
    /// new session (MCP Streamable HTTP transport).
    pub fn session_ttl(mut self, ttl: Duration) -> Self {
        self.session_ttl = ttl;
        self
    }

    /// Keep at most `n` sessions (default [`DEFAULT_MAX_SESSIONS`]); a new
    /// one past that evicts the least recently used.
    pub fn max_sessions(mut self, n: usize) -> Self {
        self.max_sessions = n.max(1);
        self
    }

    /// Accept at most `n` open `GET /mcp` SSE streams (default
    /// [`DEFAULT_MAX_SSE_CLIENTS`]); another gets `503 Service Unavailable`.
    pub fn max_sse_clients(mut self, n: usize) -> Self {
        self.max_sse_clients = n;
        self
    }

    /// A human-readable title for tool `name` (`title` in `tools/list`,
    /// MCP 2025-06-18); `name` stays the identifier. No effect for an
    /// unknown name — register the tool first.
    pub fn tool_title(self, name: &str, title: impl Into<String>) -> Self {
        let title = title.into();
        self.with_tool_meta(name, |meta| meta.title = Some(title.clone()));
        self
    }

    /// Add an icon to tool `name` (`icons` in `tools/list`, MCP 2025-11-25).
    pub fn tool_icon(self, name: &str, icon: McpIcon) -> Self {
        self.with_tool_meta(name, |meta| meta.icons.push(icon.clone()));
        self
    }

    /// Declare tool `name`'s result shape: a JSON Schema object (an `object`
    /// type at the top). Clients may validate the `structuredContent` the
    /// handler returns with [`McpContent::structured`] against it.
    pub fn tool_output_schema(self, name: &str, schema: &str) -> Self {
        let schema = schema.to_string();
        self.with_tool_meta(name, |meta| meta.output_schema = Some(schema.clone()));
        self
    }

    /// Set (or replace) the behaviour hints of tool `name` — e.g. read-only,
    /// or destructive — for a tool registered without them.
    pub fn tool_annotations(self, name: &str, annotations: ToolAnnotations) -> Self {
        for tool in self.tools.write().unwrap().iter_mut().filter(|t| t.name == name) {
            tool.annotations = Some(annotations.clone());
        }
        #[cfg(feature = "http2")]
        for tool in self.async_tools.write().unwrap().iter_mut().filter(|t| t.name == name) {
            tool.annotations = Some(annotations.clone());
        }
        self
    }

    fn with_tool_meta(&self, name: &str, mut change: impl FnMut(&mut ToolMeta)) {
        for tool in self.tools.write().unwrap().iter_mut().filter(|t| t.name == name) {
            change(&mut tool.meta);
        }
        #[cfg(feature = "http2")]
        for tool in self.async_tools.write().unwrap().iter_mut().filter(|t| t.name == name) {
            change(&mut tool.meta);
        }
    }

    /// A human-readable server title (`serverInfo.title`, MCP 2025-06-18).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.server_title = Some(title.into());
        self
    }

    /// Add a server icon (`serverInfo.icons`, MCP 2025-11-25).
    pub fn icon(mut self, icon: McpIcon) -> Self {
        self.server_icons.push(icon);
        self
    }

    /// Mark a session as used now. `false` when it doesn't exist or has been
    /// idle past the TTL (then it's evicted).
    fn touch_session(&self, session_id: &str) -> bool {
        let expired = {
            let mut sessions = self.sessions.lock().unwrap();
            match sessions.get_mut(session_id) {
                None => return false,
                Some(info) if info.last_seen.elapsed() > self.session_ttl => {
                    sessions.remove(session_id);
                    true
                }
                Some(info) => {
                    info.last_seen = std::time::Instant::now();
                    false
                }
            }
        };
        if expired {
            self.forget_sessions(&[session_id.to_string()]);
            return false;
        }
        true
    }

    /// Before a new session: drop expired ones, then the least recently used
    /// until there's room under `max_sessions`.
    fn make_room_for_a_session(&self) {
        let evicted: Vec<String> = {
            let mut sessions = self.sessions.lock().unwrap();
            let ttl = self.session_ttl;
            let mut evicted: Vec<String> = sessions.iter()
                .filter(|(_, info)| info.last_seen.elapsed() > ttl)
                .map(|(id, _)| id.clone())
                .collect();
            for id in &evicted {
                sessions.remove(id);
            }
            if sessions.len() >= self.max_sessions {
                let mut by_age: Vec<(std::time::Instant, String)> =
                    sessions.iter().map(|(id, info)| (info.last_seen, id.clone())).collect();
                by_age.sort();
                for (_, id) in by_age.into_iter().take(sessions.len() + 1 - self.max_sessions) {
                    sessions.remove(&id);
                    evicted.push(id);
                }
            }
            evicted
        };
        if !evicted.is_empty() {
            self.forget_sessions(&evicted);
        }
    }

    /// The rest of an evicted or ended session: its resource subscriptions
    /// and its SSE streams (dropping the sender ends the stream).
    fn forget_sessions(&self, ids: &[String]) {
        let mut subscriptions = self.subscriptions.lock().unwrap();
        for subscribers in subscriptions.values_mut() {
            subscribers.retain(|sid| !ids.contains(sid));
        }
        subscriptions.retain(|_, subscribers| !subscribers.is_empty());
        drop(subscriptions);
        self.sse_clients.lock().unwrap()
            .retain(|client| client.session_id.as_ref().is_none_or(|sid| !ids.contains(sid)));
    }

    /// Number of live sessions (for metrics and tests).
    pub fn session_count(&self) -> usize {
        self.sessions.lock().unwrap().len()
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
        let json = render_notification(method, params_json);
        broadcast_sse_to(&self.sse_clients, &json);
        self.deliver_to_listeners(method, params_json, None);
    }

    /// Push `notifications/resources/updated` for `uri` to every session
    /// that called `resources/subscribe` for it — the mechanism behind
    /// live-updating resource panels (e.g. Claude Desktop watching a
    /// config file or a dashboard resource for changes). Call this from
    /// wherever your application actually changes the underlying data a
    /// resource represents (a file watcher, a webhook handler, a database
    /// trigger poll, ...).
    ///
    /// A no-op if nobody has subscribed to `uri` — including if every
    /// subscriber's `GET /mcp` SSE connection has since disconnected
    /// (pruned the same way [`Self::notify`] prunes dead broadcast clients,
    /// but the `subscriptions` bookkeeping for `uri` itself is untouched
    /// either way; only [`Self::do_resource_unsubscribe`] removes that).
    ///
    /// ```rust
    /// use rust_web_server::mcp::McpServer;
    ///
    /// let server = McpServer::new("my-server", "1.0");
    /// // Elsewhere, e.g. after reloading a watched config file:
    /// server.notify_resource_updated("config://main");
    /// ```
    pub fn notify_resource_updated(&self, uri: &str) {
        let params = format!(r#"{{"uri":"{}"}}"#, json_escape(uri));
        self.deliver_to_listeners("notifications/resources/updated", Some(&params), Some(uri));
        let session_ids = match self.subscriptions.lock().unwrap().get(uri) {
            Some(ids) if !ids.is_empty() => ids.clone(),
            _ => return,
        };
        let params = format!(r#"{{"uri":"{}"}}"#, json_escape(uri));
        let json = render_notification("notifications/resources/updated", Some(&params));
        send_sse_to_sessions(&self.sse_clients, &session_ids, &json);
    }

    /// Handle `GET /mcp`: register a new SSE client and return a
    /// `text/event-stream` response that streams from its channel until the
    /// connection closes. See the module docs' SSE section for the wire
    /// details (keep-alive interval, backpressure behavior).
    ///
    /// Tags the client with this request's `Mcp-Session-Id` header, if
    /// present, so [`Self::notify_resource_updated`] can route
    /// `notifications/resources/updated` to just the sessions that
    /// subscribed via `resources/subscribe` — a client that connects
    /// without the header can still receive broadcasts (`.notify()`,
    /// `.log()`, `list_changed`) but can never be a subscription target.
    fn start_sse_stream(&self, request: &Request) -> Response {
        let session_id = request.get_header("Mcp-Session-Id".to_string()).map(|h| h.value.clone());
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(SSE_CHANNEL_CAPACITY);
        {
            let mut clients = self.sse_clients.lock().unwrap();
            if clients.len() >= self.max_sse_clients {
                // Disconnected clients are otherwise only noticed on the next
                // broadcast: a comment frame finds them now (#4).
                clients.retain(|client| !matches!(client.sender.try_send(b": ping\n\n".to_vec()), Err(mpsc::TrySendError::Disconnected(_))));
            }
            if clients.len() >= self.max_sse_clients {
                let mut busy = Response::new();
                busy.status_code = *STATUS_CODE_REASON_PHRASE.n503_service_unavailable.status_code;
                busy.reason_phrase = STATUS_CODE_REASON_PHRASE.n503_service_unavailable.reason_phrase.to_string();
                busy.headers.push(Header { name: "Retry-After".into(), value: "30".into() });
                return busy;
            }
            clients.push(SseClient { session_id: session_id.map(|v| v.into_owned()), sender: tx });
        }

        let mut response = Response::new();
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.headers.push(Header {
            name: Header::_CONTENT_TYPE.into(),
            value: "text/event-stream".into(),
        });
        response.headers.push(Header {
            name: Header::_CACHE_CONTROL.into(),
            value: "no-cache".into(),
        });
        response.headers.push(Header {
            name: "X-Accel-Buffering".into(),
            value: "no".into(),
        });
        response.stream_pipe = Some(Box::new(crate::sse::SseReader::new(rx, SSE_KEEPALIVE_INTERVAL)));
        response
    }

    /// **Deprecated by MCP 2026-07-28** (SEP-2577): Logging keeps working with
    /// clients that use it, for at least twelve months, but new servers shouldn't
    /// rely on it.
    ///
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

    /// **Deprecated by MCP 2026-07-28** (SEP-2577): Logging keeps working with
    /// clients that use it, for at least twelve months, but new servers shouldn't
    /// rely on it.
    ///
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

    // ── dynamic registration ──────────────────────────────────────────────────
    //
    // Unlike `.tool()`/`.resource()`/`.prompt()` (consuming builders, called
    // before the server starts serving requests), these take `&self` and can
    // be called at any time from any thread holding a clone of this
    // `McpServer` — e.g. after discovering a plugin, connecting to a
    // database, or reacting to a hot-reloaded config file. Every clone
    // shares the same underlying `Arc<RwLock<Vec<_>>>`, so a mutation made
    // through one clone is immediately visible to every other clone,
    // including the ones handling concurrent requests on other threads.
    //
    // Each registration/removal pushes the corresponding
    // `notifications/{tools,resources,prompts}/list_changed` event (no
    // params, per spec) to every `GET /mcp` SSE client via `.notify()`.

    /// Register a callable tool at runtime, exactly like [`Self::tool`] but
    /// without needing to own the server (and usable after it's already
    /// serving requests). Pushes `notifications/tools/list_changed`.
    ///
    /// ```rust
    /// use rust_web_server::mcp::{McpContent, McpServer};
    ///
    /// let server = McpServer::new("my-server", "1.0");
    ///
    /// // Later, from any thread holding a clone of `server`:
    /// server.register_tool("refresh_cache", "Reload the in-memory cache", "{}", |_args| {
    ///     Ok(McpContent::text("cache refreshed"))
    /// });
    /// let existed = server.remove_tool("refresh_cache");
    /// assert!(existed);
    /// ```
    pub fn register_tool<F>(&self, name: &str, description: &str, input_schema: &str, handler: F)
    where
        F: Fn(&str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.tools.write().unwrap().push(ToolDef {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema.to_string(),
            annotations: None,
            meta: ToolMeta::default(),
            handler: Arc::new(move |_ctx: McpContext, args: &str| handler(args)),
        });
        self.notify("notifications/tools/list_changed", None);
    }

    /// Register an `async fn` tool handler at runtime, exactly like
    /// [`Self::async_tool`] but usable after the server is already serving
    /// requests. Requires the `http2` feature — see `async_tool`'s docs for
    /// the async-to-sync bridging details.
    #[cfg(feature = "http2")]
    pub fn register_async_tool<F, Fut>(&self, name: &str, description: &str, input_schema: &str, handler: F)
    where
        F: Fn(&str) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<McpContent, String>> + Send + 'static,
    {
        self.async_tools.write().unwrap().push(AsyncToolDef {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema.to_string(),
            annotations: None,
            meta: ToolMeta::default(),
            handler: Arc::new(move |args: String| Box::pin(handler(&args))),
        });
        self.notify("notifications/tools/list_changed", None);
    }

    /// Remove a previously-registered tool by name — a sync tool registered
    /// via [`Self::tool`]/[`Self::register_tool`], or (with the `http2`
    /// feature) an async one via [`Self::async_tool`]/[`Self::register_async_tool`];
    /// both collections are checked. Returns `true` if a tool with that name
    /// existed in either and was removed. Pushes `notifications/tools/list_changed`
    /// only when something was actually removed.
    pub fn remove_tool(&self, name: &str) -> bool {
        #[cfg_attr(not(feature = "http2"), allow(unused_mut))]
        let mut removed = {
            let mut tools = self.tools.write().unwrap();
            let before = tools.len();
            tools.retain(|t| t.name != name);
            tools.len() != before
        };

        #[cfg(feature = "http2")]
        {
            let mut async_tools = self.async_tools.write().unwrap();
            let before = async_tools.len();
            async_tools.retain(|t| t.name != name);
            removed = removed || async_tools.len() != before;
        }

        if removed {
            self.notify("notifications/tools/list_changed", None);
        }
        removed
    }

    /// Register a readable resource at runtime, exactly like [`Self::resource`].
    /// Pushes `notifications/resources/list_changed`.
    pub fn register_resource<F>(&self, uri_template: &str, name: &str, description: &str, handler: F)
    where
        F: Fn(&str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.resources.write().unwrap().push(ResourceDef {
            uri_template: uri_template.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            handler: Arc::new(handler),
        });
        self.notify("notifications/resources/list_changed", None);
    }

    /// Remove a previously-registered resource by its exact `uri_template`
    /// (the same string passed to [`Self::register_resource`]/[`Self::resource`],
    /// not a concrete URI). Returns `true` if it existed. Pushes
    /// `notifications/resources/list_changed` only when something was removed.
    pub fn remove_resource(&self, uri_template: &str) -> bool {
        let removed = {
            let mut resources = self.resources.write().unwrap();
            let before = resources.len();
            resources.retain(|r| r.uri_template != uri_template);
            resources.len() != before
        };
        if removed {
            self.notify("notifications/resources/list_changed", None);
        }
        removed
    }

    /// Register a prompt template at runtime, exactly like [`Self::prompt`]
    /// (no argument definitions — use [`Self::remove_prompt`] +
    /// [`Self::register_prompt`] if you need to change a prompt's arguments
    /// later; there is no dynamic equivalent of [`Self::prompt_with_args`]).
    /// Pushes `notifications/prompts/list_changed`.
    pub fn register_prompt<F>(&self, name: &str, description: &str, handler: F)
    where
        F: Fn(&str) -> Result<Vec<PromptMessage>, String> + Send + Sync + 'static,
    {
        self.prompts.write().unwrap().push(PromptDef {
            name: name.to_string(),
            description: description.to_string(),
            arguments: vec![],
            handler: Arc::new(move |_ctx: McpContext, args: &str| handler(args).map(PromptReply::Messages)),
        });
        self.notify("notifications/prompts/list_changed", None);
    }

    /// Remove a previously-registered prompt by name. Returns `true` if it
    /// existed. Pushes `notifications/prompts/list_changed` only when
    /// something was removed.
    pub fn remove_prompt(&self, name: &str) -> bool {
        let removed = {
            let mut prompts = self.prompts.write().unwrap();
            let before = prompts.len();
            prompts.retain(|p| p.name != name);
            prompts.len() != before
        };
        if removed {
            self.notify("notifications/prompts/list_changed", None);
        }
        removed
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
    ///
    /// An empty token refuses every request (fails closed) rather than
    /// accepting an empty `Authorization: Bearer` (since 17.113.1). Tokens
    /// are compared in constant time.
    pub fn require_bearer(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Accept several bearer tokens, each naming a caller: a request with
    /// `Authorization: Bearer <token>` is that caller, and handlers see
    /// `{"sub":"<identity>"}` as [`McpContext::auth_claims`] — what
    /// [`Self::tool_filter`] and the [gateway](gateway::McpGateway) decide
    /// by. Any other token gets `401`. Combines with
    /// [`Self::require_bearer`] (whose token carries no identity); OAuth
    /// ([`Self::require_oauth`]) replaces both. Empty tokens never match;
    /// tokens are compared in constant time.
    ///
    /// ```rust
    /// use rust_web_server::mcp::McpServer;
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .require_bearer_tokens([("token-for-alice", "alice"), ("token-for-ci", "ci")]);
    /// ```
    pub fn require_bearer_tokens<I, T, N>(mut self, tokens: I) -> Self
    where
        I: IntoIterator<Item = (T, N)>,
        T: Into<String>,
        N: Into<String>,
    {
        self.bearer_tokens.extend(tokens.into_iter().map(|(token, identity)| (token.into(), identity.into())));
        self
    }

    /// Show and allow only the tools `allow(ctx, tool_name)` accepts for a
    /// request: `tools/list` leaves the others out, and `tools/call` answers
    /// them as it would an unknown tool. `ctx.auth_claims` identifies the
    /// caller ([`Self::require_bearer_tokens`], [`Self::require_oauth`]).
    ///
    /// ```rust
    /// use rust_web_server::mcp::McpServer;
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .tool_filter(|ctx, tool| !tool.starts_with("admin_") || ctx.auth_claims.as_deref() == Some(r#"{"sub":"root"}"#));
    /// ```
    pub fn tool_filter<F>(mut self, allow: F) -> Self
    where
        F: Fn(&McpContext, &str) -> bool + Send + Sync + 'static,
    {
        self.tool_filter = Some(Arc::new(allow));
        self
    }

    /// Which resources a caller may list and read, like
    /// [`Self::tool_filter`] for tools: `allow(ctx, uri_template)`; a hidden
    /// resource is left out of `resources/list` and
    /// `resources/templates/list`, and reading it answers "not found".
    pub fn resource_filter<F>(mut self, allow: F) -> Self
    where
        F: Fn(&McpContext, &str) -> bool + Send + Sync + 'static,
    {
        self.resource_filter = Some(Arc::new(allow));
        self
    }

    /// Which prompts a caller may list and get: `allow(ctx, name)`.
    pub fn prompt_filter<F>(mut self, allow: F) -> Self
    where
        F: Fn(&McpContext, &str) -> bool + Send + Sync + 'static,
    {
        self.prompt_filter = Some(Arc::new(allow));
        self
    }

    fn resource_allowed(&self, ctx: &McpContext, uri_template: &str) -> bool {
        self.resource_filter.as_ref().is_none_or(|allow| allow(ctx, uri_template))
    }

    fn prompt_allowed(&self, ctx: &McpContext, name: &str) -> bool {
        self.prompt_filter.as_ref().is_none_or(|allow| allow(ctx, name))
    }

    /// Run `check` before every `tools/call` that [`Self::tool_filter`]
    /// allows, with the caller and the tool's name. `Err(message)` refuses
    /// the call: the client gets a tool error (`isError: true`) with
    /// `message`, which an agent can read and act on (e.g. "rate limited,
    /// retry in 12 s"). Hooks run in the order added; the first refusal wins.
    ///
    /// ```rust
    /// use rust_web_server::mcp::McpServer;
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .before_tool_call(|_ctx, tool| if tool == "deploy" { Err("deploys are frozen today".into()) } else { Ok(()) });
    /// ```
    pub fn before_tool_call<F>(mut self, check: F) -> Self
    where
        F: Fn(&McpContext, &str) -> Result<(), String> + Send + Sync + 'static,
    {
        self.before_call.push(Arc::new(check));
        self
    }

    /// Call `observe` after every `tools/call` with the caller's context and
    /// a [`ToolCallRecord`]: the tool, how it ended (including calls
    /// [`Self::before_tool_call`] refused and tools [`Self::tool_filter`]
    /// hides) and how long it took. For audit logs and metrics; it runs on
    /// the request's thread, so keep it quick.
    pub fn after_tool_call<F>(mut self, observe: F) -> Self
    where
        F: Fn(&McpContext, &ToolCallRecord<'_>) + Send + Sync + 'static,
    {
        self.after_call.push(Arc::new(observe));
        self
    }

    fn tool_allowed(&self, ctx: &McpContext, name: &str) -> bool {
        self.tool_filter.as_ref().is_none_or(|allow| allow(ctx, name))
    }

    /// Checks a request's bearer token against [`Self::require_bearer`] and
    /// [`Self::require_bearer_tokens`]: `Ok(claims)` (`Some` for a named
    /// token), or the `401` to send.
    fn check_bearer(&self, request: &Request) -> Result<Option<String>, Response> {
        if self.auth_token.is_none() && self.bearer_tokens.is_empty() {
            return Ok(None);
        }
        let provided = request.headers.iter()
            .find(|h| h.name.eq_ignore_ascii_case("authorization"))
            .map(|h| h.value.as_ref())
            .unwrap_or("");
        let bearer = provided.strip_prefix("Bearer ").unwrap_or("");
        // Compare against every token so the time taken doesn't tell which one matched.
        let mut identity = None;
        for (token, name) in &self.bearer_tokens {
            if bearer_matches(token, bearer) && identity.is_none() {
                identity = Some(name);
            }
        }
        if let Some(identity) = identity {
            return Ok(Some(format!(r#"{{"sub":"{}"}}"#, json_escape(identity))));
        }
        match &self.auth_token {
            Some(expected) if bearer_matches(expected, bearer) => Ok(None),
            _ => Err(unauthorized()),
        }
    }

    /// Browser origins allowed to call the MCP endpoint, e.g.
    /// `["https://inspector.example.com"]`. A request carrying an `Origin`
    /// header that isn't listed gets `403` (the MCP transport spec requires
    /// validating `Origin`: without it a web page — or a DNS-rebinding
    /// attack — can drive a local MCP server from the visitor's browser).
    /// Requests without `Origin` (CLI and agent clients) are unaffected.
    /// Empty by default: no browser origin is allowed (since 17.113.1).
    pub fn allowed_origins<I, S>(mut self, origins: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_origins = origins.into_iter().map(Into::into).collect();
        self
    }

    /// Require an OAuth 2.0 / OIDC bearer JWT — verified against a live
    /// JWKS endpoint — on every request to the MCP endpoint (MCP
    /// 2025-03-26's OAuth 2.0 authorization requirement), instead of one
    /// static shared secret. Requires the `sso` feature.
    ///
    /// Reuses [`crate::sso::jwks::JwksCache`] directly — the "leverage
    /// point" this feature was scoped around — rather than reimplementing
    /// JWT/JWKS verification. `provider` supplies the issuer, JWKS URI, and
    /// (for `GET /.well-known/oauth-protected-resource`, RFC 9728) the
    /// authorization server's issuer; `audience` is the value tokens must
    /// carry in `aud` — ideally this server's canonical URL
    /// (`https://mcp.example.com/mcp`, RFC 8707), which is then also the
    /// metadata's `resource`; use a preset
    /// (`OidcProvider::google()`, etc.) or `OidcProvider::discover(issuer)?`.
    ///
    /// A request with a missing, malformed, or invalid-signature bearer
    /// token gets `401` with `WWW-Authenticate: Bearer` before any JSON-RPC
    /// processing occurs — matching [`Self::require_bearer`]'s behavior. On
    /// success, the verified claims are available to `.tool_with_context()`
    /// handlers as JSON via [`McpContext::auth_claims`].
    ///
    /// If both this and [`Self::require_bearer`] are configured, OAuth
    /// verification takes precedence and the static token is never checked
    /// — configuring both is not a supported combination, just an
    /// unambiguous tie-break for an unusual configuration.
    ///
    /// ```rust,no_run
    /// use rust_web_server::app::App;
    /// use rust_web_server::core::New;
    /// use rust_web_server::sso::OidcProvider;
    ///
    /// let app = App::new()
    ///     .mcp("my-server", "1.0")
    ///     .require_oauth(OidcProvider::google(), "my-mcp-client-id");
    /// ```
    #[cfg(feature = "sso")]
    pub fn require_oauth(mut self, provider: crate::sso::OidcProvider, audience: impl Into<String>) -> Self {
        self.oauth = Some(Arc::new(OAuthConfig {
            jwks: crate::sso::jwks::JwksCache::new(&provider.jwks_uri),
            provider,
            audience: audience.into(),
        }));
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
    pub fn tool<F>(self, name: &str, description: &str, input_schema: &str, handler: F) -> Self
    where
        F: Fn(&str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.tools.write().unwrap().push(ToolDef {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema.to_string(),
            annotations: None,
            meta: ToolMeta::default(),
            handler: Arc::new(move |_ctx: McpContext, args: &str| handler(args)),
        });
        self
    }

    /// Register an `async fn` tool handler — for a tool whose work is
    /// naturally async (an `AsyncClient` HTTP call, an async database
    /// query, awaiting another future) rather than forcing it through a
    /// blocking call inside a plain [`Self::tool`] handler.
    ///
    /// Requires the `http2` feature (tokio). `tools/call` bridges into the
    /// handler's future via [`crate::async_bridge::block_on_isolated`] —
    /// the same mechanism [`crate::proxy::H2ReverseProxy`] and
    /// [`crate::async_state::AsyncAppWithState`] already use to call async
    /// code from a synchronous [`Application::execute`] — rather than
    /// `tokio::task::block_in_place`, which panics outside a `multi_thread`
    /// runtime; `block_on_isolated` works under any runtime flavor,
    /// including `current_thread`.
    ///
    /// Like [`Self::tool`] (not [`Self::tool_with_context`]), the handler
    /// only receives `arguments` — there is no async equivalent of
    /// `tool_with_context` or `tool_annotated` yet.
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "http2")]
    /// # {
    /// use rust_web_server::mcp::{McpContent, McpServer};
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .async_tool("call_api", "Call an external API", "{}", |_args: &str| async move {
    ///         Ok(McpContent::text("response"))
    ///     });
    /// # }
    /// ```
    #[cfg(feature = "http2")]
    pub fn async_tool<F, Fut>(self, name: &str, description: &str, input_schema: &str, handler: F) -> Self
    where
        F: Fn(&str) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<McpContent, String>> + Send + 'static,
    {
        self.async_tools.write().unwrap().push(AsyncToolDef {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema.to_string(),
            annotations: None,
            meta: ToolMeta::default(),
            handler: Arc::new(move |args: String| Box::pin(handler(&args))),
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
        self,
        name: &str,
        description: &str,
        input_schema: &str,
        annotations: ToolAnnotations,
        handler: F,
    ) -> Self
    where
        F: Fn(&str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.tools.write().unwrap().push(ToolDef {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema.to_string(),
            annotations: Some(annotations),
            meta: ToolMeta::default(),
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
    pub fn tool_with_context<F>(self, name: &str, description: &str, input_schema: &str, handler: F) -> Self
    where
        F: Fn(McpContext, &str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.tools.write().unwrap().push(ToolDef {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema.to_string(),
            annotations: None,
            meta: ToolMeta::default(),
            handler: Arc::new(handler),
        });
        self
    }

    /// Register a tool whose arguments and result are Rust types: the
    /// `inputSchema` comes from `In` and the `outputSchema` from `Out`
    /// ([`JsonSchema`](crate::schema::JsonSchema), usually
    /// `#[derive(JsonSchema)]`), the arguments are deserialized into `In`, and
    /// the handler's `Out` is sent as `structuredContent` (and as JSON text
    /// for older clients). Requires the `serde` feature.
    ///
    /// Arguments that don't deserialize come back as an `isError` result
    /// naming the problem, as do the handler's errors. MCP requires an
    /// `outputSchema` to describe an object, so an `Out` that isn't a struct
    /// is sent as JSON text without one.
    ///
    /// ```rust,ignore
    /// use rust_web_server::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Deserialize, JsonSchema)]
    /// struct Lookup { sku: String }
    ///
    /// #[derive(Serialize, JsonSchema)]
    /// struct Stock { sku: String, available: u32 }
    ///
    /// let server = McpServer::new("shop", "1.0")
    ///     .typed_tool("stock", "Units available for a SKU", |_ctx, args: Lookup| {
    ///         Ok(Stock { available: 3, sku: args.sku })
    ///     });
    /// ```
    #[cfg(feature = "serde")]
    pub fn typed_tool<In, Out, F>(self, name: &str, description: &str, handler: F) -> Self
    where
        In: serde::de::DeserializeOwned + crate::schema::JsonSchema,
        Out: serde::Serialize + crate::schema::JsonSchema,
        F: Fn(McpContext, In) -> Result<Out, String> + Send + Sync + 'static,
    {
        let output_schema = Out::json_schema();
        let structured = crate::schema::is_object_schema(&output_schema);
        let server = self.tool_with_context(name, description, &In::json_schema(), move |ctx, args| {
            let args = if args.trim().is_empty() { "{}" } else { args };
            let input: In = serde_json::from_str(args).map_err(|e| format!("Invalid arguments: {e}"))?;
            let output = handler(ctx, input)?;
            let json = serde_json::to_string(&output).map_err(|e| format!("Could not serialize the result: {e}"))?;
            Ok(if structured { McpContent::structured(json) } else { McpContent::json(json) })
        });
        if structured {
            server.tool_output_schema(name, &output_schema)
        } else {
            server
        }
    }

    /// Register a readable resource.
    ///
    /// `uri_template` uses `{param}` placeholders, e.g. `"user://{id}"`.
    /// The handler receives the full concrete URI string.
    pub fn resource<F>(self, uri_template: &str, name: &str, description: &str, handler: F) -> Self
    where
        F: Fn(&str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.resources.write().unwrap().push(ResourceDef {
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
    pub fn prompt<F>(self, name: &str, description: &str, handler: F) -> Self
    where
        F: Fn(&str) -> Result<Vec<PromptMessage>, String> + Send + Sync + 'static,
    {
        self.prompts.write().unwrap().push(PromptDef {
            name: name.to_string(),
            description: description.to_string(),
            arguments: vec![],
            handler: Arc::new(move |_ctx: McpContext, args: &str| handler(args).map(PromptReply::Messages)),
        });
        self
    }

    /// Register a prompt template with explicit argument definitions.
    pub fn prompt_with_args<F>(
        self,
        name: &str,
        description: &str,
        args: Vec<PromptArgDef>,
        handler: F,
    ) -> Self
    where
        F: Fn(&str) -> Result<Vec<PromptMessage>, String> + Send + Sync + 'static,
    {
        self.prompts.write().unwrap().push(PromptDef {
            name: name.to_string(),
            description: description.to_string(),
            arguments: args,
            handler: Arc::new(move |_ctx: McpContext, args: &str| handler(args).map(PromptReply::Messages)),
        });
        self
    }

    /// [`Self::prompt_with_args`] whose handler also gets the request's
    /// [`McpContext`] and may answer [`PromptReply::InputRequired`] to ask
    /// the client for input first (MCP 2026-07-28).
    pub fn prompt_with_context<F>(
        self,
        name: &str,
        description: &str,
        args: Vec<PromptArgDef>,
        handler: F,
    ) -> Self
    where
        F: Fn(McpContext, &str) -> Result<PromptReply, String> + Send + Sync + 'static,
    {
        self.prompts.write().unwrap().push(PromptDef {
            name: name.to_string(),
            description: description.to_string(),
            arguments: args,
            handler: Arc::new(handler),
        });
        self
    }

    /// Register an argument-completion provider for one named argument of a
    /// tool or prompt, so clients like Cursor and VS Code can offer
    /// autocomplete while the user fills in that argument.
    ///
    /// `ref_type` is `"tool"` or `"prompt"` — matched against the incoming
    /// `completion/complete` request's `ref.type` (`"ref/tool"`/`"ref/prompt"`
    /// on the wire) with the `"ref/"` prefix stripped. `ref_name` is the
    /// tool or prompt name this applies to. The handler receives the
    /// argument's name and whatever partial value the user has typed so
    /// far, and returns candidate completion strings (or an error, mapped
    /// to a JSON-RPC `INVALID_PARAMS` response).
    ///
    /// `initialize` advertises the `completions` capability automatically
    /// once at least one `.completion()` has been registered — there's no
    /// separate opt-in flag to remember.
    ///
    /// ```rust
    /// use rust_web_server::mcp::McpServer;
    ///
    /// let server = McpServer::new("my-server", "1.0")
    ///     .completion("tool", "deploy", |arg_name, _partial| {
    ///         match arg_name {
    ///             "region" => Ok(vec!["us-east-1".to_string(), "eu-west-1".to_string()]),
    ///             _ => Ok(vec![]),
    ///         }
    ///     });
    /// ```
    pub fn completion<F>(self, ref_type: &str, ref_name: &str, handler: F) -> Self
    where
        F: Fn(&str, &str) -> Result<Vec<String>, String> + Send + Sync + 'static,
    {
        self.completions.write().unwrap().push(CompletionDef {
            ref_type: ref_type.to_string(),
            ref_name: ref_name.to_string(),
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

        let id = json_rpc::extract_id(body);

        let method = match json_rpc::extract_str(body, "method") {
            Some(m) => m,
            None => {
                // No `method` — either malformed, or a JSON-RPC *response*
                // to a sampling/createMessage request this server sent
                // earlier (responses never carry `method`).
                if let Some(id) = &id {
                    if self.try_deliver_sampling_response(id, body) {
                        return no_content();
                    }
                }
                return rpc_error(None, json_rpc::INVALID_REQUEST, "Missing method");
            }
        };

        if method == "notifications/cancelled" {
            self.handle_cancellation(body);
            return no_content();
        }

        if method == "notifications/roots/list_changed" {
            self.invalidate_roots_cache(&ctx.session_id);
            return no_content();
        }

        // Notifications have no `id` — acknowledge with 202 and no body.
        if method == "notifications/initialized" || (id.is_none() && method != "ping") {
            return no_content();
        }

        let result = self.dispatch_with_cancellation(&method, body, ctx, &id);
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
            let id = json_rpc::extract_id(elem);

            let method = match json_rpc::extract_str(elem, "method") {
                Some(m) => m,
                None => {
                    if let Some(id) = &id {
                        if self.try_deliver_sampling_response(id, elem) {
                            continue; // delivered; no response entry, same as any notification
                        }
                    }
                    parts.push(Self::format_result(
                        "null",
                        &Err((json_rpc::INVALID_REQUEST, "Missing method".to_string())),
                    ));
                    continue;
                }
            };

            if method == "notifications/cancelled" {
                self.handle_cancellation(elem);
                continue;
            }

            if method == "notifications/roots/list_changed" {
                self.invalidate_roots_cache(&ctx.session_id);
                continue;
            }

            if method == "notifications/initialized" || (id.is_none() && method != "ping") {
                continue;
            }

            let result = self.dispatch_with_cancellation(&method, elem, ctx.clone(), &id);
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
            "tools/list"     => self.do_tools_list(body, &ctx),
            "tools/call"     => self.do_tools_call(body, ctx),
            "resources/list" => self.do_resources_list(body, &ctx),
            "resources/read" => self.do_resources_read(body, &ctx),
            "resources/templates/list" => self.do_resource_templates_list(body, &ctx),
            "resources/subscribe"   => self.do_resource_subscribe(body, ctx.session_id),
            "resources/unsubscribe" => self.do_resource_unsubscribe(body, ctx.session_id),
            "prompts/list"   => self.do_prompts_list(body, &ctx),
            "prompts/get"    => self.do_prompts_get(body, ctx),
            "logging/setLevel" => self.do_set_log_level(body),
            "completion/complete" => self.do_completion(body),
            _                => Err((json_rpc::METHOD_NOT_FOUND, format!("Unknown method: {method}"))),
        }
    }

    /// Wraps [`Self::dispatch`] for one request, additionally registering a
    /// cancellation flag for `tools/call` (keyed by `id`, guaranteed `Some`
    /// for `tools/call` — a notification-shaped `tools/call` with no `id`
    /// never reaches this far, see the notification checks in
    /// [`Self::handle_request_with_context`]/[`Self::handle_batch`]) so a
    /// later `notifications/cancelled` referencing this exact request can
    /// flip it. The entry is removed again once dispatch returns, whether
    /// the handler checked the flag or not — this map never accumulates
    /// stale entries the way `sessions`/`sse_clients` can.
    fn dispatch_with_cancellation(
        &self,
        method: &str,
        body: &str,
        ctx: McpContext,
        id: &Option<String>,
    ) -> Result<String, (i32, String)> {
        let Some(id_str) = (method == "tools/call").then_some(id.as_ref()).flatten() else {
            return self.dispatch(method, body, ctx);
        };

        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.cancellations.lock().unwrap().insert(id_str.clone(), flag.clone());
        let ctx = McpContext { cancellation: Some(flag), ..ctx };
        let result = self.dispatch(method, body, ctx);
        self.cancellations.lock().unwrap().remove(id_str);
        result
    }

    /// Handle `notifications/cancelled`: look up `params.requestId` (the
    /// spec allows `string | number`, so this is matched as a raw JSON
    /// token, the same way [`McpContext::progress_token`] is) in
    /// `cancellations` and flip its flag if the request is still in flight.
    /// Silently does nothing for an unknown/already-finished request id —
    /// the target call may simply have already completed naturally, which
    /// isn't an error.
    fn handle_cancellation(&self, body: &str) {
        let Some(params) = json_rpc::extract_raw(body, "params") else { return };
        let Some(request_id) = json_rpc::extract_raw(&params, "requestId") else { return };
        if let Some(flag) = self.cancellations.lock().unwrap().get(&request_id) {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Handle `notifications/roots/list_changed`: clear this session's
    /// cached `roots/list` result (if any), so the next
    /// [`McpContext::list_roots`] call in this session does a fresh round
    /// trip instead of serving stale data. Correlated purely by this
    /// request's own `Mcp-Session-Id` (`session_id`) — unlike
    /// `notifications/cancelled`, this notification carries no params of
    /// its own to key off of; it's about "your cache for *this connection*
    /// is stale," not any other request.
    fn invalidate_roots_cache(&self, session_id: &Option<String>) {
        let Some(sid) = session_id else { return };
        if let Some(info) = self.sessions.lock().unwrap().get_mut(sid) {
            info.roots = None;
        }
    }

    /// If `body` is a JSON-RPC *response* (no `method`, by definition — see
    /// [`Self::handle_request_with_context`]) to a `sampling/createMessage`
    /// request this server sent and is still waiting on (`id` found in
    /// `pending_replies`), deliver it to the blocked [`McpContext::sample`]
    /// call and return `true`. Returns `false` for any `id` this server
    /// isn't tracking — the caller still needs to treat that as a genuine
    /// malformed/unrecognized message.
    fn try_deliver_sampling_response(&self, id: &str, body: &str) -> bool {
        let sender = match self.pending_replies.lock().unwrap().remove(id) {
            Some(s) => s,
            None => return false,
        };
        let payload = match json_rpc::extract_raw(body, "result") {
            Some(result) => Ok(result),
            None => {
                let message = json_rpc::extract_raw(body, "error")
                    .and_then(|e| json_rpc::extract_str(&e, "message"))
                    .unwrap_or_else(|| "sampling/createMessage request failed".to_string());
                Err(message)
            }
        };
        let _ = sender.send(payload); // the waiter may have already timed out and dropped rx
        true
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
        let params = json_rpc::extract_raw(body, "params");
        let client_info = params.as_deref().and_then(|p| json_rpc::extract_raw(p, "clientInfo"));
        let (name, version) = match &client_info {
            Some(info) => (
                json_rpc::extract_str(info, "name"),
                json_rpc::extract_str(info, "version"),
            ),
            None => (None, None),
        };
        let capabilities = params.as_deref().and_then(|p| json_rpc::extract_raw(p, "capabilities"));
        let supports_sampling = capabilities.as_deref()
            .and_then(|c| json_rpc::extract_raw(c, "sampling"))
            .is_some();
        let supports_roots = capabilities.as_deref()
            .and_then(|c| json_rpc::extract_raw(c, "roots"))
            .is_some();

        eprintln!(
            "[mcp] initialize from client {} v{}",
            name.as_deref().unwrap_or("unknown"),
            version.as_deref().unwrap_or("unknown"),
        );

        let session_id = crate::request_id::generate_request_id();
        self.make_room_for_a_session();
        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.clone(), StoredClientInfo { last_seen: std::time::Instant::now(), protocol_version: negotiate_version(
                params.as_deref().and_then(|p| json_rpc::extract_str(p, "protocolVersion")).as_deref()), name, version, supports_sampling, supports_roots, roots: None });

        response.headers.push(Header {
            name: "Mcp-Session-Id".into(),
            value: session_id.into(),
        });
    }

    // ── method handlers ───────────────────────────────────────────────────────

    /// Handle `initialize`. Per spec, the server must inspect the client's
    /// requested `protocolVersion` and respond with the version it actually
    /// supports — allowing the client to abort the session if incompatible —
    /// rather than blindly echoing `PROTOCOL_VERSION` regardless of what was
    /// asked for.
    ///
    /// A version in [`SUPPORTED_PROTOCOL_VERSIONS`] is confirmed; any other
    /// (older, newer, malformed) gets [`PROTOCOL_VERSION`], the newest. The
    /// session remembers the result, which decides e.g. whether JSON-RPC
    /// batches are accepted (only up to 2025-03-26).
    ///
    /// `clientInfo` is *not* handled here — [`Self::handle_request_with_context`]
    /// extracts and stores it (under a freshly minted session id) after this
    /// returns, so it's only ever parsed out of `body` once per call.
    fn do_initialize(&self, body: &str) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params");

        let client_version = params.as_deref().and_then(|p| json_rpc::extract_str(p, "protocolVersion"));

        let negotiated_version = negotiate_version(client_version.as_deref());

        let logging_cap = if self.logging_enabled { r#","logging":{}"# } else { "" };
        // completions is advertised iff at least one .completion() has been registered —
        // no separate opt-in flag needed, unlike logging: if nothing was registered,
        // completion/complete would just return empty results for everything anyway.
        let completions_cap = if self.completions.read().unwrap().is_empty() { "" } else { r#","completions":{}"# };
        // listChanged is always true: register_tool/remove_tool (and the resource/prompt
        // equivalents) are always available, unlike logging which is opt-in via
        // .logging_enabled(). resources.subscribe is also always true now that
        // resources/subscribe and resources/unsubscribe are implemented (MCP_TODO.md's TODO-14).
        let caps = format!(
            r#"{{"tools":{{"listChanged":true}},"resources":{{"subscribe":true,"listChanged":true}},"prompts":{{"listChanged":true}}{logging_cap}{completions_cap}}}"#
        );
        Ok(format!(
            r#"{{"protocolVersion":"{}","capabilities":{caps},"serverInfo":{{"name":"{}"{}"version":"{}"{}}}}}"#,
            json_escape(negotiated_version),
            json_escape(&self.server_name),
            self.server_title.as_ref().map(|t| format!(r#","title":"{}","#, json_escape(t))).unwrap_or_else(|| ",".to_string()),
            json_escape(&self.server_version),
            icons_json(&self.server_icons),
        ))
    }

    fn do_tools_list(&self, body: &str, ctx: &McpContext) -> Result<String, (i32, String)> {
        #[cfg_attr(not(feature = "http2"), allow(unused_mut))]
        let mut items: Vec<String> = self.tools.read().unwrap().iter()
            .filter(|t| self.tool_allowed(ctx, &t.name))
            .map(|t| Self::render_tool_list_entry(&t.name, &t.description, &t.input_schema, t.annotations, &t.meta))
            .collect();

        #[cfg(feature = "http2")]
        items.extend(self.async_tools.read().unwrap().iter()
            .filter(|t| self.tool_allowed(ctx, &t.name))
            .map(|t| Self::render_tool_list_entry(&t.name, &t.description, &t.input_schema, t.annotations, &t.meta)));

        let (page, next_cursor) = self.paginate(&items, body)?;
        Ok(format!(r#"{{"tools":[{}]{}}}"#, page.join(","), next_cursor_json(&next_cursor)))
    }

    /// Render one `tools/list` entry — shared by the sync (`tools`) and
    /// (with the `http2` feature) async (`async_tools`) collections, which
    /// are otherwise identical from the client's point of view.
    fn render_tool_list_entry(name: &str, description: &str, input_schema: &str, annotations: Option<ToolAnnotations>, meta: &ToolMeta) -> String {
        let annotations_field = match annotations {
            Some(a) => format!(r#","annotations":{}"#, a.to_json()),
            None => String::new(),
        };
        let title = meta.title.as_ref().map(|t| format!(r#","title":"{}""#, json_escape(t))).unwrap_or_default();
        let output_schema = meta.output_schema.as_ref().map(|schema| format!(r#","outputSchema":{schema}"#)).unwrap_or_default();
        format!(
            r#"{{"name":"{}"{title},"description":"{}","inputSchema":{}{output_schema}{}{}}}"#,
            json_escape(name), json_escape(description), input_schema, annotations_field, icons_json(&meta.icons),
        )
    }

    /// Check a `tools/call`'s `arguments` against the tool's input schema:
    /// an object, every `required` property present, and each declared
    /// property of the declared `type`. Not a full JSON Schema validator —
    /// the checks a model's mistakes actually trip. A failure becomes a tool
    /// result with `isError: true` (MCP 2025-11-25, SEP-1303), so the model
    /// sees why and can retry, instead of a protocol error it never reads.
    pub(crate) fn validate_arguments(schema: &str, args: &str) -> Result<(), String> {
        let Some(schema) = json_rpc::object_members(schema) else { return Ok(()) };
        let field = |name: &str| schema.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
        let Some(arguments) = json_rpc::object_members(args) else {
            return Err("Invalid arguments: expected a JSON object".to_string());
        };
        if let Some(required) = field("required") {
            for name in json_rpc::split_array_elements(required) {
                let name = name.trim().trim_matches('"');
                if !arguments.iter().any(|(k, _)| k == name) {
                    return Err(format!("Invalid arguments: missing required property \"{name}\""));
                }
            }
        }
        let Some(properties) = field("properties").and_then(json_rpc::object_members) else { return Ok(()) };
        for (name, value) in &arguments {
            let Some((_, property)) = properties.iter().find(|(k, _)| k == name) else { continue };
            let Some(declared) = json_rpc::object_members(property)
                .and_then(|p| p.into_iter().find(|(k, _)| k == "type").map(|(_, v)| v))
            else { continue };
            let allowed: Vec<String> = if declared.starts_with('[') {
                json_rpc::split_array_elements(&declared).iter().map(|t| t.trim().trim_matches('"').to_string()).collect()
            } else {
                vec![declared.trim_matches('"').to_string()]
            };
            let actual = json_type(value);
            let fits = allowed.iter().any(|t| t == actual || (t == "number" && actual == "integer"));
            if !fits {
                return Err(format!("Invalid arguments: property \"{name}\" must be {}, got {actual}", allowed.join(" or ")));
            }
        }
        Ok(())
    }

    fn do_tools_call(&self, body: &str, ctx: McpContext) -> Result<String, (i32, String)> {
        let result = match self.call_tool(body, ctx)? {
            // Only an MCP 2026-07-28 request can carry input requests.
            Ok(c) if c.kind == "input_required" => Err("this tool needs input from the client, which needs MCP protocol version 2026-07-28".to_string()),
            other => other,
        };
        Ok(Self::format_tool_result(result))
    }

    /// Run `tools/call`: the handler's result, unformatted; `Err` for a
    /// call that never reached a handler (unknown tool, no params).
    fn call_tool(&self, body: &str, ctx: McpContext) -> Result<Result<McpContent, String>, (i32, String)> {
        if self.after_call.is_empty() {
            return self.call_tool_inner(body, ctx, &mut ToolCallOutcome::Ok);
        }
        let started = std::time::Instant::now();
        let observed_ctx = ctx.clone();
        let mut outcome = ToolCallOutcome::Ok;
        let result = self.call_tool_inner(body, ctx, &mut outcome);
        if matches!(result, Ok(Err(_))) && outcome == ToolCallOutcome::Ok {
            outcome = ToolCallOutcome::ToolError;
        }
        if result.is_err() && outcome == ToolCallOutcome::Ok {
            outcome = ToolCallOutcome::Denied;
        }
        let name = json_rpc::extract_raw(body, "params").and_then(|p| json_rpc::extract_str(&p, "name")).unwrap_or_default();
        let record = ToolCallRecord { tool: &name, outcome, duration: started.elapsed() };
        for observe in &self.after_call {
            observe(&observed_ctx, &record);
        }
        result
    }

    fn call_tool_inner(&self, body: &str, ctx: McpContext, outcome: &mut ToolCallOutcome) -> Result<Result<McpContent, String>, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let name = json_rpc::extract_str(&params, "name")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing tool name".to_string()))?;
        let args = json_rpc::extract_raw(&params, "arguments")
            .unwrap_or_else(|| "{}".to_string());
        if !self.tool_allowed(&ctx, &name) {
            return Err((json_rpc::INVALID_PARAMS, format!("Unknown tool: {name}")));
        }
        for check in &self.before_call {
            if let Err(refused) = check(&ctx, &name) {
                *outcome = ToolCallOutcome::Refused;
                return Ok(Err(refused));
            }
        }

        // `_meta.progressToken` (string or number, per spec) — stored raw so
        // `McpContext::report_progress` can splice it back verbatim.
        let progress_token = json_rpc::extract_raw(&params, "_meta")
            .and_then(|meta| json_rpc::extract_raw(&meta, "progressToken"));
        let ctx = McpContext { progress_token, ..ctx };

        let sync_handler = {
            let tools = self.tools.read().unwrap();
            tools.iter().find(|t| t.name == name).map(|t| (t.handler.clone(), t.input_schema.clone()))
        };
        if let Some((handler, schema)) = sync_handler {
            if let Err(invalid) = Self::validate_arguments(&schema, &args) {
                return Ok(Err(invalid));
            }
            return Ok(handler(ctx, &args));
        }

        #[cfg(feature = "http2")]
        {
            let async_handler = {
                let async_tools = self.async_tools.read().unwrap();
                async_tools.iter().find(|t| t.name == name).map(|t| (t.handler.clone(), t.input_schema.clone()))
            };
            if let Some((handler, schema)) = async_handler {
                if let Err(invalid) = Self::validate_arguments(&schema, &args) {
                    return Ok(Err(invalid));
                }
                let args_owned = args.clone();
                return Ok(crate::async_bridge::block_on_isolated(move || handler(args_owned)));
            }
        }

        Err((json_rpc::INVALID_PARAMS, format!("Unknown tool: {name}")))
    }

    /// Render a tool handler's result as the spec's `tools/call` shape —
    /// `{"content":[...],"isError":false}` on success, or `isError:true`
    /// with the error message as a text content block. Shared by the sync
    /// and (with `http2`) async `tools/call` dispatch paths.
    fn format_tool_result(result: Result<McpContent, String>) -> String {
        match result {
            Ok(c) if c.kind == "result" => c.text,
            Ok(c) if c.kind == "blocks" => format!(r#"{{"content":[{}],"isError":false}}"#, c.text),
            Ok(c) if c.kind == "structured" => format!(
                r#"{{"content":[{{"type":"text","text":"{}"}}],"structuredContent":{},"isError":false}}"#,
                json_escape(&c.text),
                c.text,
            ),
            Ok(c) => format!(
                r#"{{"content":[{}],"isError":false}}"#,
                c.to_content_json(),
            ),
            Err(e) => {
                let escaped = json_escape(&e);
                format!(
                    r#"{{"content":[{{"type":"text","text":"{escaped}"}}],"isError":true}}"#
                )
            }
        }
    }

    fn do_resources_list(&self, body: &str, ctx: &McpContext) -> Result<String, (i32, String)> {
        let items: Vec<String> = self.resources.read().unwrap().iter().filter(|r| !r.uri_template.contains('{') && self.resource_allowed(ctx, &r.uri_template)).map(|r| {
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

    /// `resources/templates/list`: the resources registered with a URI
    /// template (`{...}` placeholders), which `resources/list` leaves out.
    fn do_resource_templates_list(&self, body: &str, ctx: &McpContext) -> Result<String, (i32, String)> {
        let items: Vec<String> = self.resources.read().unwrap().iter().filter(|r| r.uri_template.contains('{') && self.resource_allowed(ctx, &r.uri_template)).map(|r| {
            format!(
                r#"{{"uriTemplate":"{}","name":"{}","description":"{}","mimeType":"text/plain"}}"#,
                json_escape(&r.uri_template),
                json_escape(&r.name),
                json_escape(&r.description),
            )
        }).collect();
        let (page, next_cursor) = self.paginate(&items, body)?;
        Ok(format!(r#"{{"resourceTemplates":[{}]{}}}"#, page.join(","), next_cursor_json(&next_cursor)))
    }

    fn do_resources_read(&self, body: &str, ctx: &McpContext) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let uri = json_rpc::extract_str(&params, "uri")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing uri".to_string()))?;

        let handler = {
            let resources = self.resources.read().unwrap();
            resources.iter().find(|r| uri_matches(&r.uri_template, &uri) && self.resource_allowed(ctx, &r.uri_template)).map(|r| r.handler.clone())
        }.ok_or_else(|| (json_rpc::INVALID_PARAMS, format!("Resource not found: {uri}")))?;

        match handler(&uri) {
            // A complete result from elsewhere (the gateway's upstreams).
            Ok(c) if c.kind == "result" => Ok(c.text),
            Ok(c) => {
                let text_esc = json_escape(&c.text);
                let uri_esc  = json_escape(&uri);
                let field = if c.kind == "blob" { "blob" } else { "text" };
                Ok(format!(
                    r#"{{"contents":[{{"uri":"{uri_esc}","mimeType":"{}","{field}":"{text_esc}"}}]}}"#,
                    json_escape(c.mime()),
                ))
            }
            Err(e) => Err((json_rpc::INVALID_PARAMS, e)),
        }
    }

    /// Handle `resources/subscribe`: record that `session_id` (this
    /// request's `Mcp-Session-Id`) wants `notifications/resources/updated`
    /// pushed whenever [`Self::notify_resource_updated`] is called for
    /// `params.uri`. Requires a session id — without one there's no way to
    /// later match this subscription to a specific `GET /mcp` SSE
    /// connection, so a client that never sent `Mcp-Session-Id` gets
    /// `INVALID_PARAMS` rather than a subscription that can never fire.
    fn do_resource_subscribe(&self, body: &str, session_id: Option<String>) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let uri = json_rpc::extract_str(&params, "uri")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing uri".to_string()))?;
        let session_id = session_id.ok_or((
            json_rpc::INVALID_PARAMS,
            "resources/subscribe requires an Mcp-Session-Id header (call initialize first)".to_string(),
        ))?;

        let mut subs = self.subscriptions.lock().unwrap();
        let subscribers = subs.entry(uri).or_default();
        if !subscribers.contains(&session_id) {
            subscribers.push(session_id);
        }
        Ok("{}".to_string())
    }

    /// Handle `resources/unsubscribe`: the inverse of
    /// [`Self::do_resource_subscribe`]. Removing the last subscriber for a
    /// URI drops the URI's entry entirely rather than leaving an empty
    /// `Vec` behind.
    fn do_resource_unsubscribe(&self, body: &str, session_id: Option<String>) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let uri = json_rpc::extract_str(&params, "uri")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing uri".to_string()))?;
        let session_id = session_id.ok_or((
            json_rpc::INVALID_PARAMS,
            "resources/unsubscribe requires an Mcp-Session-Id header".to_string(),
        ))?;

        let mut subs = self.subscriptions.lock().unwrap();
        if let Some(subscribers) = subs.get_mut(&uri) {
            subscribers.retain(|s| s != &session_id);
            if subscribers.is_empty() {
                subs.remove(&uri);
            }
        }
        Ok("{}".to_string())
    }

    fn do_prompts_list(&self, body: &str, ctx: &McpContext) -> Result<String, (i32, String)> {
        let items: Vec<String> = self.prompts.read().unwrap().iter().filter(|p| self.prompt_allowed(ctx, &p.name)).map(|p| {
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

    fn do_prompts_get(&self, body: &str, ctx: McpContext) -> Result<String, (i32, String)> {
        match self.get_prompt(body, ctx)? {
            (description, PromptReply::Messages(msgs)) => {
                let msg_jsons: Vec<String> = msgs.iter().map(|m| m.to_json()).collect();
                Ok(format!(r#"{{"description":"{}","messages":[{}]}}"#, json_escape(&description), msg_jsons.join(",")))
            }
            (_, PromptReply::InputRequired(_)) => Err((
                json_rpc::INVALID_PARAMS,
                "this prompt needs input from the client, which needs MCP protocol version 2026-07-28".to_string(),
            )),
        }
    }

    /// Run `prompts/get`: the prompt's description and the handler's reply.
    fn get_prompt(&self, body: &str, ctx: McpContext) -> Result<(String, PromptReply), (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let name = json_rpc::extract_str(&params, "name")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing prompt name".to_string()))?;
        let args = json_rpc::extract_raw(&params, "arguments")
            .unwrap_or_else(|| "{}".to_string());

        let (description, handler) = {
            let prompts = self.prompts.read().unwrap();
            prompts.iter().find(|p| p.name == name && self.prompt_allowed(&ctx, &p.name)).map(|p| (p.description.clone(), p.handler.clone()))
        }.ok_or_else(|| (json_rpc::INVALID_PARAMS, format!("Unknown prompt: {name}")))?;

        handler(ctx, &args).map(|reply| (description, reply)).map_err(|e| (json_rpc::INVALID_PARAMS, e))
    }

    /// Handle `completion/complete`: look up the registered
    /// [`CompletionDef`] matching `params.ref.type`/`params.ref.name`, call
    /// its handler with `params.argument.name`/`params.argument.value`, and
    /// render the spec's `{"completion":{"values":[...],"hasMore":...,
    /// "total":...}}` shape. No registered provider for the given ref/name
    /// (or an unrecognized `ref.type` not stripped of `"ref/"`) returns an
    /// empty `values` list rather than an error — matching the spec's own
    /// framing of completion as a best-effort hint, not a required capability
    /// per tool/prompt.
    fn do_completion(&self, body: &str) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let reference = json_rpc::extract_raw(&params, "ref")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing ref".to_string()))?;
        let ref_type_raw = json_rpc::extract_str(&reference, "type")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing ref.type".to_string()))?;
        let ref_type = ref_type_raw.strip_prefix("ref/").unwrap_or(&ref_type_raw);
        let ref_name = json_rpc::extract_str(&reference, "name")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing ref.name".to_string()))?;

        let argument = json_rpc::extract_raw(&params, "argument")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing argument".to_string()))?;
        let arg_name = json_rpc::extract_str(&argument, "name")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing argument.name".to_string()))?;
        let partial = json_rpc::extract_str(&argument, "value").unwrap_or_default();

        let handler = {
            let completions = self.completions.read().unwrap();
            completions.iter()
                .find(|c| c.ref_type == ref_type && c.ref_name == ref_name)
                .map(|c| c.handler.clone())
        };

        let values = match handler {
            Some(h) => h(&arg_name, &partial).map_err(|e| (json_rpc::INVALID_PARAMS, e))?,
            None => vec![],
        };

        let total = values.len();
        let has_more = total > MAX_COMPLETION_VALUES;
        let page = if has_more { &values[..MAX_COMPLETION_VALUES] } else { &values[..] };
        let values_json: Vec<String> = page.iter().map(|v| format!(r#""{}""#, json_escape(v))).collect();

        Ok(format!(
            r#"{{"completion":{{"values":[{}],"hasMore":{has_more},"total":{total}}}}}"#,
            values_json.join(","),
        ))
    }

    /// Answers a `tools/call` as an SSE stream (Streamable HTTP): the
    /// handler runs on its own thread, its progress notifications go out as
    /// they happen, then the JSON-RPC response, and the stream ends.
    fn stream_tool_call(&self, body: &str, mut ctx: McpContext) -> Response {
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(32);
        ctx.request_stream = Some(tx.clone());
        let server = self.clone();
        let body = body.to_string();
        std::thread::spawn(move || {
            let response = server.handle_request_with_context(&body, ctx);
            let json = response.content_range_list.first().map(|part| String::from_utf8_lossy(&part.body).into_owned()).unwrap_or_default();
            let _ = tx.send(format!("data: {json}\n\n").into_bytes());
        });
        let mut response = Response::new();
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        response.headers.push(Header { name: Header::_CONTENT_TYPE.into(), value: "text/event-stream".into() });
        response.headers.push(Header { name: Header::_CACHE_CONTROL.into(), value: "no-cache".into() });
        response.headers.push(Header { name: "X-Accel-Buffering".into(), value: "no".into() });
        response.stream_pipe = Some(Box::new(crate::sse::SseReader::new(rx, SSE_KEEPALIVE_INTERVAL)));
        response
    }

    /// Build the [`McpContext`] for an incoming request: the `Mcp-Session-Id`
    /// header, if present, plus whatever `clientInfo` was recorded for that
    /// session at `initialize` time (if this session is recognized).
    fn context_for(&self, request: &Request) -> McpContext {
        let session_id = request
            .get_header("Mcp-Session-Id".to_string())
            .map(|h| h.value.to_string());

        let (client_name, client_version, sampling_supported, roots_supported) = match &session_id {
            Some(sid) => match self.sessions.lock().unwrap().get(sid) {
                Some(info) => (info.name.clone(), info.version.clone(), info.supports_sampling, info.supports_roots),
                None => (None, None, false, false),
            },
            None => (None, None, false, false),
        };

        McpContext {
            client_name,
            client_version,
            session_id,
            auth_claims: None,
            progress_token: None,
            sse_clients: Some(self.sse_clients.clone()),
            cancellation: None,
            sampling_supported,
            pending_replies: Some(self.pending_replies.clone()),
            roots_supported,
            sessions: Some(self.sessions.clone()),
            request_stream: None,
            modern: None,
            tasks: None,
        }
    }
}

/// Render one JSON-RPC 2.0 notification (no `id` — fire-and-forget, per
/// spec) as an SSE `data:`-ready message body. Shared by [`McpServer::notify`]
/// and [`McpContext::report_progress`].
/// A single `tools/call` POST that asks for progress (`_meta.progressToken`)
/// from a client that accepts an SSE response.
fn wants_progress_stream(request: &Request, body: &str) -> bool {
    let accepts_sse = request.headers.iter()
        .filter(|h| h.name.eq_ignore_ascii_case("accept"))
        .any(|h| h.value.to_ascii_lowercase().contains("text/event-stream"));
    if !accepts_sse || body.trim_start().starts_with('[') {
        return false;
    }
    let Some(members) = json_rpc::object_members(body) else { return false };
    let member = |key: &str| members.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    member("method") == Some(r#""tools/call""#)
        && member("id").is_some()
        && member("params")
            .and_then(json_rpc::object_members)
            .and_then(|params| params.into_iter().find(|(k, _)| k == "_meta"))
            .and_then(|(_, meta)| json_rpc::object_members(&meta))
            .is_some_and(|meta| meta.iter().any(|(k, _)| k == "progressToken"))
}

fn render_notification(method: &str, params_json: Option<&str>) -> String {
    let params_field = match params_json {
        Some(p) => format!(r#","params":{p}"#),
        None => String::new(),
    };
    format!(r#"{{"jsonrpc":"2.0","method":"{}"{}}}"#, json_escape(method), params_field)
}

/// Send a raw pre-built JSON-RPC message to every client in `clients`,
/// pruning any whose channel is full or disconnected. Shared by
/// [`McpServer::notify`] and [`McpContext::report_progress`] — the latter
/// only has a clone of the broadcast list, not a whole `McpServer`.
fn broadcast_sse_to(clients: &Arc<Mutex<Vec<SseClient>>>, json: &str) {
    let frame = format!("data: {json}\n\n").into_bytes();
    let mut clients = clients.lock().unwrap();
    clients.retain(|c| c.sender.try_send(frame.clone()).is_ok());
}

/// Send a raw pre-built JSON-RPC message only to clients whose `session_id`
/// is in `session_ids`, pruning any of *those* whose channel is full or
/// disconnected. A client with no `session_id`, or one not in
/// `session_ids`, is left untouched (kept, not sent to) — this is a
/// targeted send for [`McpServer::notify_resource_updated`], not a
/// broadcast like [`broadcast_sse_to`].
/// Sends `json` to every SSE stream of `session_ids`; returns how many took it.
fn send_sse_to_sessions(clients: &Arc<Mutex<Vec<SseClient>>>, session_ids: &[String], json: &str) -> usize {
    let frame = format!("data: {json}\n\n").into_bytes();
    let mut clients = clients.lock().unwrap();
    let mut delivered = 0;
    clients.retain(|c| {
        let is_target = c.session_id.as_deref().is_some_and(|sid| session_ids.iter().any(|s| s == sid));
        if is_target {
            let sent = c.sender.try_send(frame.clone()).is_ok();
            delivered += sent as usize;
            sent
        } else {
            true
        }
    });
    delivered
}

/// How often a server-to-client request checks for the session's SSE stream
/// while waiting for the client to open it.
const SSE_STREAM_WAIT_POLL: Duration = Duration::from_millis(20);

// ── Application ───────────────────────────────────────────────────────────────

impl Application for McpServer {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        // RFC 9728 protected-resource metadata (MCP 2025-06-18), at the
        // well-known path and at it suffixed with the endpoint's path.
        #[cfg(feature = "sso")]
        if request.method == "GET" {
            if let Some(oauth) = &self.oauth {
                let target = request.request_uri.split('?').next().unwrap_or("");
                let suffixed = format!("{}{}", PROTECTED_RESOURCE_PATH, self.path);
                if target == PROTECTED_RESOURCE_PATH || target == suffixed {
                    return Ok(protected_resource_document(oauth, request, &self.path));
                }
            }
        }

        if request.request_uri == self.path {
            // Browser requests (they carry `Origin`) only from listed origins.
            if let Some(origin) = request.headers.iter().find(|h| h.name.eq_ignore_ascii_case("origin")) {
                let origin = origin.value.trim();
                if !self.allowed_origins.iter().any(|o| o.trim_end_matches('/').eq_ignore_ascii_case(origin.trim_end_matches('/'))) {
                    return Ok(forbidden_origin());
                }
            }
            // JSON-RPC bodies only: a form or text/plain POST is what a web
            // page can send cross-site without a CORS preflight.
            if request.method == "POST" {
                let content_type = request.headers.iter()
                    .find(|h| h.name.eq_ignore_ascii_case("content-type"))
                    .map(|h| h.value.as_ref())
                    .unwrap_or("");
                let media_type = content_type.split(';').next().unwrap_or("").trim();
                if !media_type.eq_ignore_ascii_case("application/json") {
                    return Ok(unsupported_media_type());
                }
            }

            #[cfg_attr(not(feature = "sso"), allow(unused_mut))]
            let mut auth_claims_json: Option<String> = None;
            #[cfg(feature = "sso")]
            let oauth_configured = self.oauth.is_some();
            #[cfg(not(feature = "sso"))]
            let oauth_configured = false;

            // OAuth (sso feature) takes precedence over bearer tokens if both
            // are somehow configured — see require_oauth's doc comment.
            #[cfg(feature = "sso")]
            if let Some(oauth) = &self.oauth {
                let provided = request.headers.iter()
                    .find(|h| h.name.eq_ignore_ascii_case("authorization"))
                    .map(|h| h.value.as_ref())
                    .unwrap_or("");
                let bearer = provided.strip_prefix("Bearer ").unwrap_or("");
                if bearer.is_empty() {
                    return Ok(unauthorized_with_metadata(oauth, request, &self.path));
                }
                let opts = crate::sso::jwks::VerifyOptions {
                    audience: &oauth.audience,
                    issuer: &oauth.provider.issuer,
                    leeway_secs: 60,
                };
                match oauth.jwks.verify_jwt(bearer, &opts) {
                    Ok(claims) => auth_claims_json = serde_json::to_string(&claims).ok(),
                    Err(_) => return Ok(unauthorized_with_metadata(oauth, request, &self.path)),
                }
            }
            if !oauth_configured {
                match self.check_bearer(request) {
                    Ok(claims) => auth_claims_json = claims,
                    Err(response) => return Ok(response),
                }
            }

            // MCP 2026-07-28: stateless, every request self-contained (no
            // session lookup, `Mcp-Session-Id` ignored).
            if request.method == "POST" {
                let body = std::str::from_utf8(&request.body).unwrap_or("");
                if modern::is_modern(request, body) {
                    let mut ctx = self.context_for(request);
                    ctx.session_id = None;
                    ctx.client_name = None;
                    ctx.client_version = None;
                    ctx.auth_claims = auth_claims_json;
                    return Ok(self.handle_modern(request, body, ctx));
                }
            }

            // A session id this server doesn't know — never minted, ended,
            // or evicted — is 404: the client must initialize again
            // (Streamable HTTP transport). An `initialize` POST may carry a
            // stale one.
            let session_id = request.headers.iter()
                .find(|h| h.name.eq_ignore_ascii_case("mcp-session-id"))
                .map(|h| h.value.to_string());
            if let Some(sid) = &session_id {
                let initializing = request.method == "POST"
                    && json_rpc::extract_str(std::str::from_utf8(&request.body).unwrap_or(""), "method").as_deref() == Some("initialize");
                if request.method != "OPTIONS" && !initializing && !self.touch_session(sid) {
                    return Ok(session_not_found());
                }
            }
            // The protocol version in effect: the header (2025-06-18+
            // clients send it on every request after initialize), else the
            // session's, else 2025-03-26. A version this server doesn't speak
            // is 400 (Streamable HTTP transport).
            let header_version = request.headers.iter()
                .find(|h| h.name.eq_ignore_ascii_case("mcp-protocol-version"))
                .map(|h| h.value.trim().to_string());
            if let Some(v) = &header_version {
                if !SUPPORTED_PROTOCOL_VERSIONS.contains(&v.as_str()) {
                    return Ok(bad_protocol_version(v));
                }
            }
            if request.method == "POST" && request.body.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'[') {
                let version = header_version.clone()
                    .or_else(|| session_id.as_ref().and_then(|sid| self.sessions.lock().unwrap().get(sid).map(|s| s.protocol_version.to_string())))
                    .unwrap_or_else(|| ASSUMED_PROTOCOL_VERSION.to_string());
                // JSON-RPC batching was removed in 2025-06-18.
                if version.as_str() >= "2025-06-18" {
                    return Ok(rpc_error(None, json_rpc::INVALID_REQUEST,
                        &format!("JSON-RPC batches are not supported in protocol version {version}")));
                }
            }
            return Ok(match request.method.as_str() {
                "DELETE" => match session_id {
                    // The client ends its session.
                    Some(sid) => {
                        self.sessions.lock().unwrap().remove(&sid);
                        self.forget_sessions(&[sid]);
                        let mut r = Response::new();
                        r.status_code = *STATUS_CODE_REASON_PHRASE.n204_no_content.status_code;
                        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n204_no_content.reason_phrase.to_string();
                        r
                    }
                    None => {
                        let mut r = Response::new();
                        r.status_code = *STATUS_CODE_REASON_PHRASE.n400_bad_request.status_code;
                        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n400_bad_request.reason_phrase.to_string();
                        r.content_range_list = vec![Range::get_content_range(
                            b"DELETE /mcp needs an Mcp-Session-Id".to_vec(),
                            MimeType::TEXT_PLAIN.to_string(),
                        )];
                        r
                    }
                },
                "POST" => {
                    let body = std::str::from_utf8(&request.body).unwrap_or("");
                    let mut ctx = self.context_for(request);
                    ctx.auth_claims = auth_claims_json;
                    if wants_progress_stream(request, body) {
                        self.stream_tool_call(body, ctx)
                    } else {
                        self.handle_request_with_context(body, ctx)
                    }
                }
                "GET" => self.start_sse_stream(request),
                "OPTIONS" => {
                    // CORS preflight for browser-based MCP clients
                    let mut r = Response::new();
                    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
                    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
                    r.headers.push(Header {
                        name: "Allow".into(),
                        value: "GET, POST, DELETE, OPTIONS".into(),
                    });
                    r
                }
                _ => {
                    let mut r = Response::new();
                    r.status_code = *STATUS_CODE_REASON_PHRASE.n405_method_not_allowed.status_code;
                    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n405_method_not_allowed.reason_phrase.to_string();
                    r.headers.push(Header {
                        name: "Allow".into(),
                        value: "GET, POST, DELETE, OPTIONS".into(),
                    });
                    r.content_range_list = vec![Range::get_content_range(
                        b"MCP endpoint only accepts GET (SSE), POST or DELETE".to_vec(),
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

    /// MCP calls run tool handlers (user code): never inline. Other requests
    /// go to the fallback app, which decides.
    fn runs_inline(&self, request: &Request, connection: &ConnectionInfo) -> bool {
        if request.request_uri == self.path || request.request_uri.starts_with("/.well-known/") {
            return false;
        }
        match &self.fallback {
            Some(app) => app.runs_inline(request, connection),
            None => App::is_inline_request(request),
        }
    }

    /// MCP requests are JSON-RPC read whole; other paths: the wrapped app.
    fn streams_body(&self, request: &Request, connection: &ConnectionInfo) -> bool {
        if request.request_uri == self.path || request.request_uri.starts_with("/.well-known/") {
            return false;
        }
        self.fallback.as_ref().is_some_and(|app| app.streams_body(request, connection))
    }

    #[cfg(feature = "http2")]
    fn execute_async<'a>(&'a self, request: &'a Request, connection: &'a ConnectionInfo) -> Option<crate::application::AsyncResponse<'a>> {
        if request.request_uri == self.path || request.request_uri.starts_with("/.well-known/") {
            return None;
        }
        self.fallback.as_ref()?.execute_async(request, connection)
    }
}

// ── public helper ─────────────────────────────────────────────────────────────

/// Extract an argument from a tool/prompt `arguments` JSON object: a
/// string's value, or a number's or boolean's literal text (`"3"`,
/// `"true"`). Objects, arrays and `null` are `None`. Numbers and booleans
/// used to be `None` too, so a tool reading `"enabled": true` saw nothing.
///
/// ```rust
/// use rust_web_server::mcp::extract_arg;
/// assert_eq!(extract_arg(r#"{"text":"hello"}"#, "text").as_deref(), Some("hello"));
/// assert_eq!(extract_arg(r#"{"enabled":true,"n":3}"#, "enabled").as_deref(), Some("true"));
/// assert_eq!(extract_arg(r#"{"enabled":true,"n":3}"#, "n").as_deref(), Some("3"));
/// assert_eq!(extract_arg(r#"{}"#, "missing"), None);
/// ```
pub fn extract_arg(arguments: &str, name: &str) -> Option<String> {
    if let Some(members) = json_rpc::object_members(arguments) {
        let (_, raw) = members.into_iter().find(|(key, _)| key == name)?;
        return match raw.as_bytes().first() {
            Some(b'"') => json_rpc::extract_str(&format!(r#"{{"v":{raw}}}"#), "v"),
            Some(b'{' | b'[') | None => None,
            _ if raw == "null" => None,
            _ => Some(raw),
        };
    }
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

/// A configured static token against the presented one: never matches an
/// empty token (fails closed), compared in constant time.
fn bearer_matches(expected: &str, provided: &str) -> bool {
    !expected.is_empty() && crate::proxy_config::constant_time_eq(expected.as_bytes(), provided.as_bytes())
}

/// `403` for a browser `Origin` not in [`McpServer::allowed_origins`].
fn forbidden_origin() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n403_forbidden.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n403_forbidden.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        b"Origin not allowed".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    )];
    r
}

/// `415` for a POST whose body isn't `application/json`.
fn unsupported_media_type() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n415_unsupported_media_type.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n415_unsupported_media_type.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        b"MCP requests must be application/json".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    )];
    r
}

/// `400` for an `MCP-Protocol-Version` this server doesn't speak.
fn bad_protocol_version(version: &str) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n400_bad_request.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n400_bad_request.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        format!("Unsupported MCP-Protocol-Version {:?}; supported: {}", version, SUPPORTED_PROTOCOL_VERSIONS.join(", ")).into_bytes(),
        MimeType::TEXT_PLAIN.to_string(),
    )];
    r
}

/// `404` for a request in a session this server doesn't know (anymore).
fn session_not_found() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        br#"{"jsonrpc":"2.0","error":{"code":-32001,"message":"Session not found: initialize a new one"},"id":null}"#.to_vec(),
        MimeType::APPLICATION_JSON.to_string(),
    )];
    r
}

fn unauthorized() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n401_unauthorized.reason_phrase.to_string();
    r.headers.push(Header {
        name: "WWW-Authenticate".into(),
        value: "Bearer".into(),
    });
    r.content_range_list = vec![Range::get_content_range(
        b"Unauthorized".to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    )];
    r
}

/// RFC 9728's well-known path for protected-resource metadata.
#[cfg(feature = "sso")]
const PROTECTED_RESOURCE_PATH: &str = "/.well-known/oauth-protected-resource";

/// The protected resource's identifier: the configured audience when it is
/// an absolute URL (RFC 8707 — the value tokens are minted for), else this
/// server's origin from the request plus the MCP path.
#[cfg(feature = "sso")]
fn resource_identifier(oauth: &OAuthConfig, request: &Request, path: &str) -> String {
    if oauth.audience.starts_with("https://") || oauth.audience.starts_with("http://") {
        return oauth.audience.clone();
    }
    format!("{}{}", request_origin(request), path)
}

/// `scheme://host` this request was sent to (`Host`; `https` unless the
/// host is a loopback name).
#[cfg(feature = "sso")]
fn request_origin(request: &Request) -> String {
    let host = request.headers.iter()
        .find(|h| h.name.eq_ignore_ascii_case("host"))
        .map(|h| h.value.to_string())
        .unwrap_or_else(|| "localhost".to_string());
    let local = host.starts_with("localhost") || host.starts_with("127.") || host.starts_with("[::1]");
    format!("{}://{}", if local { "http" } else { "https" }, host)
}

/// The metadata document's URL, for `WWW-Authenticate`.
#[cfg(feature = "sso")]
fn metadata_url(oauth: &OAuthConfig, request: &Request, path: &str) -> String {
    let resource = resource_identifier(oauth, request, path);
    let origin = match resource.find("://").and_then(|i| resource[i + 3..].find('/').map(|j| i + 3 + j)) {
        Some(end) => resource[..end].to_string(),
        None => resource,
    };
    format!("{origin}{PROTECTED_RESOURCE_PATH}")
}

/// `GET /.well-known/oauth-protected-resource` (RFC 9728): which
/// authorization server issues tokens for this MCP server, and the resource
/// identifier a client asks it for (RFC 8707). Served whenever
/// [`McpServer::require_oauth`] is configured; it replaced the
/// authorization-server-style document earlier versions served.
#[cfg(feature = "sso")]
fn protected_resource_document(oauth: &OAuthConfig, request: &Request, path: &str) -> Response {
    let body = format!(
        r#"{{"resource":"{}","authorization_servers":["{}"],"bearer_methods_supported":["header"]}}"#,
        json_escape(&resource_identifier(oauth, request, path)),
        json_escape(&oauth.provider.issuer),
    );
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        body.into_bytes(),
        MimeType::APPLICATION_JSON.to_string(),
    )];
    r
}

/// `401` whose `WWW-Authenticate` points at the protected-resource
/// metadata, so a client can discover the authorization server.
#[cfg(feature = "sso")]
fn unauthorized_with_metadata(oauth: &OAuthConfig, request: &Request, path: &str) -> Response {
    let mut r = unauthorized();
    if let Some(header) = r.headers.iter_mut().find(|h| h.name.eq_ignore_ascii_case("www-authenticate")) {
        header.value = format!(r#"Bearer resource_metadata="{}""#, metadata_url(oauth, request, path)).into();
    }
    r
}

fn rpc_error(id: Option<&str>, code: i32, message: &str) -> Response {
    let id_str  = id.unwrap_or("null");
    let escaped = json_escape(message);
    json_response(&format!(
        r#"{{"jsonrpc":"2.0","error":{{"code":{code},"message":"{escaped}"}},"id":{id_str}}}"#
    ))
}

/// The JSON Schema type of a raw JSON value (`integer` for whole numbers).
fn json_type(raw: &str) -> &'static str {
    let raw = raw.trim();
    match raw.as_bytes().first() {
        Some(b'"') => "string",
        Some(b'{') => "object",
        Some(b'[') => "array",
        Some(b't' | b'f') => "boolean",
        Some(b'n') => "null",
        _ if raw.bytes().all(|b| b.is_ascii_digit() || b == b'-') => "integer",
        _ => "number",
    }
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
