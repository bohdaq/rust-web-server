//! `app.dev_mcp()`: an MCP endpoint that lets a coding agent look inside
//! the application it is working on — its routes and the middleware
//! around them, its effective configuration (secrets hidden), its layout
//! and `rws.app.toml` ([`crate::app_manifest`]), the last requests and
//! errors, and with a `model-*` feature its models, database schema and
//! migrations. Since 18.28.0 (roadmap #85, #87).
//!
//! Off unless the process runs with `RWS_ENV=dev` ([`RwsEnv`]) or a bearer
//! token is configured (`MCP_TOKEN` by default, [`DevTools::token`]): in
//! production without a token [`DevMcpExt::dev_mcp`] mounts nothing and
//! the app is served as it is. With a token every request to the endpoint
//! must carry `Authorization: Bearer <token>`
//! ([`McpServer::require_bearer`]); in development without one the
//! endpoint is open, so keep the port local or set a token.
//!
//! ```rust,no_run
//! # extern crate rws_proxy as rust_web_server;
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::dev_mcp::DevMcpExt;
//! use rust_web_server::response::Response;
//!
//! let app = App::with_state(())
//!     .get("/hello", |_, _, _, _| Response::new())
//!     .dev_mcp(); // RWS_ENV=dev: /mcp lists GET /hello
//! ```
//!
//! [`DevTools`] chooses what is mounted: the operations tools of the `rws`
//! binary too ([`DevTools::with_ops`], also usable alone as [`ops_tools`]),
//! a database, a migrations directory, read-only mode.

use crate::application::Application;
use crate::cache::CacheLayer;
use crate::core::json::escape as json_escape;
use crate::mcp::{extract_arg, McpContent, McpServer, ToolAnnotations};
use crate::middleware::WithMiddleware;
use crate::request::Request;
use crate::response::Response;
use crate::server::ConnectionInfo;

pub use crate::entry_point::RwsEnv;

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
mod db;
mod layout;
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
mod models;
mod ops;
#[cfg(test)]
mod tests;

pub use ops::{ops_tools, OPS_WRITE_TOOLS};

/// The tools that change something, besides [`OPS_WRITE_TOOLS`]: the
/// migration runners (with a `model-*` feature and a migrations
/// directory).
pub const DEV_WRITE_TOOLS: [&str; 2] = ["migrations_run", "migrations_rollback_last"];

/// The environment variable a token is read from unless
/// [`DevTools::token`] or [`DevTools::token_env`] says otherwise — the one
/// the `rws` binary's `/mcp` uses.
pub const DEFAULT_TOKEN_ENV: &str = "MCP_TOKEN";

/// What [`DevMcpExt::dev_mcp_with`] mounts, and when.
///
/// ```rust,no_run
/// # extern crate rws_proxy as rust_web_server;
/// use rust_web_server::app::App;
/// use rust_web_server::dev_mcp::{DevMcpExt, DevTools};
///
/// let app = App::with_state(()).dev_mcp_with(
///     DevTools::new()
///         .token_env("APP_DEV_TOKEN") // instead of MCP_TOKEN
///         .with_ops()                 // the rws binary's 25 tools too
///         .read_only(),               // but nothing that changes state
/// );
/// ```
#[derive(Clone)]
pub struct DevTools {
    name: String,
    version: String,
    path: String,
    env: Option<RwsEnv>,
    token: Option<String>,
    token_env: String,
    writes: bool,
    ops: bool,
    cache: Option<CacheLayer>,
    record_requests: bool,
    allowed_origins: Vec<String>,
    #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
    database: Option<crate::model::DbPool>,
    #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
    migrations: Option<String>,
    #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
    models: Vec<crate::model::ModelInfo>,
    app_dir: Option<std::path::PathBuf>,
}

impl Default for DevTools {
    fn default() -> Self {
        DevTools::new()
    }
}

