//! `ROLE=app`: one route per library feature the release image lacks.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rust_web_server::a2a::{A2aReply, A2aServer, AgentCard, Message, Skill};
use rust_web_server::app::App;
use rust_web_server::application::Application;
use rust_web_server::csrf::{CsrfLayer, CsrfToken};
use rust_web_server::header::Header;
use rust_web_server::mailer::{Email, Mailer, SmtpTls};
use rust_web_server::mcp::{McpContent, McpServer, PromptMessage};
use rust_web_server::middleware::Middleware;
use rust_web_server::openapi::{OpenApiConfig, Operation};
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::rsa_key::RsaPublicKey;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::session::{session_cookie, session_id_from_request, RedisSessionStore, SessionStore};
use rust_web_server::sse::{Sse, SseEvent};
use rust_web_server::state::AppWithState;
use rust_web_server::storage::Storage;
use rust_web_server::template::{Context, TeraEngine};
use rust_web_server::JsonSchema;

use crate::common::{bytes, empty, env, header, html, json, path, query, text, unavailable, Root, S};
use crate::db::{self, Db};
use crate::llm;
use crate::s3::S3;

pub const SESSION_COOKIE: &str = "rws_e2e_sid";
const SESSION_TTL: u64 = 3600;
pub const MAIL_FROM: &str = "rws-e2e@e2e.test";
const HELLO_TEMPLATE: &str = include_str!("../templates/hello.html");

pub enum Sessions {
    Memory(SessionStore),
    Redis(RedisSessionStore),
}

impl Sessions {
    fn kind(&self) -> &'static str {
        match self {
            Sessions::Memory(_) => "memory",
            Sessions::Redis(_) => "redis",
        }
    }

    /// A new session holding `user`; its id.
    fn login(&self, user: &str) -> Result<String, String> {
        match self {
            Sessions::Memory(store) => {
                let mut s = store.create();
                s.set("user", user);
                store.save(&s);
                Ok(s.id)
            }
            Sessions::Redis(store) => {
                let mut s = store.create().map_err(|e| e.to_string())?;
                s.set("user", user);
                store.save(&s).map_err(|e| e.to_string())?;
                Ok(s.id)
            }
        }
    }

    fn user(&self, id: &str) -> Result<Option<String>, String> {
        let session = match self {
            Sessions::Memory(store) => store.load(id),
            Sessions::Redis(store) => store.load(id).map_err(|e| e.to_string())?,
        };
        Ok(session.and_then(|s| s.get("user").map(str::to_string)))
    }
}

/// Everything `ROLE=app` reads from the environment, read once at start.
pub struct AppConfig {
    pub sessions: Sessions,
    pub jwt_rs256: Result<RsaPublicKey, String>,
    pub jwt_es256: Result<p256::ecdsa::VerifyingKey, String>,
    pub webhook_secret: Option<String>,
    pub smtp: Option<(String, u16)>,
    pub s3: Result<S3, String>,
    pub mcp_token: Option<String>,
    pub a2a_url: Option<String>,
    pub pod: String,
    pub db: Db,
}

/// A PEM from an env var; `\n` escapes are accepted for one-line values.
fn pem(name: &str) -> Result<String, String> {
    env(name).map(|v| v.replace("\\n", "\n")).ok_or_else(|| format!("{name} unset"))
}

pub fn rs256_key(pem_text: &str) -> Result<RsaPublicKey, String> {
    RsaPublicKey::from_public_key_pem(pem_text).map_err(|e| format!("invalid RSA public key: {e}"))
}

pub fn es256_key(pem_text: &str) -> Result<p256::ecdsa::VerifyingKey, String> {
    use p256::pkcs8::DecodePublicKey;
    p256::ecdsa::VerifyingKey::from_public_key_pem(pem_text).map_err(|e| format!("invalid P-256 public key: {e}"))
}

