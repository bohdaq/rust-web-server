//! Google Cloud credentials for `GcsStorage` — no Google SDK.
//!
//! An OAuth 2.0 access token, found the way Google's client libraries look
//! for Application Default Credentials:
//!
//! 1. `RWS_GCS_ACCESS_TOKEN` — a token you already have
//!    (`gcloud auth print-access-token`); used as is, never refreshed.
//! 2. `GOOGLE_APPLICATION_CREDENTIALS` — a service account key file (a JWT
//!    signed RS256 with its private key, exchanged at its `token_uri`; native
//!    builds only) or an `authorized_user` file (its refresh token).
//! 3. `~/.config/gcloud/application_default_credentials.json`, written by
//!    `gcloud auth application-default login`.
//! 4. The metadata server (Compute Engine, GKE Workload Identity, Cloud Run,
//!    Cloud Functions) at `GCE_METADATA_HOST` or `metadata.google.internal`.
//!
//! `RWS_GCS_CREDENTIAL_SOURCE=token|file|metadata|none` skips detection;
//! `none` sends no `Authorization` header (emulators such as
//! fake-gcs-server). Tokens are cached and refreshed two minutes before they
//! expire.

use super::StorageError;
use crate::http_client::Client;
use crate::service_discovery::json_lite::{self, JsonValue};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const REFRESH_MARGIN_SECS: u64 = 120;
const METADATA_TIMEOUT_MS: u64 = 1_000;
const HTTP_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_METADATA_HOST: &str = "metadata.google.internal";
const DEFAULT_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
pub(super) const SCOPE: &str = "https://www.googleapis.com/auth/devstorage.read_write";

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Source {
    Token(String),
    File(PathBuf),
    Metadata(String),
    Anonymous,
}

/// Where the credentials come from, from the environment alone (no I/O
/// beyond checking that the gcloud file exists).
pub(super) fn detect() -> Source {
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
    let metadata = || Source::Metadata(env("GCE_METADATA_HOST").unwrap_or_else(|| DEFAULT_METADATA_HOST.to_string()));
    let file = || env("GOOGLE_APPLICATION_CREDENTIALS").map(PathBuf::from).or_else(gcloud_file);
    match env("RWS_GCS_CREDENTIAL_SOURCE").as_deref() {
        Some("token") => Source::Token(env("RWS_GCS_ACCESS_TOKEN").unwrap_or_default()),
        Some("file") => file().map_or(Source::File(PathBuf::new()), Source::File),
        Some("metadata") => metadata(),
        Some("none") => Source::Anonymous,
        _ => {
            if let Some(token) = env("RWS_GCS_ACCESS_TOKEN") {
                Source::Token(token)
            } else if let Some(path) = file() {
                Source::File(path)
            } else {
                metadata()
            }
        }
    }
}

fn gcloud_file() -> Option<PathBuf> {
    let base = std::env::var_os("CLOUDSDK_CONFIG").map(PathBuf::from).or_else(|| {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("APPDATA"))?;
        Some(PathBuf::from(home).join(".config").join("gcloud"))
    })?;
    let path = base.join("application_default_credentials.json");
    path.is_file().then_some(path)
}

/// Resolves and caches the access token for one `GcsStorage`.
pub(super) struct GcsCredentialsProvider {
    source: Source,
    client: Client,
    cached: Mutex<Option<(String, u64)>>,
}

impl GcsCredentialsProvider {
    pub(super) fn new(source: Source) -> Self {
        GcsCredentialsProvider { source, client: Client::new(), cached: Mutex::new(None) }
    }

