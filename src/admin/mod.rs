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

pub mod ops;

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

type Reloader = Arc<dyn Fn() + Send + Sync>;

struct Inner {
    token: String,
    allowed_origins: Vec<String>,
    reloader: Reloader,
    reload_wait: Duration,
}

impl AdminApi {
    /// The API accepting `token`.
    pub fn new(token: impl Into<String>) -> AdminApi {
        started();
        crate::log::tail::enable();
        AdminApi {
            inner: Arc::new(Inner {
                token: token.into(),
                allowed_origins: Vec::new(),
                reloader: Arc::new(crate::config_reload::request_reload),
                reload_wait: Duration::from_secs(10),
            }),
        }
    }

    fn edit(self, f: impl FnOnce(&mut Inner)) -> AdminApi {
        let mut inner = Inner {
            token: self.inner.token.clone(),
            allowed_origins: self.inner.allowed_origins.clone(),
            reloader: Arc::clone(&self.inner.reloader),
            reload_wait: self.inner.reload_wait,
        };
        f(&mut inner);
        AdminApi { inner: Arc::new(inner) }
    }

    /// Browser origins allowed besides the listener's own.
    pub fn allowed_origins(self, origins: Vec<String>) -> AdminApi {
        self.edit(|i| i.allowed_origins = origins)
    }

    /// What `POST …/reload` runs (default: [`crate::config_reload::request_reload`],
    /// the same path as `SIGHUP`, so TLS certificates are reloaded too), and
    /// how long it waits for the reload to finish.
    pub fn reloader(self, reload: impl Fn() + Send + Sync + 'static, wait: Duration) -> AdminApi {
        self.edit(|i| {
            i.reloader = Arc::new(reload);
            i.reload_wait = wait;
        })
    }