impl DevTools {
    /// The defaults: server `rws-dev`, endpoint `/mcp`, `RWS_ENV` and
    /// `MCP_TOKEN` from the environment, changing tools included, requests
    /// recorded for `recent_requests`, no operations tools, no database.
    pub fn new() -> Self {
        DevTools {
            name: "rws-dev".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            path: "/mcp".to_string(),
            env: None,
            token: None,
            token_env: DEFAULT_TOKEN_ENV.to_string(),
            writes: true,
            ops: false,
            cache: None,
            record_requests: true,
            allowed_origins: Vec::new(),
            #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
            database: None,
            #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
            migrations: None,
            #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
            models: Vec::new(),
            app_dir: None,
        }
    }

    /// `serverInfo` the endpoint reports (default `rws-dev` and the rws
    /// version).
    pub fn server_info(mut self, name: impl Into<String>, version: impl Into<String>) -> Self {
        self.name = name.into();
        self.version = version.into();
        self
    }

    /// The endpoint's path (default `/mcp`) — for an app that serves its
    /// own MCP server there.
    pub fn at(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Decide as if `RWS_ENV` were `env` instead of reading it.
    pub fn env(mut self, env: RwsEnv) -> Self {
        self.env = Some(env);
        self
    }

    /// Require this bearer token (and mount the endpoint in any
    /// environment). An empty or blank token counts as none.
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Read the token from this environment variable instead of
    /// `MCP_TOKEN` ([`DEFAULT_TOKEN_ENV`]); [`Self::token`] wins over it.
    pub fn token_env(mut self, name: impl Into<String>) -> Self {
        self.token_env = name.into();
        self
    }

    /// Leave out every tool that changes something ([`OPS_WRITE_TOOLS`],
    /// [`DEV_WRITE_TOOLS`]): the agent can look, not act.
    pub fn read_only(mut self) -> Self {
        self.writes = false;
        self
    }

    /// Mount the `rws` binary's operations tools as well ([`ops_tools`]):
    /// metrics, rate limits, CORS, static files, maintenance mode, the IP
    /// blocklist, feature flags, authorization, traffic, config reload.
    pub fn with_ops(mut self) -> Self {
        self.ops = true;
        self
    }

    /// The response cache for the `cache_stats` / `cache_clear` operations
    /// tools (a clone of the layer the app wraps; clones share the store).
    /// Implies [`Self::with_ops`].
    pub fn cache(mut self, cache: CacheLayer) -> Self {
        self.cache = Some(cache);
        self.ops = true;
        self
    }

    /// Whether requests to the app are recorded into the request log that
    /// `recent_requests` / `recent_errors` read (default `true`). Turn it
    /// off when the app already has a [`crate::request_log::LogLayer`], or
    /// each request is listed twice.
    pub fn record_requests(mut self, record: bool) -> Self {
        self.record_requests = record;
        self
    }

    /// Browser origins allowed to call the endpoint
    /// ([`McpServer::allowed_origins`]); desktop and CLI clients send no
    /// `Origin` and need none.
    pub fn allowed_origins<I, S>(mut self, origins: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_origins = origins.into_iter().map(Into::into).collect();
        self
    }

    /// The database `db_schema` and the migration tools use.
    #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
    pub fn database(mut self, pool: crate::model::DbPool) -> Self {
        self.database = Some(pool);
        self
    }

    /// The directory of `*.sql` migrations ([`crate::model::DbPool::migrate`])
    /// for `migrations` (status), `migrations_run` and
    /// `migrations_rollback_last`. Needs [`Self::database`].
    #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
    pub fn migrations(mut self, dir: impl Into<String>) -> Self {
        self.migrations = Some(dir.into());
        self
    }

    /// Report the `#[derive(Model)]` type `T` in `models`: its table,
    /// columns and primary key ([`crate::model::ModelInfo`]), compared with
    /// the database's table when [`Self::database`] is set. Rust can't list
    /// a program's models at run time, so each is named once; registering
    /// one twice lists it once.
    #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
    pub fn model<T: crate::model::Model>(mut self) -> Self {
        let info = crate::model::ModelInfo::of::<T>();
        if !self.models.contains(&info) {
            self.models.push(info);
        }
        self
    }

    /// The project root `layout` reads `rws.app.toml` and the conventional
    /// paths from (default: the working directory when the tools are
    /// installed). `env!("CARGO_MANIFEST_DIR")` is the project's own root
    /// in `cargo run` and `cargo test`, whatever the working directory —
    /// the starters pass it, since they serve static files from another
    /// directory.
    pub fn app_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.app_dir = Some(dir.into());
        self
    }

