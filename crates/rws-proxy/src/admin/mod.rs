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

pub mod access;
pub mod ops;
mod ui;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use crate::application::Application;
use crate::entry_point::Config;
use crate::header::Header;
use crate::core::json::escape as json_escape;
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
    /// More users: `name:ENV_VAR:role`, comma-separated (18.22.0).
    pub users: String,
    /// Append the audit log here (empty: memory only).
    pub audit_log: String,
    /// OIDC sign-in to the admin UI (empty issuer and provider: off).
    pub oidc: AdminOidcConfig,
}

/// `[admin] oidc_*` (18.22.0): sign in to the admin UI with an OIDC provider.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AdminOidcConfig {
    pub issuer: String,
    pub provider: String,
    pub tenant: String,
    pub client_id: String,
    pub client_secret_env: String,
    pub cookie_secret_env: String,
    /// `group=role`, comma-separated; `*` matches anyone signed in.
    pub roles: String,
    /// The ID-token claim holding groups (default `groups`).
    pub role_claim: String,
}

impl AdminOidcConfig {
    pub fn enabled(&self) -> bool {
        !self.issuer.is_empty() || !self.provider.is_empty()
    }
}

/// Settings without a CLI flag (`[admin] users`, `audit_log`, `oidc_*`).
pub const RWS_CONFIG_ADMIN_USERS: &str = "RWS_CONFIG_ADMIN_USERS";
pub const RWS_CONFIG_ADMIN_AUDIT_LOG: &str = "RWS_CONFIG_ADMIN_AUDIT_LOG";
pub const RWS_CONFIG_ADMIN_OIDC_ISSUER: &str = "RWS_CONFIG_ADMIN_OIDC_ISSUER";
pub const RWS_CONFIG_ADMIN_OIDC_PROVIDER: &str = "RWS_CONFIG_ADMIN_OIDC_PROVIDER";
pub const RWS_CONFIG_ADMIN_OIDC_TENANT: &str = "RWS_CONFIG_ADMIN_OIDC_TENANT";
pub const RWS_CONFIG_ADMIN_OIDC_CLIENT_ID: &str = "RWS_CONFIG_ADMIN_OIDC_CLIENT_ID";
pub const RWS_CONFIG_ADMIN_OIDC_CLIENT_SECRET_ENV: &str = "RWS_CONFIG_ADMIN_OIDC_CLIENT_SECRET_ENV";
pub const RWS_CONFIG_ADMIN_OIDC_COOKIE_SECRET_ENV: &str = "RWS_CONFIG_ADMIN_OIDC_COOKIE_SECRET_ENV";
pub const RWS_CONFIG_ADMIN_OIDC_ROLES: &str = "RWS_CONFIG_ADMIN_OIDC_ROLES";
pub const RWS_CONFIG_ADMIN_OIDC_ROLE_CLAIM: &str = "RWS_CONFIG_ADMIN_OIDC_ROLE_CLAIM";

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
            users: read(RWS_CONFIG_ADMIN_USERS),
            audit_log: read(RWS_CONFIG_ADMIN_AUDIT_LOG),
            oidc: AdminOidcConfig {
                issuer: read(RWS_CONFIG_ADMIN_OIDC_ISSUER).trim_end_matches('/').to_string(),
                provider: read(RWS_CONFIG_ADMIN_OIDC_PROVIDER).to_ascii_lowercase(),
                tenant: read(RWS_CONFIG_ADMIN_OIDC_TENANT),
                client_id: read(RWS_CONFIG_ADMIN_OIDC_CLIENT_ID),
                client_secret_env: read(RWS_CONFIG_ADMIN_OIDC_CLIENT_SECRET_ENV),
                cookie_secret_env: read(RWS_CONFIG_ADMIN_OIDC_COOKIE_SECRET_ENV),
                roles: read(RWS_CONFIG_ADMIN_OIDC_ROLES),
                role_claim: read(RWS_CONFIG_ADMIN_OIDC_ROLE_CLAIM),
            },
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
    users: Vec<access::TokenUser>,
    allowed_origins: Vec<String>,
    reloader: Reloader,
    reload_wait: Duration,
    #[cfg(feature = "sso")]
    oidc: Option<Arc<AdminOidc>>,
}

