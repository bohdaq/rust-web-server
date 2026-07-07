//! `SamlSp` — a SAML 2.0 Service Provider middleware.
//!
//! Enterprise / B2B SSO (AD FS, Okta SAML, Google Workspace SAML), as an
//! alternative to the OIDC-based [`super::OidcAuth`] for IdPs that only
//! speak SAML.
//!
//! # Usage
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use rust_web_server::app::App;
//! use rust_web_server::core::New;
//! use rust_web_server::session::SessionStore;
//! use rust_web_server::sso::saml::{SamlSp, SamlConfig, SamlIdpMetadata, AttributeMap};
//!
//! let saml_sp = SamlSp::new(SamlConfig {
//!     sp_entity_id: "https://myapp.com/saml/metadata".into(),
//!     sp_acs_url:   "https://myapp.com/saml/acs".into(),
//!     idp_metadata: SamlIdpMetadata::from_file("idp-metadata.xml").unwrap(),
//!     sessions:     Arc::new(SessionStore::new(86_400)),
//! })
//! .attribute_map(
//!     AttributeMap::new()
//!         .map("http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress", "email")
//!         .map("http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name", "name"),
//! );
//!
//! let app = App::new().wrap(saml_sp);
//! // Registers: GET /saml/metadata, GET /saml/login, POST /saml/acs, GET /saml/logout
//! ```
//!
//! # Deviations from the original design sketch
//!
//! - **No `quick-xml` dependency.** This module hand-rolls a small,
//!   purpose-built XML parser ([`xml`], private) instead — see that
//!   module's docs for why. `sso-saml` adds no new dependency beyond what
//!   `sso` already pulls in.
//! - **Signature verification is byte-exact, not full XML C14N** — see
//!   [`assertion`]'s (private) module docs for the detailed rationale and
//!   its fail-closed-not-fail-open safety argument. Only `RSA-SHA256` is
//!   supported.
//! - **`AuthnRequest`s use the HTTP-POST binding, not HTTP-Redirect.**
//!   HTTP-Redirect requires DEFLATE-compressing the request, and this
//!   crate has no compression dependency to reuse (`gzip` output
//!   elsewhere in this crate is hand-rolled for *responses*, not a
//!   general-purpose DEFLATE encoder suitable for reuse here). HTTP-POST
//!   is an equally spec-compliant binding — an auto-submitting HTML form
//!   POSTs the base64-encoded, *unsigned* `AuthnRequest` XML to the IdP.
//! - **`AuthnRequest`s are never signed** — `sign_requests`/
//!   `sp_private_key` from the design sketch don't exist. This crate has
//!   no private-key PEM/DER parser (the same reason [`super::server`]'s
//!   `AuthServer` signs its own tokens HS256 instead of RSA/EC), and most
//!   IdPs (Okta, Azure AD, Google Workspace, Keycloak) accept unsigned
//!   `AuthnRequest`s by default — the flow's security rests on the
//!   IdP-signed *Assertion*, which this module does verify.
//! - **Logout is local-only.** `/saml/logout` destroys the SP's own
//!   session and redirects home; there is no SP-initiated
//!   `LogoutRequest`/`LogoutResponse` exchange with the IdP (true SAML
//!   Single Logout). This mirrors [`super::OidcAuth`]'s own logout, which
//!   draws the identical boundary for OIDC's `end_session_endpoint`.
//! - **`AttributeMap` maps into a plain `name_id`/`attributes` struct
//!   ([`SamlClaims`]), not [`super::OidcClaims`].** The design sketch's own
//!   example maps a `groups` attribute, but `OidcClaims` (Phase 2) was
//!   deliberately built without a `groups` field — reusing it here would
//!   silently drop exactly the attribute the sketch's own example asks to
//!   map. SAML's free-form attribute model is a better fit for a plain map
//!   than OIDC's fixed claim set.

