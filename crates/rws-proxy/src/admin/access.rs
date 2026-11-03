//! Who may do what on the admin API (18.22.0, #75): users, roles and the
//! audit log.
//!
//! - **Users**: the `[admin] token_env` token is user `admin` with role
//!   `admin`; `[admin] users = ["deploy-bot:RWS_ADMIN_DEPLOY:operator"]`
//!   adds named tokens (`name:ENV_VAR:role`), and OIDC sign-in maps groups
//!   to roles.
//! - **Roles**: `viewer` reads everything (every `GET`); `operator` also
//!   reloads, drains, switches maintenance, blocks IPs, flips flags, purges
//!   caches and renews certificates; `admin` also changes authorization
//!   tuples. [`required_role`] is the one table.
//! - **Audit log**: every change (and every refused attempt) as a JSON line
//!   — time, user, role, how they signed in, source address, method, path,
//!   the body with secrets redacted, the status — kept in memory (the last
//!   [`AUDIT_CAPACITY`]) and appended to `[admin] audit_log` when set. MCP
//!   tools that change the server record here too.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use crate::core::json::escape as json_escape;

/// Entries kept in memory.
pub const AUDIT_CAPACITY: usize = 1000;

/// What a user may do; ordered (`Viewer < Operator < Admin`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Viewer,
    Operator,
    Admin,
}

impl Role {
    pub fn parse(text: &str) -> Option<Role> {
        match text.trim().to_ascii_lowercase().as_str() {
            "viewer" => Some(Role::Viewer),
            "operator" => Some(Role::Operator),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Operator => "operator",
            Role::Admin => "admin",
        }
    }
}

/// A user with a bearer token.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenUser {
    pub name: String,
    pub token: String,
    pub role: Role,
}

/// `name:ENV_VAR:role` entries (comma-separated) → users whose token is in
/// `ENV_VAR`. `Err` names the first bad entry or unset variable.
pub fn parse_users(list: &str, env: &dyn Fn(&str) -> Option<String>) -> Result<Vec<TokenUser>, String> {
    let mut users = Vec::new();
    for entry in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let parts: Vec<&str> = entry.split(':').map(str::trim).collect();
        let [name, var, role] = parts.as_slice() else {
            return Err(format!("[admin] users: {entry:?} is not name:ENV_VAR:role"));
        };
        let role = Role::parse(role).ok_or_else(|| format!("[admin] users: {entry:?}: role must be viewer, operator or admin"))?;
        if name.is_empty() || var.is_empty() {
            return Err(format!("[admin] users: {entry:?} is not name:ENV_VAR:role"));
        }
        let token = env(var).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).ok_or_else(|| format!("[admin] users: {name}'s token variable {var} is unset or empty"))?;
        if token.len() < 16 {
            return Err(format!("[admin] users: {name}'s token (in {var}) is shorter than 16 characters"));
        }
        users.push(TokenUser { name: name.to_string(), token, role });
    }
    Ok(users)
}

/// `group=role` entries (comma-separated): a group (or an email address, or
/// `*` for anyone signed in) → role. First match in order wins.
pub fn parse_role_map(list: &str) -> Result<Vec<(String, Role)>, String> {
    list.split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|entry| {
            let (who, role) = entry.rsplit_once('=').ok_or_else(|| format!("[admin] oidc_roles: {entry:?} is not group=role"))?;
            let role = Role::parse(role).ok_or_else(|| format!("[admin] oidc_roles: {entry:?}: role must be viewer, operator or admin"))?;
            Ok((who.trim().to_string(), role))
        })
        .collect()
}

/// The role `map` gives a user with `email` and `groups`: the highest of the
/// entries that match; `None` when none does.
pub fn role_for(map: &[(String, Role)], email: Option<&str>, groups: &[String]) -> Option<Role> {
    map.iter()
        .filter(|(who, _)| who == "*" || groups.iter().any(|g| g == who) || email.is_some_and(|e| who.contains('@') && who.eq_ignore_ascii_case(e)))
        .map(|(_, role)| *role)
        .max()
}

