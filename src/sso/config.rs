//! OIDC client configuration and provider presets.
//!
//! [`OidcConfig`] bundles the OAuth 2.0 / OIDC parameters for a specific
//! application registration with an identity provider.  Create one via a
//! provider preset (e.g. [`OidcConfig::google`]) or load from environment
//! variables with [`OidcConfig::from_env`].

use super::{discovery::OidcProvider, SsoError};

/// All parameters needed to perform an OIDC / OAuth 2.0 authorization flow.
#[derive(Clone)]
pub struct OidcConfig {
    /// Provider endpoint URLs.
    pub provider:            OidcProvider,
    /// OAuth client ID.
    pub client_id:           String,
    /// OAuth client secret (empty for PKCE-only public clients).
    pub client_secret:       String,
    /// Redirect URI registered with the identity provider.
    pub redirect_uri:        String,
    /// Requested OAuth scopes.
    pub scopes:              Vec<String>,
    /// Path to redirect to after a successful login.
    pub post_login_redirect: String,
}

impl OidcConfig {
    /// Google OIDC configuration.
    pub fn google(client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        OidcConfig {
            provider:            OidcProvider::google(),
            client_id:           client_id.into(),
            client_secret:       client_secret.into(),
            redirect_uri:        redirect_uri.into(),
            scopes:              vec!["openid".into(), "email".into(), "profile".into()],
            post_login_redirect: "/".into(),
        }
    }

