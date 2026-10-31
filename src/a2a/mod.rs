//! A2A (Agent2Agent) 1.0 server, JSON-RPC binding (18.18.0, #65): other
//! agents discover this one by its Agent Card at
//! `/.well-known/agent-card.json` and talk to it with `SendMessage`,
//! `GetTask`, `ListTasks`, `CancelTask`.
//!
//! ```rust
//! use rust_web_server::a2a::{A2aReply, A2aServer, AgentCard, Artifact, Message, Skill};
//!
//! let agent = A2aServer::new(
//!     AgentCard::new("Echo agent", "Repeats what you say", "1.0.0")
//!         .skill(Skill::new("echo", "Echo", "Repeats the message").tag("demo")),
//! )
//! .on_message(|ctx, message| {
//!     if message.text().is_empty() {
//!         return A2aReply::Message(Message::agent_text("Say something."));
//!     }
//!     A2aReply::Task(ctx.complete(vec![Artifact::text("echo", message.text())]))
//! });
//! ```
//!
//! Mount it beside your routes with [`A2aServer::wrap`]; it answers the
//! card and `POST` at its path (default `/a2a`) and passes everything else
//! on.

mod model;
mod tasks;

pub use model::{Artifact, Message, Part, PartContent, Role, Task, TaskState, TaskStatus};
pub use tasks::TaskHandle;

use std::sync::Arc;
use std::time::Duration;

use tasks::Shared;

use model::{quote, to_json, Obj};

use crate::application::Application;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::service_discovery::json_lite::{self, JsonValue};

/// The protocol version served (`A2A-Version`).
pub const PROTOCOL_VERSION: &str = "1.0";
/// Where the Agent Card is served.
pub const AGENT_CARD_PATH: &str = "/.well-known/agent-card.json";
/// Request bodies larger than this are refused.
pub const MAX_BODY: usize = 4 * 1024 * 1024;
/// Tasks kept (oldest dropped first).
pub const MAX_TASKS: usize = 10_000;

/// A random UUID v4.
pub(crate) fn new_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let half = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
        h.finish()
    };
    let (a, b) = (half(), half());
    let bytes: Vec<u8> = a.to_be_bytes().into_iter().chain(b.to_be_bytes()).collect();
    let hex: String = bytes
        .iter()
        .enumerate()
        .map(|(i, byte)| match i {
            6 => format!("{:02x}", (byte & 0x0f) | 0x40),
            8 => format!("{:02x}", (byte & 0x3f) | 0x80),
            _ => format!("{byte:02x}"),
        })
        .collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

// ── Agent Card ───────────────────────────────────────────────────────────────

/// Something the agent can do, as listed on its card.
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub examples: Vec<String>,
    pub input_modes: Vec<String>,
    pub output_modes: Vec<String>,
}

impl Skill {
    pub fn new(id: &str, name: &str, description: &str) -> Skill {
        Skill { id: id.into(), name: name.into(), description: description.into(), tags: Vec::new(), examples: Vec::new(), input_modes: Vec::new(), output_modes: Vec::new() }
    }

    pub fn tag(mut self, tag: &str) -> Skill {
        self.tags.push(tag.into());
        self
    }

    pub fn example(mut self, example: &str) -> Skill {
        self.examples.push(example.into());
        self
    }

    fn to_json(&self) -> String {
        let mut o = Obj::new().str("id", &self.id).str("name", &self.name).str("description", &self.description).strs("tags", &self.tags);
        if !self.examples.is_empty() {
            o = o.strs("examples", &self.examples);
        }
        if !self.input_modes.is_empty() {
            o = o.strs("inputModes", &self.input_modes);
        }
        if !self.output_modes.is_empty() {
            o = o.strs("outputModes", &self.output_modes);
        }
        o.done()
    }
}

/// The agent's self-description, served at [`AGENT_CARD_PATH`].
#[derive(Debug, Clone, PartialEq)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub version: String,
    /// The JSON-RPC endpoint's public URL; `None`: from the request's
    /// `Host` and the server's path.
    pub url: Option<String>,
    pub skills: Vec<Skill>,
    pub default_input_modes: Vec<String>,
    pub default_output_modes: Vec<String>,
    pub provider: Option<(String, String)>,
    pub documentation_url: Option<String>,
    pub icon_url: Option<String>,
}