/// The role a request needs: reads are `viewer`, authorization tuples
/// `admin`, any other change `operator`.
pub fn required_role(method: &str, route: &str) -> Role {
    match method {
        "GET" | "HEAD" | "OPTIONS" => Role::Viewer,
        _ if route == "/authz/tuples" || route.starts_with("/authz/tuples/") => Role::Admin,
        // A check reads; it's a POST only to carry its arguments.
        _ if route == "/authz/check" => Role::Viewer,
        _ => Role::Operator,
    }
}

/// Who made a request.
#[derive(Debug, Clone, PartialEq)]
pub struct Principal {
    pub user: String,
    pub role: Role,
    /// `token`, `oidc` or `mcp`.
    pub via: &'static str,
}

// ── Audit log ────────────────────────────────────────────────────────────────

/// One audited change.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditEntry {
    /// Unix seconds.
    pub at: u64,
    pub user: String,
    pub role: String,
    pub via: String,
    pub address: String,
    pub method: String,
    pub path: String,
    /// The request body, secrets redacted, at most 2 KB.
    pub detail: String,
    pub status: u16,
    /// The replica that served it (`[cluster] node`, else the host name).
    pub node: String,
}

impl AuditEntry {
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"at":{},"time":"{}","user":"{}","role":"{}","via":"{}","address":"{}","method":"{}","path":"{}","detail":"{}","status":{},"node":"{}"}}"#,
            self.at,
            crate::http::date::format(self.at),
            json_escape(&self.user),
            json_escape(&self.role),
            json_escape(&self.via),
            json_escape(&self.address),
            json_escape(&self.method),
            json_escape(&self.path),
            json_escape(&self.detail),
            self.status,
            json_escape(&self.node),
        )
    }
}

/// Keys whose values never reach the audit log.
fn secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["token", "secret", "password", "passwd", "key", "credential", "authorization", "cookie"].iter().any(|s| key.contains(s))
}

/// A request body for the log: a JSON object's secret-looking values
/// replaced by `"(redacted)"`, anything longer than 2 KB cut.
pub fn redact(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let text = text.trim();
    let redacted = match crate::mcp::json_rpc::object_members(text) {
        Some(members) if !members.is_empty() => {
            let parts: Vec<String> = members
                .iter()
                .map(|(k, v)| if secret_key(k) { format!("\"{}\":\"(redacted)\"", json_escape(k)) } else { format!("\"{}\":{}", json_escape(k), v.trim()) })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        _ => text.to_string(),
    };
    if redacted.len() > 2048 {
        let mut end = 2048;
        while !redacted.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &redacted[..end])
    } else {
        redacted
    }
}

struct Audit {
    recent: VecDeque<AuditEntry>,
    file: Option<String>,
}

fn audit() -> &'static Mutex<Audit> {
    static AUDIT: OnceLock<Mutex<Audit>> = OnceLock::new();
    AUDIT.get_or_init(|| Mutex::new(Audit { recent: VecDeque::new(), file: None }))
}

/// Append audit entries to `path` too (empty: memory only).
pub fn set_audit_file(path: &str) {
    audit().lock().unwrap_or_else(|e| e.into_inner()).file = Some(path.trim().to_string()).filter(|p| !p.is_empty());
}

/// This replica's name in audit entries and cluster heartbeats.
pub fn node_name() -> String {
    std::env::var("RWS_CONFIG_CLUSTER_NODE")
        .ok()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok().filter(|h| !h.is_empty()))
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()))
        .unwrap_or_else(|| "rws".to_string())
}

