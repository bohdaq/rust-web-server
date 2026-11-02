//! OIDC provider discovery and hardcoded presets.
//!
//! [`OidcProvider`] holds the endpoint URLs for an identity provider.
//! Use one of the preset constructors (`google()`, `microsoft()`, etc.) or call
//! [`OidcProvider::discover`] to fetch endpoints from the standard
//! `{issuer}/.well-known/openid-configuration` URL.

use crate::http_client::Client;
use super::SsoError;

/// Endpoint URLs for an OIDC / OAuth 2.0 identity provider.
#[derive(Debug, Clone)]
pub struct OidcProvider {
    pub issuer:                 String,
    pub authorization_endpoint: String,
    pub token_endpoint:         String,
    pub jwks_uri:               String,
    pub userinfo_endpoint:      Option<String>,
    pub end_session_endpoint:   Option<String>,
}

impl OidcProvider {
    /// Fetch and parse `{issuer}/.well-known/openid-configuration`.
    pub fn discover(issuer: &str) -> Result<Self, SsoError> {
        let url = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        let resp = Client::new()
            .get(&url)
            .timeout_ms(10_000)
            .send()
            .map_err(|e| SsoError(format!("discovery fetch failed: {e}")))?;
        if !resp.is_success() {
            return Err(SsoError(format!("discovery returned {}", resp.status())));
        }
        let body = resp.text().map_err(|e| SsoError(e.to_string()))?;
        parse_discovery_json(&body)
    }

    /// Google OIDC preset.
    pub fn google() -> Self {
        OidcProvider {
            issuer:                 "https://accounts.google.com".into(),
            authorization_endpoint: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_endpoint:         "https://oauth2.googleapis.com/token".into(),
            jwks_uri:               "https://www.googleapis.com/oauth2/v3/certs".into(),
            userinfo_endpoint:      Some("https://openidconnect.googleapis.com/v1/userinfo".into()),
            end_session_endpoint:   None,
        }
    }

    /// Microsoft Azure AD / Entra ID preset.
    ///
    /// `tenant_id` can be a GUID, `"common"`, `"organizations"`, or `"consumers"`.
    pub fn microsoft(tenant_id: &str) -> Self {
        let base = format!("https://login.microsoftonline.com/{tenant_id}");
        OidcProvider {
            issuer:                 format!("{base}/v2.0"),
            authorization_endpoint: format!("{base}/oauth2/v2.0/authorize"),
            token_endpoint:         format!("{base}/oauth2/v2.0/token"),
            jwks_uri:               format!("{base}/discovery/v2.0/keys"),
            userinfo_endpoint:      Some("https://graph.microsoft.com/oidc/userinfo".into()),
            end_session_endpoint:   Some(format!("{base}/oauth2/v2.0/logout")),
        }
    }

    /// GitHub OAuth 2.0 preset.
    ///
    /// GitHub is not a full OIDC provider — it does not issue JWTs, so
    /// `jwks_uri` is empty and user info is fetched from the `/user` API.
    pub fn github() -> Self {
        OidcProvider {
            issuer:                 "https://github.com".into(),
            authorization_endpoint: "https://github.com/login/oauth/authorize".into(),
            token_endpoint:         "https://github.com/login/oauth/access_token".into(),
            jwks_uri:               String::new(),
            userinfo_endpoint:      Some("https://api.github.com/user".into()),
            end_session_endpoint:   None,
        }
    }

    /// Okta preset.
    ///
    /// `domain` is your Okta org URL, e.g. `"dev-12345.okta.com"`.
    pub fn okta(domain: &str) -> Self {
        let base = format!("https://{}", domain.trim_start_matches("https://"));
        OidcProvider {
            issuer:                 format!("{base}/oauth2/default"),
            authorization_endpoint: format!("{base}/oauth2/default/v1/authorize"),
            token_endpoint:         format!("{base}/oauth2/default/v1/token"),
            jwks_uri:               format!("{base}/oauth2/default/v1/keys"),
            userinfo_endpoint:      Some(format!("{base}/oauth2/default/v1/userinfo")),
            end_session_endpoint:   Some(format!("{base}/oauth2/default/v1/logout")),
        }
    }

    /// Auth0 preset.
    ///
    /// `domain` is your Auth0 domain, e.g. `"myapp.us.auth0.com"`.
    pub fn auth0(domain: &str) -> Self {
        let base = format!("https://{}", domain.trim_start_matches("https://"));
        OidcProvider {
            issuer:                 format!("{base}/"),
            authorization_endpoint: format!("{base}/authorize"),
            token_endpoint:         format!("{base}/oauth/token"),
            jwks_uri:               format!("{base}/.well-known/jwks.json"),
            userinfo_endpoint:      Some(format!("{base}/userinfo")),
            end_session_endpoint:   Some(format!("{base}/v2/logout")),
        }
    }

    /// Keycloak preset.
    ///
    /// `base_url` is the root URL of your Keycloak instance, e.g.
    /// `"https://keycloak.example.com"`.  `realm` is the realm name.
    pub fn keycloak(base_url: &str, realm: &str) -> Self {
        let base = format!("{}/realms/{}", base_url.trim_end_matches('/'), realm);
        OidcProvider {
            issuer:                 base.clone(),
            authorization_endpoint: format!("{base}/protocol/openid-connect/auth"),
            token_endpoint:         format!("{base}/protocol/openid-connect/token"),
            jwks_uri:               format!("{base}/protocol/openid-connect/certs"),
            userinfo_endpoint:      Some(format!("{base}/protocol/openid-connect/userinfo")),
            end_session_endpoint:   Some(format!("{base}/protocol/openid-connect/logout")),
        }
    }
}

// ── internal JSON parser ──────────────────────────────────────────────────────

fn parse_discovery_json(json: &str) -> Result<OidcProvider, SsoError> {
    fn extract(json: &str, key: &str) -> Option<String> {
        let needle = format!("\"{key}\"");
        let start = json.find(&needle)? + needle.len();
        let rest = json[start..].trim_start_matches(|c: char| c.is_whitespace() || c == ':');
        if rest.starts_with('"') {
            let inner = &rest[1..];
            let end = inner.find('"')?;
            Some(inner[..end].to_string())
        } else {
            None
        }
    }

    Ok(OidcProvider {
        issuer: extract(json, "issuer").unwrap_or_default(),
        authorization_endpoint: extract(json, "authorization_endpoint")
            .ok_or_else(|| SsoError("missing authorization_endpoint".into()))?,
        token_endpoint: extract(json, "token_endpoint")
            .ok_or_else(|| SsoError("missing token_endpoint".into()))?,
        jwks_uri: extract(json, "jwks_uri")
            .ok_or_else(|| SsoError("missing jwks_uri".into()))?,
        userinfo_endpoint: extract(json, "userinfo_endpoint"),
        end_session_endpoint: extract(json, "end_session_endpoint"),
    })
}