mod assertion;
mod der;
mod metadata;
mod xml;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::application::Application;
use crate::body::form_urlencoded::FormUrlEncoded;
use crate::core::New;
use crate::extract::{FromRequest, Query};
use crate::header::Header;
use crate::middleware::Middleware;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{Request, METHOD};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;
use crate::session::{destroy_cookie, session_cookie, session_id_from_request, SessionStore};

pub use metadata::SamlIdpMetadata;

use metadata::decode_base64_flexible;

const SESSION_COOKIE: &str = "_rws_saml_sid";
const SESSION_TTL: u64 = 86_400;
/// Request header name used to pass SAML claims into downstream handlers.
pub const CLAIMS_HEADER: &str = "X-Rws-Saml-Claims";
const METADATA_PATH: &str = "/saml/metadata";
const LOGIN_PATH: &str = "/saml/login";
const ACS_PATH: &str = "/saml/acs";
const LOGOUT_PATH: &str = "/saml/logout";

/// Maps IdP-specific SAML attribute names to caller-chosen field names.
///
/// # Example
///
/// ```rust
/// use rust_web_server::sso::saml::AttributeMap;
///
/// let map = AttributeMap::new()
///     .map("http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress", "email")
///     .map("http://schemas.microsoft.com/ws/2008/06/identity/claims/groups", "groups");
/// ```
#[derive(Clone, Default)]
pub struct AttributeMap {
    mappings: Vec<(String, String)>,
}

impl AttributeMap {
    /// An empty map — attributes are exposed only under their raw SAML
    /// `Attribute/@Name`.
    pub fn new() -> Self {
        AttributeMap { mappings: Vec::new() }
    }

    /// Map a raw SAML attribute name to a field name in [`SamlClaims::attributes`].
    pub fn map(mut self, saml_attribute_name: &str, field: &str) -> Self {
        self.mappings.push((saml_attribute_name.to_string(), field.to_string()));
        self
    }

    fn apply(&self, raw: &HashMap<String, String>) -> HashMap<String, String> {
        let mut out = HashMap::new();
        for (saml_name, field) in &self.mappings {
            if let Some(v) = raw.get(saml_name) {
                out.insert(field.clone(), v.clone());
            }
        }
        out
    }
}

/// The verified identity from a completed SAML login, injected as the
/// [`CLAIMS_HEADER`] request header (JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlClaims {
    /// The `Subject/NameID` value.
    pub name_id: String,
    /// Attributes translated via [`AttributeMap`] (mapped field name → value).
    pub attributes: HashMap<String, String>,
}

/// Configuration for [`SamlSp`].
pub struct SamlConfig {
    /// This SP's own entity ID (an arbitrary, globally-unique URI —
    /// conventionally its own metadata URL).
    pub sp_entity_id: String,
    /// This SP's Assertion Consumer Service URL — must exactly match what
    /// is registered with the IdP and what `SubjectConfirmationData/@Recipient`
    /// names on every assertion.
    pub sp_acs_url: String,
    /// The IdP's metadata (entity ID, SSO URL, signing certificate).
    pub idp_metadata: SamlIdpMetadata,
    /// Session store used to track the pre-login `AuthnRequest` ID and,
    /// after a successful login, the established [`SamlClaims`].
    pub sessions: Arc<SessionStore>,
}

/// A SAML 2.0 Service Provider — see the [module docs](self).
pub struct SamlSp {
    config: Arc<SamlConfig>,
    attribute_map: AttributeMap,
    post_login_redirect: String,
    login_path: String,
    acs_path: String,
    logout_path: String,
    metadata_path: String,
}

