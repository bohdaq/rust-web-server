//! `OidcAuth` middleware — full OIDC / OAuth 2.0 authorization code flow with
//! PKCE, session management, and claim injection.
//!
//! # Usage
//!
//! ```rust,no_run
//! # extern crate rws_server as rust_web_server;
//! use std::sync::Arc;
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::session::SessionStore;
//! use rust_web_server::sso::{OidcAuth, OidcConfig};
//!
//! let sessions = Arc::new(SessionStore::new(86400));
//! let config   = OidcConfig::google("client-id", "client-secret", "https://example.com/auth/callback");
//! let app      = App::new().wrap(OidcAuth::new(config, sessions));
//! ```
//!
//! The middleware intercepts three paths:
//! - `GET /auth/login`    — generates PKCE + state + nonce, stores them in a
//!   pre-auth session, and redirects the user to the identity provider.
//! - `GET /auth/callback` — validates state, exchanges the code, verifies the
//!   id_token (or fetches UserInfo for GitHub), stores claims in the session,
//!   and redirects to the app.
//! - `GET /auth/logout`   — destroys the session and redirects to `/`.
//!
//! On all other paths the middleware checks the session for `_oidc_claims`.
//! If found, the claims are injected into the request as the
//! `X-Rws-Oidc-Claims` header (JSON) and the request is passed to the next
//! layer.  If not found, the user is redirected to `/auth/login`.
//!
//! Use [`OidcAuth::claims`] inside a handler to read the injected claims.
//!
//! Since 18.21.0 (#73):
//! - [`OidcAuth::signed_cookies`] keeps the session in an HMAC-signed cookie
//!   instead of the [`SessionStore`], so sessions survive a restart and work
//!   across replicas;
//! - [`OidcAuth::allow_emails`] / [`allow_domains`](OidcAuth::allow_domains) /
//!   [`allow_groups`](OidcAuth::allow_groups) admit only some users (a
//!   verified email, its domain, or a group in the ID token);
//! - [`OidcAuth::forward_headers`] sends `X-Auth-Request-User`, `-Email` and
//!   `-Groups` upstream (the names oauth2-proxy uses);
//! - an empty `redirect_uri` means `<scheme>://<Host><callback_path>` of the
//!   request;
//! - claim headers a client sends itself are always removed.

use std::sync::Arc;

use rand_core::{OsRng, RngCore};
use serde_json;

use crate::application::Application;
use crate::core::New;
use crate::error::{AppError, IntoResponse};
use crate::header::Header;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::session::{destroy_cookie, session_cookie, session_id_from_request, SessionStore};

use super::{
    client::OidcClient,
    config::OidcConfig,
    jwks::{JwksCache, OidcClaims, VerifyOptions},
    pkce::PkceVerifier,
};

const SESSION_COOKIE: &str = "_rws_sid";
const SESSION_TTL:    u64  = 86_400; // 24 h
/// Request header name used to pass OIDC claims into downstream handlers.
pub const CLAIMS_HEADER: &str = "X-Rws-Oidc-Claims";
const LOGIN_PATH:    &str = "/auth/login";
const CALLBACK_PATH: &str = "/auth/callback";
const LOGOUT_PATH:   &str = "/auth/logout";
const SIGNED_COOKIE:   &str = "_rws_oidc";
const PRE_AUTH_COOKIE: &str = "_rws_oidc_pre";
const PRE_AUTH_TTL:    u64  = 600;

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A signed-in user ([`OidcAuth::signed_in`]).
#[derive(Debug, Clone, PartialEq)]
pub struct SignedIn {
    pub sub: String,
    /// Only when the provider says it's verified.
    pub email: Option<String>,
    pub name: Option<String>,
    pub groups: Vec<String>,
}

/// Who signed in: the claims kept for a session.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Identity {
    sub: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(default)]
    email_verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    groups: Vec<String>,
}

impl Identity {
    fn from_claims(claims: &OidcClaims, groups: Vec<String>) -> Identity {
        Identity {
            sub: claims.sub.clone(),
            email: claims.email.clone(),
            // Providers that don't say (GitHub's verified primary email) count as verified.
            email_verified: claims.email_verified.unwrap_or(true),
            name: claims.name.clone(),
            groups,
        }
    }

    fn from_claims_json(json: &str) -> Option<Identity> {
        let claims: OidcClaims = serde_json::from_str(json).ok()?;
        Some(Identity::from_claims(&claims, Vec::new()))
    }

