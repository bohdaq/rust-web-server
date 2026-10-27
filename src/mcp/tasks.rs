//! The MCP Tasks extension (`io.modelcontextprotocol/tasks`, SEP-2663) on
//! MCP 2026-07-28: a tool hands long work to a task with
//! [`McpContext::start_task`] and answers at once with a handle; the client
//! polls `tasks/get`, answers the task's input requests with `tasks/update`
//! and may `tasks/cancel` it. Tasks live in memory for their TTL (an hour
//! by default, [`McpServer::task_ttl`]) and belong to the caller that
//! started them.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use super::{json_escape, json_rpc, McpContent, McpContext, McpServer};
use crate::mcp::modern::InputRequired;

/// The extension's identifier.
pub(crate) const EXTENSION: &str = "io.modelcontextprotocol/tasks";
/// At most this many tasks are kept; the oldest finished ones go first.
const MAX_TASKS: usize = 10_000;
const POLL_INTERVAL_MS: u64 = 1000;

/// Why a task's work ended without a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskError {
    /// The tool ran and reports an error: the task completes with a result
    /// marked `isError`, as a synchronous call would.
    Tool(String),
    /// A protocol-level failure: the task's status is `failed`, with this
    /// JSON-RPC error.
    Failed { code: i32, message: String },
    /// The client cancelled the task.
    Cancelled,
}

impl From<String> for TaskError {
    fn from(message: String) -> Self {
        TaskError::Tool(message)
    }
}

impl From<&str> for TaskError {
    fn from(message: &str) -> Self {
        TaskError::Tool(message.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Working,
    InputRequired,
    Completed,
    Failed,
    Cancelled,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Working => "working",
            Status::InputRequired => "input_required",
            Status::Completed => "completed",
            Status::Failed => "failed",
            Status::Cancelled => "cancelled",
        }
    }

    fn is_terminal(self) -> bool {
        matches!(self, Status::Completed | Status::Failed | Status::Cancelled)
    }
}

#[derive(Debug)]
struct State {
    status: Status,
    status_message: Option<String>,
    updated_ms: u64,
    /// Outstanding input requests: `(key, request JSON)`.
    pending: Vec<(String, String)>,
    /// Answers to the current input round.
    answers: HashMap<String, String>,
    /// Every key ever issued: a key is never reused.
    issued: Vec<String>,
    /// `result` (completed) or `error` (failed), JSON.
    payload: Option<String>,
}

/// Lets [`McpContext::start_task`] reach the server's tasks, for the caller.
#[derive(Clone)]
pub(crate) struct TaskHandle {
    pub(crate) server: McpServer,
    pub(crate) principal: String,
}

impl std::fmt::Debug for TaskHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskHandle").field("principal", &self.principal).finish()
    }
}

/// One task.
#[derive(Debug)]
pub(crate) struct Task {
    id: String,
    principal: String,
    created_ms: u64,
    ttl_ms: u64,
    state: Mutex<State>,
    changed: Condvar,
    cancelled: Arc<AtomicBool>,
}

