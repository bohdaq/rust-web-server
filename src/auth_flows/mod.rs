//! Sign-up, login, logout, password reset and email confirmation (18.35.0,
//! #92): the account pages and API every app needs, as one middleware.
//! Features `crypto`, `mailer` and `serde` (all in the `web` preset).
//!
//! ```rust
//! use std::sync::Arc;
//! use rust_web_server::app::App;
//! use rust_web_server::auth_flows::{AuthFlows, CurrentUser, MemoryUserStore};
//! use rust_web_server::extract::FromRequest;
//! use rust_web_server::mailer::TestMailbox;
//! use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
//!
//! let auth = AuthFlows::new(b"a secret of at least 32 bytes, from config!", "https://example.com", MemoryUserStore::new(), TestMailbox::new());
//! let app = App::with_state(())
//!     .group("/account", |g| {
//!         g.guard(rust_web_server::auth_flows::signed_in)
//!             .get("/", |request, _, _, _| {
//!                 let user = CurrentUser::from_request(request).unwrap();
//!                 Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &user.email)
//!             })
//!     })
//!     .wrap(auth);
//! ```
//!
//! | Request | JSON (`Content-Type: application/json`) | Form (HTML) |
//! |---|---|---|
//! | `POST /auth/signup` `{email, password}` | `201 {user}` and signed in (or `confirmation_required`) | redirect + flash |
//! | `POST /auth/login` `{email, password}` | `200 {user}`, `401`, `403` unconfirmed | redirect to `return_to` or the after-login path |
//! | `POST /auth/logout` | `204` | redirect |
//! | `GET /auth/me` | `200 {user}` or `401` | — |
//! | `POST /auth/password/forgot` `{email}` | `202`, whether or not the account exists | redirect + flash |
//! | `POST /auth/password/reset` `{token, password}` | `200 {user}` and signed in, `400` bad link | redirect + flash |
//! | `GET /auth/confirm?token=` / `POST /auth/confirm` `{token}` | `200 {user}`, `400` | redirect + flash |
//! | `POST /auth/confirm/resend` `{email}` | `202` | redirect + flash |
//! | `GET /auth/login`, `/auth/signup`, `/auth/password/forgot`, `/auth/password/reset?token=` | — | the forms |
//!
//! **Sessions** are a signed cookie (`rws_auth`, `HttpOnly`, `SameSite=Lax`,
//! `Secure` when the base URL is `https`) holding the user id, an expiry
//! and a fingerprint of the password hash — no session store, any replica
//! can check it, and changing the password signs every session out.
//! **Links** in the emails carry signed tokens the same way: a reset link
//! stops working once the password changes, and expires after an hour; a
//! confirmation link after three days. Logins, sign-ups and the email
//! requests are rate limited per client IP (10 a minute by default): the
//! address a proxy in `trusted_proxies` (private ranges by default)
//! forwarded, else the peer's.
//!
//! For every other request the middleware checks the session cookie and
//! passes the account to the app as [`CurrentUser`] (a handler argument);
//! [`signed_in`] is a route guard. Headers a client sends in its place are
//! removed.

#[cfg(test)]
mod tests;
mod pages;
mod store;

pub use store::{AuthUser, MemoryUserStore, UserStore};
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub use store::DbUserStore;

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::application::Application;
use crate::cookie::{signed_cookie, verify_signed_cookie, CookieJar, SetCookie};
use crate::error::IntoResponse;
use crate::extract::FromRequest;
use crate::flash::Redirect;
use crate::header::Header;
use crate::mailer::{Deliver, Email, Mail};
use crate::middleware::Middleware;
use crate::rate_limit::RateLimiter;
use crate::request::Request;
use crate::response::{Response, StatusCodeReasonPhrase, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

const USER_ID_HEADER: &str = "X-Rws-Auth-User-Id";
const USER_EMAIL_HEADER: &str = "X-Rws-Auth-User-Email";
const USER_CONFIRMED_HEADER: &str = "X-Rws-Auth-User-Confirmed";
/// Carries a per-process random value, so [`CurrentUser`] only believes
/// headers this middleware set.
const MARK_HEADER: &str = "X-Rws-Auth-Mark";

const RESET_TTL: u64 = 3600;
const CONFIRM_TTL: u64 = 3 * 86_400;

fn mark() -> &'static str {
    static MARK: OnceLock<String> = OnceLock::new();
    MARK.get_or_init(|| crate::crypto::generate_token(16))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

/// 16 hex digits of SHA-256: ties a session or link to a value that changes
/// (the password hash, the email).
fn fingerprint(value: &str) -> String {
    hex(&Sha256::digest(value.as_bytes())[..8])
}

/// The signed-in account, as [`AuthFlows`] passed it on: a handler argument
/// that answers `401` without a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentUser {
    pub id: String,
    pub email: String,
    pub confirmed: bool,
}