    /// The `X-Rws-Oidc-Claims` value: the [`OidcClaims`] fields known.
    fn claims_json(&self) -> String {
        serde_json::json!({
            "sub": self.sub, "iss": "", "aud": [], "exp": 0, "iat": 0,
            "email": self.email, "email_verified": self.email_verified, "name": self.name,
        })
        .to_string()
    }
}

/// The groups claim of an (already verified) ID token: an array of strings,
/// or one string.
fn groups_claim(id_token: &str, claim: &str) -> Vec<String> {
    let Some(payload) = id_token.split('.').nth(1).and_then(|p| super::pkce::base64url_decode(p).ok()) else { return Vec::new() };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&payload) else { return Vec::new() };
    match value.get(claim) {
        Some(serde_json::Value::Array(items)) => items.iter().filter_map(|g| g.as_str().map(str::to_string)).collect(),
        Some(serde_json::Value::String(g)) => vec![g.clone()],
        _ => Vec::new(),
    }
}

fn cookie_value(request: &Request, name: &str) -> Option<String> {
    request.headers.iter().filter(|h| h.name.eq_ignore_ascii_case("cookie")).flat_map(|h| h.value.split(';').map(str::trim).map(str::to_string).collect::<Vec<_>>()).find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k.trim() == name && !v.is_empty()).then(|| v.trim().to_string())
    })
}

/// OIDC / OAuth 2.0 authentication middleware.
///
/// See the [module documentation](self) for a usage example.
pub struct OidcAuth {
    config:        Arc<OidcConfig>,
    signed:        Option<Arc<Vec<u8>>>,
    ttl:           u64,
    allow_emails:  Vec<String>,
    allow_domains: Vec<String>,
    allow_groups:  Vec<String>,
    groups_claim:  String,
    forward:       bool,
    jwks:          Option<Arc<JwksCache>>,
    client:        OidcClient,
    sessions:      Arc<SessionStore>,
    excluded:      Vec<String>,
    login_path:    String,
    callback_path: String,
    logout_path:   String,
}

impl OidcAuth {
    /// Create a new `OidcAuth` middleware.
    ///
    /// `sessions` is the session store shared with the application.
    pub fn new(config: OidcConfig, sessions: Arc<SessionStore>) -> Self {
        let jwks = if !config.provider.jwks_uri.is_empty() {
            Some(Arc::new(JwksCache::new(&config.provider.jwks_uri)))
        } else {
            None
        };
        let config = Arc::new(config);
        let client = OidcClient::new((*config).clone());
        OidcAuth {
            config,
            signed:        None,
            ttl:           SESSION_TTL,
            allow_emails:  Vec::new(),
            allow_domains: Vec::new(),
            allow_groups:  Vec::new(),
            groups_claim:  "groups".into(),
            forward:       false,
            jwks,
            client,
            sessions,
            excluded:      Vec::new(),
            login_path:    LOGIN_PATH.into(),
            callback_path: CALLBACK_PATH.into(),
            logout_path:   LOGOUT_PATH.into(),
        }
    }

    /// Exclude a path prefix from authentication checks.
    ///
    /// Requests whose path starts with `prefix` bypass the session check and
    /// are passed directly to the next layer.  Use this for public paths like
    /// `/healthz` or `/public/`.
    pub fn exclude(mut self, prefix: &str) -> Self {
        self.excluded.push(prefix.into());
        self
    }

    /// Override the login redirect path (default: `"/auth/login"`).
    pub fn login_path(mut self, path: &str) -> Self {
        self.login_path = path.into();
        self
    }

    /// Override the callback path (default: `"/auth/callback"`).
    pub fn callback_path(mut self, path: &str) -> Self {
        self.callback_path = path.into();
        self
    }

    /// Override the logout path (default: `"/auth/logout"`).
    pub fn logout_path(mut self, path: &str) -> Self {
        self.logout_path = path.into();
        self
    }

    /// Keep sessions in an HMAC-SHA256-signed cookie (`_rws_oidc`) instead
    /// of the [`SessionStore`]: nothing is held in memory, so sessions
    /// survive a restart and every replica sharing `secret` accepts them.
    pub fn signed_cookies(mut self, secret: impl Into<Vec<u8>>) -> Self {
        self.signed = Some(Arc::new(secret.into()));
        self
    }

    /// How long a sign-in lasts (default 24 h).
    pub fn session_ttl(mut self, secs: u64) -> Self {
        self.ttl = secs.max(60);
        self
    }