impl AgentCard {
    pub fn new(name: &str, description: &str, version: &str) -> AgentCard {
        AgentCard {
            name: name.into(),
            description: description.into(),
            version: version.into(),
            url: None,
            skills: Vec::new(),
            default_input_modes: vec!["text/plain".into()],
            default_output_modes: vec!["text/plain".into()],
            provider: None,
            documentation_url: None,
            icon_url: None,
        }
    }

    pub fn skill(mut self, skill: Skill) -> AgentCard {
        self.skills.push(skill);
        self
    }

    /// The public URL of the JSON-RPC endpoint (behind a proxy, say).
    pub fn url(mut self, url: &str) -> AgentCard {
        self.url = Some(url.into());
        self
    }

    /// Media types accepted and produced (default `text/plain`).
    pub fn modes(mut self, input: &[&str], output: &[&str]) -> AgentCard {
        self.default_input_modes = input.iter().map(|s| s.to_string()).collect();
        self.default_output_modes = output.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn provider(mut self, organization: &str, url: &str) -> AgentCard {
        self.provider = Some((organization.into(), url.into()));
        self
    }

    pub fn documentation_url(mut self, url: &str) -> AgentCard {
        self.documentation_url = Some(url.into());
        self
    }

    pub(crate) fn to_json(&self, url: &str, bearer: bool) -> String {
        let interface = Obj::new().str("url", url).str("protocolBinding", "JSONRPC").str("protocolVersion", PROTOCOL_VERSION).done();
        let capabilities = r#"{"streaming":true,"pushNotifications":false,"extendedAgentCard":false}"#;
        let skills: Vec<String> = self.skills.iter().map(Skill::to_json).collect();
        let provider = self.provider.as_ref().map(|(org, url)| Obj::new().str("organization", org).str("url", url).done());
        Obj::new()
            .str("name", &self.name)
            .str("description", &self.description)
            .str("version", &self.version)
            .list("supportedInterfaces", &[interface])
            .raw("capabilities", capabilities)
            .strs("defaultInputModes", &self.default_input_modes)
            .strs("defaultOutputModes", &self.default_output_modes)
            .list("skills", &skills)
            .opt_raw("provider", provider.as_deref())
            .opt_str("documentationUrl", self.documentation_url.as_deref())
            .opt_str("iconUrl", self.icon_url.as_deref())
            .opt_raw("securitySchemes", bearer.then_some(r#"{"bearer":{"httpAuthSecurityScheme":{"scheme":"Bearer"}}}"#))
            .opt_raw("securityRequirements", bearer.then_some(r#"[{"schemes":{"bearer":{"list":[]}}}]"#))
            .done()
    }
}

// ── Handler API ──────────────────────────────────────────────────────────────

/// What a message handler sees besides the message.
pub struct A2aContext {
    task_id: String,
    context_id: String,
    user_message: Message,
    shared: Arc<Shared>,
    owner: String,
    /// The task this message continues (one waiting for input), if any.
    pub task: Option<Task>,
    /// Media types the client accepts in replies (empty: any).
    pub accepted_output_modes: Vec<String>,
}

impl A2aContext {
    /// The task id this message belongs to (new, or the continued task's).
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// The conversation's context id.
    pub fn context_id(&self) -> &str {
        &self.context_id
    }

    /// A task in `state` with an optional agent message and artifacts; its
    /// history holds the earlier messages and this one.
    pub fn task(&self, state: TaskState, message: Option<Message>, artifacts: Vec<Artifact>) -> Task {
        let mut history = self.task.as_ref().map(|t| t.history.clone()).unwrap_or_default();
        history.push(self.user_message.clone());
        let message = message.map(|mut m| {
            m.task_id = Some(self.task_id.clone());
            m.context_id = Some(self.context_id.clone());
            m
        });
        if let Some(m) = &message {
            history.push(m.clone());
        }
        let mut all = self.task.as_ref().map(|t| t.artifacts.clone()).unwrap_or_default();
        all.extend(artifacts);
        Task {
            id: self.task_id.clone(),
            context_id: self.context_id.clone(),
            status: TaskStatus { state, message, timestamp: model::timestamp() },
            artifacts: all,
            history,
            metadata: None,
        }
    }

    /// A completed task with these artifacts.
    pub fn complete(&self, artifacts: Vec<Artifact>) -> Task {
        self.task(TaskState::Completed, None, artifacts)
    }

    /// A failed task, saying why.
    pub fn fail(&self, reason: &str) -> Task {
        self.task(TaskState::Failed, Some(Message::agent_text(reason)), Vec::new())
    }

    /// A refused task, saying why.
    pub fn reject(&self, reason: &str) -> Task {
        self.task(TaskState::Rejected, Some(Message::agent_text(reason)), Vec::new())
    }

    /// A task waiting for the client to answer `question`.
    pub fn input_required(&self, question: &str) -> Task {
        self.task(TaskState::InputRequired, Some(Message::agent_text(question)), Vec::new())
    }

    /// Start working on this message in the background: the task is stored
    /// (working) at once; return `A2aReply::Running(handle)` and update it
    /// through the handle from any thread. `SendMessage` waits for it to
    /// finish (or ask for input), streams follow it.
    pub fn start(&self) -> TaskHandle {
        let task = self.task(TaskState::Working, None, Vec::new());
        let mut store = self.shared.lock();
        store.put(&self.owner, task);
        let canceled = store.get_mut(&self.owner, &self.task_id).map(|s| Arc::clone(&s.canceled)).unwrap_or_default();
        drop(store);
        TaskHandle { shared: Arc::clone(&self.shared), owner: self.owner.clone(), id: self.task_id.clone(), canceled }
    }
}

impl std::fmt::Debug for TaskHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskHandle").field("id", &self.id).finish()
    }
}

/// A handler's answer to a message.
#[derive(Debug, Clone)]
pub enum A2aReply {
    /// A direct reply; no task.
    Message(Message),
    /// A task (stored, so `GetTask` and later messages find it).
    Task(Task),
    /// A task started with [`A2aContext::start`], still being worked on.
    Running(TaskHandle),
}

type Handler = Arc<dyn Fn(&A2aContext, Message) -> A2aReply + Send + Sync>;

/// The A2A server: an [`Application`] serving the Agent Card and the
/// JSON-RPC endpoint.
#[derive(Clone)]
pub struct A2aServer {
    card: Arc<AgentCard>,
    path: String,
    handler: Handler,
    tasks: Arc<Shared>,
    fallback: Option<Arc<dyn Application + Send + Sync>>,
    /// Bearer tokens and the identity each stands for; empty: no auth.
    tokens: Arc<Vec<(String, String)>>,
    blocking_timeout: Duration,
}

impl A2aServer {
    /// An agent described by `card`; until [`on_message`](Self::on_message)
    /// it rejects every message.
    pub fn new(card: AgentCard) -> A2aServer {
        A2aServer {
            card: Arc::new(card),
            path: "/a2a".to_string(),
            handler: Arc::new(|ctx: &A2aContext, _| A2aReply::Task(ctx.reject("this agent has no message handler"))),
            tasks: Arc::new(Shared::default()),
            fallback: None,
            tokens: Arc::new(Vec::new()),
            blocking_timeout: Duration::from_secs(300),
        }
    }

    /// The JSON-RPC endpoint's path (default `/a2a`).
    pub fn at(mut self, path: &str) -> A2aServer {
        self.path = path.to_string();
        self
    }

    /// Answer each message.
    pub fn on_message<F>(mut self, handler: F) -> A2aServer
    where
        F: Fn(&A2aContext, Message) -> A2aReply + Send + Sync + 'static,
    {
        self.handler = Arc::new(handler);
        self
    }

    /// Require `Authorization: Bearer <token>` on the JSON-RPC endpoint (the
    /// card stays public and lists the scheme).
    pub fn require_bearer(self, token: &str) -> A2aServer {
        self.require_bearer_tokens([(token, "default")])
    }

    /// Several tokens, each standing for an identity; callers only see their
    /// own tasks.
    pub fn require_bearer_tokens<'a>(mut self, tokens: impl IntoIterator<Item = (&'a str, &'a str)>) -> A2aServer {
        self.tokens = Arc::new(tokens.into_iter().filter(|(t, _)| !t.is_empty()).map(|(t, i)| (t.to_string(), i.to_string())).collect());
        self
    }

    /// How long `SendMessage` waits for a running task to finish or ask for
    /// input before answering with its current state (default 300 s).
    pub fn blocking_timeout(mut self, timeout: Duration) -> A2aServer {
        self.blocking_timeout = timeout;
        self
    }

    /// Requests that aren't A2A go to `app`.
    pub fn wrap(mut self, app: impl Application + Send + Sync + 'static) -> A2aServer {
        self.fallback = Some(Arc::new(app));
        self
    }

    /// The caller's identity: `""` without auth, the token's identity with
    /// it, `None` when the token is missing or wrong.
    fn caller(&self, request: &Request) -> Option<String> {
        if self.tokens.is_empty() {
            return Some(String::new());
        }
        let sent = request.header(crate::header::Header::_AUTHORIZATION)?.value.strip_prefix("Bearer ")?.trim().to_string();
        self.tokens.iter().find(|(token, _)| crate::core::constant_time_eq(token.as_bytes(), sent.as_bytes())).map(|(_, identity)| identity.clone())
    }

    fn endpoint_url(&self, request: &Request, connection: &ConnectionInfo) -> String {
        if let Some(url) = &self.card.url {
            return url.clone();
        }
        let host = request.header(crate::header::Header::_HOST).map(|h| h.value.to_string()).unwrap_or_else(|| format!("{}:{}", connection.server.ip, connection.server.port));
        format!("{}://{host}{}", if connection.tls { "https" } else { "http" }, self.path)
    }

    fn card_response(&self, request: &Request, connection: &ConnectionInfo) -> Response {
        let mut response = Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, self.card.to_json(&self.endpoint_url(request, connection), !self.tokens.is_empty()).into_bytes());
        response.headers.push(crate::header::Header::new("Cache-Control", "public, max-age=300".to_string()));
        response
    }

    fn rpc(&self, request: &Request, owner: &str) -> Response {
        let reply = |body: String| Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, body.into_bytes());
        if request.body.len() > MAX_BODY {
            return crate::error::problem(STATUS_CODE_REASON_PHRASE.n413_payload_too_large, "A2A request too large");
        }
        let Ok(text) = std::str::from_utf8(&request.body) else {
            return reply(error_json("null", RpcError::Parse("the body isn't UTF-8".into())));
        };
        let call = match json_lite::parse(text) {
            Ok(call) => call,
            Err(e) => return reply(error_json("null", RpcError::Parse(e))),
        };
        let id = call.get("id").map(to_json).unwrap_or_else(|| "null".to_string());
        let method = match (call.get("jsonrpc").and_then(JsonValue::as_str), call.get("method").and_then(JsonValue::as_str)) {
            (Some("2.0"), Some(method)) => method,
            _ => return reply(error_json(&id, RpcError::InvalidRequest)),
        };
        if let Err(e) = check_version(request) {
            return reply(error_json(&id, e));
        }
        let params = call.get("params").cloned().unwrap_or(JsonValue::Object(Vec::new()));
        if method == "SendStreamingMessage" || method == "SubscribeToTask" {
            return match self.stream(method, &params, owner, &id) {
                Ok(response) => response,
                Err(e) => reply(error_json(&id, e)),
            };
        }
        match self.dispatch(method, &params, owner) {
            Ok(result) => reply(format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{result}}}"#)),
            Err(e) => reply(error_json(&id, e)),
        }
    }

    fn dispatch(&self, method: &str, params: &JsonValue, owner: &str) -> Result<String, RpcError> {
        match method {
            "SendMessage" => self.send_message(params, owner),
            "GetTask" => {
                let id = params.get("id").and_then(JsonValue::as_str).ok_or_else(|| RpcError::InvalidParams("id is required".into()))?;
                let tasks = self.tasks.lock();
                let task = tasks.get(owner, id).ok_or(RpcError::TaskNotFound)?;
                Ok(task.to_json(history_length(params)?))
            }
            "ListTasks" => self.list_tasks(params, owner),
            "CancelTask" => {
                let id = params.get("id").and_then(JsonValue::as_str).ok_or_else(|| RpcError::InvalidParams("id is required".into()))?;
                if !tasks::set_state(&self.tasks, owner, id, TaskState::Canceled, None) {
                    return Err(match self.tasks.lock().get(owner, id) {
                        Some(_) => RpcError::TaskNotCancelable,
                        None => RpcError::TaskNotFound,
                    });
                }
                Ok(self.tasks.lock().get(owner, id).map(|t| t.to_json(None)).unwrap_or_default())
            }
            "GetExtendedAgentCard" => Err(RpcError::UnsupportedOperation("this agent has no extended card".into())),
            "CreateTaskPushNotificationConfig" | "GetTaskPushNotificationConfig" | "ListTaskPushNotificationConfigs" | "DeleteTaskPushNotificationConfig" => {
                Err(RpcError::PushNotificationNotSupported)
            }
            _ => Err(RpcError::MethodNotFound(method.to_string())),
        }
    }

    /// Run the handler for a `SendMessage` / `SendStreamingMessage`.
    fn handle_message(&self, params: &JsonValue, owner: &str) -> Result<Outcome, RpcError> {
        let message = Message::from_json(params.get("message").ok_or_else(|| RpcError::InvalidParams("message is required".into()))?).map_err(RpcError::InvalidParams)?;
        let configuration = params.get("configuration");
        let accepted_output_modes = configuration
            .and_then(|c| c.get("acceptedOutputModes"))
            .and_then(JsonValue::as_array)
            .map(|a| a.iter().filter_map(|m| m.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let existing = match &message.task_id {
            Some(id) => {
                let task = self.tasks.lock().get(owner, id).cloned().ok_or(RpcError::TaskNotFound)?;
                if task.status.state.is_terminal() {
                    return Err(RpcError::UnsupportedOperation(format!("task {id} is {} and takes no more messages", task.status.state.as_str())));
                }
                Some(task)
            }
            None => None,
        };
        let task_id = existing.as_ref().map(|t| t.id.clone()).unwrap_or_else(new_id);
        let context_id = existing.as_ref().map(|t| t.context_id.clone()).or_else(|| message.context_id.clone()).unwrap_or_else(new_id);
        let mut user_message = message.clone();
        user_message.context_id = Some(context_id.clone());
        let ctx = A2aContext {
            task_id,
            context_id: context_id.clone(),
            user_message,
            shared: Arc::clone(&self.tasks),
            owner: owner.to_string(),
            task: existing,
            accepted_output_modes,
        };
        let handler = Arc::clone(&self.handler);
        let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&ctx, message))).unwrap_or_else(|_| A2aReply::Task(ctx.fail("the agent failed")));
        Ok(match reply {
            A2aReply::Message(mut m) => {
                m.context_id.get_or_insert(context_id);
                Outcome::Message(m)
            }
            A2aReply::Task(task) => {
                let id = task.id.clone();
                self.tasks.lock().put(owner, task);
                Outcome::Task(id, false)
            }
            A2aReply::Running(handle) => Outcome::Task(handle.id, true),
        })
    }

    fn send_message(&self, params: &JsonValue, owner: &str) -> Result<String, RpcError> {
        let configuration = params.get("configuration");
        let history = match configuration {
            Some(c) => history_length(c)?,
            None => None,
        };
        let return_immediately = configuration.and_then(|c| c.get("returnImmediately")) == Some(&JsonValue::Bool(true));
        match self.handle_message(params, owner)? {
            Outcome::Message(m) => Ok(format!(r#"{{"message":{}}}"#, m.to_json())),
            Outcome::Task(id, running) => {
                // Blocking by default: until the task finishes or waits on
                // the client (A2A 1.0 §3.2.2), within blocking_timeout.
                let deadline = std::time::Instant::now() + self.blocking_timeout;
                let mut store = self.tasks.lock();
                loop {
                    let Some(task) = store.get(owner, &id) else { return Err(RpcError::TaskNotFound) };
                    let state = task.status.state;
                    let now = std::time::Instant::now();
                    if !running || return_immediately || state.is_terminal() || state.is_interrupted() || now >= deadline {
                        return Ok(format!(r#"{{"task":{}}}"#, task.to_json(history)));
                    }
                    store = self.tasks.changed.wait_timeout(store, deadline - now).map(|(g, _)| g).unwrap_or_else(|e| e.into_inner().0);
                }
            }
        }
    }

    /// `SendStreamingMessage` and `SubscribeToTask`: an SSE stream whose
    /// first event is the task (or the one message), then its updates until
    /// it finishes or waits on the client.
    fn stream(&self, method: &str, params: &JsonValue, owner: &str, rpc_id: &str) -> Result<Response, RpcError> {
        let id = if method == "SubscribeToTask" {
            let id = params.get("id").and_then(JsonValue::as_str).ok_or_else(|| RpcError::InvalidParams("id is required".into()))?.to_string();
            let store = self.tasks.lock();
            let task = store.get(owner, &id).ok_or(RpcError::TaskNotFound)?;
            if task.status.state.is_terminal() {
                return Err(RpcError::UnsupportedOperation(format!("task {id} is {}", task.status.state.as_str())));
            }
            id
        } else {
            match self.handle_message(params, owner)? {
                Outcome::Message(m) => {
                    let (sender, response) = crate::sse::SseStream::new().start();
                    let _ = sender.try_send(tasks::event(rpc_id, "message", &m.to_json()));
                    return Ok(response);
                }
                Outcome::Task(id, _) => id,
            }
        };
        let (sender, response) = crate::sse::SseStream::new().buffer(64).start();
        let mut store = self.tasks.lock();
        let stored = store.get_mut(owner, &id).ok_or(RpcError::TaskNotFound)?;
        let _ = sender.try_send(tasks::event(rpc_id, "task", &stored.task.to_json(None)));
        let state = stored.task.status.state;
        let follow = !state.is_terminal() && (method == "SubscribeToTask" || !state.is_interrupted());
        if follow {
            if stored.subscribers.len() >= tasks::MAX_SUBSCRIBERS {
                return Err(RpcError::UnsupportedOperation(format!("task {id} has {} streams already", tasks::MAX_SUBSCRIBERS)));
            }
            stored.subscribers.push(tasks::Subscriber { sender, rpc_id: rpc_id.to_string() });
        }
        Ok(response)
    }

    fn list_tasks(&self, params: &JsonValue, owner: &str) -> Result<String, RpcError> {
        let page_size = match params.get("pageSize").and_then(JsonValue::as_f64) {
            Some(n) if (1.0..=100.0).contains(&n) && n.fract() == 0.0 => n as usize,
            Some(_) => return Err(RpcError::InvalidParams("pageSize must be 1 to 100".into())),
            None => 50,
        };
        let state = match params.get("status").and_then(JsonValue::as_str) {
            Some(s) => Some(TaskState::parse(s).ok_or_else(|| RpcError::InvalidParams(format!("unknown status {s}")))?),
            None => None,
        };
        let context = params.get("contextId").and_then(JsonValue::as_str);
        let offset: usize = match params.get("pageToken").and_then(JsonValue::as_str).filter(|t| !t.is_empty()) {
            Some(t) => t.parse().map_err(|_| RpcError::InvalidParams("bad pageToken".into()))?,
            None => 0,
        };
        let history = history_length(params)?.or(Some(0));
        let tasks = self.tasks.lock();
        // Newest first.
        let matching: Vec<&Task> = tasks
            .order
            .iter()
            .rev()
            .filter_map(|id| tasks.get(owner, id))
            .filter(|t| state.is_none_or(|s| t.status.state == s) && context.is_none_or(|c| t.context_id == c))
            .collect();
        let page: Vec<String> = matching.iter().skip(offset).take(page_size).map(|t| t.to_json(history)).collect();
        let next = if offset + page_size < matching.len() { (offset + page_size).to_string() } else { String::new() };
        Ok(Obj::new()
            .list("tasks", &page)
            .raw("totalSize", &matching.len().to_string())
            .raw("pageSize", &page_size.to_string())
            .str("nextPageToken", &next)
            .done())
    }
}

/// What a message led to.
enum Outcome {
    Message(Message),
    /// A stored task's id, and whether a worker is still updating it.
    Task(String, bool),
}

fn history_length(params: &JsonValue) -> Result<Option<usize>, RpcError> {
    match params.get("historyLength") {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::Number(n)) if *n >= 0.0 && n.fract() == 0.0 => Ok(Some(*n as usize)),
        Some(_) => Err(RpcError::InvalidParams("historyLength must be a non-negative integer".into())),
    }
}

/// `A2A-Version` (header or query parameter): `1.0` is served.
fn check_version(request: &Request) -> Result<(), RpcError> {
    let from_query = || {
        let query = request.request_uri.split_once('?')?.1;
        query.split('&').find_map(|p| p.strip_prefix("A2A-Version=")).map(str::to_string)
    };
    let version = request.header("A2A-Version").map(|h| h.value.trim().to_string()).or_else(from_query).unwrap_or_default();
    let major_minor: String = version.split('.').take(2).collect::<Vec<_>>().join(".");
    if major_minor == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(RpcError::VersionNotSupported(if version.is_empty() { "0.3 (no A2A-Version)".to_string() } else { version }))
    }
}

/// A JSON-RPC error.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RpcError {
    Parse(String),
    InvalidRequest,
    MethodNotFound(String),
    InvalidParams(String),
    TaskNotFound,
    TaskNotCancelable,
    PushNotificationNotSupported,
    UnsupportedOperation(String),
    VersionNotSupported(String),
}