impl CurrentUser {
    /// The account the request is signed in as, if any.
    pub fn of(request: &Request) -> Option<CurrentUser> {
        if request.header(MARK_HEADER).map(|h| h.value.as_ref()) != Some(mark()) {
            return None;
        }
        Some(CurrentUser {
            id: request.header(USER_ID_HEADER)?.value.to_string(),
            email: request.header(USER_EMAIL_HEADER)?.value.to_string(),
            confirmed: request.header(USER_CONFIRMED_HEADER).map(|h| h.value == "true").unwrap_or(false),
        })
    }
}

impl FromRequest for CurrentUser {
    fn from_request(request: &Request) -> Result<Self, Response> {
        CurrentUser::of(request).ok_or_else(|| crate::error::AppError::Unauthorized.into_response())
    }
}

/// A route guard: `401` without a session (`group(…).guard(signed_in)`).
pub fn signed_in<S>(request: &Request, _connection: &ConnectionInfo, _state: &S) -> Result<(), Response> {
    CurrentUser::from_request(request).map(|_| ())
}

/// A route guard for pages: without a session, a redirect to `login`
/// (with `return_to` set to the page asked for).
pub fn signed_in_or_redirect<S>(login: &str) -> impl Fn(&Request, &ConnectionInfo, &S) -> Result<(), Response> + Send + Sync + 'static {
    let login = login.to_string();
    move |request, _, _| match CurrentUser::of(request) {
        Some(_) => Ok(()),
        None => {
            let back = crate::router::build_path("/", &[("return_to", &request.request_uri)]).unwrap_or_default();
            Err(Redirect::to(&format!("{login}{}", back.trim_start_matches('/'))).alert("Please sign in.").into_response())
        }
    }
}

struct Inner {
    secret: Vec<u8>,
    base_url: String,
    prefix: String,
    store: Arc<dyn UserStore>,
    mailer: Arc<dyn Deliver>,
    app_name: String,
    require_confirmation: bool,
    min_password: usize,
    session_ttl: u64,
    cookie_name: String,
    secure: bool,
    after_login: String,
    after_logout: String,
    limiter: Option<Arc<RateLimiter>>,
    on_signup: Vec<Arc<dyn Fn(&AuthUser) + Send + Sync>>,
    #[cfg(feature = "tera")]
    templates: Option<Arc<crate::template::TeraEngine>>,
}

/// The auth flows. A middleware: `app.wrap(auth)`, after (inside) `FlashLayer`
/// and `CsrfLayer`, which must see the pages it draws. See the [module docs](self).
#[derive(Clone)]
pub struct AuthFlows {
    inner: Arc<Inner>,
}

impl AuthFlows {
    /// `secret` signs sessions and links: at least 32 random bytes, the same
    /// on every replica, kept out of the code. `base_url` (`https://example.com`)
    /// starts the links in emails — never taken from the request's `Host`.
    ///
    /// # Panics
    ///
    /// When `secret` is shorter than 32 bytes.
    pub fn new(secret: &[u8], base_url: &str, store: impl UserStore + 'static, mailer: impl Deliver + 'static) -> Self {
        Self::with_arcs(secret, base_url, Arc::new(store), Arc::new(mailer))
    }