impl SamlSp {
    /// Create a new `SamlSp`. Defaults `post_login_redirect` to `/` and the
    /// four intercepted paths to `/saml/{metadata,login,acs,logout}`.
    pub fn new(config: SamlConfig) -> Self {
        SamlSp {
            config: Arc::new(config),
            attribute_map: AttributeMap::new(),
            post_login_redirect: "/".to_string(),
            login_path: LOGIN_PATH.to_string(),
            acs_path: ACS_PATH.to_string(),
            logout_path: LOGOUT_PATH.to_string(),
            metadata_path: METADATA_PATH.to_string(),
        }
    }

    /// Set the [`AttributeMap`] translating IdP attribute names to claim
    /// field names.
    pub fn attribute_map(mut self, map: AttributeMap) -> Self {
        self.attribute_map = map;
        self
    }

    /// Override the default post-login redirect path (default: `/`).
    pub fn post_login_redirect(mut self, path: &str) -> Self {
        self.post_login_redirect = path.to_string();
        self
    }

    /// Override the login path (default: `/saml/login`).
    pub fn login_path(mut self, path: &str) -> Self {
        self.login_path = path.to_string();
        self
    }

    /// Override the ACS path (default: `/saml/acs`).
    pub fn acs_path(mut self, path: &str) -> Self {
        self.acs_path = path.to_string();
        self
    }

    /// Override the logout path (default: `/saml/logout`).
    pub fn logout_path(mut self, path: &str) -> Self {
        self.logout_path = path.to_string();
        self
    }

    /// Override the SP metadata path (default: `/saml/metadata`).
    pub fn metadata_path(mut self, path: &str) -> Self {
        self.metadata_path = path.to_string();
        self
    }