impl RpcError {
    fn parts(&self) -> (i32, &'static str, String) {
        match self {
            RpcError::Parse(e) => (-32700, "PARSE_ERROR", format!("invalid JSON: {e}")),
            RpcError::InvalidRequest => (-32600, "INVALID_REQUEST", "not a JSON-RPC 2.0 request".to_string()),
            RpcError::MethodNotFound(m) => (-32601, "METHOD_NOT_FOUND", format!("unknown method {m}")),
            RpcError::InvalidParams(e) => (-32602, "INVALID_PARAMS", e.clone()),
            RpcError::TaskNotFound => (-32001, "TASK_NOT_FOUND", "task not found".to_string()),
            RpcError::TaskNotCancelable => (-32002, "TASK_NOT_CANCELABLE", "the task is finished and can't be canceled".to_string()),
            RpcError::PushNotificationNotSupported => (-32003, "PUSH_NOTIFICATION_NOT_SUPPORTED", "push notifications aren't supported by this agent".to_string()),
            RpcError::UnsupportedOperation(e) => (-32004, "UNSUPPORTED_OPERATION", e.clone()),
            RpcError::VersionNotSupported(v) => (-32009, "VERSION_NOT_SUPPORTED", format!("A2A version {v} isn't supported; this agent speaks {PROTOCOL_VERSION}")),
        }
    }
}