    /// [`new`](Self::new) with shared store and mailer.
    pub fn with_arcs(secret: &[u8], base_url: &str, store: Arc<dyn UserStore>, mailer: Arc<dyn Deliver>) -> Self {
        assert!(secret.len() >= 32, "AuthFlows: the secret must be at least 32 bytes");
        let base_url = base_url.trim_end_matches('/').to_string();
        AuthFlows {
            inner: Arc::new(Inner {
                secret: secret.to_vec(),
                secure: base_url.starts_with("https://"),
                base_url,
                prefix: "/auth".into(),
                store,
                mailer,
                app_name: "the app".into(),
                require_confirmation: false,
                min_password: 8,
                session_ttl: 30 * 86_400,
                cookie_name: "rws_auth".into(),
                after_login: "/".into(),
                after_logout: "/".into(),
                limiter: Some(Arc::new(RateLimiter::new(10, 60))),
                on_signup: Vec::new(),
                #[cfg(feature = "tera")]
                templates: None,
            }),
        }
    }

    fn edit(mut self, f: impl FnOnce(&mut Inner)) -> Self {
        f(Arc::get_mut(&mut self.inner).expect("configure AuthFlows before cloning it"));
        self
    }

    /// Where the flows live (default `/auth`).
    pub fn prefix(self, prefix: &str) -> Self {
        let prefix = format!("/{}", prefix.trim_matches('/'));
        self.edit(|i| i.prefix = prefix)
    }

    /// The app's name in emails (default "the app").
    pub fn app_name(self, name: &str) -> Self {
        self.edit(|i| i.app_name = name.to_string())
    }

    /// Refuse to sign in an account until its email is confirmed (default:
    /// sign in at once; the confirmation email is sent either way).
    pub fn require_confirmation(self, require: bool) -> Self {
        self.edit(|i| i.require_confirmation = require)
    }

    /// The shortest password accepted (default 8).
    pub fn min_password_length(self, n: usize) -> Self {
        self.edit(|i| i.min_password = n.max(1))
    }

    /// How long a session lasts (default 30 days).
    pub fn session_ttl(self, ttl: std::time::Duration) -> Self {
        self.edit(|i| i.session_ttl = ttl.as_secs().max(60))
    }

    /// The session cookie's name (default `rws_auth`).
    pub fn cookie_name(self, name: &str) -> Self {
        self.edit(|i| i.cookie_name = name.to_string())
    }

    /// Mark the session cookie `Secure` (default: when the base URL is `https`).
    pub fn secure_cookie(self, secure: bool) -> Self {
        self.edit(|i| i.secure = secure)
    }

    /// Where a form sign-in, sign-up or reset lands (default `/`).
    pub fn after_login(self, path: &str) -> Self {
        self.edit(|i| i.after_login = path.to_string())
    }

    /// Where a form sign-out lands (default `/`).
    pub fn after_logout(self, path: &str) -> Self {
        self.edit(|i| i.after_logout = path.to_string())
    }

    /// At most `max` sign-ins, sign-ups and email requests per client IP per
    /// `window_secs` (default 10 a minute); `max = 0` turns the limit off.
    pub fn rate_limit(self, max: u32, window_secs: u64) -> Self {
        let limiter = (max > 0).then(|| Arc::new(RateLimiter::new(max, window_secs)));
        self.edit(|i| i.limiter = limiter)
    }

    /// Call `hook` with each new account, after it's created (a welcome
    /// email, a default workspace). Runs on the request's thread: enqueue
    /// anything slow.
    pub fn on_signup(self, hook: impl Fn(&AuthUser) + Send + Sync + 'static) -> Self {
        self.edit(|i| i.on_signup.push(Arc::new(hook)))
    }

    /// Render the pages from `engine`'s `auth/login.html`, `auth/signup.html`,
    /// `auth/forgot.html`, `auth/reset.html` and the emails from
    /// `auth/mail/confirm.{txt,html}`, `auth/mail/reset.{txt,html}` — each
    /// that exists; the built-in ones otherwise (`tera` feature).
    #[cfg(feature = "tera")]
    pub fn templates(self, engine: Arc<crate::template::TeraEngine>) -> Self {
        self.edit(|i| i.templates = Some(engine))
    }

