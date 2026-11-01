//! `[route.middleware.auth] type = "oidc"` (18.21.0, #73): an
//! [`OidcAuth`] built from the config file. The provider's discovery
//! document is fetched on the first request (not at startup or reload, so
//! an unreachable provider doesn't hold the server up), and again every
//! 10 seconds while it fails — those requests get a `503`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::application::Application;
use crate::middleware::Middleware;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::session::SessionStore;
use crate::sso::{OidcAuth, OidcConfig, OidcProvider};

use super::OidcRouteAuth;

const RETRY: Duration = Duration::from_secs(10);

pub(crate) struct LazyOidc {
    config: OidcRouteAuth,
    client_secret: String,
    cookie_secret: Vec<u8>,
    state: Mutex<State>,
}

enum State {
    Pending(Option<Instant>),
    Ready(Arc<OidcAuth>),
}

impl LazyOidc {
    /// `Err` when a secret it needs is unset.
    pub(crate) fn new(config: OidcRouteAuth) -> Result<LazyOidc, String> {
        let client_secret = if config.client_secret_env.is_empty() { String::new() } else { std::env::var(&config.client_secret_env).unwrap_or_default() };
        if !config.client_secret_env.is_empty() && client_secret.is_empty() {
            return Err(format!("oidc auth: env var '{}' is unset or empty", config.client_secret_env));
        }
        if config.client_id.is_empty() {
            return Err("oidc auth: no client_id".to_string());
        }
        let cookie_secret = if config.cookie_secret_env.is_empty() {
            let mut key = [0u8; 32];
            rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut key);
            eprintln!("[proxy_config] oidc auth without cookie_secret_env: sign-ins last until rws restarts");
            key.to_vec()
        } else {
            let secret = std::env::var(&config.cookie_secret_env).unwrap_or_default();
            if secret.len() < 32 {
                return Err(format!("oidc auth: env var '{}' must hold at least 32 characters", config.cookie_secret_env));
            }
            secret.into_bytes()
        };
        Ok(LazyOidc { config, client_secret, cookie_secret, state: Mutex::new(State::Pending(None)) })
    }

    fn provider(&self) -> Result<OidcProvider, String> {
        if !self.config.issuer.is_empty() {
            return OidcProvider::discover(&self.config.issuer).map_err(|e| e.0);
        }
        match self.config.provider.as_str() {
            "google" => Ok(OidcProvider::google()),
            "microsoft" => Ok(OidcProvider::microsoft(if self.config.tenant.is_empty() { "common" } else { &self.config.tenant })),
            "github" => Ok(OidcProvider::github()),
            other => Err(format!("unknown oidc provider {other:?}")),
        }
    }

    fn build(&self) -> Result<OidcAuth, String> {
        let provider = self.provider()?;
        let github = provider.jwks_uri.is_empty();
        let scopes = if !self.config.scopes.is_empty() {
            self.config.scopes.clone()
        } else if github {
            vec!["read:user".to_string(), "user:email".to_string()]
        } else {
            vec!["openid".to_string(), "email".to_string(), "profile".to_string()]
        };
        let config = OidcConfig {
            provider,
            client_id: self.config.client_id.clone(),
            client_secret: self.client_secret.clone(),
            redirect_uri: self.config.redirect_uri.clone(),
            scopes,
            post_login_redirect: "/".to_string(),
        };
        let mut auth = OidcAuth::new(config, Arc::new(SessionStore::new(self.config.session_ttl_secs)))
            .signed_cookies(self.cookie_secret.clone())
            .session_ttl(self.config.session_ttl_secs)
            .allow_emails(self.config.allowed_emails.clone())
            .allow_domains(self.config.allowed_domains.clone())
            .allow_groups(&self.config.groups_claim, self.config.allowed_groups.clone())
            .forward_headers(self.config.forward_headers);
        for prefix in &self.config.exclude {
            auth = auth.exclude(prefix);
        }
        Ok(auth)
    }

    fn auth(&self) -> Option<Arc<OidcAuth>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match &*state {
            State::Ready(auth) => return Some(Arc::clone(auth)),
            State::Pending(Some(last)) if last.elapsed() < RETRY => return None,
            State::Pending(_) => {}
        }
        match self.build() {
            Ok(auth) => {
                let auth = Arc::new(auth);
                *state = State::Ready(Arc::clone(&auth));
                Some(auth)
            }
            Err(e) => {
                eprintln!("[proxy_config] oidc auth: {e} (retrying in {}s)", RETRY.as_secs());
                *state = State::Pending(Some(Instant::now()));
                None
            }
        }
    }
}

impl Middleware for LazyOidc {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        match self.auth() {
            Some(auth) => auth.handle(request, connection, next),
            None => {
                let mut r = crate::error::problem(STATUS_CODE_REASON_PHRASE.n503_service_unavailable, "the identity provider can't be reached; try again shortly");
                r.headers.push(crate::header::Header::new("Retry-After", "10"));
                Ok(r)
            }
        }
    }
}