    /// The token in effect: [`Self::token`], else the token environment
    /// variable; blank is none.
    fn effective_token(&self) -> Option<String> {
        let token = match &self.token {
            Some(token) => token.clone(),
            None => std::env::var(&self.token_env).unwrap_or_default(),
        };
        let token = token.trim();
        (!token.is_empty()).then(|| token.to_string())
    }

    fn effective_env(&self) -> RwsEnv {
        self.env.unwrap_or_else(RwsEnv::current)
    }

    /// Whether [`Self::mount`] mounts the endpoint: `RWS_ENV=dev`, or a
    /// token configured.
    pub fn enabled(&self) -> bool {
        self.effective_env().is_dev() || self.effective_token().is_some()
    }

    /// Register the tools on `mcp`, ungated: `routes`, `config`, `layout`,
    /// `recent_requests`, `recent_errors`, with a `model-*` feature
    /// `models`, with a database `db_schema`,
    /// with migrations `migrations` (and `migrations_run`,
    /// `migrations_rollback_last` unless [`Self::read_only`]), and the
    /// operations tools with [`Self::with_ops`]. The routes are those of
    /// the app `mcp` wraps ([`McpServer::wrap`]). For an app that builds its
    /// own MCP server and gates it itself; [`Self::mount`] is the gated
    /// form.
    pub fn register(&self, mcp: McpServer) -> McpServer {
        let routes = mcp.registered_routes().into_iter().map(|r| (r.method, r.pattern)).collect();
        let middleware = mcp.middleware_names();
        self.install(mcp, routes, middleware)
    }

    fn install(&self, mcp: McpServer, routes: Vec<(String, String)>, middleware: Vec<&'static str>) -> McpServer {
        let mut mcp = routes_tool(mcp, routes.clone(), middleware);
        let env = self.effective_env();
        mcp = mcp.tool(
            "config",
            "The app's effective configuration: RWS_ENV, compiled-in features and every RWS_*, OTEL_* and MCP_* environment setting (config file and flags included), secrets hidden",
            r#"{"type":"object"}"#,
            move |_| Ok(McpContent::json(config_json(env))),
        );
        let dir = self.app_dir.clone().or_else(|| std::env::current_dir().ok()).unwrap_or_default();
        mcp = layout::install(mcp, dir);
        #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
        {
            mcp = models::install(mcp, self.models.clone(), self.database.clone());
        }
        mcp = if self.ops {
            ops::install(mcp, routes, self.cache.clone(), self.writes)
        } else {
            recent_tools(mcp)
        };
        #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
        if let Some(pool) = &self.database {
            mcp = db::install(mcp, pool.clone(), self.migrations.clone(), self.writes);
        }
        mcp.tool_annotations("routes", read_only_hint()).tool_annotations("config", read_only_hint())
    }

