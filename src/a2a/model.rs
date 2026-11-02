//! The A2A 1.0 data model and its JSON form (ProtoJSON: camelCase fields,
//! enums as `TASK_STATE_*` / `ROLE_*` strings, no `kind` discriminator).

use crate::service_discovery::json_lite::JsonValue;

/// Who sent a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Agent,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::User => "ROLE_USER",
            Role::Agent => "ROLE_AGENT",
        }
    }
}

/// One piece of a message or artifact: exactly one of text, raw bytes, a
/// URL or structured data, with an optional media type and file name.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub content: PartContent,
    pub media_type: Option<String>,
    pub filename: Option<String>,
    /// Raw JSON object, as sent.
    pub metadata: Option<String>,
}

/// What a [`Part`] holds.
#[derive(Debug, Clone, PartialEq)]
pub enum PartContent {
    Text(String),
    /// Bytes (base64 on the wire).
    Raw(Vec<u8>),
    Url(String),
    /// Any JSON value, kept as its JSON text.
    Data(String),
}

impl Part {
    pub fn text(text: impl Into<String>) -> Part {
        Part { content: PartContent::Text(text.into()), media_type: None, filename: None, metadata: None }
    }

    /// Structured data from its JSON text (`{"total": 42}`).
    pub fn data(json: impl Into<String>) -> Part {
        Part { content: PartContent::Data(json.into()), media_type: Some("application/json".to_string()), filename: None, metadata: None }
    }

    pub fn raw(bytes: Vec<u8>, media_type: &str) -> Part {
        Part { content: PartContent::Raw(bytes), media_type: Some(media_type.to_string()), filename: None, metadata: None }
    }

    pub fn url(url: impl Into<String>, media_type: &str) -> Part {
        Part { content: PartContent::Url(url.into()), media_type: Some(media_type.to_string()), filename: None, metadata: None }
    }

    pub fn filename(mut self, name: impl Into<String>) -> Part {
        self.filename = Some(name.into());
        self
    }

    /// The text, when this is a text part.
    pub fn as_text(&self) -> Option<&str> {
        match &self.content {
            PartContent::Text(t) => Some(t),
            _ => None,
        }
    }
}

/// A message between a client and an agent.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub message_id: String,
    pub context_id: Option<String>,
    pub task_id: Option<String>,
    pub role: Role,
    pub parts: Vec<Part>,
    pub metadata: Option<String>,
    pub extensions: Vec<String>,
    pub reference_task_ids: Vec<String>,
}

impl Message {
    /// An agent message with one text part.
    pub fn agent_text(text: impl Into<String>) -> Message {
        Message::agent(vec![Part::text(text)])
    }

    /// An agent message.
    pub fn agent(parts: Vec<Part>) -> Message {
        Message {
            message_id: super::new_id(),
            context_id: None,
            task_id: None,
            role: Role::Agent,
            parts,
            metadata: None,
            extensions: Vec::new(),
            reference_task_ids: Vec::new(),
        }
    }

    /// All text parts, joined by newlines.
    pub fn text(&self) -> String {
        self.parts.iter().filter_map(Part::as_text).collect::<Vec<_>>().join("\n")
    }
}

/// A task's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Submitted,
    Working,
    InputRequired,
    AuthRequired,
    Completed,
    Failed,
    Canceled,
    Rejected,
}

impl TaskState {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskState::Submitted => "TASK_STATE_SUBMITTED",
            TaskState::Working => "TASK_STATE_WORKING",
            TaskState::InputRequired => "TASK_STATE_INPUT_REQUIRED",
            TaskState::AuthRequired => "TASK_STATE_AUTH_REQUIRED",
            TaskState::Completed => "TASK_STATE_COMPLETED",
            TaskState::Failed => "TASK_STATE_FAILED",
            TaskState::Canceled => "TASK_STATE_CANCELED",
            TaskState::Rejected => "TASK_STATE_REJECTED",
        }
    }

    pub fn parse(s: &str) -> Option<TaskState> {
        [
            TaskState::Submitted,
            TaskState::Working,
            TaskState::InputRequired,
            TaskState::AuthRequired,
            TaskState::Completed,
            TaskState::Failed,
            TaskState::Canceled,
            TaskState::Rejected,
        ]
        .into_iter()
        .find(|t| t.as_str() == s)
    }

    /// Completed, failed, canceled or rejected: nothing more happens.
    pub fn is_terminal(self) -> bool {
        matches!(self, TaskState::Completed | TaskState::Failed | TaskState::Canceled | TaskState::Rejected)
    }

    /// Waiting on the client (input or authentication).
    pub fn is_interrupted(self) -> bool {
        matches!(self, TaskState::InputRequired | TaskState::AuthRequired)
    }
}