impl AppConfig {
    pub fn from_env(migrations_dir: String) -> AppConfig {
        let sessions = match env("SESSION_REDIS") {
            Some(addr) => Sessions::Redis(RedisSessionStore::new(addr, env("SESSION_REDIS_PASSWORD"), SESSION_TTL)),
            None => Sessions::Memory(SessionStore::new(SESSION_TTL)),
        };
        let pod = env("HOSTNAME").unwrap_or_else(|| "unknown-pod".to_string());
        let config = AppConfig {
            sessions,
            jwt_rs256: pem("JWT_RS256_PUBLIC_KEY").and_then(|p| rs256_key(&p)),
            jwt_es256: pem("JWT_ES256_PUBLIC_KEY").and_then(|p| es256_key(&p)),
            webhook_secret: env("WEBHOOK_SECRET"),
            smtp: env("SMTP_HOST").map(|h| (h, env("SMTP_PORT").and_then(|p| p.parse().ok()).unwrap_or(25))),
            s3: S3::from_env().map_err(|missing| format!("{missing} unset")),
            mcp_token: env("MCP_TOKEN"),
            a2a_url: env("A2A_PUBLIC_URL"),
            db: Db::start(migrations_dir, pod.clone()),
            pod,
        };
        for (what, err) in [
            ("JWT RS256", config.jwt_rs256.as_ref().err()),
            ("JWT ES256", config.jwt_es256.as_ref().err()),
            ("S3", config.s3.as_ref().err()),
        ] {
            if let Some(e) = err {
                eprintln!("[e2e] {what}: {e}");
            }
        }
        config
    }

    /// No external service, nothing from the environment.
    pub fn for_tests() -> AppConfig {
        AppConfig {
            sessions: Sessions::Memory(SessionStore::new(SESSION_TTL)),
            jwt_rs256: Err("JWT_RS256_PUBLIC_KEY unset".into()),
            jwt_es256: Err("JWT_ES256_PUBLIC_KEY unset".into()),
            webhook_secret: None,
            smtp: None,
            s3: Err("S3_ENDPOINT unset".into()),
            mcp_token: None,
            a2a_url: None,
            pod: "test-pod".into(),
            db: Db::missing("RWS_DB_NAME"),
        }
    }
}

pub struct State {
    config: AppConfig,
    tera: TeraEngine,
}

// ── OpenAPI types ────────────────────────────────────────────────────────────

#[derive(serde::Deserialize, JsonSchema)]
#[allow(dead_code)]
struct NewTodo {
    title: String,
}

#[derive(serde::Serialize, JsonSchema)]
#[allow(dead_code)]
struct TodoOut {
    id: i64,
    title: String,
}

fn openapi() -> OpenApiConfig {
    OpenApiConfig::new("rws-e2e-app", env!("CARGO_PKG_VERSION"))
        .description("Test app for the rws stage end-to-end suite")
        .operation("POST", "/app/todos", Operation::new("Create a todo").tag("todos").body::<NewTodo>().response::<TodoOut>(201, "Created"))
        .operation("GET", "/app/todos/:id", Operation::new("Get a todo").tag("todos").path_param::<i64>("id").response::<TodoOut>(200, "The todo").status(404, "No such todo"))
        .operation("DELETE", "/app/todos/:id", Operation::new("Delete a todo").tag("todos").path_param::<i64>("id").status(204, "Deleted"))
}

// ── CSRF, only under /app/csrf/ ──────────────────────────────────────────────

/// `CsrfLayer` checks every mutating request; scope it to the CSRF routes
/// so the other POST routes (webhooks, MCP, the LLM mock…) aren't refused.
pub struct ScopedCsrf(CsrfLayer);

impl Middleware for ScopedCsrf {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        if path(request).starts_with("/app/csrf/") {
            self.0.handle(request, connection, next)
        } else {
            next.execute(request, connection)
        }
    }
}

// ── routes ───────────────────────────────────────────────────────────────────

fn bearer_sub(request: &Request, verify: impl Fn(&str) -> Option<rust_web_server::auth::Claims>) -> Response {
    match rust_web_server::auth::extract_bearer_token(request).and_then(|t| verify(&t)).and_then(|c| c.sub) {
        Some(sub) => text(S.n200_ok, &sub),
        None => text(S.n401_unauthorized, "invalid token"),
    }
}

fn db_error(e: String) -> Response {
    text(S.n500_internal_server_error, &format!("database error: {e}"))
}