    /// Microsoft Entra ID / Azure AD configuration.
    ///
    /// `tenant_id` can be a GUID, `"common"`, `"organizations"`, or `"consumers"`.
    pub fn microsoft(
        tenant_id: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> Self {
        OidcConfig {
            provider:            OidcProvider::microsoft(tenant_id),
            client_id:           client_id.into(),
            client_secret:       client_secret.into(),
            redirect_uri:        redirect_uri.into(),
            scopes:              vec!["openid".into(), "email".into(), "profile".into()],
            post_login_redirect: "/".into(),
        }
    }

    /// GitHub OAuth 2.0 configuration.
    pub fn github(client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        OidcConfig {
            provider:            OidcProvider::github(),
            client_id:           client_id.into(),
            client_secret:       client_secret.into(),
            redirect_uri:        redirect_uri.into(),
            scopes:              vec!["read:user".into(), "user:email".into()],
            post_login_redirect: "/".into(),
        }
    }

    /// Okta configuration.
    ///
    /// `domain` is your Okta org URL, e.g. `"dev-12345.okta.com"`.
    pub fn okta(domain: &str, client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        OidcConfig {
            provider:            OidcProvider::okta(domain),
            client_id:           client_id.into(),
            client_secret:       client_secret.into(),
            redirect_uri:        redirect_uri.into(),
            scopes:              vec!["openid".into(), "email".into(), "profile".into()],
            post_login_redirect: "/".into(),
        }
    }

    /// Auth0 configuration.
    ///
    /// `domain` is your Auth0 domain, e.g. `"myapp.us.auth0.com"`.
    pub fn auth0(domain: &str, client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        OidcConfig {
            provider:            OidcProvider::auth0(domain),
            client_id:           client_id.into(),
            client_secret:       client_secret.into(),
            redirect_uri:        redirect_uri.into(),
            scopes:              vec!["openid".into(), "email".into(), "profile".into()],
            post_login_redirect: "/".into(),
        }
    }

    /// Keycloak configuration.
    pub fn keycloak(
        base_url: &str,
        realm: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> Self {
        OidcConfig {
            provider:            OidcProvider::keycloak(base_url, realm),
            client_id:           client_id.into(),
            client_secret:       client_secret.into(),
            redirect_uri:        redirect_uri.into(),
            scopes:              vec!["openid".into(), "email".into(), "profile".into()],
            post_login_redirect: "/".into(),
        }
    }

    /// Discover a custom OIDC provider via `{issuer}/.well-known/openid-configuration`.
    pub fn discover(
        issuer: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> Result<Self, SsoError> {
        Ok(OidcConfig {
            provider:            OidcProvider::discover(issuer)?,
            client_id:           client_id.into(),
            client_secret:       client_secret.into(),
            redirect_uri:        redirect_uri.into(),
            scopes:              vec!["openid".into(), "email".into(), "profile".into()],
            post_login_redirect: "/".into(),
        })
    }

    /// Load configuration from environment variables.
    ///
    /// Required variables:
    /// - `RWS_OIDC_PROVIDER` — `google`, `github`, `microsoft`, `okta`, `auth0`, `keycloak`, or `custom`
    /// - `RWS_OIDC_CLIENT_ID`
    /// - `RWS_OIDC_REDIRECT_URI`
    ///
    /// Optional variables:
    /// - `RWS_OIDC_CLIENT_SECRET` (default: empty)
    /// - `RWS_OIDC_ISSUER` — required for `okta`, `auth0`, `keycloak`, `custom`
    /// - `RWS_OIDC_TENANT_ID` — required for `microsoft` (tenant) and `keycloak` (realm)
    /// - `RWS_OIDC_SCOPES` — space-separated (default: `openid email profile`)
    /// - `RWS_OIDC_POST_LOGIN_REDIRECT` (default: `/`)
    pub fn from_env() -> Result<Self, SsoError> {
        let provider_name = std::env::var("RWS_OIDC_PROVIDER")
            .map_err(|_| SsoError("RWS_OIDC_PROVIDER not set".into()))?;
        let client_id = std::env::var("RWS_OIDC_CLIENT_ID")
            .map_err(|_| SsoError("RWS_OIDC_CLIENT_ID not set".into()))?;
        let client_secret = std::env::var("RWS_OIDC_CLIENT_SECRET").unwrap_or_default();
        let redirect_uri = std::env::var("RWS_OIDC_REDIRECT_URI")
            .map_err(|_| SsoError("RWS_OIDC_REDIRECT_URI not set".into()))?;
        let post_login_redirect =
            std::env::var("RWS_OIDC_POST_LOGIN_REDIRECT").unwrap_or_else(|_| "/".into());
        let scopes: Vec<String> = std::env::var("RWS_OIDC_SCOPES")
            .unwrap_or_else(|_| "openid email profile".into())
            .split_whitespace()
            .map(String::from)
            .collect();

        let provider = match provider_name.as_str() {
            "google" => OidcProvider::google(),
            "github" => OidcProvider::github(),
            "microsoft" => {
                let tenant = std::env::var("RWS_OIDC_TENANT_ID")
                    .map_err(|_| SsoError("RWS_OIDC_TENANT_ID required for microsoft".into()))?;
                OidcProvider::microsoft(&tenant)
            }
            "okta" => {
                let domain = std::env::var("RWS_OIDC_ISSUER")
                    .map_err(|_| SsoError("RWS_OIDC_ISSUER required for okta".into()))?;
                OidcProvider::okta(&domain)
            }
            "auth0" => {
                let domain = std::env::var("RWS_OIDC_ISSUER")
                    .map_err(|_| SsoError("RWS_OIDC_ISSUER required for auth0".into()))?;
                OidcProvider::auth0(&domain)
            }
            "keycloak" => {
                let base = std::env::var("RWS_OIDC_ISSUER").map_err(|_| {
                    SsoError("RWS_OIDC_ISSUER required for keycloak (base_url)".into())
                })?;
                let realm = std::env::var("RWS_OIDC_TENANT_ID").map_err(|_| {
                    SsoError("RWS_OIDC_TENANT_ID required for keycloak (realm)".into())
                })?;
                OidcProvider::keycloak(&base, &realm)
            }
            _ => {
                // "custom" or any unrecognised value
                let issuer = std::env::var("RWS_OIDC_ISSUER")
                    .map_err(|_| SsoError("RWS_OIDC_ISSUER required for custom provider".into()))?;
                OidcProvider::discover(&issuer)?
            }
        };

        Ok(OidcConfig {
            provider,
            client_id,
            client_secret,
            redirect_uri,
            scopes,
            post_login_redirect,
        })
    }

    /// Override the post-login redirect path (default: `"/"`).
    pub fn post_login_redirect(mut self, path: &str) -> Self {
        self.post_login_redirect = path.into();
        self
    }

    /// Override the requested scopes.
    pub fn scopes(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }
}