/// Record a change.
pub fn record(entry: AuditEntry) {
    let line = entry.to_json();
    let mut audit = audit().lock().unwrap_or_else(|e| e.into_inner());
    if audit.recent.len() >= AUDIT_CAPACITY {
        audit.recent.pop_front();
    }
    audit.recent.push_back(entry);
    if let Some(path) = audit.file.clone() {
        let written = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| writeln!(f, "{line}"));
        if let Err(e) = written {
            eprintln!("[admin] can't append to the audit log {path}: {e}");
        }
    }
}

/// Record a change made by `principal` from `address`.
pub fn record_change(principal: &Principal, address: &str, method: &str, path: &str, body: &[u8], status: u16) {
    record(AuditEntry {
        at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        user: principal.user.clone(),
        role: principal.role.as_str().to_string(),
        via: principal.via.to_string(),
        address: address.to_string(),
        method: method.to_string(),
        path: path.to_string(),
        detail: redact(body),
        status,
        node: node_name(),
    });
}

/// The `n` newest entries, newest first, as a JSON array.
pub fn recent_json(n: usize) -> String {
    let audit = audit().lock().unwrap_or_else(|e| e.into_inner());
    let entries: Vec<String> = audit.recent.iter().rev().take(n).map(AuditEntry::to_json).collect();
    format!("[{}]", entries.join(","))
}

/// Audits `tools/call` requests for the named MCP tools (the ones that
/// change the server) as user `mcp`, with the arguments redacted like an
/// admin request body. Wrap the MCP server with it.
pub struct McpAuditLayer {
    tools: Vec<String>,
}

impl McpAuditLayer {
    pub fn new(tools: impl IntoIterator<Item = impl Into<String>>) -> McpAuditLayer {
        McpAuditLayer { tools: tools.into_iter().map(Into::into).collect() }
    }

    /// The audited tool a JSON-RPC body calls, and its arguments.
    fn call<'a>(&self, body: &'a [u8]) -> Option<(String, String)> {
        let text = std::str::from_utf8(body).ok()?;
        let members = crate::mcp::json_rpc::object_members(text)?;
        let field = |k: &str| members.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        if field("method")?.trim() != "\"tools/call\"" {
            return None;
        }
        let params = crate::mcp::json_rpc::object_members(&field("params")?)?;
        let name = params.iter().find(|(k, _)| k == "name").map(|(_, v)| v.trim().trim_matches('"').to_string())?;
        let arguments = params.iter().find(|(k, _)| k == "arguments").map(|(_, v)| v.clone()).unwrap_or_default();
        self.tools.contains(&name).then_some((name, arguments))
    }
}

impl crate::middleware::Middleware for McpAuditLayer {
    fn handle(&self, request: &crate::request::Request, connection: &crate::server::ConnectionInfo, next: &dyn crate::application::Application) -> Result<crate::response::Response, String> {
        let call = if request.method == "POST" { self.call(&request.body) } else { None };
        let response = next.execute(request, connection)?;
        if let Some((tool, arguments)) = call {
            let principal = Principal { user: "mcp".to_string(), role: Role::Admin, via: "mcp" };
            record_change(&principal, &crate::ip_filter::client_ip(request, &connection.client.ip), "TOOL", &tool, arguments.as_bytes(), response.status_code as u16);
        }
        Ok(response)
    }

    /// Only `POST /mcp…` bodies are looked at; everything else passes
    /// straight through (streamed bodies, the async forwarder, inline runs).
    fn accepts_streamed_body(&self, request: &crate::request::Request, _connection: &crate::server::ConnectionInfo) -> bool {
        !is_mcp_post(request)
    }

    fn is_passthrough(&self, request: &crate::request::Request, _connection: &crate::server::ConnectionInfo) -> bool {
        !is_mcp_post(request)
    }

    fn runs_inline(&self, _request: &crate::request::Request, _connection: &crate::server::ConnectionInfo) -> bool {
        true
    }
}

fn is_mcp_post(request: &crate::request::Request) -> bool {
    request.method == "POST" && crate::url::URL::request_target_path(&request.request_uri).is_some_and(|p| p == "/mcp" || p.starts_with("/mcp/"))
}
