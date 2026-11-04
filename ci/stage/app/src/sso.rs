//! `ROLE=idp` (rws's `AuthServer`) and `ROLE=sp` (rws's `OidcAuth`).

use std::sync::Arc;
use std::time::Duration;

use rust_web_server::app::App;
use rust_web_server::header::Header;
use rust_web_server::session::{session_cookie, session_id_from_request, SessionStore};
use rust_web_server::sso::{
    AuthServer, AuthServerConfig, ClientStore, GrantType, OAuthClient, OidcAuth, OidcConfig, OidcProvider,
};
use rust_web_server::state::AppWithState;

use crate::common::{env, json, path, query, redirect, text, unavailable, Root, S};

/// The session cookie `AuthServer` reads at `/oauth/authorize`. It is a
/// private `const` in rws (`sso/server.rs` `SESSION_COOKIE`), so a login
/// page has to repeat it.
pub const AUTHZ_SESSION_COOKIE: &str = "_rws_authz_sid";
/// The session key `AuthServer` reads the subject from (its default).
pub const SUBJECT_KEY: &str = "user_id";
const SESSION_TTL: u64 = 3600;

pub struct IdpConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub signing_secret: String,
}

impl IdpConfig {
    pub fn from_env() -> Result<IdpConfig, String> {
        let get = |name: &str| env(name).ok_or_else(|| name.to_string());
        Ok(IdpConfig {
            issuer: get("ISSUER")?.trim_end_matches('/').to_string(),
            client_id: get("OIDC_CLIENT_ID")?,
            client_secret: get("OIDC_CLIENT_SECRET")?,
            redirect_uri: get("OIDC_REDIRECT_URI")?,
            signing_secret: get("IDP_SIGNING_SECRET")?,
        })
    }
}

struct IdpState {
    sessions: Arc<SessionStore>,
    signing_secret: String,
}

fn health() -> AppWithState<()> {
    App::with_state(()).get("/healthz", |_, _, _, _| text(S.n200_ok, "ok"))
}

/// Every route answers `503` naming `missing` (except `/healthz`).
pub fn unconfigured(role: &str, missing: String) -> Root {
    eprintln!("[e2e] ROLE={role}: {missing} unset; answering 503");
    Root::new(health()).guard(move |request| {
        (path(request) != "/healthz").then(|| unavailable(&missing))
    })
}

pub fn idp(config: IdpConfig) -> Root {
    let sessions = Arc::new(SessionStore::new(SESSION_TTL));
    let auth_server = AuthServer::new(AuthServerConfig {
        issuer: config.issuer.clone(),
        signing_secret: config.signing_secret.clone(),
        access_token_ttl: Duration::from_secs(3600),
        refresh_token_ttl: Duration::from_secs(86_400),
        clients: ClientStore::new().add(OAuthClient {
            client_id: config.client_id.clone(),
            client_secret: Some(config.client_secret.clone()),
            redirect_uris: vec![config.redirect_uri.clone()],
            grants: vec![GrantType::AuthorizationCode, GrantType::RefreshToken, GrantType::ClientCredentials],
            scopes: vec!["openid".into(), "profile".into(), "email".into()],
        }),
        sessions: Arc::clone(&sessions),
    })
    .login_url("/login")
    .subject_session_key(SUBJECT_KEY);

    let state = IdpState { sessions, signing_secret: config.signing_secret };
    let app = App::with_state(state)
        .get("/healthz", |_, _, _, _| text(S.n200_ok, "ok"))
        // No form: the test passes the user. Sets the session AuthServer reads.
        .get("/login", |request, _, _, state| {
            let q = query(request);
            let Some(user) = q.get("user").filter(|u| !u.is_empty()) else {
                return text(S.n400_bad_request, "missing ?user=");
            };
            let return_to = q.get("return_to").cloned().unwrap_or_else(|| "/".to_string());
            // Same-origin paths only (no open redirect).
            if !return_to.starts_with('/') || return_to.starts_with("//") {
                return text(S.n400_bad_request, "return_to must be a path");
            }
            let mut session = session_id_from_request(request, AUTHZ_SESSION_COOKIE)
                .and_then(|id| state.sessions.load(&id))
                .unwrap_or_else(|| state.sessions.create());
            session.set(SUBJECT_KEY, user.clone());
            state.sessions.save(&session);
            let mut r = redirect(&return_to);
            r.headers.push(Header::new("Set-Cookie", session_cookie(&session.id, AUTHZ_SESSION_COOKIE, SESSION_TTL)));
            r
        })
        // AuthServer has no userinfo endpoint; the SP needs one (see README).
        .get("/userinfo", |request, _, _, state| {
            let claims = rust_web_server::auth::extract_bearer_token(request)
                .and_then(|t| rust_web_server::auth::verify_jwt(&t, state.signing_secret.as_bytes()));
            match claims {
                Some(c) => {
                    let raw: serde_json::Value = serde_json::from_str(&c.raw).unwrap_or_default();
                    json(S.n200_ok, &serde_json::json!({ "sub": c.sub, "iss": raw.get("iss"), "scope": raw.get("scope") }))
                }
                None => text(S.n401_unauthorized, "invalid token"),
            }
        })
        .wrap(auth_server);
    Root::new(app)
}