    /// A session cookie for `user` (`Set-Cookie` value): sign someone in
    /// from your own handler.
    pub fn session_cookie(&self, user: &AuthUser) -> String {
        let i = &self.inner;
        let value = format!("s.{}.{}.{}", hex(user.id.as_bytes()), now() + i.session_ttl, fingerprint(&user.password_hash));
        let mut cookie = SetCookie::new(&i.cookie_name, signed_cookie(&value, &i.secret)).path("/").http_only().same_site("Lax").max_age(i.session_ttl as i64);
        if i.secure {
            cookie = cookie.secure();
        }
        cookie.build()
    }

    fn clear_cookie(&self) -> String {
        let mut cookie = SetCookie::new(&self.inner.cookie_name, "").path("/").http_only().same_site("Lax").max_age(0);
        if self.inner.secure {
            cookie = cookie.secure();
        }
        cookie.build()
    }

    /// The account `request`'s session cookie belongs to — valid signature,
    /// not expired, password unchanged since.
    pub fn session_user(&self, request: &Request) -> Option<AuthUser> {
        let header = request.header("cookie")?;
        let jar = CookieJar::parse(&header.value);
        let value = verify_signed_cookie(&jar.get(&self.inner.cookie_name)?.value, &self.inner.secret)?;
        let mut parts = value.split('.');
        let (Some("s"), Some(id), Some(expires), Some(fp), None) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()) else {
            return None;
        };
        if expires.parse::<u64>().ok()? < now() {
            return None;
        }
        let id = String::from_utf8(unhex(id)?).ok()?;
        let user = self.inner.store.find_by_id(&id).ok()??;
        (fingerprint(&user.password_hash) == fp).then_some(user)
    }

    /// A signed link token: `purpose`, the user, an expiry and a fingerprint.
    fn token(&self, purpose: &str, user: &AuthUser, ttl: u64) -> String {
        let bound = if purpose == "reset" { &user.password_hash } else { &user.email };
        let value = format!("{purpose}.{}.{}.{}", hex(user.id.as_bytes()), now() + ttl, fingerprint(bound));
        signed_cookie(&hex(value.as_bytes()), &self.inner.secret)
    }

    /// The account a valid, unexpired `purpose` token names.
    fn check_token(&self, purpose: &str, token: &str) -> Option<AuthUser> {
        let value = verify_signed_cookie(token.trim(), &self.inner.secret)?;
        let value = String::from_utf8(unhex(&value)?).ok()?;
        let mut parts = value.split('.');
        let (Some(p), Some(id), Some(expires), Some(fp), None) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()) else {
            return None;
        };
        if p != purpose || expires.parse::<u64>().ok()? < now() {
            return None;
        }
        let user = self.inner.store.find_by_id(&String::from_utf8(unhex(id)?).ok()?).ok()??;
        let bound = if purpose == "reset" { &user.password_hash } else { &user.email };
        (fingerprint(bound) == fp).then_some(user)
    }

    /// A link to `path` with the token for `purpose`.
    fn link(&self, path: &str, purpose: &str, user: &AuthUser, ttl: u64) -> String {
        let token = self.token(purpose, user, ttl);
        format!("{}{}{}?token={token}", self.inner.base_url, self.inner.prefix, path)
    }

    fn send(&self, user: &AuthUser, kind: &str, link: &str) {
        let i = &self.inner;
        let (subject, text) = match kind {
            "confirm" => (
                format!("Confirm your email for {}", i.app_name),
                format!("Confirm your email address for {}:\n\n{link}\n\nThe link works for 3 days.", i.app_name),
            ),
            _ => (
                format!("Reset your password for {}", i.app_name),
                format!("Choose a new password for {}:\n\n{link}\n\nThe link works for an hour. If you didn't ask for it, ignore this email.", i.app_name),
            ),
        };
        let builder = Email::builder().to(&user.email).subject(&subject);
        #[cfg(feature = "tera")]
        let builder = match &i.templates {
            Some(engine) if engine.has_template(&format!("auth/mail/{kind}.txt")) || engine.has_template(&format!("auth/mail/{kind}.html")) => {
                let mut ctx = crate::template::Context::new();
                ctx.insert("link", link);
                ctx.insert("app", &i.app_name);
                ctx.insert("email", &user.email);
                match builder.render_with(engine, &format!("auth/mail/{kind}"), &ctx) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("[auth] {kind} email template: {e}");
                        return;
                    }
                }
            }
            _ => builder.text(&text),
        };
        #[cfg(not(feature = "tera"))]
        let builder = builder.text(&text);
        let result = builder.build().and_then(|email| i.mailer.deliver(&Mail::new(email)));
        if let Err(e) = result {
            eprintln!("[auth] the {kind} email to {} failed: {e}", user.email);
        }
    }

    /// Over the rate limit for `action`, per client address: the peer, or
    /// behind a proxy listed in `trusted_proxies` the address it forwarded
    /// ([`ip_filter::client_ip`](crate::ip_filter::client_ip)).
    fn limited(&self, request: &Request, connection: &ConnectionInfo, action: &str) -> bool {
        match &self.inner.limiter {
            Some(limiter) => !limiter.check(&format!("{}|{action}", crate::ip_filter::client_ip(request, &connection.client.ip))),
            None => false,
        }
    }

    fn validate_email(email: &str) -> Result<String, String> {
        let email = email.trim().to_lowercase();
        let ok = email.len() <= 254
            && !email.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | ',' | ';' | '"'))
            && email.split_once('@').is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.'));
        if ok { Ok(email) } else { Err("that doesn't look like an email address".into()) }
    }

    fn validate_password(&self, password: &str) -> Result<(), String> {
        let n = password.chars().count();
        if n < self.inner.min_password {
            Err(format!("the password needs at least {} characters", self.inner.min_password))
        } else if password.len() > 1024 {
            Err("the password is too long".into())
        } else {
            Ok(())
        }
    }

    /// The request with the session's account (or none) in the headers the
    /// app reads, and any the client sent itself removed.
    fn with_identity(&self, request: &Request) -> Request {
        let mut request = request.clone();
        request.headers.retain(|h| !h.name.to_ascii_lowercase().starts_with("x-rws-auth-"));
        if let Some(user) = self.session_user(&request) {
            request.headers.push(Header::new(USER_ID_HEADER, user.id));
            request.headers.push(Header::new(USER_EMAIL_HEADER, user.email));
            request.headers.push(Header::new(USER_CONFIRMED_HEADER, if user.confirmed { "true" } else { "false" }));
            request.headers.push(Header::new(MARK_HEADER, mark()));
        }
        request
    }

    fn route(&self, request: &Request, connection: &ConnectionInfo) -> Option<Response> {
        let path = request.request_uri.split('?').next().unwrap_or("");
        let rest = path.strip_prefix(&self.inner.prefix)?;
        let rest = rest.trim_end_matches('/');
        let get = request.method == "GET" || request.method == "HEAD";
        let post = request.method == "POST";
        let input = Input::read(request);
        let ctx = Ctx { flows: self, request, json: wants_json(request), input: &input };
        Some(match (rest, get, post) {
            ("/signup", _, true) => ctx.signup(connection),
            ("/login", _, true) => ctx.login(connection),
            ("/logout", _, true) => ctx.logout(),
            ("/me", true, _) => ctx.me(),
            ("/password/forgot", _, true) => ctx.forgot(connection),
            ("/password/reset", _, true) => ctx.reset(),
            ("/confirm", true, _) | ("/confirm", _, true) => ctx.confirm(),
            ("/confirm/resend", _, true) => ctx.resend(connection),
            ("/login", true, _) | ("/signup", true, _) | ("/password/forgot", true, _) | ("/password/reset", true, _) => pages::page(self, request, rest),
            _ => return None,
        })
    }
}