fn error_json(id: &str, error: RpcError) -> String {
    let (code, reason, message) = error.parts();
    let data = format!(r#"[{{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":{},"domain":"a2a-protocol.org"}}]"#, quote(reason));
    format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":{code},"message":{},"data":{data}}}}}"#, quote(&message))
}

impl Application for A2aServer {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        let path = crate::url::URL::request_target_path(&request.request_uri).unwrap_or("");
        if path == AGENT_CARD_PATH && (request.method == "GET" || request.method == "HEAD") {
            return Ok(self.card_response(request, connection));
        }
        if path == self.path {
            if request.method == "POST" {
                let owner = match self.caller(request) {
                    Some(owner) => owner,
                    None => {
                        let mut response = crate::error::problem(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "A2A requests need a bearer token");
                        response.headers.push(crate::header::Header::new("WWW-Authenticate", r#"Bearer realm="a2a""#.to_string()));
                        return Ok(response);
                    }
                };
                return Ok(self.rpc(request, &owner));
            }
            let mut response = crate::error::problem(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed, "A2A requests are JSON-RPC POSTs");
            response.headers.push(crate::header::Header::new("Allow", "POST".to_string()));
            return Ok(response);
        }
        match &self.fallback {
            Some(app) => app.execute(request, connection),
            None => Ok(crate::error::problem(STATUS_CODE_REASON_PHRASE.n404_not_found, "")),
        }
    }
}

#[cfg(test)]
mod tests;