    /// `app` with the endpoint in front of it when [`Self::enabled`];
    /// otherwise `app` alone, and a one-line notice on stdout.
    pub fn mount<A: Application + Send + Sync + 'static>(self, app: A) -> DevMcp<A> {
        let env = self.effective_env();
        let token = self.effective_token();
        if !env.is_dev() && token.is_none() {
            println!(
                "dev MCP not mounted: RWS_ENV is {env} and {} is not set (set RWS_ENV=dev, or a token, to enable {}).",
                self.token_env, self.path
            );
            return DevMcp::Off(app);
        }
        let routes = app.registered_routes().into_iter().map(|r| (r.method, r.pattern)).collect();
        let middleware = app.middleware_names();
        let mut mcp = McpServer::new(self.name.clone(), self.version.clone())
            .at(self.path.clone())
            .allowed_origins(self.allowed_origins.clone());
        match &token {
            Some(token) => mcp = mcp.require_bearer(token.clone()),
            None => println!("dev MCP at {} without a token (RWS_ENV=dev): keep the port local, or set {}.", self.path, self.token_env),
        }
        mcp = if self.record_requests {
            mcp.wrap(WithMiddleware::new(app).wrap(crate::request_log::LogLayer))
        } else {
            mcp.wrap(app)
        };
        let mcp = self.install(mcp, routes, middleware);
        // Changes made through the tools land in the admin audit log.
        let audited = crate::admin::access::McpAuditLayer::new(OPS_WRITE_TOOLS.iter().chain(DEV_WRITE_TOOLS.iter()).copied());
        DevMcp::On(WithMiddleware::new(mcp).wrap(audited))
    }
}

/// An app with the dev MCP endpoint in front of it ([`DevMcp::On`]), or
/// the app alone ([`DevMcp::Off`]) — what [`DevMcpExt::dev_mcp`] returns.
/// An [`Application`] either way.
#[derive(Clone)]
pub enum DevMcp<A> {
    /// Mounted: requests to the endpoint go to the tools, everything else
    /// to the app.
    On(WithMiddleware<McpServer>),
    /// Not mounted (production without a token): the app as it was.
    Off(A),
}

impl<A> DevMcp<A> {
    /// `true` when the endpoint is mounted.
    pub fn is_mounted(&self) -> bool {
        matches!(self, DevMcp::On(_))
    }
}

impl<A: Application> Application for DevMcp<A> {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        match self {
            DevMcp::On(m) => m.execute(request, connection),
            DevMcp::Off(a) => a.execute(request, connection),
        }
    }

    fn runs_inline(&self, request: &Request, connection: &ConnectionInfo) -> bool {
        match self {
            DevMcp::On(m) => m.runs_inline(request, connection),
            DevMcp::Off(a) => a.runs_inline(request, connection),
        }
    }

    #[cfg(feature = "http2")]
    fn execute_async<'a>(&'a self, request: &'a Request, connection: &'a ConnectionInfo) -> Option<crate::application::AsyncResponse<'a>> {
        match self {
            DevMcp::On(m) => m.execute_async(request, connection),
            DevMcp::Off(a) => a.execute_async(request, connection),
        }
    }

    fn streams_body(&self, request: &Request, connection: &ConnectionInfo) -> bool {
        match self {
            DevMcp::On(m) => m.streams_body(request, connection),
            DevMcp::Off(a) => a.streams_body(request, connection),
        }
    }

    fn registered_routes(&self) -> Vec<crate::router::RouteInfo> {
        match self {
            DevMcp::On(m) => m.registered_routes(),
            DevMcp::Off(a) => a.registered_routes(),
        }
    }

    fn middleware_names(&self) -> Vec<&'static str> {
        match self {
            DevMcp::On(m) => m.middleware_names(),
            DevMcp::Off(a) => a.middleware_names(),
        }
    }
}

/// `.dev_mcp()` on every [`Application`] — `AppWithState`, `App`,
/// `WithMiddleware`, `AsyncAppWithState` and the rest.
pub trait DevMcpExt: Application + Sized + Send + Sync + 'static {
    /// The dev MCP endpoint with the default [`DevTools`], in front of
    /// this app — or this app alone outside development without a token.
    fn dev_mcp(self) -> DevMcp<Self> {
        DevTools::new().mount(self)
    }

    /// The same with the tools and gating `tools` describes.
    fn dev_mcp_with(self, tools: DevTools) -> DevMcp<Self> {
        tools.mount(self)
    }
}

impl<A: Application + Send + Sync + 'static> DevMcpExt for A {}

// ── tools shared by the dev and operations sets ──────────────────────────────

pub(crate) fn read_only_hint() -> ToolAnnotations {
    ToolAnnotations { read_only_hint: Some(true), open_world_hint: Some(false), ..ToolAnnotations::default() }
}