/// OIDC sign-in for the admin UI: the lazily discovered layer and the
/// group → role map.
#[cfg(feature = "sso")]
struct AdminOidc {
    layer: crate::proxy_config::oidc_route::LazyOidc,
    roles: Vec<(String, access::Role)>,
}

impl AdminApi {
    /// The API accepting `token`.
    pub fn new(token: impl Into<String>) -> AdminApi {
        started();
        crate::log::tail::enable();
        AdminApi {
            inner: Arc::new(Inner {
                users: vec![access::TokenUser { name: "admin".to_string(), token: token.into(), role: access::Role::Admin }],
                allowed_origins: Vec::new(),
                reloader: Arc::new(crate::config_reload::request_reload),
                reload_wait: Duration::from_secs(10),
                #[cfg(feature = "sso")]
                oidc: None,
            }),
        }
    }

    fn edit(self, f: impl FnOnce(&mut Inner)) -> AdminApi {
        let mut inner = Inner {
            users: self.inner.users.clone(),
            allowed_origins: self.inner.allowed_origins.clone(),
            reloader: Arc::clone(&self.inner.reloader),
            reload_wait: self.inner.reload_wait,
            #[cfg(feature = "sso")]
            oidc: self.inner.oidc.clone(),
        };
        f(&mut inner);
        AdminApi { inner: Arc::new(inner) }
    }

    /// More users with their own tokens and roles (18.22.0).
    pub fn users(self, users: Vec<access::TokenUser>) -> AdminApi {
        self.edit(|i| i.users.extend(users))
    }