    /// Admit users with one of these (verified) email addresses.
    pub fn allow_emails(mut self, emails: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allow_emails.extend(emails.into_iter().map(|e| e.into().trim().to_ascii_lowercase()).filter(|e| !e.is_empty()));
        self
    }

    /// Admit users whose (verified) email is in one of these domains.
    pub fn allow_domains(mut self, domains: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allow_domains.extend(domains.into_iter().map(|d| d.into().trim().trim_start_matches('@').to_ascii_lowercase()).filter(|d| !d.is_empty()));
        self
    }

    /// Admit users with one of these groups in the ID token's `claim`
    /// (`groups` by default; Keycloak, Okta, Azure AD and Authentik send one).
    pub fn allow_groups(mut self, claim: &str, groups: impl IntoIterator<Item = impl Into<String>>) -> Self {
        if !claim.is_empty() {
            self.groups_claim = claim.to_string();
        }
        self.allow_groups.extend(groups.into_iter().map(Into::into).filter(|g: &String| !g.is_empty()));
        self
    }

    /// Send `X-Auth-Request-User` (sub), `X-Auth-Request-Email` and
    /// `X-Auth-Request-Groups` to the next layer.
    pub fn forward_headers(mut self, on: bool) -> Self {
        self.forward = on;
        self
    }

    /// Who is signed in on `request` (a valid session cookie), without
    /// redirecting: for callers that check identity themselves, like the
    /// admin API (18.22.0). Allow-lists are not applied.
    pub fn signed_in(&self, request: &Request) -> Option<SignedIn> {
        self.identity(request).map(|i| SignedIn { sub: i.sub, email: i.email.filter(|_| i.email_verified), name: i.name, groups: i.groups })
    }