pub struct SpConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub cookie_secret: String,
    /// `userinfo` (default) or `jwks` (discovery + id_token verification).
    pub verify: String,
}

impl SpConfig {
    pub fn from_env() -> Result<SpConfig, String> {
        let get = |name: &str| env(name).ok_or_else(|| name.to_string());
        let client_secret = get("OIDC_CLIENT_SECRET")?;
        Ok(SpConfig {
            issuer: get("ISSUER")?.trim_end_matches('/').to_string(),
            client_id: get("OIDC_CLIENT_ID")?,
            redirect_uri: get("OIDC_REDIRECT_URI")?,
            cookie_secret: env("SP_COOKIE_SECRET").unwrap_or_else(|| client_secret.clone()),
            client_secret,
            verify: env("SP_VERIFY").unwrap_or_else(|| "userinfo".to_string()),
        })
    }
}

/// The provider for `config`: with `SP_VERIFY=jwks` the discovered one (the
/// standard OIDC path: `id_token` checked against the JWKS); otherwise the
/// endpoints by hand with no `jwks_uri`, so `OidcAuth` takes its OAuth-only
/// path and asks `/userinfo` who signed in.
fn provider(config: &SpConfig) -> Result<OidcProvider, String> {
    if config.verify == "jwks" {
        return OidcProvider::discover(&config.issuer).map_err(|e| format!("discovery failed: {}", e.0));
    }
    Ok(OidcProvider {
        issuer: config.issuer.clone(),
        authorization_endpoint: format!("{}/oauth/authorize", config.issuer),
        token_endpoint: format!("{}/oauth/token", config.issuer),
        jwks_uri: String::new(),
        userinfo_endpoint: Some(format!("{}/userinfo", config.issuer)),
        end_session_endpoint: None,
    })
}

pub fn sp(config: SpConfig) -> Root {
    let provider = match provider(&config) {
        Ok(p) => p,
        Err(e) => return unconfigured("sp", e),
    };
    let callback_path = url_path(&config.redirect_uri);
    let oidc = OidcConfig {
        provider,
        client_id: config.client_id.clone(),
        client_secret: config.client_secret.clone(),
        redirect_uri: config.redirect_uri.clone(),
        scopes: vec!["openid".into()],
        post_login_redirect: "/sp/me".into(),
    };
    let auth = OidcAuth::new(oidc, Arc::new(SessionStore::new(SESSION_TTL)))
        // Works across replicas: no server-side session.
        .signed_cookies(config.cookie_secret.clone().into_bytes())
        .callback_path(&callback_path)
        .exclude("/healthz");
    let verify = config.verify.clone();
    let app = App::with_state(verify)
        .get("/healthz", |_, _, _, _| text(S.n200_ok, "ok"))
        .get("/sp/me", |request, _, _, verify| match OidcAuth::claims(request) {
            Some(c) => json(S.n200_ok, &serde_json::json!({
                "sub": c.sub, "email": c.email, "name": c.name, "verified_by": verify,
            })),
            None => text(S.n401_unauthorized, "not signed in"),
        })
        .wrap(auth);
    Root::new(app)
}