/// Fields of a JSON object or an urlencoded form.
struct Input(HashMap<String, String>);

impl Input {
    fn read(request: &Request) -> Input {
        let mut fields = HashMap::new();
        if let Some((_, query)) = request.request_uri.split_once('?') {
            fields.extend(parse_form(query));
        }
        let content_type = request.header("content-type").map(|h| h.value.to_ascii_lowercase()).unwrap_or_default();
        if content_type.starts_with("application/json") {
            if let Ok(serde_json::Value::Object(map)) = serde_json::from_slice::<serde_json::Value>(&request.body) {
                for (k, v) in map {
                    if let serde_json::Value::String(s) = v {
                        fields.insert(k, s);
                    }
                }
            }
        } else if content_type.starts_with("application/x-www-form-urlencoded") {
            fields.extend(parse_form(&String::from_utf8_lossy(&request.body)));
        }
        Input(fields)
    }

    fn get(&self, name: &str) -> &str {
        self.0.get(name).map(String::as_str).unwrap_or("")
    }
}

fn parse_form(text: &str) -> Vec<(String, String)> {
    text.split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            Some((form_decode(k)?, form_decode(v)?))
        })
        .collect()
}

/// `application/x-www-form-urlencoded` decoding: `+` is a space, `%XX` a byte.
fn form_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                out.push(u8::from_str_radix(text.get(i + 1..i + 3)?, 16).ok()?);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn wants_json(request: &Request) -> bool {
    let content_type = request.header("content-type").map(|h| h.value.to_ascii_lowercase()).unwrap_or_default();
    if content_type.starts_with("application/json") {
        return true;
    }
    if content_type.starts_with("application/x-www-form-urlencoded") || content_type.starts_with("multipart/form-data") {
        return false;
    }
    let accept = request.header("accept").map(|h| h.value.to_ascii_lowercase()).unwrap_or_default();
    accept.contains("application/json") && !accept.contains("text/html")
}