    /// Read the OIDC claims injected by `OidcAuth` from a request.
    ///
    /// Returns `None` if the request did not pass through an authenticated
    /// `OidcAuth` layer.
    pub fn claims(req: &Request) -> Option<OidcClaims> {
        req.headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(CLAIMS_HEADER))
            .and_then(|h| serde_json::from_str(&h.value).ok())
    }

    /// Shortcut: return the `sub` (subject / user ID) from the injected claims.
    pub fn sub(req: &Request) -> Option<String> {
        Self::claims(req).map(|c| c.sub)
    }

    /// Shortcut: return the `email` from the injected claims.
    pub fn email(req: &Request) -> Option<String> {
        Self::claims(req).and_then(|c| c.email)
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn random_token() -> String {
        let mut bytes = [0u8; 16];
        OsRng.fill_bytes(&mut bytes);
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn redirect(url: &str) -> Response {
        let mut r = Response::new();
        r.status_code  = *STATUS_CODE_REASON_PHRASE.n302_found.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n302_found.reason_phrase.to_string();
        r.headers.push(Header { name: "Location".into(), value: url.to_string().into() });
        r
    }

    /// Whether the allow-lists admit `identity` (no lists: everyone).
    fn allowed(&self, identity: &Identity) -> bool {
        if self.allow_emails.is_empty() && self.allow_domains.is_empty() && self.allow_groups.is_empty() {
            return true;
        }
        let email = identity.email.as_deref().filter(|_| identity.email_verified).map(str::to_ascii_lowercase);
        if let Some(email) = &email {
            if self.allow_emails.iter().any(|e| e == email) {
                return true;
            }
            if let Some((_, domain)) = email.rsplit_once('@') {
                if self.allow_domains.iter().any(|d| d == domain) {
                    return true;
                }
            }
        }
        identity.groups.iter().any(|g| self.allow_groups.contains(g))
    }

    /// The redirect URI for this request: the configured one, else
    /// `<scheme>://<Host><callback_path>`.
    fn redirect_uri(&self, request: &Request, connection: &ConnectionInfo) -> String {
        if !self.config.redirect_uri.is_empty() {
            return self.config.redirect_uri.clone();
        }
        let host = request.header("Host").map(|h| h.value.to_string()).unwrap_or_else(|| "localhost".into());
        // Behind a TLS-terminating load balancer the connection is plain
        // HTTP: the scheme from a trusted proxy's X-Forwarded-Proto.
        format!("{}://{}{}", crate::ip_filter::request_scheme(request, &connection.client.ip, connection.tls), host, self.callback_path)
    }

    fn client_for(&self, redirect_uri: &str) -> OidcClient {
        if redirect_uri == self.config.redirect_uri {
            return self.client.clone();
        }
        let mut config = (*self.config).clone();
        config.redirect_uri = redirect_uri.to_string();
        OidcClient::new(config)
    }

    fn cookie(&self, name: &str, value: &str, max_age: u64, request: &Request, connection: &ConnectionInfo) -> Header {
        // Secure when the client is on HTTPS, a TLS-terminating proxy's too.
        let secure = if crate::ip_filter::request_scheme(request, &connection.client.ip, connection.tls) == "https" { "; Secure" } else { "" };
        Header { name: "Set-Cookie".into(), value: format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}").into() }
    }

    fn sign(&self, payload: &str) -> String {
        let secret = self.signed.as_deref().map(Vec::as_slice).unwrap_or_default();
        let body = super::pkce::base64url_encode(payload.as_bytes());
        let mac = crate::core::sha256::hmac_sha256(secret, body.as_bytes());
        format!("{body}.{}", super::pkce::base64url_encode(&mac))
    }

    /// The payload of a cookie [`sign`](Self::sign) made, if the MAC and its
    /// `exp` hold.
    fn verify_signed(&self, value: &str) -> Option<serde_json::Value> {
        let secret = self.signed.as_deref()?;
        let (body, mac) = value.split_once('.')?;
        let expected = crate::core::sha256::hmac_sha256(secret, body.as_bytes());
        let mac = super::pkce::base64url_decode(mac).ok()?;
        if !crate::core::constant_time_eq(&mac, &expected) {
            return None;
        }
        let payload: serde_json::Value = serde_json::from_slice(&super::pkce::base64url_decode(body).ok()?).ok()?;
        let exp = payload.get("exp")?.as_u64()?;
        (exp > unix_now()).then_some(payload)
    }

    /// The signed-in identity of a request, from the cookie or the session.
    fn identity(&self, request: &Request) -> Option<Identity> {
        if self.signed.is_some() {
            let value = cookie_value(request, SIGNED_COOKIE)?;
            let payload = self.verify_signed(&value)?;
            return serde_json::from_value(payload.get("id")?.clone()).ok();
        }
        let sid = session_id_from_request(request, SESSION_COOKIE)?;
        let session = self.sessions.load(&sid)?;
        let claims = session.get("_oidc_claims")?;
        let mut identity = Identity::from_claims_json(claims)?;
        identity.groups = session.get("_oidc_groups").and_then(|g| serde_json::from_str(g).ok()).unwrap_or_default();
        Some(identity)
    }

    fn forbidden(identity: &Identity, logout: &str) -> Response {
        let who = identity.email.clone().unwrap_or_else(|| identity.sub.clone());
        let mut r = crate::error::problem::problem_with_text(
            STATUS_CODE_REASON_PHRASE.n403_forbidden,
            &format!("{who} is signed in but not allowed here. Sign out at {logout}."),
            "signed in, but not allowed",
        );
        r.headers.push(Header { name: "Cache-Control".into(), value: "no-store".into() });
        r
    }

    fn is_excluded(&self, path: &str) -> bool {
        self.excluded.iter().any(|p| path.starts_with(p.as_str()))
    }

    // ── route handlers ────────────────────────────────────────────────────────

    fn handle_login(&self, request: &Request, connection: &ConnectionInfo) -> Response {
        let verifier = PkceVerifier::new();
        let state    = Self::random_token();
        let nonce    = Self::random_token();
        let redirect_uri = self.redirect_uri(request, connection);
        let return_to = query_param(request, "return_to")
            .filter(|r| local_path(r))
            .unwrap_or_else(|| self.config.post_login_redirect.clone());

        if self.signed.is_some() {
            let pre = serde_json::json!({
                "state": state, "nonce": nonce, "pkce": verifier.as_str(),
                "return_to": return_to, "redirect_uri": redirect_uri, "exp": unix_now() + PRE_AUTH_TTL,
            });
            let url = self.client_for(&redirect_uri).authorization_url(&verifier, &state, &nonce);
            let mut response = Self::redirect(&url);
            response.headers.push(self.cookie(PRE_AUTH_COOKIE, &self.sign(&pre.to_string()), PRE_AUTH_TTL, request, connection));
            response.headers.push(Header { name: "Cache-Control".into(), value: "no-store".into() });
            return response;
        }

        let mut session = self.sessions.create();
        session.set("_oidc_state",     &state);
        session.set("_oidc_nonce",     &nonce);
        session.set("_oidc_pkce",      verifier.as_str());

        session.set("_oidc_return_to", &return_to);
        session.set("_oidc_redirect_uri", &redirect_uri);
        self.sessions.save(&session);

        let url = self.client_for(&redirect_uri).authorization_url(&verifier, &state, &nonce);
        let mut response = Self::redirect(&url);
        response.headers.push(Header {
            name:  "Set-Cookie".into(),
            value: session_cookie(&session.id, SESSION_COOKIE, SESSION_TTL).into(),
        });
        response
    }

    fn handle_callback(&self, request: &Request, connection: &ConnectionInfo) -> Response {
        // The pre-auth values: from the signed cookie, or the session.
        let mut session = None;
        let pre: serde_json::Value = if self.signed.is_some() {
            match cookie_value(request, PRE_AUTH_COOKIE).and_then(|v| self.verify_signed(&v)) {
                Some(p) => p,
                None => return AppError::Forbidden.into_response(),
            }
        } else {
            let sid = match session_id_from_request(request, SESSION_COOKIE) {
                Some(id) => id,
                None     => return AppError::Forbidden.into_response(),
            };
            let s = match self.sessions.load(&sid) {
                Some(s) => s,
                None    => return AppError::Forbidden.into_response(),
            };
            let get = |k: &str| serde_json::Value::String(s.get(k).unwrap_or("").to_string());
            let pre = serde_json::json!({
                "state": get("_oidc_state"), "nonce": get("_oidc_nonce"), "pkce": get("_oidc_pkce"),
                "return_to": s.get("_oidc_return_to").unwrap_or("/"), "redirect_uri": get("_oidc_redirect_uri"),
            });
            session = Some(s);
            pre
        };
        let field = |k: &str| pre.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();

        // Validate state
        let stored_state   = field("state");
        let received_state = query_param(request, "state").unwrap_or_default();
        if stored_state.is_empty() || stored_state != received_state {
            return AppError::Forbidden.into_response();
        }

        // Check for provider error
        let code = match query_param(request, "code") {
            Some(c) => c,
            None => {
                let error = query_param(request, "error").unwrap_or_else(|| "unknown".into());
                return error_response(&format!("OAuth2 error: {error}"));
            }
        };

        let pkce_verifier = field("pkce");
        let stored_nonce  = field("nonce");
        let return_to     = Some(field("return_to")).filter(|r| !r.is_empty()).unwrap_or_else(|| "/".into());
        let redirect_uri  = Some(field("redirect_uri")).filter(|r| !r.is_empty()).unwrap_or_else(|| self.config.redirect_uri.clone());

        // Exchange code for tokens
        let tokens = match self.client_for(&redirect_uri).exchange_code(&code, &pkce_verifier) {
            Ok(t)  => t,
            Err(e) => return error_response(&e.0),
        };

        // Get claims — prefer id_token (OIDC), fall back to UserInfo for GitHub
        let claims = if let (Some(id_token), Some(jwks)) = (&tokens.id_token, &self.jwks) {
            let opts = VerifyOptions {
                audience:    &self.config.client_id,
                issuer:      &self.config.provider.issuer,
                leeway_secs: 60,
            };
            match jwks.verify_jwt(id_token, &opts) {
                Ok(c) => {
                    if !stored_nonce.is_empty() && c.nonce.as_deref() != Some(&stored_nonce) {
                        return AppError::Forbidden.into_response();
                    }
                    c
                }
                Err(e) => return error_response(&e.0),
            }
        } else {
            match self.client.fetch_user_info(&tokens.access_token) {
                Ok(c)  => c,
                Err(e) => return error_response(&e.0),
            }
        };

        let groups = tokens.id_token.as_deref().map(|t| groups_claim(t, &self.groups_claim)).unwrap_or_default();
        let identity = Identity::from_claims(&claims, groups);
        if !self.allowed(&identity) {
            return Self::forbidden(&identity, &self.logout_path);
        }

        let Some(mut session) = session else {
            // Signed cookies: the identity (groups kept to the allowed ones,
            // so the cookie stays small) and an expiry.
            let mut stored = identity.clone();
            if !self.allow_groups.is_empty() {
                stored.groups.retain(|g| self.allow_groups.contains(g));
            }
            stored.groups.truncate(32);
            let payload = serde_json::json!({ "id": stored, "exp": unix_now() + self.ttl });
            let mut response = Self::redirect(&return_to);
            response.headers.push(self.cookie(SIGNED_COOKIE, &self.sign(&payload.to_string()), self.ttl, request, connection));
            response.headers.push(self.cookie(PRE_AUTH_COOKIE, "", 0, request, connection));
            return response;
        };

        // Promote session: clear pre-auth keys, store claims
        session.remove("_oidc_state");
        session.remove("_oidc_nonce");
        session.remove("_oidc_pkce");
        session.remove("_oidc_return_to");
        session.remove("_oidc_redirect_uri");
        let claims_json = serde_json::to_string(&claims).unwrap_or_default();
        session.set("_oidc_claims", &claims_json);
        session.set("_oidc_groups", &serde_json::to_string(&identity.groups).unwrap_or_default());
        self.sessions.save(&session);

        let mut response = Self::redirect(&return_to);
        response.headers.push(Header {
            name:  "Set-Cookie".into(),
            value: session_cookie(&session.id, SESSION_COOKIE, self.ttl).into(),
        });
        response
    }

    fn handle_logout(&self, request: &Request, connection: &ConnectionInfo) -> Response {
        if self.signed.is_some() {
            let mut response = Self::redirect(&self.config.post_login_redirect);
            response.headers.push(self.cookie(SIGNED_COOKIE, "", 0, request, connection));
            return response;
        }
        if let Some(sid) = session_id_from_request(request, SESSION_COOKIE) {
            self.sessions.destroy(&sid);
        }
        let mut response = Self::redirect("/");
        response.headers.push(Header {
            name:  "Set-Cookie".into(),
            value: destroy_cookie(SESSION_COOKIE).into(),
        });
        response
    }
}

impl Middleware for OidcAuth {
    fn handle(
        &self,
        request: &Request,
        connection: &ConnectionInfo,
        next: &dyn Application,
    ) -> Result<Response, String> {
        let path = request.request_uri.split('?').next().unwrap_or("");

        // Handle built-in auth routes first
        if path == self.login_path {
            return Ok(self.handle_login(request, connection));
        }
        if path == self.callback_path {
            return Ok(self.handle_callback(request, connection));
        }
        if path == self.logout_path {
            return Ok(self.handle_logout(request, connection));
        }

        // Identity headers only ever come from this layer: drop a client's.
        let spoofed = |name: &str| name.eq_ignore_ascii_case(CLAIMS_HEADER) || name.len() > 15 && name[..15].eq_ignore_ascii_case("x-auth-request-");
        let mut req = request.clone();
        req.headers.retain(|h| !spoofed(&h.name));

        // Excluded paths bypass auth
        if self.is_excluded(path) {
            return next.execute(&req, connection);
        }

        if let Some(identity) = self.identity(request) {
            if !self.allowed(&identity) {
                return Ok(Self::forbidden(&identity, &self.logout_path));
            }
            let claims_json = identity.claims_json();
            req.headers.push(Header { name: CLAIMS_HEADER.to_string().into(), value: claims_json.clone().into() });
            if self.forward {
                req.headers.push(Header { name: "X-Auth-Request-User".into(), value: identity.sub.clone().into() });
                if let Some(email) = &identity.email {
                    req.headers.push(Header { name: "X-Auth-Request-Email".into(), value: email.clone().into() });
                }
                if !identity.groups.is_empty() {
                    req.headers.push(Header { name: "X-Auth-Request-Groups".into(), value: identity.groups.join(",").into() });
                }
            }
            return match crate::authz::Identity::from_claims(&claims_json, "sub", "oidc") {
                Some(authz) => crate::authz::with_identity(authz, || next.execute(&req, connection)),
                None => next.execute(&req, connection),
            };
        }

        // No valid session — redirect to login
        let return_to = super::client::url_encode(&request.request_uri);
        let login_url = format!("{}?return_to={}", self.login_path, return_to);
        Ok(Self::redirect(&login_url))
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn query_param(request: &Request, name: &str) -> Option<String> {
    let uri = &request.request_uri;
    let qs  = uri.splitn(2, '?').nth(1)?;
    for pair in qs.split('&') {
        let mut parts = pair.splitn(2, '=');
        let k = parts.next()?.trim();
        if k == name {
            let v = parts.next().unwrap_or("").trim();
            return Some(crate::url::URL::form_decode(v));
        }
    }
    None
}


/// A `return_to` that stays on this site: a path starting with one `/`, no
/// backslash and no control or whitespace characters (browsers drop tabs
/// and newlines from URLs, so `/\t/evil.example` would become
/// `//evil.example`, 18.24.0).
fn local_path(r: &str) -> bool {
    r.starts_with('/') && !r.starts_with("//") && !r.contains('\\') && !r.chars().any(|c| c.is_control() || c.is_whitespace())
}

fn error_response(msg: &str) -> Response {
    crate::error::problem::problem_with_text(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, msg, msg)
}