/// A task's state with an optional message and when it was set.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskStatus {
    pub state: TaskState,
    pub message: Option<Message>,
    /// ISO 8601 UTC.
    pub timestamp: String,
}

/// An output of a task.
#[derive(Debug, Clone, PartialEq)]
pub struct Artifact {
    pub artifact_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub parts: Vec<Part>,
    pub metadata: Option<String>,
}

impl Artifact {
    pub fn new(name: impl Into<String>, parts: Vec<Part>) -> Artifact {
        Artifact { artifact_id: super::new_id(), name: Some(name.into()), description: None, parts, metadata: None }
    }

    /// An artifact with one text part.
    pub fn text(name: impl Into<String>, text: impl Into<String>) -> Artifact {
        Artifact::new(name, vec![Part::text(text)])
    }
}

/// A unit of work the agent tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: String,
    pub context_id: String,
    pub status: TaskStatus,
    pub artifacts: Vec<Artifact>,
    pub history: Vec<Message>,
    pub metadata: Option<String>,
}

// ── JSON out ─────────────────────────────────────────────────────────────────


/// Builds a JSON object field by field, leaving out absent ones.
pub(crate) struct Obj(String);

impl Obj {
    pub(crate) fn new() -> Obj {
        Obj(String::from("{"))
    }

    fn key(&mut self, key: &str) {
        if self.0.len() > 1 {
            self.0.push(',');
        }
        self.0.push_str(&quote(key));
        self.0.push(':');
    }

    pub(crate) fn str(mut self, key: &str, value: &str) -> Obj {
        self.key(key);
        self.0.push_str(&quote(value));
        self
    }

    pub(crate) fn opt_str(self, key: &str, value: Option<&str>) -> Obj {
        match value {
            Some(v) => self.str(key, v),
            None => self,
        }
    }

    /// Already-serialized JSON.
    pub(crate) fn raw(mut self, key: &str, json: &str) -> Obj {
        self.key(key);
        self.0.push_str(json);
        self
    }

    pub(crate) fn opt_raw(self, key: &str, json: Option<&str>) -> Obj {
        match json {
            Some(j) => self.raw(key, j),
            None => self,
        }
    }

    pub(crate) fn list(self, key: &str, items: &[String]) -> Obj {
        let json = format!("[{}]", items.join(","));
        self.raw(key, &json)
    }

    pub(crate) fn strs(self, key: &str, items: &[String]) -> Obj {
        let quoted: Vec<String> = items.iter().map(|s| quote(s)).collect();
        self.list(key, &quoted)
    }

    pub(crate) fn done(mut self) -> String {
        self.0.push('}');
        self.0
    }
}

impl Part {
    pub(crate) fn to_json(&self) -> String {
        let o = match &self.content {
            PartContent::Text(t) => Obj::new().str("text", t),
            PartContent::Raw(b) => Obj::new().str("raw", &crate::core::base64::Base64::encode(b).unwrap_or_default()),
            PartContent::Url(u) => Obj::new().str("url", u),
            PartContent::Data(d) => Obj::new().raw("data", d),
        };
        o.opt_str("mediaType", self.media_type.as_deref()).opt_str("filename", self.filename.as_deref()).opt_raw("metadata", self.metadata.as_deref()).done()
    }
}

impl Message {
    pub(crate) fn to_json(&self) -> String {
        let parts: Vec<String> = self.parts.iter().map(Part::to_json).collect();
        let mut o = Obj::new()
            .str("messageId", &self.message_id)
            .opt_str("contextId", self.context_id.as_deref())
            .opt_str("taskId", self.task_id.as_deref())
            .str("role", self.role.as_str())
            .list("parts", &parts)
            .opt_raw("metadata", self.metadata.as_deref());
        if !self.extensions.is_empty() {
            o = o.strs("extensions", &self.extensions);
        }
        if !self.reference_task_ids.is_empty() {
            o = o.strs("referenceTaskIds", &self.reference_task_ids);
        }
        o.done()
    }
}

impl TaskStatus {
    pub(crate) fn to_json(&self) -> String {
        let message = self.message.as_ref().map(Message::to_json);
        Obj::new().str("state", self.state.as_str()).opt_raw("message", message.as_deref()).str("timestamp", &self.timestamp).done()
    }
}

impl Artifact {
    pub(crate) fn to_json(&self) -> String {
        let parts: Vec<String> = self.parts.iter().map(Part::to_json).collect();
        Obj::new()
            .str("artifactId", &self.artifact_id)
            .opt_str("name", self.name.as_deref())
            .opt_str("description", self.description.as_deref())
            .list("parts", &parts)
            .opt_raw("metadata", self.metadata.as_deref())
            .done()
    }
}

