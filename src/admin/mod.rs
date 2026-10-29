//! The admin API (18.10.0, #29): change and inspect a running server on a
//! listener of its own, never on the public routes.
//!
//! ```toml
//! [admin]
//! listen          = "127.0.0.1:9901"     # off when unset
//! token_env       = "RWS_ADMIN_TOKEN"    # env var holding the bearer token (this is the default)
//! allowed_origins = ["https://ops.example.com"]   # browsers on other origins (the UI's own is always allowed)
//! ```
//!
//! Every request needs `Authorization: Bearer <token>`, on loopback too;
//! without a token the listener doesn't start (and `rws --check` says so).
//! A browser request from an origin other than the listener's own or one in
//! `allowed_origins` is refused, so a page elsewhere can't drive it with a
//! token it learned.
//!
//! The API lives under `/admin/api/v1/`; [`AdminApi`] is an
//! [`Application`], so tests and embedders can drive it without a socket.
//! Requests are served one thread per connection (at most
//! [`MAX_CONNECTIONS`] at once), apart from the server's own workers, so a
//! saturated server can still be administered.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use crate::application::Application;
use crate::entry_point::Config;
use crate::header::Header;
use crate::mcp::json_escape;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

/// The token's environment variable when `token_env` is unset.
pub const DEFAULT_TOKEN_ENV: &str = "RWS_ADMIN_TOKEN";
/// Admin connections served at once; more are closed at accept.
pub const MAX_CONNECTIONS: usize = 32;
/// Path prefix of the API.
pub const API_PREFIX: &str = "/admin/api/v1";

/// `[admin]` from the environment (`RWS_CONFIG_ADMIN_*`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AdminConfig {
    /// `host:port` to listen on; empty = no admin listener.
    pub listen: String,
    /// The environment variable holding the bearer token.
    pub token_env: String,
    /// Browser origins allowed besides the listener's own.
    pub allowed_origins: Vec<String>,
}

impl AdminConfig {
    pub fn from_env() -> AdminConfig {
        let read = |name: &str| std::env::var(name).unwrap_or_default().trim().to_string();
        let token_env = read(Config::RWS_CONFIG_ADMIN_TOKEN_ENV);
        AdminConfig {
            listen: read(Config::RWS_CONFIG_ADMIN_LISTEN),
            token_env: if token_env.is_empty() { DEFAULT_TOKEN_ENV.to_string() } else { token_env },
            allowed_origins: read(Config::RWS_CONFIG_ADMIN_ALLOWED_ORIGINS)
                .split(',')
                .map(|o| o.trim().trim_end_matches('/').to_string())
                .filter(|o| !o.is_empty())
                .collect(),
        }
    }

    pub fn enabled(&self) -> bool {
        !self.listen.is_empty()
    }

    /// The token, or why there is none.
    pub fn token(&self) -> Result<String, String> {
        match std::env::var(&self.token_env) {
            Ok(t) if !t.trim().is_empty() => Ok(t.trim().to_string()),
            _ => Err(format!("the admin token variable {} is unset or empty", self.token_env)),
        }
    }
}

/// When the process started serving the admin API (for `uptime`).
fn started() -> Instant {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    *STARTED.get_or_init(Instant::now)
}

/// The admin API. See the [module docs](self).
#[derive(Clone)]
pub struct AdminApi {
    inner: Arc<Inner>,
}

struct Inner {
    token: String,
    allowed_origins: Vec<String>,
}

impl AdminApi {
    /// The API accepting `token`.
    pub fn new(token: impl Into<String>) -> AdminApi {
        started();
        AdminApi { inner: Arc::new(Inner { token: token.into(), allowed_origins: Vec::new() }) }
    }

    /// Browser origins allowed besides the listener's own.
    pub fn allowed_origins(self, origins: Vec<String>) -> AdminApi {
        let token = self.inner.token.clone();
        AdminApi { inner: Arc::new(Inner { token, allowed_origins: origins }) }
    }

