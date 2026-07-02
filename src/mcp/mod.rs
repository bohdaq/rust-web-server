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
//! # Environment variables
//!
//! None — configure the server programmatically via the builder.

mod json_rpc;

#[cfg(test)]
mod tests;

use std::sync::Arc;

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
/// Create with [`McpContent::text`] (plain text or JSON strings) or
/// [`McpContent::json`] (marks MIME type as `application/json`).
#[derive(Clone, Debug)]
pub struct McpContent {
    /// Always `"text"` in the current MCP spec.
    pub kind: &'static str,
    /// The content string.
    pub text: String,
    /// Optional MIME type override (default `"text/plain"`).
    pub mime_type: Option<String>,
}

impl McpContent {
    /// Plain-text content.
    pub fn text(s: impl Into<String>) -> Self {
        McpContent { kind: "text", text: s.into(), mime_type: None }
    }

    /// JSON content — sets `mimeType` to `application/json`.
    pub fn json(s: impl Into<String>) -> Self {
        McpContent { kind: "text", text: s.into(), mime_type: Some("application/json".to_string()) }
    }

    fn to_content_json(&self) -> String {
        let escaped = json_escape(&self.text);
        format!(r#"{{"type":"{}","text":"{}"}}"#, self.kind, escaped)
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

// ── internal handler registrations ───────────────────────────────────────────

type ToolFn     = Arc<dyn Fn(&str) -> Result<McpContent, String>    + Send + Sync>;
type ResourceFn = Arc<dyn Fn(&str) -> Result<McpContent, String>    + Send + Sync>;
type PromptFn   = Arc<dyn Fn(&str) -> Result<Vec<PromptMessage>, String> + Send + Sync>;

#[derive(Clone)]
struct ToolDef {
    name: String,
    description: String,
    input_schema: String,
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
}

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
        }
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
    pub fn tool<F>(mut self, name: &str, description: &str, input_schema: &str, handler: F) -> Self
    where
        F: Fn(&str) -> Result<McpContent, String> + Send + Sync + 'static,
    {
        self.tools.push(ToolDef {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: input_schema.to_string(),
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
    pub fn handle_request(&self, body: &str) -> Response {
        let method = match json_rpc::extract_str(body, "method") {
            Some(m) => m,
            None => return rpc_error(None, json_rpc::INVALID_REQUEST, "Missing method"),
        };

        let id = json_rpc::extract_id(body);

        // Notifications have no `id` — acknowledge with 202 and no body.
        if method == "notifications/initialized" || (id.is_none() && method != "ping") {
            return no_content();
        }

        let result: Result<String, (i32, String)> = match method.as_str() {
            "initialize"     => self.do_initialize(),
            "ping"           => Ok("{}".to_string()),
            "tools/list"     => self.do_tools_list(),
            "tools/call"     => self.do_tools_call(body),
            "resources/list" => self.do_resources_list(),
            "resources/read" => self.do_resources_read(body),
            "prompts/list"   => self.do_prompts_list(),
            "prompts/get"    => self.do_prompts_get(body),
            _                => Err((json_rpc::METHOD_NOT_FOUND, format!("Unknown method: {method}"))),
        };

        let id_str = id.as_deref().unwrap_or("null");

        match result {
            Ok(result_json) => json_response(&format!(
                r#"{{"jsonrpc":"2.0","result":{result_json},"id":{id_str}}}"#
            )),
            Err((code, msg)) => {
                let escaped = json_escape(&msg);
                json_response(&format!(
                    r#"{{"jsonrpc":"2.0","error":{{"code":{code},"message":"{escaped}"}},"id":{id_str}}}"#
                ))
            }
        }
    }

    // ── method handlers ───────────────────────────────────────────────────────

    fn do_initialize(&self) -> Result<String, (i32, String)> {
        let caps = format!(
            r#"{{"tools":{{"listChanged":false}},"resources":{{"subscribe":false,"listChanged":false}},"prompts":{{"listChanged":false}}}}"#
        );
        Ok(format!(
            r#"{{"protocolVersion":"{PROTOCOL_VERSION}","capabilities":{caps},"serverInfo":{{"name":"{}","version":"{}"}}}}"#,
            json_escape(&self.server_name),
            json_escape(&self.server_version),
        ))
    }

    fn do_tools_list(&self) -> Result<String, (i32, String)> {
        let items: Vec<String> = self.tools.iter().map(|t| {
            format!(
                r#"{{"name":"{}","description":"{}","inputSchema":{}}}"#,
                json_escape(&t.name),
                json_escape(&t.description),
                t.input_schema,
            )
        }).collect();
        Ok(format!(r#"{{"tools":[{}]}}"#, items.join(",")))
    }

    fn do_tools_call(&self, body: &str) -> Result<String, (i32, String)> {
        let params = json_rpc::extract_raw(body, "params")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing params".to_string()))?;
        let name = json_rpc::extract_str(&params, "name")
            .ok_or((json_rpc::INVALID_PARAMS, "Missing tool name".to_string()))?;
        let args = json_rpc::extract_raw(&params, "arguments")
            .unwrap_or_else(|| "{}".to_string());

        let tool = self.tools.iter().find(|t| t.name == name)
            .ok_or_else(|| (json_rpc::INVALID_PARAMS, format!("Unknown tool: {name}")))?;

        match (tool.handler)(&args) {
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

    fn do_resources_list(&self) -> Result<String, (i32, String)> {
        let items: Vec<String> = self.resources.iter().map(|r| {
            format!(
                r#"{{"uri":"{}","name":"{}","description":"{}","mimeType":"text/plain"}}"#,
                json_escape(&r.uri_template),
                json_escape(&r.name),
                json_escape(&r.description),
            )
        }).collect();
        Ok(format!(r#"{{"resources":[{}]}}"#, items.join(",")))
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

    fn do_prompts_list(&self) -> Result<String, (i32, String)> {
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
        Ok(format!(r#"{{"prompts":[{}]}}"#, items.join(",")))
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
                    self.handle_request(body)
                }
                "OPTIONS" => {
                    // CORS preflight for browser-based MCP clients
                    let mut r = Response::new();
                    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
                    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
                    r.headers.push(Header {
                        name: "Allow".to_string(),
                        value: "POST, OPTIONS".to_string(),
                    });
                    r
                }
                _ => {
                    let mut r = Response::new();
                    r.status_code = *STATUS_CODE_REASON_PHRASE.n405_method_not_allowed.status_code;
                    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n405_method_not_allowed.reason_phrase.to_string();
                    r.headers.push(Header {
                        name: "Allow".to_string(),
                        value: "POST, OPTIONS".to_string(),
                    });
                    r.content_range_list = vec![Range::get_content_range(
                        b"MCP endpoint only accepts POST".to_vec(),
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

fn uri_matches(template: &str, uri: &str) -> bool {
    // Template `"user://{id}"` matches any URI starting with `"user://"`.
    match template.find('{') {
        Some(pos) => uri.starts_with(&template[..pos]),
        None      => template == uri,
    }
}