/// The tasks of one [`McpServer`] (shared by its clones).
#[derive(Debug, Default)]
pub(crate) struct TaskStore {
    tasks: Mutex<HashMap<String, Arc<Task>>>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// `2026-07-28T10:30:00.123Z`.
pub(crate) fn iso8601(ms: u64) -> String {
    let (days, rest) = (ms / 86_400_000, ms % 86_400_000);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let secs = rest / 1000;
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z", secs / 3600, secs % 3600 / 60, secs % 60, rest % 1000)
}

fn task_id() -> String {
    super::modern::random_key()[..16].iter().map(|b| format!("{b:02x}")).collect()
}

impl Task {
    /// The `DetailedTask` fields: status, metadata and the status's payload.
    fn detail_json(&self) -> String {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut json = format!(
            r#""taskId":"{}","status":"{}","createdAt":"{}","lastUpdatedAt":"{}","ttlMs":{},"pollIntervalMs":{POLL_INTERVAL_MS}"#,
            self.id,
            state.status.as_str(),
            iso8601(self.created_ms),
            iso8601(state.updated_ms),
            self.ttl_ms,
        );
        if let Some(message) = &state.status_message {
            json.push_str(&format!(r#","statusMessage":"{}""#, json_escape(message)));
        }
        match state.status {
            Status::InputRequired => {
                let requests: Vec<String> = state.pending.iter().map(|(key, request)| format!(r#""{}":{request}"#, json_escape(key))).collect();
                json.push_str(&format!(r#","inputRequests":{{{}}}"#, requests.join(",")));
            }
            Status::Completed => json.push_str(&format!(r#","result":{}"#, state.payload.as_deref().unwrap_or("{}"))),
            Status::Failed => json.push_str(&format!(r#","error":{}"#, state.payload.as_deref().unwrap_or(r#"{"code":-32603,"message":"failed"}"#))),
            Status::Working | Status::Cancelled => {}
        }
        json
    }

    fn set(&self, store: &McpServer, change: impl FnOnce(&mut State)) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.status.is_terminal() {
                return;
            }
            change(&mut state);
            state.updated_ms = now_ms();
        }
        self.changed.notify_all();
        let params = format!("{{{}}}", self.detail_json());
        store.deliver_task_notification(&self.id, &params);
    }
}

/// What a task's work runs with: input from the client through
/// `tasks/update`, cancellation, a status message.
pub struct TaskContext {
    task: Option<(Arc<Task>, McpServer)>,
}

impl TaskContext {
    /// Whether this work runs as a task (else inline, the client not having
    /// declared the extension).
    pub fn is_task(&self) -> bool {
        self.task.is_some()
    }

    /// Whether the client cancelled the task.
    pub fn is_cancelled(&self) -> bool {
        self.task.as_ref().is_some_and(|(task, _)| task.cancelled.load(Ordering::Relaxed))
    }

    /// Show `message` as the task's `statusMessage`.
    pub fn status_message(&self, message: &str) {
        if let Some((task, server)) = &self.task {
            task.set(server, |state| state.status_message = Some(message.to_string()));
        }
    }

    /// Ask the client for input and wait for all of it: the task shows
    /// `input_required` with `request`'s requests until `tasks/update` has
    /// answered every key; returns the answers `(key, raw JSON)`. Keys must
    /// be new for the task. `Err(Cancelled)` when the task is cancelled;
    /// an error when the work runs inline (no tasks extension).
    pub fn input(&self, request: InputRequired) -> Result<Vec<(String, String)>, TaskError> {
        let Some((task, server)) = &self.task else {
            return Err(TaskError::Tool("this tool asks for input during a task, which needs the client's io.modelcontextprotocol/tasks extension".to_string()));
        };
        let requests = request.requests_json();
        {
            let state = task.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((key, _)) = requests.iter().find(|(key, _)| state.issued.contains(key)) {
                return Err(TaskError::Failed { code: json_rpc::INTERNAL_ERROR, message: format!("input key {key:?} was already used by this task") });
            }
        }
        let keys: Vec<String> = requests.iter().map(|(key, _)| key.clone()).collect();
        task.set(server, |state| {
            state.issued.extend(keys.iter().cloned());
            state.answers.clear();
            state.pending = requests;
            state.status = Status::InputRequired;
        });
        let mut state = task.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if task.cancelled.load(Ordering::Relaxed) || state.status == Status::Cancelled {
                return Err(TaskError::Cancelled);
            }
            if state.pending.is_empty() && state.status == Status::Working {
                return Ok(keys.iter().map(|key| (key.clone(), state.answers.get(key).cloned().unwrap_or_default())).collect());
            }
            let (next, _) = task.changed.wait_timeout(state, Duration::from_secs(1)).unwrap_or_else(|e| e.into_inner());
            state = next;
        }
    }
}

impl McpContext {
    /// Run `work` as a task (MCP Tasks extension): when the client declared
    /// `io.modelcontextprotocol/tasks` on this MCP 2026-07-28 request, the
    /// call answers at once with the task's handle and `work` runs on a
    /// thread of its own; the client polls `tasks/get` for its result.
    /// Otherwise `work` runs now and its result is the call's. Return this
    /// from a tool handler:
    ///
    /// ```rust
    /// use rust_web_server::mcp::{McpContent, McpServer};
    ///
    /// let server = McpServer::new("ci", "1.0").tool_with_context("build", "Builds the project", "{}", |ctx, _args| {
    ///     ctx.start_task(|task| {
    ///         task.status_message("compiling");
    ///         std::thread::sleep(std::time::Duration::from_secs(2));
    ///         Ok(McpContent::text("build ok"))
    ///     })
    /// });
    /// ```
    pub fn start_task<F>(&self, work: F) -> Result<McpContent, String>
    where
        F: FnOnce(TaskContext) -> Result<McpContent, TaskError> + Send + 'static,
    {
        let Some(TaskHandle { server, principal }) = self.tasks.clone() else {
            return match work(TaskContext { task: None }) {
                Ok(content) => Ok(content),
                Err(TaskError::Tool(message)) | Err(TaskError::Failed { message, .. }) => Err(message),
                Err(TaskError::Cancelled) => Err("cancelled".to_string()),
            };
        };
        let now = now_ms();
        let task = Arc::new(Task {
            id: task_id(),
            principal,
            created_ms: now,
            ttl_ms: server.task_ttl_ms,
            state: Mutex::new(State {
                status: Status::Working,
                status_message: None,
                updated_ms: now,
                pending: Vec::new(),
                answers: HashMap::new(),
                issued: Vec::new(),
                payload: None,
            }),
            changed: Condvar::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
        });
        server.task_store.insert(Arc::clone(&task));
        let id = task.id.clone();
        std::thread::spawn(move || {
            let context = TaskContext { task: Some((Arc::clone(&task), server.clone())) };
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(context)));
            task.set(&server, |state| {
                state.pending.clear();
                let (status, payload) = match outcome {
                    Ok(Ok(content)) if content.kind == "input_required" || content.kind == "task" => (
                        Status::Failed,
                        format!(r#"{{"code":{},"message":"a task's work can't return {}"}}"#, json_rpc::INTERNAL_ERROR, content.kind),
                    ),
                    Ok(Ok(content)) => (Status::Completed, McpServer::format_tool_result(Ok(content))),
                    Ok(Err(TaskError::Tool(message))) => (Status::Completed, McpServer::format_tool_result(Err(message))),
                    Ok(Err(TaskError::Failed { code, message })) => {
                        state.status_message = Some(message.clone());
                        (Status::Failed, format!(r#"{{"code":{code},"message":"{}"}}"#, json_escape(&message)))
                    }
                    Ok(Err(TaskError::Cancelled)) => (Status::Cancelled, String::new()),
                    Err(_) => {
                        state.status_message = Some("the task's work panicked".to_string());
                        (Status::Failed, format!(r#"{{"code":{},"message":"the task's work panicked"}}"#, json_rpc::INTERNAL_ERROR))
                    }
                };
                state.status = status;
                state.payload = (!payload.is_empty()).then_some(payload);
            });
        });
        Ok(McpContent { kind: "task", text: id, mime_type: None, uri: None })
    }

    /// Whether this request's client declared the tasks extension.
    pub fn supports_tasks(&self) -> bool {
        self.tasks.is_some()
    }
}

impl TaskStore {
    fn insert(&self, task: Arc<Task>) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let now = now_ms();
        tasks.retain(|_, t| now <= t.created_ms + t.ttl_ms);
        if tasks.len() >= MAX_TASKS {
            // The oldest finished task, else the oldest.
            let finished = |t: &&Arc<Task>| t.state.lock().map(|s| s.status.is_terminal()).unwrap_or(true);
            let oldest = tasks
                .values()
                .filter(finished)
                .min_by_key(|t| t.created_ms)
                .or_else(|| tasks.values().min_by_key(|t| t.created_ms))
                .map(|t| t.id.clone());
            if let Some(id) = oldest {
                tasks.remove(&id);
            }
        }
        tasks.insert(task.id.clone(), task);
    }

    /// The task `id` of `principal`, if it exists and hasn't expired.
    fn get(&self, id: &str, principal: &str) -> Option<Arc<Task>> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.get(id).filter(|t| t.principal == principal && now_ms() <= t.created_ms + t.ttl_ms).cloned()
    }
}

impl McpServer {
    /// How long a task is kept after it starts (default an hour).
    pub fn task_ttl(mut self, ttl: Duration) -> Self {
        self.task_ttl_ms = ttl.as_millis() as u64;
        self
    }

    /// Mark the tool `name` as usable only as a task: a client that didn't
    /// declare the tasks extension gets `-32021` instead of the tool running.
    pub fn task_required(self, name: &str) -> Self {
        self.task_required_tools.write().unwrap().push(name.to_string());
        self
    }

    pub(crate) fn is_task_required(&self, name: &str) -> bool {
        self.task_required_tools.read().unwrap().iter().any(|n| n == name)
    }

    /// `CreateTaskResult`'s fields for the task `id`.
    pub(crate) fn create_task_fields(&self, id: &str, principal: &str) -> Option<String> {
        self.task_store.get(id, principal).map(|task| task.detail_json())
    }

    /// `tasks/get`: the task's `DetailedTask` fields.
    pub(crate) fn task_get(&self, id: &str, principal: &str) -> Result<String, (i32, String)> {
        self.task_store.get(id, principal).map(|task| task.detail_json()).ok_or_else(not_found)
    }

    /// `tasks/update`: record answers to outstanding requests (others are
    /// ignored); the task resumes once every one is answered.
    pub(crate) fn task_update(&self, id: &str, principal: &str, responses: &str) -> Result<(), (i32, String)> {
        let task = self.task_store.get(id, principal).ok_or_else(not_found)?;
        let answers = json_rpc::object_members(responses).ok_or((json_rpc::INVALID_PARAMS, "inputResponses must be an object".to_string()))?;
        task.set(self, |state| {
            for (key, answer) in answers {
                if let Some(at) = state.pending.iter().position(|(k, _)| *k == key) {
                    state.pending.remove(at);
                    state.answers.insert(key, answer);
                }
            }
            if state.pending.is_empty() && state.status == Status::InputRequired {
                state.status = Status::Working;
            }
        });
        Ok(())
    }

    /// `tasks/cancel`: a running task becomes `cancelled` at once (its work
    /// sees [`TaskContext::is_cancelled`]); a finished one stays as it is.
    pub(crate) fn task_cancel(&self, id: &str, principal: &str) -> Result<(), (i32, String)> {
        let task = self.task_store.get(id, principal).ok_or_else(not_found)?;
        task.cancelled.store(true, Ordering::Relaxed);
        task.set(self, |state| {
            state.status = Status::Cancelled;
            state.pending.clear();
            state.status_message = Some("cancelled by the client".to_string());
        });
        Ok(())
    }
}

fn not_found() -> (i32, String) {
    (json_rpc::INVALID_PARAMS, "Failed to retrieve task: Task not found".to_string())
}

#[cfg(test)]
mod tests;