    /// Sign-in to the admin UI with an OIDC provider; `roles` maps groups
    /// (or emails, or `*`) to roles (18.22.0).
    #[cfg(feature = "sso")]
    pub fn oidc(self, config: &AdminOidcConfig) -> Result<AdminApi, String> {
        let roles = access::parse_role_map(&config.roles)?;
        if roles.is_empty() {
            return Err("[admin] oidc_roles is empty: nobody signed in would get a role".to_string());
        }
        let layer = crate::proxy_config::oidc_route::LazyOidc::new(crate::proxy_config::OidcRouteAuth {
            issuer: config.issuer.clone(),
            provider: config.provider.clone(),
            tenant: config.tenant.clone(),
            client_id: config.client_id.clone(),
            client_secret_env: config.client_secret_env.clone(),
            cookie_secret_env: config.cookie_secret_env.clone(),
            groups_claim: config.role_claim.clone(),
            session_ttl_secs: 12 * 3600,
            path_prefix: "/admin".to_string(),
            ..Default::default()
        })?;
        Ok(self.edit(|i| i.oidc = Some(Arc::new(AdminOidc { layer, roles }))))
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

    /// Who sent `request`: a bearer token's user, else (with OIDC sign-in)
    /// the signed-in user with the role the group map gives them.
    fn authenticate(&self, request: &Request) -> Authenticated {
        if let Some(sent) = request.header("Authorization").and_then(|h| h.value.strip_prefix("Bearer ").map(str::trim)) {
            // Every user compared, so timing doesn't tell which name matched.
            let mut found = None;
            for user in &self.inner.users {
                if crate::proxy_config::constant_time_eq(sent.as_bytes(), user.token.as_bytes()) {
                    found = Some(access::Principal { user: user.name.clone(), role: user.role, via: "token" });
                }
            }
            return found.map_or(Authenticated::No, Authenticated::Yes);
        }
        #[cfg(feature = "sso")]
        if let Some(oidc) = &self.inner.oidc {
            if let Some(user) = oidc.layer.auth().and_then(|auth| auth.signed_in(request)) {
                let name = user.email.clone().unwrap_or_else(|| user.sub.clone());
                return match access::role_for(&oidc.roles, user.email.as_deref(), &user.groups) {
                    Some(role) => Authenticated::Yes(access::Principal { user: name, role, via: "oidc" }),
                    None => Authenticated::NoRole(name),
                };
            }
        }
        Authenticated::No
    }

    fn oidc_enabled(&self) -> bool {
        #[cfg(feature = "sso")]
        return self.inner.oidc.is_some();
        #[cfg(not(feature = "sso"))]
        false
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

#[doc(hidden)]
pub fn json(status: &crate::response::StatusCodeReasonPhrase, body: String) -> Response {
    let mut response = Response::json(status, body.into_bytes());
    response.headers.push(Header::new("Cache-Control", "no-store"));
    response
}

#[doc(hidden)]
pub fn error(status: &crate::response::StatusCodeReasonPhrase, message: &str) -> Response {
    json(status, format!(r#"{{"error":"{}"}}"#, json_escape(message)))
}

impl Application for AdminApi {
    fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
        if !self.origin_allowed(request) {
            return Ok(error(STATUS_CODE_REASON_PHRASE.n403_forbidden, "origin not allowed"));
        }
        let path = crate::url::URL::request_target_path(&request.request_uri).unwrap_or_default();
        if request.method == "GET" || request.method == "HEAD" {
            if path == "/admin" || path == "/" {
                return Ok(ui::redirect());
            }
            if let Some(file) = ui::file(path) {
                return Ok(file);
            }
        }
        // OIDC sign-in: /admin/auth/login, /callback, /logout.
        if path.starts_with("/admin/auth/") {
            #[cfg(feature = "sso")]
            if let Some(oidc) = &self.inner.oidc {
                return match oidc.layer.auth() {
                    Some(auth) => crate::middleware::Middleware::handle(&*auth, request, connection, &NotFound),
                    None => Ok(error(STATUS_CODE_REASON_PHRASE.n503_service_unavailable, "the identity provider can't be reached; try again shortly")),
                };
            }
            return Ok(error(STATUS_CODE_REASON_PHRASE.n404_not_found, "OIDC sign-in isn't configured"));
        }
        let route = path.strip_prefix(API_PREFIX).map(|r| r.trim_end_matches('/'));
        // How to sign in, for the UI's login page: no credentials needed.
        if route == Some("/auth") && request.method == "GET" {
            return Ok(ok(format!(r#"{{"token":true,"oidc":{}}}"#, self.oidc_enabled())));
        }
        let address = crate::ip_filter::client_ip(request, &connection.client.ip).into_owned();
        let principal = match self.authenticate(request) {
            Authenticated::Yes(p) => p,
            Authenticated::NoRole(name) => {
                let response = error(STATUS_CODE_REASON_PHRASE.n403_forbidden, &format!("{name} is signed in but has no admin role (see [admin] oidc_roles)"));
                if !is_read(&request.method) {
                    access::record_change(&access::Principal { user: name, role: access::Role::Viewer, via: "oidc" }, &address, &request.method, path, &request.body, 403);
                }
                return Ok(response);
            }
            Authenticated::No => {
                if !is_read(&request.method) && request.header("Authorization").is_some() {
                    access::record_change(&access::Principal { user: "(bad token)".to_string(), role: access::Role::Viewer, via: "token" }, &address, &request.method, path, &request.body, 401);
                }
                let mut response = error(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "missing or wrong bearer token");
                response.headers.push(Header::new("WWW-Authenticate", r#"Bearer realm="rws admin""#));
                return Ok(response);
            }
        };
        let Some(route) = route else {
            return Ok(error(STATUS_CODE_REASON_PHRASE.n404_not_found, "not found"));
        };
        // A browser signed in with a cookie: changes need the header only
        // this origin's script can set (CSRF), besides the Origin check.
        if principal.via == "oidc" && !is_read(&request.method) && request.header("X-Rws-Admin").is_none() {
            access::record_change(&principal, &address, &request.method, path, &request.body, 403);
            return Ok(error(STATUS_CODE_REASON_PHRASE.n403_forbidden, "missing X-Rws-Admin header"));
        }
        let needed = access::required_role(&request.method, route);
        if principal.role < needed {
            access::record_change(&principal, &address, &request.method, path, &request.body, 403);
            return Ok(error(STATUS_CODE_REASON_PHRASE.n403_forbidden, &format!("{} has role {}; this needs {}", principal.user, principal.role.as_str(), needed.as_str())));
        }
        let response = match (request.method.as_str(), route) {
            ("GET", "/whoami") => ok(format!(r#"{{"user":"{}","role":"{}","via":"{}"}}"#, json_escape(&principal.user), principal.role.as_str(), principal.via)),
            ("GET", "/audit") => ok(access::recent_json(query(request, "n").and_then(|n| n.parse().ok()).unwrap_or(100).min(access::AUDIT_CAPACITY))),
            ("GET", "/audit/verify") => match access::verify_audit_file() {
                Ok((file, report)) => {
                    let report = report.to_json();
                    ok(format!(r#"{{"file":"{}",{}"#, json_escape(&file), &report[1..]))
                }
                Err(e) => error(STATUS_CODE_REASON_PHRASE.n404_not_found, &e),
            },
            (_, "/whoami" | "/audit" | "/audit/verify") => wrong_method("GET"),
            _ => self.route(request, connection, route),
        };
        if !is_read(&request.method) && !(request.method == "POST" && route == "/authz/check") {
            access::record_change(&principal, &address, &request.method, path, &request.body, response.status_code as u16);
        }
        Ok(response)
    }
}

/// The outcome of [`AdminApi::authenticate`].
enum Authenticated {
    Yes(access::Principal),
    /// Signed in (OIDC), but no role maps to them.
    #[cfg_attr(not(feature = "sso"), allow(dead_code))]
    NoRole(String),
    No,
}

fn is_read(method: &str) -> bool {
    matches!(method, "GET" | "HEAD" | "OPTIONS")
}

/// The rest of the stack behind the OIDC layer on `/admin/auth/*`: the
/// layer answers those paths itself.
#[cfg(feature = "sso")]
struct NotFound;

#[cfg(feature = "sso")]
impl Application for NotFound {
    fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
        Ok(error(STATUS_CODE_REASON_PHRASE.n404_not_found, "not found"))
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

/// `405`, naming the methods that work.
fn wrong_method(allowed: &str) -> Response {
    let mut response = error(STATUS_CODE_REASON_PHRASE.n405_method_not_allowed, &format!("use {allowed}"));
    response.headers.push(Header::new("Allow", allowed.to_string()));
    response
}

/// `/authz`, `/authz/tuples`, `/authz/check` (#32).
fn authz_route(method: &str, route: &str, request: &Request) -> Response {
    let decoded = |key: &str| query(request, key).map(|v| crate::url::URL::percent_decode(&v)).filter(|v| !v.is_empty());
    // A configuration problem is a 404 ("nothing to manage"), a bad request
    // a 400.
    let answer = |result: Result<String, String>, missing_is_404: bool| match result {
        Ok(body) => ok(body),
        Err(e) if missing_is_404 && e.starts_with("no authorization configured") => error(STATUS_CODE_REASON_PHRASE.n404_not_found, &e),
        Err(e) => error(STATUS_CODE_REASON_PHRASE.n400_bad_request, &e),
    };
    match (method, route) {
        ("GET", "/authz") => answer(ops::authz_schema_json(), true),
        (_, "/authz") => wrong_method("GET"),
        ("GET", "/authz/tuples") => {
            let limit = decoded("limit").and_then(|n| n.parse().ok()).unwrap_or(100);
            answer(ops::authz_tuples_json(decoded("object").as_deref(), decoded("relation").as_deref(), decoded("subject").as_deref(), limit), true)
        }
        ("POST", "/authz/tuples") => match body_str(request, "tuple") {
            Some(tuple) => answer(ops::authz_write(&tuple), true),
            None => bad_request(r#"expected {"tuple":"doc:42#viewer@user:alice"}"#),
        },
        ("DELETE", "/authz/tuples") => match decoded("tuple").or_else(|| body_str(request, "tuple")) {
            Some(tuple) => answer(ops::authz_delete(&tuple), true),
            None => bad_request(r#"expected ?tuple=… or {"tuple":"doc:42#viewer@user:alice"}"#),
        },
        (_, "/authz/tuples") => wrong_method("GET, POST, DELETE"),
        ("POST", "/authz/check") => match (body_str(request, "subject"), body_str(request, "relation"), body_str(request, "object")) {
            (Some(subject), Some(relation), Some(object)) => answer(ops::authz_check(&subject, &relation, &object), true),
            _ => bad_request(r#"expected {"subject":"user:alice","relation":"viewer","object":"doc:42"}"#),
        },
        (_, "/authz/check") => wrong_method("POST"),
        _ => error(STATUS_CODE_REASON_PHRASE.n404_not_found, "unknown /authz endpoint"),
    }
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
        if let Some(name) = route.strip_prefix("/caches/") {
            let name = crate::url::URL::percent_decode(name);
            let decoded = |key: &str| query(request, key).map(|v| crate::url::URL::percent_decode(&v));
            return match method {
                "DELETE" => match ops::purge_cache(&name, decoded("path").as_deref(), decoded("prefix").as_deref()) {
                    Some(body) => ok(body),
                    None => error(STATUS_CODE_REASON_PHRASE.n404_not_found, &format!("no cache named {name:?}")),
                },
                _ => wrong_method("DELETE"),
            };
        }
        if route == "/traffic" {
            if method != "GET" {
                return wrong_method("GET");
            }
            let hours = query(request, "hours").and_then(|h| h.parse().ok()).unwrap_or(24);
            let host = query(request, "host").map(|h| crate::url::URL::percent_decode(&h));
            return match ops::traffic_json(hours, host.as_deref()) {
                Ok(body) => ok(body),
                Err(e) => error(STATUS_CODE_REASON_PHRASE.n404_not_found, &e),
            };
        }
        if route == "/authz" || route.starts_with("/authz/") {
            return authz_route(method, route, request);
        }
        match (method, route) {
            ("GET", "/caches") => ok(ops::caches_json()),
            (_, "/caches") => wrong_method("GET"),
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
    feature!("http1", "http2", "http3", "acme", "auth", "sso", "sso-saml", "sso-server", "gateway-api", "tower", "openapi", "mimalloc", "brotli", "zstd", "wasm-plugins", "fips", "geo");
    on
}

fn fleet_status() -> String {
    #[cfg(feature = "http2")]
    return crate::fleet::status_json().unwrap_or_else(|| "null".to_string());
    #[cfg(not(feature = "http2"))]
    "null".to_string()
}

fn status_json(connection: &ConnectionInfo) -> String {
    let env = |name: &str| std::env::var(name).unwrap_or_default();
    let tls = !env(Config::RWS_CONFIG_TLS_CERT_FILE).is_empty() || !env(Config::RWS_CONFIG_ACME_DOMAINS).is_empty();
    let redirect = env(Config::RWS_CONFIG_HTTP_REDIRECT_PORT);
    let features: Vec<String> = features().iter().map(|f| format!("\"{f}\"")).collect();
    // Where rws writes files (access log, disk cache): free space and
    // whether it's under RWS_CONFIG_MIN_FREE_DISK (18.15.0).
    let disk: Vec<String> = crate::disk_guard::status()
        .into_iter()
        .map(|(dir, free, total, low)| format!(r#"{{"dir":"{}","free_bytes":{free},"total_bytes":{total},"low":{low}}}"#, json_escape(&dir)))
        .collect();
    format!(
        r#"{{"version":"{}","pid":{},"uptime_secs":{},"ready":{},"features":[{}],"listeners":{{"main":{{"address":"{}:{}","tls":{tls}}},"redirect":{},"admin":"{}:{}"}},"disk":[{}],"cluster":{},"fleet":{}}}"#,
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
        disk.join(","),
        crate::cluster::status_json().unwrap_or_else(|| "null".to_string()),
        fleet_status(),
    )
}

/// Semantic checks of `[admin]` for `rws --check`; `get` reads a setting
/// of the file.
#[doc(hidden)]
pub fn validate(get: &dyn Fn(&str) -> Option<String>, report: &mut crate::config_check::ConfigReport) {
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
    // 18.22.0: users, the audit log, OIDC sign-in.
    let users = get(RWS_CONFIG_ADMIN_USERS).unwrap_or_default();
    let env = |name: &str| if crate::config_check::offline() { Some("offline-check-placeholder".to_string()) } else { std::env::var(name).ok() };
    if let Err(e) = access::parse_users(&users, &env) {
        report.error(e);
    }
    let audit_log = get(RWS_CONFIG_ADMIN_AUDIT_LOG).unwrap_or_default();
    if let Some(dir) = std::path::Path::new(audit_log.trim()).parent().filter(|d| !audit_log.trim().is_empty() && !d.as_os_str().is_empty()) {
        if !crate::config_check::offline() && !dir.is_dir() {
            report.error(format!("[admin] audit_log: directory {} doesn't exist", dir.display()));
        }
    }
    let issuer = get(RWS_CONFIG_ADMIN_OIDC_ISSUER).unwrap_or_default();
    let provider = get(RWS_CONFIG_ADMIN_OIDC_PROVIDER).unwrap_or_default();
    if !issuer.trim().is_empty() || !provider.trim().is_empty() {
        if get(RWS_CONFIG_ADMIN_OIDC_CLIENT_ID).unwrap_or_default().trim().is_empty() {
            report.error("[admin] oidc sign-in has no oidc_client_id");
        }
        match access::parse_role_map(&get(RWS_CONFIG_ADMIN_OIDC_ROLES).unwrap_or_default()) {
            Ok(map) if map.is_empty() => report.error("[admin] oidc sign-in needs oidc_roles (e.g. [\"rws-admins=admin\", \"*=viewer\"]): nobody would get a role"),
            Ok(_) => {}
            Err(e) => report.error(e),
        }
        if !provider.trim().is_empty() && !matches!(provider.trim().to_ascii_lowercase().as_str(), "google" | "microsoft" | "github") {
            report.error(format!("[admin] oidc_provider = {provider:?}: expected google, microsoft or github (others: set oidc_issuer)"));
        }
        let cookie = get(RWS_CONFIG_ADMIN_OIDC_COOKIE_SECRET_ENV).unwrap_or_default();
        if cookie.trim().is_empty() {
            report.warn("[admin] oidc sign-in without oidc_cookie_secret_env: sign-ins end when rws restarts");
        }
        #[cfg(not(feature = "sso"))]
        report.error("[admin] oidc sign-in needs the sso feature, which this build doesn't include; the admin API won't start");
    }
}

/// Start the admin listener `config` describes, on a thread of its own.
/// `Ok(None)` when it is off.
pub fn start(config: &AdminConfig) -> Result<Option<std::net::SocketAddr>, String> {
    if !config.enabled() {
        return Ok(None);
    }
    let token = config.token()?;
    #[cfg(all(unix, any(feature = "http1", feature = "http2")))]
    let inherited = config.listen.parse().ok().and_then(|addr| crate::systemd::tcp(addr, Some("admin")));
    #[cfg(not(all(unix, any(feature = "http1", feature = "http2"))))]
    let inherited: Option<std::net::TcpListener> = None;
    let listener = match inherited {
        Some(listener) => listener,
        None => std::net::TcpListener::bind(&config.listen).map_err(|e| format!("can't listen on {}: {e}", config.listen))?,
    };
    let local = listener.local_addr().map_err(|e| e.to_string())?;
    let users = access::parse_users(&config.users, &|name| std::env::var(name).ok())?;
    access::set_audit_file(&config.audit_log);
    #[allow(unused_mut)]
    let mut api = AdminApi::new(token).allowed_origins(config.allowed_origins.clone()).users(users);
    if config.oidc.enabled() {
        #[cfg(feature = "sso")]
        {
            api = api.oidc(&config.oidc)?;
        }
        #[cfg(not(feature = "sso"))]
        return Err("[admin] oidc_* needs the sso feature, which this build doesn't include".to_string());
    }
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
        Ok(Some(addr)) => {
            println!("admin API at http://{addr}{API_PREFIX}/");
            // Traffic insights are shown by the admin UI: counted when it's
            // on, unless `[traffic] enabled = false` (18.15.0).
            if crate::traffic::config::enabled_setting() != Some(false) {
                crate::traffic::start();
            }
        }
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
