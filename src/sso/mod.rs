//! OAuth 2.0 / OIDC SSO client support.
//!
//! Enabled by the `sso` Cargo feature:
//!
//! ```toml
//! rust-web-server = { version = "17", features = ["sso"] }
//! ```
//!
//! # Quick start
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::session::SessionStore;
//! use rust_web_server::sso::{OidcAuth, OidcConfig};
//!
//! let sessions = Arc::new(SessionStore::new(86_400));
//! let config   = OidcConfig::google(
//!     "my-client-id",
//!     "my-client-secret",
//!     "https://example.com/auth/callback",
//! );
//! let app = App::new().wrap(OidcAuth::new(config, sessions));
//! ```
//!
//! # Module layout
//!
//! | Sub-module      | Purpose                                                |
//! |-----------------|--------------------------------------------------------|
//! | [`config`]      | [`OidcConfig`] + provider presets + `from_env()`       |
//! | [`discovery`]   | [`OidcProvider`] endpoints + hardcoded presets         |
//! | [`jwks`]        | [`JwksCache`], JWT RS256/ES256 verify, [`OidcClaims`]  |
//! | [`pkce`]        | [`PkceVerifier`], [`PkceChallenge`], base64url         |
//! | [`client`]      | [`OidcClient`]: auth URL, token exchange, user info    |
//! | [`oidc_auth`]   | [`OidcAuth`] middleware                                |

#[cfg(test)]
pub(crate) mod tests;

pub mod client;
pub mod config;
pub mod discovery;
pub mod jwks;
pub mod oidc_auth;
pub mod pkce;

// ── public re-exports ─────────────────────────────────────────────────────────

pub use client::{OidcClient, TokenResponse};
pub use config::OidcConfig;
pub use discovery::OidcProvider;
pub use jwks::{JwksCache, OidcClaims, VerifyOptions};
pub use oidc_auth::OidcAuth;
pub use pkce::{PkceChallenge, PkceVerifier};

// ── error type ────────────────────────────────────────────────────────────────

/// An error produced by the SSO / OIDC flow.
#[derive(Debug)]
pub struct SsoError(pub String);

impl std::fmt::Display for SsoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SsoError: {}", self.0)
    }
}

impl std::error::Error for SsoError {}

impl From<String> for SsoError {
    fn from(s: String) -> Self {
        SsoError(s)
    }
}

impl From<&str> for SsoError {
    fn from(s: &str) -> Self {
        SsoError(s.to_string())
    }
}