fn quoted(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

/// `recent_requests` and `recent_errors` (both sets have them; registered
/// once).
fn recent_tools(mcp: McpServer) -> McpServer {
    mcp.tool(
        "recent_requests",
        "Last N requests recorded by the server (default 20, max 100)",
        r#"{"type":"object","properties":{"n":{"type":"integer","description":"Number of entries to return (default 20)"}}}"#,
        |args| {
            let n = extract_arg(args, "n").and_then(|v| v.parse::<usize>().ok()).unwrap_or(20);
            Ok(McpContent::json(crate::admin::ops::recent_json(n, false)))
        },
    )
    .tool(
        "recent_errors",
        "Last N requests with a 4xx or 5xx status code (default 20, max 100)",
        r#"{"type":"object","properties":{"n":{"type":"integer","description":"Number of error entries to return (default 20)"}}}"#,
        |args| {
            let n = extract_arg(args, "n").and_then(|v| v.parse::<usize>().ok()).unwrap_or(20);
            Ok(McpContent::json(crate::admin::ops::recent_json(n, true)))
        },
    )
    .tool_annotations("recent_requests", read_only_hint())
    .tool_annotations("recent_errors", read_only_hint())
}

/// `routes`: the app's routes and the middleware around them.
fn routes_tool(mcp: McpServer, routes: Vec<(String, String)>, middleware: Vec<&'static str>) -> McpServer {
    let json = routes_json(&routes, &middleware);
    mcp.tool(
        "routes",
        "The app's routes (method and pattern, in registration order) and the middleware layers around them, in the order a request passes them; the layers apply to every route listed",
        r#"{"type":"object"}"#,
        move |_| Ok(McpContent::json(json.clone())),
    )
}

pub(crate) fn routes_json(routes: &[(String, String)], middleware: &[&str]) -> String {
    let routes: Vec<String> =
        routes.iter().map(|(m, p)| format!(r#"{{"method":{},"pattern":{}}}"#, quoted(m), quoted(p))).collect();
    let middleware: Vec<String> = middleware.iter().map(|m| quoted(m)).collect();
    format!(r#"{{"routes":[{}],"middleware":[{}]}}"#, routes.join(","), middleware.join(","))
}

/// The environment variables `config` shows.
fn shown_setting(name: &str) -> bool {
    ["RWS_", "OTEL_", "MCP_"].iter().any(|prefix| name.starts_with(prefix)) || name == "DATABASE_URL"
}

/// `value` as `config` shows it: [`crate::entry_point::shown`] (secrets by
/// name, PKCS#11 PINs), and a password in a URL's user info
/// (`postgres://app:pw@db/app`) hidden too.
pub fn masked(name: &str, value: &str) -> String {
    let shown = crate::entry_point::shown(name, value);
    hide_url_password(&shown)
}

fn hide_url_password(value: &str) -> String {
    let Some(scheme_end) = value.find("://") else { return value.to_string() };
    let authority_start = scheme_end + 3;
    let rest = &value[authority_start..];
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let Some(at) = authority.rfind('@') else { return value.to_string() };
    let Some(colon) = authority[..at].find(':') else { return value.to_string() };
    format!("{}(hidden){}", &value[..authority_start + colon + 1], &value[authority_start + at..])
}

/// What `config` answers: `{"rws_env":…,"features":{…},"settings":{…}}`.
pub(crate) fn config_json(env: RwsEnv) -> String {
    let mut pairs: Vec<(String, String)> = std::env::vars().filter(|(k, _)| shown_setting(k)).collect();
    pairs.sort();
    let settings: Vec<String> = pairs.iter().map(|(k, v)| format!("{}:{}", quoted(k), quoted(&masked(k, v)))).collect();
    format!(
        r#"{{"rws_env":{},"features":{},"settings":{{{}}}}}"#,
        quoted(env.as_str()),
        crate::admin::ops::build_features_json(),
        settings.join(",")
    )
}