    /// Read the SAML claims injected by `SamlSp` from a request. Returns
    /// `None` if the request did not pass through an authenticated
    /// `SamlSp` layer.
    pub fn claims(req: &Request) -> Option<SamlClaims> {
        req.headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(CLAIMS_HEADER))
            .and_then(|h| serde_json::from_str(&h.value).ok())
    }

    /// Shortcut: return the `NameID` from the injected claims.
    pub fn name_id(req: &Request) -> Option<String> {
        Self::claims(req).map(|c| c.name_id)
    }

    /// Shortcut: return a mapped attribute field from the injected claims.
    pub fn attr(req: &Request, field: &str) -> Option<String> {
        Self::claims(req).and_then(|c| c.attributes.get(field).cloned())
    }

    // ── route handlers ────────────────────────────────────────────────────────

    fn handle_metadata(&self) -> Response {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><EntityDescriptor xmlns="urn:oasis:names:tc:SAML:2.0:metadata" entityID="{entity}"><SPSSODescriptor AuthnRequestsSigned="false" WantAssertionsSigned="true" protocolSupportEnabled="urn:oasis:names:tc:SAML:2.0:protocol"><AssertionConsumerService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST" Location="{acs}" index="0" isDefault="true"/></SPSSODescriptor></EntityDescriptor>"#,
            entity = xml_escape(&self.config.sp_entity_id),
            acs = xml_escape(&self.config.sp_acs_url),
        );
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![Range::get_content_range(xml.into_bytes(), "application/samlmetadata+xml".to_string())];
        r
    }

    fn handle_login(&self, request: &Request) -> Response {
        let query = Query::from_request(request).map(|q| q.0).unwrap_or_default();
        let return_to = query.get("return_to").cloned().unwrap_or_else(|| self.post_login_redirect.clone());

        let request_id = format!("_{}", random_hex(16));
        let authn_request_xml = format!(
            r#"<samlp:AuthnRequest xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol" xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="{id}" Version="2.0" IssueInstant="{issued}" Destination="{dest}" AssertionConsumerServiceURL="{acs}" ProtocolBinding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"><saml:Issuer>{issuer}</saml:Issuer></samlp:AuthnRequest>"#,
            id = request_id,
            issued = iso8601_now(),
            dest = xml_escape(&self.config.idp_metadata.sso_url),
            acs = xml_escape(&self.config.sp_acs_url),
            issuer = xml_escape(&self.config.sp_entity_id),
        );
        let saml_request_b64 = base64_standard_encode(authn_request_xml.as_bytes());

        let mut session = self.config.sessions.create();
        session.set("_saml_request_id", &request_id);
        session.set("_saml_return_to", &return_to);
        self.config.sessions.save(&session);

        let html = format!(
            r#"<!doctype html><html><body onload="document.forms[0].submit()"><noscript><p>Click the button to continue.</p></noscript><form method="POST" action="{action}"><input type="hidden" name="SAMLRequest" value="{req}"/><noscript><button type="submit">Continue</button></noscript></form></body></html>"#,
            action = xml_escape(&self.config.idp_metadata.sso_url),
            req = xml_escape(&saml_request_b64),
        );
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
        r.content_range_list = vec![Range::get_content_range(html.into_bytes(), MimeType::TEXT_HTML.to_string())];
        r.headers.push(Header { name: "Set-Cookie".to_string(), value: session_cookie(&session.id, SESSION_COOKIE, SESSION_TTL) });
        r
    }

    fn handle_acs(&self, request: &Request) -> Response {
        let sid = match session_id_from_request(request, SESSION_COOKIE) {
            Some(id) => id,
            None => return Self::error_response("missing SP session cookie"),
        };
        let mut session = match self.config.sessions.load(&sid) {
            Some(s) => s,
            None => return Self::error_response("unknown or expired SP session"),
        };
        let request_id = session.get("_saml_request_id").map(|s| s.to_string());
        let return_to = session.get("_saml_return_to").unwrap_or(&self.post_login_redirect).to_string();

        let form = match FormUrlEncoded::parse(request.body.clone()) {
            Ok(f) => f,
            Err(e) => return Self::error_response(&format!("malformed ACS request body: {e}")),
        };
        let saml_response_b64 = match form.get("SAMLResponse") {
            Some(v) => v,
            None => return Self::error_response("missing SAMLResponse field"),
        };
        let raw_xml_bytes = match decode_base64_flexible(saml_response_b64) {
            Ok(b) => b,
            Err(e) => return Self::error_response(&e.0),
        };
        let raw_xml = match String::from_utf8(raw_xml_bytes) {
            Ok(s) => s,
            Err(_) => return Self::error_response("SAMLResponse is not valid UTF-8"),
        };

        let verified = assertion::parse_and_verify(
            &raw_xml,
            &self.config.idp_metadata.entity_id,
            &self.config.idp_metadata.signing_key,
            &self.config.sp_entity_id,
            &self.config.sp_acs_url,
            request_id.as_deref(),
            unix_now(),
        );
        let verified = match verified {
            Ok(v) => v,
            Err(e) => return Self::error_response(&e.0),
        };

        let claims = SamlClaims {
            name_id: verified.name_id,
            attributes: self.attribute_map.apply(&verified.attributes),
        };

        session.remove("_saml_request_id");
        session.remove("_saml_return_to");
        let claims_json = serde_json::to_string(&claims).unwrap_or_default();
        session.set("_saml_claims", &claims_json);
        self.config.sessions.save(&session);

        let mut r = Self::redirect(&return_to);
        r.headers.push(Header { name: "Set-Cookie".to_string(), value: session_cookie(&session.id, SESSION_COOKIE, SESSION_TTL) });
        r
    }

    fn handle_logout(&self, request: &Request) -> Response {
        if let Some(sid) = session_id_from_request(request, SESSION_COOKIE) {
            self.config.sessions.destroy(&sid);
        }
        let mut r = Self::redirect("/");
        r.headers.push(Header { name: "Set-Cookie".to_string(), value: destroy_cookie(SESSION_COOKIE) });
        r
    }

    fn redirect(url: &str) -> Response {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n302_found.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n302_found.reason_phrase.to_string();
        r.headers.push(Header { name: "Location".to_string(), value: url.to_string() });
        r
    }

    fn error_response(msg: &str) -> Response {
        let mut r = Response::new();
        r.status_code = *STATUS_CODE_REASON_PHRASE.n500_internal_server_error.status_code;
        r.reason_phrase = STATUS_CODE_REASON_PHRASE.n500_internal_server_error.reason_phrase.to_string();
        r.content_range_list = vec![Range::get_content_range(msg.as_bytes().to_vec(), MimeType::TEXT_PLAIN.to_string())];
        r
    }
}