/// The path of an absolute URL (`https://h/x/y?z` → `/x/y`).
fn url_path(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    match rest.find('/') {
        Some(i) => rest[i..].split('?').next().unwrap_or("/").to_string(),
        None => "/".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    fn idp_app() -> Root {
        idp(IdpConfig {
            issuer: "https://idp.e2e.test".into(),
            client_id: "sp".into(),
            client_secret: "sp-secret".into(),
            redirect_uri: "https://sp.e2e.test/auth/callback".into(),
            signing_secret: "signing".into(),
        })
    }

    fn cookie_pair(set_cookie: &str) -> String {
        set_cookie.split(';').next().unwrap().to_string()
    }

    #[test]
    fn idp_code_flow_issues_a_token_for_the_logged_in_user() {
        let client = TestClient::new(idp_app());
        let authorize = "/oauth/authorize?response_type=code&client_id=sp&redirect_uri=https%3A%2F%2Fsp.e2e.test%2Fauth%2Fcallback&state=xyz";
        let r = client.get(authorize).send();
        assert_eq!(302, r.status());
        let to_login = r.header("Location").unwrap().to_string();
        assert!(to_login.starts_with("/login?return_to="), "{to_login}");

        let r = client.get(&format!("{to_login}&user=alice")).send();
        assert_eq!(302, r.status());
        let cookie = cookie_pair(r.header("Set-Cookie").unwrap());
        assert!(cookie.starts_with("_rws_authz_sid="));
        let back = r.header("Location").unwrap().to_string();
        assert!(back.starts_with("/oauth/authorize?"), "{back}");

        let r = client.get(&back).header("Cookie", &cookie).send();
        assert_eq!(302, r.status());
        let location = r.header("Location").unwrap().to_string();
        assert!(location.starts_with("https://sp.e2e.test/auth/callback?code="), "{location}");
        assert!(location.ends_with("&state=xyz"));
        let code = location.split("code=").nth(1).unwrap().split('&').next().unwrap().to_string();

        let form = format!("grant_type=authorization_code&code={code}&client_id=sp&client_secret=sp-secret&redirect_uri=https%3A%2F%2Fsp.e2e.test%2Fauth%2Fcallback");
        let r = client.post("/oauth/token").header("Content-Type", "application/x-www-form-urlencoded").body_bytes(form.into_bytes()).send();
        assert_eq!(200, r.status(), "{}", r.body_text());
        let body: serde_json::Value = serde_json::from_str(r.body_text()).unwrap();
        let token = body["access_token"].as_str().unwrap();
        let claims = rust_web_server::auth::verify_jwt(token, b"signing").unwrap();
        assert_eq!(Some("alice".to_string()), claims.sub);

        let r = client.get("/userinfo").header("Authorization", &format!("Bearer {token}")).send();
        assert_eq!(200, r.status());
        assert!(r.body_text().contains(r#""sub":"alice""#));
        assert_eq!(401, client.get("/userinfo").header("Authorization", "Bearer nope").send().status());
    }

    #[test]
    fn idp_publishes_an_empty_jwks() {
        let client = TestClient::new(idp_app());
        assert_eq!(r#"{"keys":[]}"#, client.get("/.well-known/jwks.json").send().body_text());
        let discovery = client.get("/.well-known/openid-configuration").send();
        assert!(discovery.body_text().contains(r#""issuer":"https://idp.e2e.test""#));
    }

    #[test]
    fn login_refuses_absolute_return_to() {
        let client = TestClient::new(idp_app());
        assert_eq!(400, client.get("/login?user=a&return_to=https://evil.example/").send().status());
        assert_eq!(400, client.get("/login?return_to=/").send().status());
    }

    #[test]
    fn sp_redirects_to_login_and_keeps_healthz_public() {
        let app = sp(SpConfig {
            issuer: "https://idp.e2e.test".into(),
            client_id: "sp".into(),
            client_secret: "sp-secret".into(),
            redirect_uri: "https://sp.e2e.test/auth/callback".into(),
            cookie_secret: "cookie".into(),
            verify: "userinfo".into(),
        });
        let client = TestClient::new(app);
        assert_eq!("ok", client.get("/healthz").send().body_text());
        let r = client.get("/sp/me").send();
        assert_eq!(302, r.status());
        let r = client.get("/auth/login").send();
        assert_eq!(302, r.status());
        let location = r.header("Location").unwrap();
        assert!(location.starts_with("https://idp.e2e.test/oauth/authorize?response_type=code&client_id=sp"), "{location}");
        assert!(!location.contains("code_challenge"), "no PKCE without a jwks_uri: {location}");
    }

    #[test]
    fn unconfigured_role_names_the_variable() {
        let client = TestClient::new(unconfigured("idp", "ISSUER".into()));
        assert_eq!(200, client.get("/healthz").send().status());
        let r = client.get("/oauth/authorize").send();
        assert_eq!(503, r.status());
        assert!(r.body_text().contains("ISSUER"));
    }

    #[test]
    fn url_paths() {
        assert_eq!("/auth/callback", url_path("https://sp.e2e.test/auth/callback?x=1"));
        assert_eq!("/", url_path("https://sp.e2e.test"));
    }
}