    /// Request a reload and wait for it: the reload count moving, then the
    /// errors it reported.
    fn reload(&self) -> Response {
        if !crate::metrics::SERVER_READY.load(Ordering::SeqCst) {
            // Before the servers run, SIGHUP has no handler yet.
            return error(STATUS_CODE_REASON_PHRASE.n503_service_unavailable, "the server is starting or stopping");
        }
        let (before, _) = crate::config_reload::last_reload();
        (self.inner.reloader)();
        let deadline = Instant::now() + self.inner.reload_wait;
        loop {
            let (count, errors) = crate::config_reload::last_reload();
            if count > before {
                let errors: Vec<String> = errors.iter().map(|e| format!("\"{}\"", json_escape(e))).collect();
                let applied = errors.is_empty();
                return json(STATUS_CODE_REASON_PHRASE.n200_ok, format!(r#"{{"reloaded":true,"applied":{applied},"reloads":{count},"errors":[{}]}}"#, errors.join(",")));
            }
            if Instant::now() >= deadline {
                return error(STATUS_CODE_REASON_PHRASE.n504_gateway_timeout, "the reload didn't finish in time; check the server log");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
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
        Ok(self.route(request, connection, route.trim_end_matches('/')))
    }
}

fn ok(body: String) -> Response {
    json(STATUS_CODE_REASON_PHRASE.n200_ok, body)
}

fn bad_request(message: &str) -> Response {
    error(STATUS_CODE_REASON_PHRASE.n400_bad_request, message)
}

/// The JSON body's top-level boolean `key`.
fn body_bool(request: &Request, key: &str) -> Option<bool> {
    let body = std::str::from_utf8(&request.body).ok()?;
    let value = crate::mcp::json_rpc::object_members(body)?.into_iter().find(|(k, _)| k == key)?.1;
    match value.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// The JSON body's top-level string `key`.
fn body_str(request: &Request, key: &str) -> Option<String> {
    let body = std::str::from_utf8(&request.body).ok()?;
    let value = crate::mcp::json_rpc::object_members(body)?.into_iter().find(|(k, _)| k == key)?.1;
    value.starts_with('"').then(|| crate::mcp::json_rpc::extract_str(&format!(r#"{{"v":{value}}}"#), "v")).flatten()
}

/// Query parameter `key` of the request target.
fn query(request: &Request, key: &str) -> Option<String> {
    let (_, query) = request.request_uri.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == key).then(|| v.to_string())
    })
}

fn parse_ip(ip: &str) -> Option<String> {
    let ip = crate::url::URL::percent_decode(ip.trim());
    ip.parse::<std::net::IpAddr>().ok().map(|ip| ip.to_string())
}

impl AdminApi {
    fn route(&self, request: &Request, connection: &ConnectionInfo, route: &str) -> Response {
        let method = request.method.as_str();
        let wrong_method = |allowed: &str| {
            let mut response = error(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed, &format!("use {allowed}"));
            response.headers.push(Header::new("Allow", allowed.to_string()));
            response
        };
        if let Some(ip) = route.strip_prefix("/blocklist/") {
            return match (method, parse_ip(ip)) {
                ("DELETE", Some(ip)) => ok(ops::unblock(&ip)),
                ("DELETE", None) => bad_request("not an IP address"),
                _ => wrong_method("DELETE"),
            };
        }
        if let Some(name) = route.strip_prefix("/flags/") {
            let name = crate::url::URL::percent_decode(name);
            return match method {
                "PUT" => match body_bool(request, "enabled") {
                    Some(enabled) if !name.is_empty() => ok(ops::set_flag(&name, enabled)),
                    _ => bad_request(r#"expected {"enabled":true|false}"#),
                },
                _ => wrong_method("PUT"),
            };
        }
        if let Some(name) = route.strip_prefix("/upstreams/").and_then(|r| r.strip_suffix("/drain")) {
            let name = crate::url::URL::percent_decode(name);
            return match (method, body_str(request, "backend"), body_bool(request, "drained")) {
                ("PUT", Some(backend), Some(drained)) => match ops::set_drained(&name, &backend, drained) {
                    Ok(body) => ok(body),
                    Err(e) => error(STATUS_CODE_REASON_PHRASE.n404_not_found, &e),
                },
                ("PUT", _, _) => bad_request(r#"expected {"backend":"<url>","drained":true|false}"#),
                _ => wrong_method("PUT"),
            };
        }
        match (method, route) {
            ("GET", "/logs") => {
                let n = query(request, "n").and_then(|n| n.parse().ok()).unwrap_or(100);
                ok(ops::logs_json(n))
            }
            (_, "/logs") => wrong_method("GET"),
            ("GET", "/logs/stream") => {
                let (sender, mut response) = crate::sse::SseStream::new().buffer(256).start();
                if !crate::log::tail::follow(sender) {
                    return error(STATUS_CODE_REASON_PHRASE.n503_service_unavailable, "too many log followers");
                }
                response.headers.push(Header::new("Cache-Control", "no-store"));
                response
            }
            (_, "/logs/stream") => wrong_method("GET"),
            ("GET", "/upstreams") => ok(ops::upstreams_json()),
            (_, "/upstreams") => wrong_method("GET"),
            ("GET", "/certificates") => ok(ops::certificates_json()),
            (_, "/certificates") => wrong_method("GET"),
            ("POST", "/certificates/renew") => match ops::request_renewal() {
                true => json(STATUS_CODE_REASON_PHRASE.n202_accepted, r#"{"requested":true}"#.to_string()),
                false => error(STATUS_CODE_REASON_PHRASE.n404_not_found, "ACME isn't configured: nothing to renew"),
            },
            (_, "/certificates/renew") => wrong_method("POST"),
            ("GET", "/status") => ok(status_json(connection)),
            (_, "/status") => wrong_method("GET"),
            ("GET", "/config") => ok(ops::config_json()),
            (_, "/config") => wrong_method("GET"),
            ("POST", "/reload") => self.reload(),
            (_, "/reload") => wrong_method("POST"),
            ("GET", "/metrics") => ok(ops::metrics_json()),
            (_, "/metrics") => wrong_method("GET"),
            ("GET", "/build") => ok(ops::build_features_json()),
            (_, "/build") => wrong_method("GET"),
            ("GET", "/rate-limit") => ok(ops::rate_limit_json()),
            (_, "/rate-limit") => wrong_method("GET"),
            ("GET", "/rate-limit/check") => match query(request, "ip").as_deref().and_then(parse_ip) {
                Some(ip) => ok(ops::check_rate_limit_json(&ip)),
                None => bad_request("expected ?ip=<address>"),
            },
            (_, "/rate-limit/check") => wrong_method("GET"),
            ("GET", "/cors") => ok(ops::cors_json()),
            (_, "/cors") => wrong_method("GET"),
            ("GET", "/maintenance") => ok(ops::maintenance_json()),
            ("PUT", "/maintenance") => match body_bool(request, "active") {
                Some(active) => ok(ops::set_maintenance(active)),
                None => bad_request(r#"expected {"active":true|false}"#),
            },
            (_, "/maintenance") => wrong_method("GET, PUT"),
            ("GET", "/blocklist") => ok(ops::blocklist_json()),
            ("POST", "/blocklist") => match body_str(request, "ip").as_deref().and_then(parse_ip) {
                Some(ip) => ok(ops::block(&ip)),
                None => bad_request(r#"expected {"ip":"<address>"}"#),
            },
            (_, "/blocklist") => wrong_method("GET, POST"),
            ("GET", "/flags") => ok(ops::flags_json()),
            (_, "/flags") => wrong_method("GET"),
            ("GET", "/cache") => match ops::registered_cache_json() {
                Some(stats) => ok(stats),
                None => error(STATUS_CODE_REASON_PHRASE.n404_not_found, "no response cache in this server"),
            },
            ("DELETE", "/cache") => match ops::clear_registered_cache() {
                true => ok(r#"{"cleared":true}"#.to_string()),
                false => error(STATUS_CODE_REASON_PHRASE.n404_not_found, "no response cache in this server"),
            },
            (_, "/cache") => wrong_method("GET, DELETE"),
            ("GET", "/routes") => ok(ops::registered_routes_json()),
            (_, "/routes") => wrong_method("GET"),
            ("GET", "/requests" | "/errors") => {
                let n = query(request, "n").and_then(|n| n.parse().ok()).unwrap_or(20);
                ok(ops::recent_json(n, route == "/errors"))
            }
            (_, "/requests" | "/errors") => wrong_method("GET"),
            _ => error(STATUS_CODE_REASON_PHRASE.n404_not_found, "not found"),
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
