//! `AuthServer` — a minimal OAuth 2.0 Authorization Server / token issuer.
//!
//! Lets `rws` be the identity provider for downstream services or
//! single-page apps, rather than always delegating to an external OIDC
//! provider (that's [`super::oidc_auth::OidcAuth`]'s job — this is the
//! reverse role).
//!
//! # Usage
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::session::SessionStore;
//! use rust_web_server::sso::server::{AuthServer, AuthServerConfig};
//! use rust_web_server::sso::client_store::{ClientStore, OAuthClient, GrantType};
//!
//! let auth_server = AuthServer::new(AuthServerConfig {
//!     issuer:            "https://myapp.com".into(),
//!     signing_secret:    std::env::var("RWS_AUTH_SIGNING_SECRET").unwrap(),
//!     access_token_ttl:  Duration::from_secs(3600),
//!     refresh_token_ttl: Duration::from_secs(86_400 * 30),
//!     clients: ClientStore::new().add(OAuthClient {
//!         client_id:     "backend-service".into(),
//!         client_secret: Some("s3cr3t".into()),
//!         redirect_uris: vec![],
//!         grants:        vec![GrantType::ClientCredentials],
//!         scopes:        vec!["api:read".into()],
//!     }),
//!     sessions: Arc::new(SessionStore::new(86_400)),
//! });
//!
//! let app = App::new().wrap(auth_server);
//! // Registers, and intercepts: POST /oauth/token, GET /oauth/authorize,
//! // GET /.well-known/openid-configuration, GET /.well-known/jwks.json
//! ```
//!
//! # Deviations from a full OAuth 2.0 Authorization Server
//!
//! - **HS256, not RSA/EC.** Tokens are signed with the existing
//!   [`crate::auth::build_jwt`]/[`crate::auth::verify_jwt`] HS256 machinery
//!   (`auth` feature) rather than an RSA/EC private key loaded from PEM.
//!   This crate has no PEM/DER private-key parser; adding one — plus the
//!   signing-side counterpart to [`super::jwks::JwksCache`]'s
//!   verification-only RSA/ES code — would be a large addition for a
//!   feature that already has a fully adequate, already-tested symmetric
//!   alternative when the issuer and every resource server share one
//!   secret (the common case for "rws issues its own tokens"). Consequently
//!   `GET /.well-known/jwks.json` always returns an empty key set — there
//!   is no public key to publish for a symmetric algorithm. Resource
//!   servers must be configured with the same `signing_secret` directly
//!   (e.g. `auth::JwtLayer` or `auth::verify_jwt`).
//! - **`/oauth/authorize` trusts an existing application session**, not a
//!   built-in login page. The spec for this feature says "redirect to
//!   login page or IdP, then back with code" but building a username/
//!   password login form is out of scope (the same boundary
//!   [`super::oidc_auth::OidcAuth`] draws around needing an *external* IdP
//!   to already exist). `AuthServer` instead checks the browser's session
//!   (via the existing [`crate::session::SessionStore`]) for a configurable
//!   key (default `"user_id"`, override with
//!   [`AuthServer::subject_session_key`]) — if present, that value becomes
//!   the issued token's `sub` claim and a code is minted immediately; if
//!   absent, the browser is redirected to a configurable `login_url`
//!   (default `/login`, override with [`AuthServer::login_url`]) with
//!   `?return_to=<original authorize URL>`. Populating that session is the
//!   embedding application's own responsibility.
//! - **No `ClientStore::from_env()`/`::from_db()`.** Client secrets belong
//!   in code or a real secret store, not flat env vars — and unlike
//!   [`super::config::OidcConfig`] (one client per process), a `ClientStore`
//!   is a *list* of clients with no natural flat-env-var encoding. The
//!   builder (`ClientStore::new().add(...)`) already covers registering
//!   clients from wherever the caller wants to load them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::application::Application;
use crate::body::form_urlencoded::FormUrlEncoded;
use crate::core::New;
use crate::extract::{FromRequest, Query};
use crate::header::Header;
use crate::middleware::Middleware;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{Request, METHOD};
use crate::response::{Response, StatusCodeReasonPhrase, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::session::{session_id_from_request, SessionStore};

use super::client_store::{ClientStore, GrantType};
use super::pkce::base64url_encode;

const TOKEN_PATH: &str = "/oauth/token";
const AUTHORIZE_PATH: &str = "/oauth/authorize";
const DISCOVERY_PATH: &str = "/.well-known/openid-configuration";
const JWKS_PATH: &str = "/.well-known/jwks.json";
const SESSION_COOKIE: &str = "_rws_authz_sid";
const AUTH_CODE_TTL_SECS: u64 = 60;

/// Configuration for [`AuthServer`].
pub struct AuthServerConfig {
    /// This server's issuer URL, embedded in every issued token's `iss`
    /// claim and in the discovery document.
    pub issuer: String,
    /// HMAC-SHA256 (HS256) shared secret used to sign every issued JWT.
    /// See the module-level docs for why this is HS256, not an RSA/EC key.
    pub signing_secret: String,
    /// Lifetime of an issued access token.
    pub access_token_ttl: Duration,
    /// Lifetime of an issued refresh token.
    pub refresh_token_ttl: Duration,
    /// Registered OAuth 2.0 clients.
    pub clients: ClientStore,
    /// Session store used to recognize an already-authenticated browser at
    /// `/oauth/authorize`. See the module-level docs.
    pub sessions: Arc<SessionStore>,
}

struct AuthCodeRecord {
    client_id: String,
    redirect_uri: String,
    scope: String,
    subject: String,
    code_challenge: Option<String>,
    expires_at: Instant,
}

#[derive(Clone)]
struct RefreshTokenRecord {
    client_id: String,
    subject: String,
    scope: String,
    expires_at: Instant,
}

/// A minimal OAuth 2.0 Authorization Server — see the [module docs](self).
pub struct AuthServer {
    config: Arc<AuthServerConfig>,
    login_url: String,
    subject_session_key: String,
    codes: Mutex<HashMap<String, AuthCodeRecord>>,
    refresh_tokens: Mutex<HashMap<String, RefreshTokenRecord>>,
}

impl AuthServer {
    /// Create a new `AuthServer`. Defaults `login_url` to `/login` and
    /// `subject_session_key` to `"user_id"` — override with
    /// [`AuthServer::login_url`] / [`AuthServer::subject_session_key`].
    pub fn new(config: AuthServerConfig) -> Self {
        AuthServer {
            config: Arc::new(config),
            login_url: "/login".to_string(),
            subject_session_key: "user_id".to_string(),
            codes: Mutex::new(HashMap::new()),
            refresh_tokens: Mutex::new(HashMap::new()),
        }
    }

    /// Override where an unauthenticated browser is sent from
    /// `/oauth/authorize` (default: `/login`).
    pub fn login_url(mut self, url: &str) -> Self {
        self.login_url = url.to_string();
        self
    }

    /// Override the session key read to identify the logged-in user at
    /// `/oauth/authorize` (default: `"user_id"`).
    pub fn subject_session_key(mut self, key: &str) -> Self {
        self.subject_session_key = key.to_string();
        self
    }

    // ── /oauth/authorize ─────────────────────────────────────────────────────

    fn handle_authorize(&self, request: &Request) -> Response {
        let query = match Query::from_request(request) {
            Ok(q) => q,
            Err(_) => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_request", "malformed query string"),
        };

        if query.get("response_type").map(String::as_str) != Some("code") {
            return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "unsupported_response_type", "only response_type=code is supported");
        }
        let client_id = match query.get("client_id") {
            Some(c) => c.clone(),
            None => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_request", "missing client_id"),
        };
        let redirect_uri = match query.get("redirect_uri") {
            Some(r) => r.clone(),
            None => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_request", "missing redirect_uri"),
        };
        let client = match self.config.clients.get(&client_id) {
            Some(c) => c,
            None => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_client", "unknown client_id"),
        };
        if !client.grants.contains(&GrantType::AuthorizationCode) {
            return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "unauthorized_client", "client is not authorized for the authorization_code grant");
        }
        if !client.redirect_uris.iter().any(|u| u == &redirect_uri) {
            return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_request", "redirect_uri is not registered for this client");
        }
        let code_challenge = query.get("code_challenge").cloned();
        if let Some(method) = query.get("code_challenge_method") {
            if method != "S256" {
                return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_request", "only code_challenge_method=S256 is supported");
            }
        }
        let scope = query.get("scope").cloned().unwrap_or_default();
        let state = query.get("state").cloned();

        let subject = session_id_from_request(request, SESSION_COOKIE)
            .and_then(|sid| self.config.sessions.load(&sid))
            .and_then(|s| s.get(&self.subject_session_key).map(|v| v.to_string()));

        let subject = match subject {
            Some(s) => s,
            None => {
                let return_to = super::client::url_encode(&request.request_uri);
                return Self::redirect(&format!("{}?return_to={}", self.login_url, return_to));
            }
        };

        let code = Self::random_token();
        self.codes.lock().unwrap().insert(
            code.clone(),
            AuthCodeRecord {
                client_id,
                redirect_uri: redirect_uri.clone(),
                scope,
                subject,
                code_challenge,
                expires_at: Instant::now() + Duration::from_secs(AUTH_CODE_TTL_SECS),
            },
        );

        let mut location = format!("{redirect_uri}?code={code}");
        if let Some(s) = state {
            location.push_str(&format!("&state={}", super::client::url_encode(&s)));
        }
        Self::redirect(&location)
    }

    // ── /oauth/token ─────────────────────────────────────────────────────────

    fn handle_token(&self, request: &Request) -> Response {
        let form = match FormUrlEncoded::parse(request.body.clone()) {
            Ok(f) => f,
            Err(_) => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_request", "malformed request body"),
        };
        match form.get("grant_type").map(String::as_str) {
            Some("client_credentials") => self.handle_client_credentials(&form),
            Some("authorization_code") => self.handle_authorization_code(&form),
            Some("refresh_token") => self.handle_refresh_token(&form),
            _ => Self::oauth_error(
                STATUS_CODE_REASON_PHRASE.n400_bad_request,
                "unsupported_grant_type",
                "grant_type must be client_credentials, authorization_code, or refresh_token",
            ),
        }
    }

    fn handle_client_credentials(&self, form: &HashMap<String, String>) -> Response {
        let client_id = form.get("client_id").cloned().unwrap_or_default();
        let client_secret = form.get("client_secret").cloned().unwrap_or_default();
        let client = match self.config.clients.get(&client_id) {
            Some(c) => c,
            None => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "invalid_client", "unknown client_id"),
        };
        if client.client_secret.as_deref() != Some(client_secret.as_str()) {
            return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "invalid_client", "client authentication failed");
        }
        if !client.grants.contains(&GrantType::ClientCredentials) {
            return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "unauthorized_client", "client is not authorized for the client_credentials grant");
        }
        let scope = match Self::negotiate_scope(form.get("scope").map(String::as_str), &client.scopes) {
            Some(s) => s,
            None => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_scope", "requested scope exceeds client's authorized scopes"),
        };

        let access_token = self.issue_jwt(&client_id, &client_id, &scope);
        Self::token_response(&access_token, self.config.access_token_ttl, None, None)
    }

    fn handle_authorization_code(&self, form: &HashMap<String, String>) -> Response {
        let code = match form.get("code") {
            Some(c) => c.clone(),
            None => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_request", "missing code"),
        };
        let redirect_uri = form.get("redirect_uri").cloned().unwrap_or_default();
        let client_id = form.get("client_id").cloned().unwrap_or_default();

        let record = self.codes.lock().unwrap().remove(&code);
        let record = match record {
            Some(r) if r.expires_at > Instant::now() => r,
            _ => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_grant", "code is invalid, expired, or already used"),
        };
        if record.client_id != client_id || record.redirect_uri != redirect_uri {
            return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_grant", "client_id or redirect_uri does not match the original request");
        }
        let client = match self.config.clients.get(&client_id) {
            Some(c) => c,
            None => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_client", "unknown client_id"),
        };
        if let Some(secret) = &client.client_secret {
            if form.get("client_secret").map(String::as_str) != Some(secret.as_str()) {
                return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "invalid_client", "client authentication failed");
            }
        }
        if let Some(challenge) = &record.code_challenge {
            let verifier = form.get("code_verifier").map(String::as_str).unwrap_or("");
            let computed = base64url_encode(&Sha256::digest(verifier.as_bytes()));
            if &computed != challenge {
                return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_grant", "code_verifier does not match code_challenge");
            }
        }

        // The id_token and access_token describe the same authenticated
        // subject with no meaningfully different claim set here, so both
        // fields carry the same signed JWT.
        let token = self.issue_jwt(&record.subject, &client_id, &record.scope);
        let refresh_token = Self::random_token();
        self.refresh_tokens.lock().unwrap().insert(
            refresh_token.clone(),
            RefreshTokenRecord {
                client_id,
                subject: record.subject,
                scope: record.scope,
                expires_at: Instant::now() + self.config.refresh_token_ttl,
            },
        );
        Self::token_response(&token, self.config.access_token_ttl, Some(&refresh_token), Some(&token))
    }

    fn handle_refresh_token(&self, form: &HashMap<String, String>) -> Response {
        let token = match form.get("refresh_token") {
            Some(t) => t.clone(),
            None => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_request", "missing refresh_token"),
        };
        let record = self.refresh_tokens.lock().unwrap().get(&token).cloned();
        let record = match record {
            Some(r) if r.expires_at > Instant::now() => r,
            _ => return Self::oauth_error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "invalid_grant", "refresh_token is invalid or expired"),
        };
        let access_token = self.issue_jwt(&record.subject, &record.client_id, &record.scope);
        Self::token_response(&access_token, self.config.access_token_ttl, None, None)
    }

    fn negotiate_scope(requested: Option<&str>, allowed: &[String]) -> Option<String> {
        match requested {
            None => Some(allowed.join(" ")),
            Some(r) if r.is_empty() => Some(allowed.join(" ")),
            Some(r) => {
                let tokens: Vec<&str> = r.split_whitespace().collect();
                if tokens.iter().all(|t| allowed.iter().any(|a| a == t)) {
                    Some(tokens.join(" "))
                } else {
                    None
                }
            }
        }
    }

    fn issue_jwt(&self, sub: &str, aud: &str, scope: &str) -> String {
        let now = unix_now();
        let exp = now + self.config.access_token_ttl.as_secs();
        let claims = format!(
            r#"{{"sub":"{}","iss":"{}","aud":"{}","exp":{},"iat":{},"scope":"{}"}}"#,
            json_escape(sub),
            json_escape(&self.config.issuer),
            json_escape(aud),
            exp,
            now,
            json_escape(scope),
        );
        crate::auth::build_jwt(&claims, self.config.signing_secret.as_bytes())
    }

    // ── discovery / jwks ─────────────────────────────────────────────────────

    fn discovery_document(&self) -> Response {
        let issuer = &self.config.issuer;
        let body = format!(
            r#"{{"issuer":"{issuer}","authorization_endpoint":"{issuer}{AUTHORIZE_PATH}","token_endpoint":"{issuer}{TOKEN_PATH}","jwks_uri":"{issuer}{JWKS_PATH}","response_types_supported":["code"],"grant_types_supported":["authorization_code","client_credentials","refresh_token"]}}"#
        );
        Self::json_response(STATUS_CODE_REASON_PHRASE.n200_ok, &body)
    }

    fn jwks_document(&self) -> Response {
        // HS256 is symmetric — there is no public key to publish. See the
        // module-level docs.
        Self::json_response(STATUS_CODE_REASON_PHRASE.n200_ok, r#"{"keys":[]}"#)
    }

    // ── response helpers ─────────────────────────────────────────────────────

    fn random_token() -> String {
        use rand_core::{OsRng, RngCore};
        let mut bytes = [0u8; 24];
        OsRng.fill_bytes(&mut bytes);
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn redirect(url: &str) -> Response {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n302_found.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n302_found.reason_phrase.to_string();
        r.headers.push(Header { name: "Location".to_string(), value: url.to_string() });
        r
    }

    fn json_response(status: &'static StatusCodeReasonPhrase, body: &str) -> Response {
        let mut r = Response::new();
        r.status_code = *status.status_code;
        r.reason_phrase = status.reason_phrase.to_string();
        r.content_range_list = vec![Range::get_content_range(body.as_bytes().to_vec(), MimeType::APPLICATION_JSON.to_string())];
        r
    }

    fn oauth_error(status: &'static StatusCodeReasonPhrase, error: &str, description: &str) -> Response {
        let body = format!(r#"{{"error":"{error}","error_description":"{}"}}"#, json_escape(description));
        Self::json_response(status, &body)
    }

    fn token_response(access_token: &str, ttl: Duration, refresh_token: Option<&str>, id_token: Option<&str>) -> Response {
        let mut body = format!(
            r#"{{"access_token":"{}","token_type":"Bearer","expires_in":{}"#,
            access_token,
            ttl.as_secs(),
        );
        if let Some(rt) = refresh_token {
            body.push_str(&format!(r#","refresh_token":"{rt}""#));
        }
        if let Some(idt) = id_token {
            body.push_str(&format!(r#","id_token":"{idt}""#));
        }
        body.push('}');
        Self::json_response(STATUS_CODE_REASON_PHRASE.n200_ok, &body)
    }
}

impl Middleware for AuthServer {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        let path = request.request_uri.split('?').next().unwrap_or("");
        let is_get = request.method.eq_ignore_ascii_case(METHOD.get);
        let is_post = request.method.eq_ignore_ascii_case(METHOD.post);

        if path == AUTHORIZE_PATH && is_get {
            return Ok(self.handle_authorize(request));
        }
        if path == TOKEN_PATH && is_post {
            return Ok(self.handle_token(request));
        }
        if path == DISCOVERY_PATH && is_get {
            return Ok(self.discovery_document());
        }
        if path == JWKS_PATH && is_get {
            return Ok(self.jwks_document());
        }

        next.execute(request, connection)
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out
}