    fn authorized(&self, request: &Request) -> bool {
        let Some(sent) = request.header("Authorization").and_then(|h| h.value.strip_prefix("Bearer ").map(str::trim)) else {
            return false;
        };
        crate::proxy_config::constant_time_eq(sent.as_bytes(), self.inner.token.as_bytes())
    }

    /// A browser on another origin is refused; non-browser clients send none.
    fn origin_allowed(&self, request: &Request) -> bool {
        let Some(origin) = request.header("Origin").map(|h| h.value.trim().trim_end_matches('/').to_string()) else {
            return true;
        };
        let host = request.header("Host").map(|h| h.value.trim().to_string()).unwrap_or_default();
        let own = [format!("http://{host}"), format!("https://{host}")];
        own.contains(&origin) || self.inner.allowed_origins.contains(&origin)
    }
}

pub(crate) fn json(status: &crate::response::StatusCodeReasonPhrase, body: String) -> Response {
    let mut response = Response::json(status, body.into_bytes());
    response.headers.push(Header::new("Cache-Control", "no-store"));
    response
}

pub(crate) fn error(status: &crate::response::StatusCodeReasonPhrase, message: &str) -> Response {
    json(status, format!(r#"{{"error":"{}"}}"#, json_escape(message)))
}

impl Application for AdminApi {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        if !self.origin_allowed(request) {
            return Ok(error(STATUS_CODE_REASON_PHRASE.n403_forbidden, "origin not allowed"));
        }
        if !self.authorized(request) {
            let mut response = error(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "missing or wrong bearer token");
            response.headers.push(Header::new("WWW-Authenticate", r#"Bearer realm="rws admin""#));
            return Ok(response);
        }
        let path = crate::url::URL::request_target_path(&request.request_uri).unwrap_or_default();
        let Some(route) = path.strip_prefix(API_PREFIX) else {
            return Ok(error(STATUS_CODE_REASON_PHRASE.n404_not_found, "not found"));
        };
        match (request.method.as_str(), route.trim_end_matches('/')) {
            ("GET", "/status") => Ok(json(STATUS_CODE_REASON_PHRASE.n200_ok, status_json(connection))),
            (_, "/status") => Ok(error(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed, "use GET")),
            _ => Ok(error(STATUS_CODE_REASON_PHRASE.n404_not_found, "not found")),
        }
    }
}

/// Compile-time features of this build that matter to an operator.
pub fn features() -> Vec<&'static str> {
    let mut on = Vec::new();
    macro_rules! feature {
        ($($name:literal),*) => { $( if cfg!(feature = $name) { on.push($name); } )* };
    }
    feature!("http1", "http2", "http3", "acme", "auth", "sso", "sso-saml", "sso-server", "gateway-api", "tower", "openapi", "mimalloc", "brotli", "zstd");
    on
}

fn status_json(connection: &ConnectionInfo) -> String {
    let env = |name: &str| std::env::var(name).unwrap_or_default();
    let tls = !env(Config::RWS_CONFIG_TLS_CERT_FILE).is_empty() || !env(Config::RWS_CONFIG_ACME_DOMAINS).is_empty();
    let redirect = env(Config::RWS_CONFIG_HTTP_REDIRECT_PORT);
    let features: Vec<String> = features().iter().map(|f| format!("\"{f}\"")).collect();
    format!(
        r#"{{"version":"{}","pid":{},"uptime_secs":{},"ready":{},"features":[{}],"listeners":{{"main":{{"address":"{}:{}","tls":{tls}}},"redirect":{},"admin":"{}:{}"}}}}"#,
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        started().elapsed().as_secs(),
        crate::metrics::SERVER_READY.load(Ordering::SeqCst),
        features.join(","),
        json_escape(&env(Config::RWS_CONFIG_IP)),
        json_escape(&env(Config::RWS_CONFIG_PORT)),
        if redirect.trim().is_empty() { "null".to_string() } else { format!("\"{}\"", json_escape(redirect.trim())) },
        json_escape(&connection.server.ip),
        connection.server.port,
    )
}

/// Semantic checks of `[admin]` for `rws --check`; `get` reads a setting
/// of the file.
pub(crate) fn validate(get: &dyn Fn(&str) -> Option<String>, report: &mut crate::config_check::ConfigReport) {
    let listen = get(Config::RWS_CONFIG_ADMIN_LISTEN).unwrap_or_default();
    let listen = listen.trim();
    if listen.is_empty() {
        for key in [Config::RWS_CONFIG_ADMIN_TOKEN_ENV, Config::RWS_CONFIG_ADMIN_ALLOWED_ORIGINS] {
            if get(key).is_some_and(|v| !v.trim().is_empty()) {
                report.warn("[admin] has settings but no listen: the admin API is off");
                break;
            }
        }
        return;
    }
    match listen.parse::<std::net::SocketAddr>() {
        Err(_) => report.error(format!("[admin] listen = {listen:?}: expected host:port with an IP address, e.g. \"127.0.0.1:9901\"")),
        Ok(addr) if !addr.ip().is_loopback() => report.warn(format!(
            "[admin] listen = {listen:?} is reachable from other hosts: firewall it, or listen on 127.0.0.1 and use an SSH tunnel"
        )),
        Ok(_) => {}
    }
    let token_env = get(Config::RWS_CONFIG_ADMIN_TOKEN_ENV).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let token_env = token_env.unwrap_or_else(|| DEFAULT_TOKEN_ENV.to_string());
    if !crate::config_check::offline() && std::env::var(&token_env).unwrap_or_default().trim().is_empty() {
        report.error(format!("[admin] reads its token from {token_env}, which is unset or empty: the admin API won't start"));
    }
    for origin in get(Config::RWS_CONFIG_ADMIN_ALLOWED_ORIGINS).unwrap_or_default().split(',').map(str::trim).filter(|o| !o.is_empty()) {
        if !(origin.starts_with("http://") || origin.starts_with("https://")) || origin.trim_end_matches('/').matches('/').count() > 2 {
            report.error(format!("[admin] allowed_origins: {origin:?} is not an origin (scheme://host[:port])"));
        }
    }
}

/// Start the admin listener `config` describes, on a thread of its own.
/// `Ok(None)` when it is off.
pub fn start(config: &AdminConfig) -> Result<Option<std::net::SocketAddr>, String> {
    if !config.enabled() {
        return Ok(None);
    }
    let token = config.token()?;
    let listener = std::net::TcpListener::bind(&config.listen).map_err(|e| format!("can't listen on {}: {e}", config.listen))?;
    let local = listener.local_addr().map_err(|e| e.to_string())?;
    let api = AdminApi::new(token).allowed_origins(config.allowed_origins.clone());
    std::thread::Builder::new()
        .name("rws-admin".to_string())
        .spawn(move || serve(listener, api))
        .map_err(|e| e.to_string())?;
    Ok(Some(local))
}

/// [`start`] from `RWS_CONFIG_ADMIN_*`, reporting the outcome: the binary's
/// entry point. A misconfigured admin API is logged, not fatal.
pub fn start_from_env() {
    match start(&AdminConfig::from_env()) {
        Ok(Some(addr)) => println!("admin API at http://{addr}{API_PREFIX}/"),
        Ok(None) => {}
        Err(e) => eprintln!("admin API not started: {e}"),
    }
}

fn serve(listener: std::net::TcpListener, api: AdminApi) {
    let open = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if open.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            open.fetch_sub(1, Ordering::SeqCst);
            continue;
        }
        let (api, held) = (api.clone(), Arc::clone(&open));
        let spawned = std::thread::Builder::new().name("rws-admin-conn".to_string()).spawn(move || {
            let address = |a: Option<std::net::SocketAddr>| match a {
                Some(a) => Address { ip: a.ip().to_string(), port: a.port() as i32 },
                None => Address { ip: String::new(), port: 0 },
            };
            let mut connection = ConnectionInfo::new(address(stream.peer_addr().ok()), address(stream.local_addr().ok()));
            connection.request_size = 16 * 1024;
            let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
            let _ = stream.set_nodelay(true);
            let _ = crate::server::Server::process(stream, connection, api);
            held.fetch_sub(1, Ordering::SeqCst);
        });
        if spawned.is_err() {
            open.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests;