fn routes(config: AppConfig) -> AppWithState<State> {
    let tera = TeraEngine::from_raw(&[("hello.html", HELLO_TEMPLATE)]).expect("template");
    App::with_state(State { config, tera })
        .get("/healthz", |_, _, _, _| text(S.n200_ok, "ok"))
        // Which replica answered (HOSTNAME).
        .get("/app/pod", |_, _, _, state| text(S.n200_ok, &state.config.pod))
        // 1. sessions
        .post("/app/session/login", |request, _, _, state| {
            let q = query(request);
            let Some(user) = q.get("user").filter(|u| !u.is_empty()) else {
                return text(S.n400_bad_request, "missing ?user=");
            };
            match state.config.sessions.login(user) {
                Ok(id) => {
                    let mut r = text(S.n200_ok, user);
                    r.headers.push(Header::new("Set-Cookie", session_cookie(&id, SESSION_COOKIE, SESSION_TTL)));
                    r
                }
                Err(e) => text(S.n503_service_unavailable, &format!("session store: {e}")),
            }
        })
        .get("/app/session/me", |request, _, _, state| {
            let Some(id) = session_id_from_request(request, SESSION_COOKIE) else {
                return text(S.n401_unauthorized, "no session");
            };
            match state.config.sessions.user(&id) {
                Ok(Some(user)) => text(S.n200_ok, &user),
                Ok(None) => text(S.n401_unauthorized, "no session"),
                Err(e) => text(S.n503_service_unavailable, &format!("session store: {e}")),
            }
        })
        .get("/app/session/store", |_, _, _, state| text(S.n200_ok, state.config.sessions.kind()))
        // 2. CSRF (CsrfLayer, scoped by ScopedCsrf)
        .get("/app/csrf/form", |request, _, _, _| {
            let token = CsrfToken::from_request(request).map(|t| t.value().to_string()).unwrap_or_default();
            html(S.n200_ok, &format!(
                "<!doctype html>\n<form method=\"POST\" action=\"/app/csrf/submit\">\n<input type=\"hidden\" name=\"_csrf\" value=\"{token}\">\n<button type=\"submit\">Submit</button>\n</form>\n"
            ))
        })
        .post("/app/csrf/submit", |_, _, _, _| text(S.n200_ok, "csrf-ok"))
        // 3. asymmetric JWT
        .get("/app/jwt/rs256", |request, _, _, state| match &state.config.jwt_rs256 {
            Ok(key) => bearer_sub(request, |t| rust_web_server::auth::verify_jwt_rs256(t, key)),
            Err(e) => unavailable(e),
        })
        .get("/app/jwt/es256", |request, _, _, state| match &state.config.jwt_es256 {
            Ok(key) => bearer_sub(request, |t| rust_web_server::auth::verify_jwt_es256(t, key)),
            Err(e) => unavailable(e),
        })
        // 4. incoming webhook signatures
        .post("/app/webhook/github", |request, _, _, state| {
            let Some(secret) = &state.config.webhook_secret else { return unavailable("WEBHOOK_SECRET unset") };
            let signature = header(request, "X-Hub-Signature-256").unwrap_or("");
            if rust_web_server::webhook::verify_github_signature(&request.body, secret.as_bytes(), signature) {
                text(S.n200_ok, "webhook-ok")
            } else {
                text(S.n401_unauthorized, "bad signature")
            }
        })
        // 5. SSE
        .get("/app/sse", |request, _, _, _| {
            let q = query(request);
            let count: u64 = q.get("count").and_then(|v| v.parse().ok()).unwrap_or(3).min(10_000);
            let every_ms: u64 = q.get("every_ms").and_then(|v| v.parse().ok()).unwrap_or(100).min(60_000);
            let (events, response) = Sse::stream();
            std::thread::spawn(move || {
                for n in 1..=count {
                    if events.send(sse_event(n)).is_err() {
                        return;
                    }
                    if n < count {
                        std::thread::sleep(Duration::from_millis(every_ms));
                    }
                }
            });
            response
        })
        // 6. templates
        .get("/app/hello", |request, _, _, state| {
            let name = query(request).get("name").cloned().unwrap_or_else(|| "world".to_string());
            let mut ctx = Context::new();
            ctx.insert("name", &name);
            match state.tera.render("hello.html", &ctx) {
                Ok(body) => html(S.n200_ok, &body),
                Err(e) => text(S.n500_internal_server_error, &e),
            }
        })
        // 7. ORM on Postgres
        .post("/app/todos", |request, _, _, state| {
            let pool = match state.config.db.pool() {
                Ok(p) => p,
                Err(e) => return unavailable(&e),
            };
            let input: NewTodo = match serde_json::from_slice(&request.body) {
                Ok(v) => v,
                Err(e) => return text(S.n400_bad_request, &format!("expected {{\"title\":\"…\"}}: {e}")),
            };
            match db::create_todo(pool, input.title) {
                Ok(t) => json(S.n201_created, &serde_json::json!({ "id": t.id, "title": t.title })),
                Err(e) => db_error(e),
            }
        })
        .get("/app/todos/:id", |_, params, _, state| {
            let pool = match state.config.db.pool() {
                Ok(p) => p,
                Err(e) => return unavailable(&e),
            };
            let Some(id) = params.get("id").and_then(|v| v.parse::<i64>().ok()) else {
                return text(S.n404_not_found, "no such todo");
            };
            match db::find_todo(pool, id) {
                Ok(Some(t)) => json(S.n200_ok, &serde_json::json!({ "id": t.id, "title": t.title })),
                Ok(None) => text(S.n404_not_found, "no such todo"),
                Err(e) => db_error(e),
            }
        })
        .delete("/app/todos/:id", |_, params, _, state| {
            let pool = match state.config.db.pool() {
                Ok(p) => p,
                Err(e) => return unavailable(&e),
            };
            let Some(id) = params.get("id").and_then(|v| v.parse::<i64>().ok()) else {
                return text(S.n404_not_found, "no such todo");
            };
            match db::delete_todo(pool, id) {
                Ok(()) => empty(S.n204_no_content),
                Err(e) => db_error(e),
            }
        })
        // 8. jobs
        .post("/app/jobs", |request, _, _, state| {
            let pool = match state.config.db.pool() {
                Ok(p) => p,
                Err(e) => return unavailable(&e),
            };
            let q = query(request);
            let Some(key) = q.get("key").filter(|k| !k.is_empty()).cloned() else {
                return text(S.n400_bad_request, "missing ?key=");
            };
            let fail_first = q.get("fail_first").map(|v| v == "1" || v == "true").unwrap_or(false);
            match db::enqueue(pool, key.clone(), fail_first) {
                Ok(id) => json(S.n202_accepted, &serde_json::json!({ "id": id, "key": key, "fail_first": fail_first })),
                Err(e) => db_error(e),
            }
        })
        .get("/app/jobs/:key", |_, params, _, state| {
            let pool = match state.config.db.pool() {
                Ok(p) => p,
                Err(e) => return unavailable(&e),
            };
            match db::runs(pool, params.get("key").unwrap_or("").to_string()) {
                Ok(v) => json(S.n200_ok, &v),
                Err(e) => db_error(e),
            }
        })
        // 9. mailer
        .post("/app/mail", |request, _, _, state| {
            let Some((host, port)) = &state.config.smtp else { return unavailable("SMTP_HOST unset") };
            let q = query(request);
            let (Some(to), Some(subject)) = (q.get("to"), q.get("subject")) else {
                return text(S.n400_bad_request, "missing ?to= or ?subject=");
            };
            let email = match Email::builder().to(to).subject(subject).text(&format!("rws-e2e-app test mail: {subject}\n")).build() {
                Ok(e) => e,
                Err(e) => return text(S.n400_bad_request, &format!("{e:?}")),
            };
            let mailer = Mailer {
                host: host.clone(),
                port: *port,
                user: None,
                password: None,
                from: MAIL_FROM.to_string(),
                tls: SmtpTls::None,
                timeout_ms: 10_000,
            };
            match mailer.send(&email) {
                Ok(()) => text(S.n200_ok, "sent"),
                Err(e) => text(S.n502_bad_gateway, &format!("SMTP: {e:?}")),
            }
        })
        // 10. S3
        .put("/app/files/:name", |request, params, _, state| {
            let s3 = match &state.config.s3 {
                Ok(s) => s,
                Err(e) => return unavailable(e),
            };
            let name = params.get("name").unwrap_or("");
            let content_type = header(request, "Content-Type").unwrap_or("application/octet-stream");
            match s3.storage.put(name, &request.body, content_type) {
                Ok(_) => text(S.n201_created, name),
                Err(e) => text(S.n502_bad_gateway, &e.to_string()),
            }
        })
        .get("/app/files/:name", |_, params, _, state| {
            let s3 = match &state.config.s3 {
                Ok(s) => s,
                Err(e) => return unavailable(e),
            };
            match s3.storage.get(params.get("name").unwrap_or("")) {
                Ok(data) => bytes(S.n200_ok, data, "application/octet-stream"),
                Err(e) if e.to_string().contains("HTTP 404") => text(S.n404_not_found, &e.to_string()),
                Err(e) => text(S.n502_bad_gateway, &e.to_string()),
            }
        })
        .get("/app/files/:name/presign", |_, params, _, state| {
            let s3 = match &state.config.s3 {
                Ok(s) => s,
                Err(e) => return unavailable(e),
            };
            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            text(S.n200_ok, &s3.presign_get(params.get("name").unwrap_or(""), 300, now))
        })
        .delete("/app/files/:name", |_, params, _, state| {
            let s3 = match &state.config.s3 {
                Ok(s) => s,
                Err(e) => return unavailable(e),
            };
            match s3.storage.delete(params.get("name").unwrap_or("")) {
                Ok(()) => empty(S.n204_no_content),
                Err(e) => text(S.n502_bad_gateway, &e.to_string()),
            }
        })
        // 14. mock OpenAI-compatible LLM
        .post("/v1/chat/completions", |request, _, _, _| llm::chat_completions(request))
        .get("/v1/models", |_, _, _, _| llm::models())
        // 11. OpenAPI (after the routes it describes)
        .openapi(openapi())
}

