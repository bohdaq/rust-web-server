//! `OidcAuth` middleware — full OIDC / OAuth 2.0 authorization code flow with
//! PKCE, session management, and claim injection.
//!
//! # Usage
//!
//! ```rust,no_run
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

use std::sync::Arc;

use rand_core::{OsRng, RngCore};
use serde_json;

use crate::application::Application;
use crate::core::New;
use crate::error::{AppError, IntoResponse};
use crate::header::Header;
use crate::middleware::Middleware;
use crate::mime_type::MimeType;
use crate::range::Range;
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

/// OIDC / OAuth 2.0 authentication middleware.
///
/// See the [module documentation](self) for a usage example.
pub struct OidcAuth {
    config:        Arc<OidcConfig>,
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
        r.headers.push(Header { name: "Location".to_string(), value: url.to_string() });
        r
    }

    fn is_excluded(&self, path: &str) -> bool {
        self.excluded.iter().any(|p| path.starts_with(p.as_str()))
    }

    // ── route handlers ────────────────────────────────────────────────────────

    fn handle_login(&self, request: &Request) -> Response {
        let verifier = PkceVerifier::new();
        let state    = Self::random_token();
        let nonce    = Self::random_token();

        let mut session = self.sessions.create();
        session.set("_oidc_state",     &state);
        session.set("_oidc_nonce",     &nonce);
        session.set("_oidc_pkce",      verifier.as_str());

        let return_to = query_param(request, "return_to")
            .unwrap_or_else(|| self.config.post_login_redirect.clone());
        session.set("_oidc_return_to", &return_to);
        self.sessions.save(&session);

        let url = self.client.authorization_url(&verifier, &state, &nonce);
        let mut response = Self::redirect(&url);
        response.headers.push(Header {
            name:  "Set-Cookie".to_string(),
            value: session_cookie(&session.id, SESSION_COOKIE, SESSION_TTL),
        });
        response
    }

    fn handle_callback(&self, request: &Request) -> Response {
        let sid = match session_id_from_request(request, SESSION_COOKIE) {
            Some(id) => id,
            None     => return AppError::Forbidden.into_response(),
        };
        let mut session = match self.sessions.load(&sid) {
            Some(s) => s,
            None    => return AppError::Forbidden.into_response(),
        };

        // Validate state
        let stored_state   = session.get("_oidc_state").unwrap_or("").to_string();
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

        let pkce_verifier = session.get("_oidc_pkce").unwrap_or("").to_string();
        let stored_nonce  = session.get("_oidc_nonce").unwrap_or("").to_string();
        let return_to     = session.get("_oidc_return_to").unwrap_or("/").to_string();

        // Exchange code for tokens
        let tokens = match self.client.exchange_code(&code, &pkce_verifier) {
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

        // Promote session: clear pre-auth keys, store claims
        session.remove("_oidc_state");
        session.remove("_oidc_nonce");
        session.remove("_oidc_pkce");
        session.remove("_oidc_return_to");
        let claims_json = serde_json::to_string(&claims).unwrap_or_default();
        session.set("_oidc_claims", &claims_json);
        self.sessions.save(&session);

        let mut response = Self::redirect(&return_to);
        response.headers.push(Header {
            name:  "Set-Cookie".to_string(),
            value: session_cookie(&session.id, SESSION_COOKIE, SESSION_TTL),
        });
        response
    }

    fn handle_logout(&self, request: &Request) -> Response {
        if let Some(sid) = session_id_from_request(request, SESSION_COOKIE) {
            self.sessions.destroy(&sid);
        }
        let mut response = Self::redirect("/");
        response.headers.push(Header {
            name:  "Set-Cookie".to_string(),
            value: destroy_cookie(SESSION_COOKIE),
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
            return Ok(self.handle_login(request));
        }
        if path == self.callback_path {
            return Ok(self.handle_callback(request));
        }
        if path == self.logout_path {
            return Ok(self.handle_logout(request));
        }

        // Excluded paths bypass auth
        if self.is_excluded(path) {
            return next.execute(request, connection);
        }

        // Check session for authenticated claims
        if let Some(sid) = session_id_from_request(request, SESSION_COOKIE) {
            if let Some(session) = self.sessions.load(&sid) {
                if let Some(claims_json) = session.get("_oidc_claims") {
                    // Inject claims header so downstream handlers can read them
                    let mut req = request.clone();
                    req.headers.push(Header {
                        name:  CLAIMS_HEADER.to_string(),
                        value: claims_json.to_string(),
                    });
                    return next.execute(&req, connection);
                }
            }
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
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let mut out   = String::with_capacity(s.len());
    let bytes      = s.as_bytes();
    let mut i      = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(b as char);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(' ');
        } else {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    out
}

fn error_response(msg: &str) -> Response {
    let mut r = Response::new();
    r.status_code   = *STATUS_CODE_REASON_PHRASE.n500_internal_server_error.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE
        .n500_internal_server_error
        .reason_phrase
        .to_string();
    r.content_range_list = vec![Range::get_content_range(
        msg.as_bytes().to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    )];
    r
}
