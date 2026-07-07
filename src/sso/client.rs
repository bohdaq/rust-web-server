//! OIDC / OAuth 2.0 client — authorization URL builder, token exchange,
//! and UserInfo fetch.

use crate::http_client::Client;

use super::{
    config::OidcConfig,
    jwks::{json_str, OidcClaims},
    pkce::PkceVerifier,
    SsoError,
};

/// Result of a successful token-endpoint exchange.
pub struct TokenResponse {
    /// The access token returned by the provider.
    pub access_token:  String,
    /// Token type (typically `"Bearer"`).
    pub token_type:    String,
    /// Lifetime of the access token in seconds.
    pub expires_in:    Option<u64>,
    /// Refresh token, if the provider returned one.
    pub refresh_token: Option<String>,
    /// OIDC id_token (a signed JWT), if the provider returned one.
    pub id_token:      Option<String>,
    /// Granted scopes.
    pub scope:         Option<String>,
}

/// A thin OIDC / OAuth 2.0 client that performs the authorization code flow.
pub struct OidcClient {
    config: OidcConfig,
}

impl OidcClient {
    /// Create a new client for the given configuration.
    pub fn new(config: OidcConfig) -> Self {
        OidcClient { config }
    }

    /// Build the URL to redirect the user to for authorization.
    ///
    /// Returns the full URL including `response_type`, `client_id`,
    /// `redirect_uri`, `scope`, `state`, `nonce`, and (for OIDC providers)
    /// PKCE `code_challenge` / `code_challenge_method`.
    ///
    /// Store `state`, `nonce`, and `pkce.as_str()` in the pre-auth session
    /// before issuing the redirect.
    pub fn authorization_url(&self, pkce: &PkceVerifier, state: &str, nonce: &str) -> String {
        let scopes = self.config.scopes.join(" ");
        let mut url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}",
            self.config.provider.authorization_endpoint,
            url_encode(&self.config.client_id),
            url_encode(&self.config.redirect_uri),
            url_encode(&scopes),
            url_encode(state),
            url_encode(nonce),
        );
        // Only add PKCE for providers that issue JWTs (i.e., have a JWKS URI).
        // GitHub does not support PKCE on its token endpoint.
        if !self.config.provider.jwks_uri.is_empty() {
            url.push_str(&format!(
                "&code_challenge={}&code_challenge_method=S256",
                url_encode(pkce.challenge().as_str())
            ));
        }
        url
    }

    /// Exchange an authorization code for tokens at the token endpoint.
    pub fn exchange_code(
        &self,
        code: &str,
        pkce_verifier: &str,
    ) -> Result<TokenResponse, SsoError> {
        let mut form: Vec<(&str, &str)> = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.config.redirect_uri),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
        ];
        if !self.config.provider.jwks_uri.is_empty() {
            form.push(("code_verifier", pkce_verifier));
        }

        let resp = Client::new()
            .post(&self.config.provider.token_endpoint)
            .header("Accept", "application/json")
            .form(&form)
            .send()
            .map_err(|e| SsoError(format!("token exchange failed: {e}")))?;

        if !resp.is_success() {
            let body_text = resp.text().unwrap_or_default();
            return Err(SsoError(format!(
                "token endpoint returned {}: {}",
                resp.status(),
                body_text
            )));
        }
        let json = resp.text().map_err(|e| SsoError(e.to_string()))?;
        parse_token_response(&json)
    }

    /// Fetch user info from the provider's UserInfo endpoint using `access_token`.
    pub fn fetch_user_info(&self, access_token: &str) -> Result<OidcClaims, SsoError> {
        let endpoint = self
            .config
            .provider
            .userinfo_endpoint
            .as_ref()
            .ok_or_else(|| SsoError("provider has no userinfo_endpoint".into()))?;

        let resp = Client::new()
            .get(endpoint)
            .header("Authorization", &format!("Bearer {access_token}"))
            .header("Accept", "application/json")
            .send()
            .map_err(|e| SsoError(format!("userinfo fetch failed: {e}")))?;

        if !resp.is_success() {
            return Err(SsoError(format!("userinfo returned {}", resp.status())));
        }
        let json = resp.text().map_err(|e| SsoError(e.to_string()))?;
        parse_userinfo_json(&json)
    }
}

// ── internal parsers ──────────────────────────────────────────────────────────

fn parse_token_response(json: &str) -> Result<TokenResponse, SsoError> {
    let access_token = json_str(json, "access_token")
        .ok_or_else(|| SsoError("token response missing access_token".into()))?;
    Ok(TokenResponse {
        access_token,
        token_type:    json_str(json, "token_type").unwrap_or_else(|| "Bearer".into()),
        expires_in:    json_u64(json, "expires_in"),
        refresh_token: json_str(json, "refresh_token"),
        id_token:      json_str(json, "id_token"),
        scope:         json_str(json, "scope"),
    })
}

fn parse_userinfo_json(json: &str) -> Result<OidcClaims, SsoError> {
    // GitHub returns { "id": 12345, "login": "user", "email": "...", "name": "..." }
    // OIDC providers return { "sub": "...", "email": "...", ... }
    let sub = json_str(json, "sub")
        .or_else(|| json_int_as_string(json, "id").map(|id| format!("github:{id}")))
        .unwrap_or_else(|| "unknown".into());

    Ok(OidcClaims {
        sub,
        iss:            json_str(json, "iss").unwrap_or_default(),
        aud:            vec![],
        exp:            0,
        iat:            0,
        nonce:          None,
        email:          json_str(json, "email"),
        email_verified: json_bool(json, "email_verified"),
        name:           json_str(json, "name"),
        given_name:     json_str(json, "given_name"),
        family_name:    json_str(json, "family_name"),
        picture:        json_str(json, "picture").or_else(|| json_str(json, "avatar_url")),
        locale:         json_str(json, "locale"),
    })
}

fn json_u64(json: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = json[start..].trim_start_matches(|c: char| c.is_whitespace() || c == ':');
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Extract a bare integer field (not quoted) and return it as a String.
fn json_int_as_string(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = json[start..].trim_start_matches(|c: char| c.is_whitespace() || c == ':');
    if rest.starts_with(|c: char| c.is_ascii_digit()) {
        let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        Some(rest[..end].to_string())
    } else {
        None
    }
}

fn json_bool(json: &str, key: &str) -> Option<bool> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = json[start..].trim_start_matches(|c: char| c.is_whitespace() || c == ':');
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// Minimal percent-encoding for URL query parameter values.
pub(crate) fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