pub fn sse_event(n: u64) -> SseEvent {
    SseEvent::data(format!("{{\"n\":{n}}}")).id(n.to_string())
}

fn mcp(token: Option<String>) -> McpServer {
    let server = McpServer::new("rws-e2e-app", env!("CARGO_PKG_VERSION"))
        .tool(
            "add",
            "Add two integers",
            r#"{"type":"object","properties":{"a":{"type":"integer"},"b":{"type":"integer"}},"required":["a","b"]}"#,
            |args| {
                let v: serde_json::Value = serde_json::from_str(args).map_err(|e| e.to_string())?;
                let a = v.get("a").and_then(|x| x.as_i64()).ok_or("a must be an integer")?;
                let b = v.get("b").and_then(|x| x.as_i64()).ok_or("b must be an integer")?;
                let sum = a.checked_add(b).ok_or("overflow")?;
                Ok(McpContent::text(sum.to_string()))
            },
        )
        .resource("e2e://hello", "hello", "A fixed greeting", |_| Ok(McpContent::text("hello from rws-e2e-app")))
        .prompt("greet", "Greet someone (argument: name)", |args| {
            let v: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
            let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("world").to_string();
            Ok(vec![PromptMessage::user(format!("Say hello to {name}."))])
        });
    match token {
        Some(t) => server.require_bearer(t),
        None => server,
    }
}

fn a2a(url: Option<String>) -> A2aServer {
    let mut card = AgentCard::new("rws-e2e echo agent", "Echoes the user's text back", env!("CARGO_PKG_VERSION"))
        .skill(Skill::new("echo", "Echo", "Replies with the text it was sent").tag("e2e"));
    if let Some(u) = url {
        card = card.url(&u);
    }
    A2aServer::new(card).on_message(|_ctx, message| A2aReply::Message(Message::agent_text(message.text())))
}

pub fn build(config: AppConfig) -> Root {
    let mcp_token = config.mcp_token.clone();
    let a2a_url = config.a2a_url.clone();
    let has_token = mcp_token.is_some();
    let app = routes(config).wrap(ScopedCsrf(CsrfLayer::new()));
    let stack = a2a(a2a_url).wrap(mcp(mcp_token).wrap(app));
    Root::new(stack).guard(move |request| {
        let p = path(request);
        (!has_token && (p == "/mcp" || p.starts_with("/mcp/"))).then(|| unavailable("MCP_TOKEN unset"))
    })
}

#[cfg(test)]
mod tests;