fn user_json(user: &AuthUser) -> serde_json::Value {
    serde_json::json!({ "id": user.id, "email": user.email, "confirmed": user.confirmed })
}

fn json(status: &StatusCodeReasonPhrase, value: serde_json::Value) -> Response {
    Response::json(status, value.to_string().into_bytes())
}

fn problem(status: &StatusCodeReasonPhrase, detail: &str) -> Response {
    crate::error::problem::problem(status, detail)
}

/// A local path to go back to (not `//host`, not a URL).
fn local_path(path: &str) -> Option<&str> {
    (path.starts_with('/') && !path.starts_with("//") && !path.contains(['\\', '\r', '\n'])).then_some(path)
}

/// One request through a flow.
struct Ctx<'a> {
    flows: &'a AuthFlows,
    request: &'a Request,
    json: bool,
    input: &'a Input,
}

impl Ctx<'_> {
    fn i(&self) -> &Inner {
        &self.flows.inner
    }

    fn page(&self, page: &str) -> String {
        format!("{}{page}", self.i().prefix)
    }

    /// An error: problem details for JSON, a redirect to `back` with the
    /// message for forms.
    fn fail(&self, status: &StatusCodeReasonPhrase, message: &str, back: &str) -> Response {
        if self.json {
            problem(status, message)
        } else {
            let mut location = self.page(back);
            let return_to = self.input.get("return_to");
            if let Some(path) = local_path(return_to) {
                location = crate::router::build_path(&location, &[("return_to", path)]).unwrap_or(location);
            }
            Redirect::to(&location).alert(message).into_response()
        }
    }

    fn internal(&self, e: String) -> Response {
        eprintln!("[auth] {e}");
        problem(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, "something went wrong")
    }

    fn signed_in(&self, user: &AuthUser, status: &StatusCodeReasonPhrase, notice: &str) -> Response {
        let mut response = if self.json {
            json(status, serde_json::json!({ "user": user_json(user) }))
        } else {
            let target = local_path(self.input.get("return_to")).unwrap_or(&self.i().after_login);
            Redirect::to(target).notice(notice).into_response()
        };
        response.headers.push(Header::new(Header::_SET_COOKIE, self.flows.session_cookie(user)));
        response
    }

    fn too_many(&self) -> Response {
        let mut response = if self.json {
            problem(STATUS_CODE_REASON_PHRASE.n429_too_many_requests, "too many attempts; try again in a minute")
        } else {
            Response::text(STATUS_CODE_REASON_PHRASE.n429_too_many_requests, "Too many attempts; try again in a minute.")
        };
        response.headers.push(Header::new("Retry-After", "60"));
        response
    }

    fn signup(&self, connection: &ConnectionInfo) -> Response {
        if self.flows.limited(self.request, connection, "signup") {
            return self.too_many();
        }
        let email = match AuthFlows::validate_email(self.input.get("email")) {
            Ok(email) => email,
            Err(e) => return self.fail(STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity, &e, "/signup"),
        };
        let password = self.input.get("password");
        if let Err(e) = self.flows.validate_password(password) {
            return self.fail(STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity, &e, "/signup");
        }
        let hash = match crate::crypto::hash_password(password) {
            Ok(hash) => hash,
            Err(e) => return self.internal(e.to_string()),
        };
        let user = match self.i().store.create(&email, &hash) {
            Ok(Some(user)) => user,
            Ok(None) => return self.fail(STATUS_CODE_REASON_PHRASE.n409_conflict, "an account with that email already exists", "/signup"),
            Err(e) => return self.internal(e),
        };
        self.flows.send(&user, "confirm", &self.flows.link("/confirm", "confirm", &user, CONFIRM_TTL));
        for hook in &self.i().on_signup {
            hook(&user);
        }
        if self.i().require_confirmation {
            if self.json {
                json(STATUS_CODE_REASON_PHRASE.n201_created, serde_json::json!({ "user": user_json(&user), "confirmation_required": true }))
            } else {
                Redirect::to(&self.page("/login")).notice("Check your email to confirm your account.").into_response()
            }
        } else {
            self.signed_in(&user, STATUS_CODE_REASON_PHRASE.n201_created, "Welcome! We sent you an email to confirm your address.")
        }
    }

    fn login(&self, connection: &ConnectionInfo) -> Response {
        if self.flows.limited(self.request, connection, "login") {
            return self.too_many();
        }
        let email = self.input.get("email").trim().to_lowercase();
        let password = self.input.get("password");
        let user = match self.i().store.find_by_email(&email) {
            Ok(user) => user,
            Err(e) => return self.internal(e),
        };
        let valid = match &user {
            Some(user) => crate::crypto::verify_password(password, &user.password_hash).unwrap_or(false),
            None => {
                // The same work as a real check, so timing doesn't tell
                // which emails have accounts.
                let _ = crate::crypto::verify_password(password, dummy_hash());
                false
            }
        };
        let Some(user) = user.filter(|_| valid) else {
            return self.fail(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "wrong email or password", "/login");
        };
        if self.i().require_confirmation && !user.confirmed {
            return self.fail(STATUS_CODE_REASON_PHRASE.n403_forbidden, "please confirm your email first (we can send the link again)", "/login");
        }
        self.signed_in(&user, STATUS_CODE_REASON_PHRASE.n200_ok, "Signed in.")
    }

    fn logout(&self) -> Response {
        let mut response = if self.json {
            Response::get_response(STATUS_CODE_REASON_PHRASE.n204_no_content, None, None)
        } else {
            Redirect::to(&self.i().after_logout).notice("Signed out.").into_response()
        };
        response.headers.push(Header::new(Header::_SET_COOKIE, self.flows.clear_cookie()));
        response
    }

    fn me(&self) -> Response {
        match self.flows.session_user(self.request) {
            Some(user) => json(STATUS_CODE_REASON_PHRASE.n200_ok, serde_json::json!({ "user": user_json(&user) })),
            None => problem(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "not signed in"),
        }
    }

    fn accepted(&self, notice: &str, back: &str) -> Response {
        if self.json {
            json(STATUS_CODE_REASON_PHRASE.n202_accepted, serde_json::json!({ "status": "accepted" }))
        } else {
            Redirect::to(&self.page(back)).notice(notice).into_response()
        }
    }

    fn forgot(&self, connection: &ConnectionInfo) -> Response {
        if self.flows.limited(self.request, connection, "email") {
            return self.too_many();
        }
        let email = self.input.get("email").trim().to_lowercase();
        match self.i().store.find_by_email(&email) {
            Ok(Some(user)) => self.flows.send(&user, "reset", &self.flows.link("/password/reset", "reset", &user, RESET_TTL)),
            Ok(None) => {}
            Err(e) => return self.internal(e),
        }
        // The same answer either way: it doesn't tell who has an account.
        self.accepted("If that email has an account, we sent it a link to reset the password.", "/login")
    }

    fn reset(&self) -> Response {
        let Some(user) = self.flows.check_token("reset", self.input.get("token")) else {
            return self.fail(STATUS_CODE_REASON_PHRASE.n400_bad_request, "that link has expired or was already used; ask for a new one", "/password/forgot");
        };
        let password = self.input.get("password");
        if let Err(e) = self.flows.validate_password(password) {
            if self.json {
                return problem(STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity, &e);
            }
            let back = format!("{}?token={}", self.page("/password/reset"), self.input.get("token"));
            return Redirect::to(&back).alert(&e).into_response();
        }
        let hash = match crate::crypto::hash_password(password) {
            Ok(hash) => hash,
            Err(e) => return self.internal(e.to_string()),
        };
        if let Err(e) = self.i().store.set_password_hash(&user.id, &hash) {
            return self.internal(e);
        }
        // The link reached their inbox: the address is theirs.
        if !user.confirmed {
            let _ = self.i().store.confirm(&user.id);
        }
        let user = AuthUser { password_hash: hash, confirmed: true, ..user };
        self.signed_in(&user, STATUS_CODE_REASON_PHRASE.n200_ok, "Your password was changed.")
    }

    fn confirm(&self) -> Response {
        let Some(user) = self.flows.check_token("confirm", self.input.get("token")) else {
            return self.fail(STATUS_CODE_REASON_PHRASE.n400_bad_request, "that confirmation link has expired; ask for a new one", "/login");
        };
        if let Err(e) = self.i().store.confirm(&user.id) {
            return self.internal(e);
        }
        let user = AuthUser { confirmed: true, ..user };
        if self.json {
            json(STATUS_CODE_REASON_PHRASE.n200_ok, serde_json::json!({ "user": user_json(&user) }))
        } else {
            let target = if self.flows.session_user(self.request).is_some() { self.i().after_login.clone() } else { self.page("/login") };
            Redirect::to(&target).notice("Your email is confirmed.").into_response()
        }
    }

    fn resend(&self, connection: &ConnectionInfo) -> Response {
        if self.flows.limited(self.request, connection, "email") {
            return self.too_many();
        }
        let email = self.input.get("email").trim().to_lowercase();
        match self.i().store.find_by_email(&email) {
            Ok(Some(user)) if !user.confirmed => self.flows.send(&user, "confirm", &self.flows.link("/confirm", "confirm", &user, CONFIRM_TTL)),
            Ok(_) => {}
            Err(e) => return self.internal(e),
        }
        self.accepted("If that email has an unconfirmed account, we sent it a new link.", "/login")
    }
}

/// An Argon2id hash of nothing anyone types, for timing-equal failed logins.
fn dummy_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| crate::crypto::hash_password(&crate::crypto::generate_token(16)).unwrap_or_default())
}

impl Middleware for AuthFlows {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        if let Some(response) = self.route(request, connection) {
            return Ok(response);
        }
        next.execute(&self.with_identity(request), connection)
    }

    /// Nothing to check or strip: no cookie of ours, no identity headers,
    /// not a flow path. Async handlers then stay async.
    fn is_passthrough(&self, request: &Request, _connection: &ConnectionInfo) -> bool {
        let ours = request.request_uri.starts_with(&self.inner.prefix);
        let spoofed = request.headers.iter().any(|h| h.name.to_ascii_lowercase().starts_with("x-rws-auth-"));
        let cookie = request.header("cookie").is_some_and(|h| CookieJar::parse(&h.value).get(&self.inner.cookie_name).is_some());
        !(ours || spoofed || cookie)
    }

    fn layer_name(&self) -> &'static str {
        "AuthFlows"
    }
}