    /// The `Authorization` header value, or `None` for [`Source::Anonymous`].
    pub(super) fn authorization(&self) -> Result<Option<String>, StorageError> {
        let token = match &self.source {
            Source::Anonymous => return Ok(None),
            Source::Token(token) if token.is_empty() => {
                return Err(StorageError::new("RWS_GCS_CREDENTIAL_SOURCE=token but RWS_GCS_ACCESS_TOKEN is not set"))
            }
            Source::Token(token) => token.clone(),
            Source::File(_) | Source::Metadata(_) => {
                let now = epoch_now();
                if let Some((token, expires_at)) = self.cached.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                    if now + REFRESH_MARGIN_SECS < *expires_at {
                        return Ok(Some(format!("Bearer {token}")));
                    }
                }
                // Not holding the lock while fetching: concurrent callers may
                // each fetch once, which is harmless.
                let (token, expires_in) = self.fetch()?;
                *self.cached.lock().unwrap_or_else(|e| e.into_inner()) = Some((token.clone(), now + expires_in));
                token
            }
        };
        Ok(Some(format!("Bearer {token}")))
    }

    fn fetch(&self) -> Result<(String, u64), StorageError> {
        match &self.source {
            Source::Metadata(host) => {
                let url = format!("http://{host}/computeMetadata/v1/instance/service-accounts/default/token");
                let resp = self
                    .client
                    .get(&url)
                    .header("Metadata-Flavor", "Google")
                    .timeout_ms(METADATA_TIMEOUT_MS)
                    .send()
                    .map_err(|e| StorageError::new(format!("GCS: no credentials (no RWS_GCS_ACCESS_TOKEN, GOOGLE_APPLICATION_CREDENTIALS or gcloud login, and the metadata server at {host} didn't answer: {e})")))?;
                token_from(resp, "metadata server")
            }
            Source::File(path) => {
                if path.as_os_str().is_empty() {
                    return Err(StorageError::new("RWS_GCS_CREDENTIAL_SOURCE=file but no credentials file was found"));
                }
                let text = std::fs::read_to_string(path)
                    .map_err(|e| StorageError::new(format!("GCS: can't read credentials file {}: {e}", path.display())))?;
                let json = json_lite::parse(&text)
                    .map_err(|e| StorageError::new(format!("GCS: credentials file {} isn't JSON: {e}", path.display())))?;
                let field = |name: &str| json.get(name).and_then(JsonValue::as_str).unwrap_or("").to_string();
                let token_uri = Some(field("token_uri")).filter(|u| !u.is_empty()).unwrap_or_else(|| DEFAULT_TOKEN_URI.to_string());
                match field("type").as_str() {
                    "service_account" => self.service_account_token(&field("client_email"), &field("private_key"), &token_uri),
                    "authorized_user" => {
                        let resp = self
                            .client
                            .post(&token_uri)
                            .form(&[
                                ("grant_type", "refresh_token"),
                                ("client_id", &field("client_id")),
                                ("client_secret", &field("client_secret")),
                                ("refresh_token", &field("refresh_token")),
                            ])
                            .timeout_ms(HTTP_TIMEOUT_MS)
                            .send()
                            .map_err(|e| StorageError::new(format!("GCS: token request to {token_uri} failed: {e}")))?;
                        token_from(resp, &token_uri)
                    }
                    other => Err(StorageError::new(format!(
                        "GCS: credentials file {} has type {other:?}; service_account and authorized_user are supported",
                        path.display()
                    ))),
                }
            }
            Source::Token(_) | Source::Anonymous => unreachable!("no token to fetch"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn service_account_token(&self, email: &str, private_key_pem: &str, token_uri: &str) -> Result<(String, u64), StorageError> {
        let assertion = service_account_jwt(email, private_key_pem, token_uri, epoch_now())?;
        let resp = self
            .client
            .post(token_uri)
            .form(&[("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"), ("assertion", &assertion)])
            .timeout_ms(HTTP_TIMEOUT_MS)
            .send()
            .map_err(|e| StorageError::new(format!("GCS: token request to {token_uri} failed: {e}")))?;
        token_from(resp, token_uri)
    }

    #[cfg(target_arch = "wasm32")]
    fn service_account_token(&self, _: &str, _: &str, _: &str) -> Result<(String, u64), StorageError> {
        Err(StorageError::new("GCS: service account key files need a native build; use RWS_GCS_ACCESS_TOKEN"))
    }
}

/// The signed JWT a service account trades for an access token (RFC 7523).
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn service_account_jwt(email: &str, private_key_pem: &str, token_uri: &str, now: u64) -> Result<String, StorageError> {
    if email.is_empty() || private_key_pem.is_empty() {
        return Err(StorageError::new("GCS: the service account file has no client_email or private_key"));
    }
    let der = pem_body(private_key_pem).ok_or_else(|| StorageError::new("GCS: the service account's private_key isn't a PEM private key"))?;
    let key = aws_lc_rs::rsa::KeyPair::from_pkcs8(&der)
        .map_err(|e| StorageError::new(format!("GCS: the service account's private_key can't be used: {e}")))?;
    let claims = format!(
        r#"{{"iss":{},"scope":"{SCOPE}","aud":{},"iat":{now},"exp":{}}}"#,
        json_string(email),
        json_string(token_uri),
        now + 3600
    );
    let signing_input = format!("{}.{}", base64url(br#"{"alg":"RS256","typ":"JWT"}"#), base64url(claims.as_bytes()));
    let mut signature = vec![0u8; key.public_modulus_len()];
    key.sign(&aws_lc_rs::signature::RSA_PKCS1_SHA256, &aws_lc_rs::rand::SystemRandom::new(), signing_input.as_bytes(), &mut signature)
        .map_err(|e| StorageError::new(format!("GCS: signing the token request failed: {e}")))?;
    Ok(format!("{signing_input}.{}", base64url(&signature)))
}

fn token_from(resp: crate::http_client::Response, from: &str) -> Result<(String, u64), StorageError> {
    let body = resp.text().unwrap_or_default();
    if !resp.is_success() {
        return Err(StorageError::new(format!("GCS: token request to {from} failed: HTTP {} {body}", resp.status())));
    }
    let json = json_lite::parse(&body).map_err(|e| StorageError::new(format!("GCS: token response from {from} isn't JSON: {e}")))?;
    let token = json
        .get("access_token")
        .and_then(JsonValue::as_str)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| StorageError::new(format!("GCS: token response from {from} has no access_token")))?;
    let expires_in = json.get("expires_in").and_then(JsonValue::as_f64).unwrap_or(3600.0).max(0.0) as u64;
    Ok((token.to_string(), expires_in))
}

fn epoch_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub(super) fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..=chunk.len() {
            out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// The DER inside a PEM block (standard base64, any line breaks; the JSON
/// key file writes them as `\n`, which `json_lite` has already decoded).
pub(super) fn pem_body(pem: &str) -> Option<Vec<u8>> {
    let start = pem.find("-----BEGIN ")?;
    let body_start = start + pem[start..].find('\n')? + 1;
    let body_end = body_start + pem[body_start..].find("-----END ")?;
    let out = crate::core::base64::Decoder::STANDARD.skip_whitespace().decode(&pem[body_start..body_end]).ok()?;
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests;