impl Middleware for SamlSp {
    fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
        let path = request.request_uri.split('?').next().unwrap_or("");
        let is_get = request.method.eq_ignore_ascii_case(METHOD.get);
        let is_post = request.method.eq_ignore_ascii_case(METHOD.post);

        if path == self.metadata_path && is_get {
            return Ok(self.handle_metadata());
        }
        if path == self.login_path && is_get {
            return Ok(self.handle_login(request));
        }
        if path == self.acs_path && is_post {
            return Ok(self.handle_acs(request));
        }
        if path == self.logout_path && is_get {
            return Ok(self.handle_logout(request));
        }

        if let Some(sid) = session_id_from_request(request, SESSION_COOKIE) {
            if let Some(session) = self.config.sessions.load(&sid) {
                if let Some(claims_json) = session.get("_saml_claims") {
                    let mut req = request.clone();
                    req.headers.push(Header { name: CLAIMS_HEADER.to_string(), value: claims_json.to_string() });
                    return next.execute(&req, connection);
                }
            }
        }

        let return_to = crate::sso::client::url_encode(&request.request_uri);
        Ok(Self::redirect(&format!("{}?return_to={}", self.login_path, return_to)))
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn iso8601_now() -> String {
    format_iso8601(unix_now())
}

/// Format Unix seconds as `xs:dateTime` UTC (`2024-01-01T00:00:00Z`).
fn format_iso8601(total_secs: u64) -> String {
    let days = total_secs / 86_400;
    let secs_of_day = total_secs % 86_400;
    let (h, mi, s) = (secs_of_day / 3600, (secs_of_day / 60) % 60, secs_of_day % 60);
    let (y, m, d) = days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Inverse of `scheduler::cron::ymd_to_days` (days since the Unix epoch to
/// a proleptic-Gregorian calendar date) — Howard Hinnant's algorithm.
fn days_to_ymd(days_since_epoch: u64) -> (u32, u32, u32) {
    let z = days_since_epoch as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as u32, m, d)
}

fn random_hex(n_bytes: usize) -> String {
    let mut bytes = vec![0u8; n_bytes];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

const STD_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard (not base64url) base64 with `=` padding — the alphabet the
/// `SAMLRequest`/`SAMLResponse` form fields and `X509Certificate` elements
/// use per the SAML/XML-DSig specs.
fn base64_standard_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(((bytes.len() + 2) / 3) * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let (b0, b1, b2) = (bytes[i] as usize, bytes[i + 1] as usize, bytes[i + 2] as usize);
        out.push(STD_TABLE[b0 >> 2] as char);
        out.push(STD_TABLE[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(STD_TABLE[((b1 & 0xf) << 2) | (b2 >> 6)] as char);
        out.push(STD_TABLE[b2 & 0x3f] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let b0 = bytes[i] as usize;
        out.push(STD_TABLE[b0 >> 2] as char);
        out.push(STD_TABLE[(b0 & 3) << 4] as char);
        out.push_str("==");
    } else if rem == 2 {
        let (b0, b1) = (bytes[i] as usize, bytes[i + 1] as usize);
        out.push(STD_TABLE[b0 >> 2] as char);
        out.push(STD_TABLE[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(STD_TABLE[(b1 & 0xf) << 2] as char);
        out.push('=');
    }
    out
}

/// Minimal XML attribute/text escaping for values this SP embeds into XML
/// or HTML it generates itself (entity IDs, URLs, base64 payloads).
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