impl Task {
    /// The task with at most `history_length` most recent messages
    /// (`None`: all of them).
    pub(crate) fn to_json(&self, history_length: Option<usize>) -> String {
        let keep = history_length.unwrap_or(usize::MAX).min(self.history.len());
        let history: Vec<String> = self.history[self.history.len() - keep..].iter().map(Message::to_json).collect();
        let artifacts: Vec<String> = self.artifacts.iter().map(Artifact::to_json).collect();
        let mut o = Obj::new().str("id", &self.id).str("contextId", &self.context_id).raw("status", &self.status.to_json());
        if !artifacts.is_empty() {
            o = o.list("artifacts", &artifacts);
        }
        if !history.is_empty() {
            o = o.list("history", &history);
        }
        o.opt_raw("metadata", self.metadata.as_deref()).done()
    }
}

// The JSON writer is shared with MCP's OpenAPI tools (`json_lite`).
pub(crate) use crate::service_discovery::json_lite::{quote, to_text as to_json};

// ── JSON in ──────────────────────────────────────────────────────────────────

/// A field by its lowerCamelCase name or its proto (snake_case) name:
/// ProtoJSON parsers accept both, and clients send either.
pub(crate) fn field<'a>(value: &'a JsonValue, camel: &str) -> Option<&'a JsonValue> {
    value.get(camel).or_else(|| {
        let mut snake = String::with_capacity(camel.len() + 4);
        for c in camel.chars() {
            if c.is_ascii_uppercase() {
                snake.push('_');
                snake.push(c.to_ascii_lowercase());
            } else {
                snake.push(c);
            }
        }
        (snake != camel).then(|| value.get(&snake)).flatten()
    })
}

fn string_field(value: &JsonValue, key: &str) -> Option<String> {
    field(value, key).and_then(JsonValue::as_str).map(str::to_string)
}

fn object_field(value: &JsonValue, key: &str) -> Option<String> {
    field(value, key).filter(|v| matches!(v, JsonValue::Object(_))).map(to_json)
}

fn strings(value: &JsonValue, key: &str) -> Vec<String> {
    field(value, key).and_then(JsonValue::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

impl Part {
    pub(crate) fn from_json(value: &JsonValue) -> Result<Part, String> {
        let present: Vec<&str> = ["text", "raw", "url", "data"].into_iter().filter(|k| value.get(k).is_some()).collect();
        if present.len() != 1 {
            return Err("a part must contain exactly one of text, raw, url, data".to_string());
        }
        let content = match present[0] {
            "text" => PartContent::Text(string_field(value, "text").ok_or("part text must be a string")?),
            "url" => PartContent::Url(string_field(value, "url").ok_or("part url must be a string")?),
            "raw" => {
                let encoded = string_field(value, "raw").ok_or("part raw must be a base64 string")?;
                PartContent::Raw(crate::core::base64::Base64::decode(encoded).map_err(|_| "part raw isn't valid base64".to_string())?)
            }
            _ => PartContent::Data(to_json(value.get("data").unwrap_or(&JsonValue::Null))),
        };
        Ok(Part { content, media_type: string_field(value, "mediaType"), filename: string_field(value, "filename"), metadata: object_field(value, "metadata") })
    }
}

impl Message {
    pub(crate) fn from_json(value: &JsonValue) -> Result<Message, String> {
        if !matches!(value, JsonValue::Object(_)) {
            return Err("message must be an object".to_string());
        }
        let message_id = string_field(value, "messageId").filter(|s| !s.is_empty()).ok_or("message.messageId is required")?;
        let role = match value.get("role").and_then(JsonValue::as_str) {
            Some("ROLE_USER") | Some("user") => Role::User,
            Some("ROLE_AGENT") | Some("agent") => Role::Agent,
            _ => return Err("message.role must be ROLE_USER or ROLE_AGENT".to_string()),
        };
        let parts_json = value.get("parts").and_then(JsonValue::as_array).ok_or("message.parts is required")?;
        if parts_json.is_empty() {
            return Err("message.parts must not be empty".to_string());
        }
        let parts = parts_json.iter().map(Part::from_json).collect::<Result<Vec<_>, _>>()?;
        Ok(Message {
            message_id,
            context_id: string_field(value, "contextId").filter(|s| !s.is_empty()),
            task_id: string_field(value, "taskId").filter(|s| !s.is_empty()),
            role,
            parts,
            metadata: object_field(value, "metadata"),
            extensions: strings(value, "extensions"),
            reference_task_ids: strings(value, "referenceTaskIds"),
        })
    }
}

/// ISO 8601 UTC with milliseconds (`2026-10-31T10:30:00.000Z`).
pub(crate) fn timestamp() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z", rest / 3600, rest % 3600 / 60, rest % 60, now.subsec_millis())
}
