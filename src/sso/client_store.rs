//! Registered OAuth 2.0 clients for [`super::server::AuthServer`].

/// An OAuth 2.0 grant type a registered client is authorized to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantType {
    /// `grant_type=client_credentials` — machine-to-machine, no end user.
    ClientCredentials,
    /// `grant_type=authorization_code` — end-user login via `/oauth/authorize`.
    AuthorizationCode,
    /// `grant_type=refresh_token` — exchange a refresh token for a new access token.
    RefreshToken,
}

/// A registered OAuth 2.0 client.
#[derive(Debug, Clone)]
pub struct OAuthClient {
    /// The client identifier sent as `client_id`.
    pub client_id: String,
    /// The client secret, for confidential clients. `None` for a public
    /// client (e.g. a single-page app using PKCE with no secret).
    pub client_secret: Option<String>,
    /// Redirect URIs this client is allowed to receive an authorization
    /// code at. Required (non-empty) for clients using
    /// [`GrantType::AuthorizationCode`]; ignored otherwise.
    pub redirect_uris: Vec<String>,
    /// Grant types this client may use.
    pub grants: Vec<GrantType>,
    /// Scopes this client may request. A token request naming a scope
    /// outside this list is rejected with `invalid_scope`.
    pub scopes: Vec<String>,
}

/// In-memory registry of [`OAuthClient`]s, looked up by `client_id`.
///
/// # Example
///
/// ```rust
/// use rust_web_server::sso::client_store::{ClientStore, OAuthClient, GrantType};
///
/// let clients = ClientStore::new()
///     .add(OAuthClient {
///         client_id:     "spa-frontend".into(),
///         client_secret: None,
///         redirect_uris: vec!["https://spa.example.com/callback".into()],
///         grants:        vec![GrantType::AuthorizationCode],
///         scopes:        vec!["openid".into(), "email".into()],
///     })
///     .add(OAuthClient {
///         client_id:     "backend-service".into(),
///         client_secret: Some("s3cr3t".into()),
///         redirect_uris: vec![],
///         grants:        vec![GrantType::ClientCredentials],
///         scopes:        vec!["api:read".into(), "api:write".into()],
///     });
///
/// assert!(clients.get("spa-frontend").is_some());
/// assert!(clients.get("unknown-client").is_none());
/// ```
#[derive(Debug, Clone, Default)]
pub struct ClientStore {
    clients: Vec<OAuthClient>,
}

impl ClientStore {
    /// Create an empty client store.
    pub fn new() -> Self {
        ClientStore { clients: Vec::new() }
    }

    /// Register a client. Later registrations with a duplicate `client_id`
    /// are still stored; [`ClientStore::get`] returns the first match.
    pub fn add(mut self, client: OAuthClient) -> Self {
        self.clients.push(client);
        self
    }

    /// Look up a registered client by `client_id`.
    pub fn get(&self, client_id: &str) -> Option<&OAuthClient> {
        self.clients.iter().find(|c| c.client_id == client_id)
    }
}
