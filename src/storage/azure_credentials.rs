//! Azure Storage credential resolution for `AzureBlobStorage` — no Azure SDK.
//!
//! When `RWS_AZURE_ACCOUNT_KEY` is unset, [`AzureCredentialsProvider`]
//! auto-detects a Managed Identity OAuth token from the environment instead:
//!
//! 1. **App Service / Container Apps** — `IDENTITY_ENDPOINT` +
//!    `IDENTITY_HEADER` (injected by the platform).
//! 2. **VM / AKS IMDS** — last resort; each request uses a short timeout so
//!    a non-Azure host (local dev, CI, AWS/GCP) fails fast instead of
//!    hanging.
//!
//! Set `RWS_AZURE_CREDENTIAL_SOURCE=key|managed-identity` to force a source
//! and skip detection. Tokens are cached in memory and refreshed shortly
//! before they expire — unlike the AWS `Expiration` field, Azure's
//! `expires_on` is already Unix epoch seconds, so no date parsing is needed.

use super::StorageError;
use crate::http_client::Client;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const REFRESH_MARGIN_SECS: u64 = 120;
const IMDS_HOST: &str = "http://169.254.169.254";
const IMDS_TIMEOUT_MS: u64 = 400;
const HTTP_TIMEOUT_MS: u64 = 5_000;
const STORAGE_RESOURCE: &str = "https://storage.azure.com/";

/// A credential ready to attach to a request: either the account's Shared
/// Key (used with `azure_signature::sign`) or a Managed Identity bearer
/// token (used directly as `Authorization: Bearer {token}`).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Credential {
    SharedKey(String),
    Bearer(String),
}

#[derive(Debug, Clone, PartialEq)]
enum IdentityEndpoint {
    AppService { endpoint: String, header: String },
    Imds,
}

#[derive(Debug, Clone, PartialEq)]
enum Source {
    SharedKey(String),
    ManagedIdentity(IdentityEndpoint),
}

fn managed_identity_endpoint_from_env() -> IdentityEndpoint {
    let endpoint = std::env::var("IDENTITY_ENDPOINT").ok();
    let header = std::env::var("IDENTITY_HEADER").ok();
    match (endpoint, header) {
        (Some(endpoint), Some(header)) => IdentityEndpoint::AppService { endpoint, header },
        _ => IdentityEndpoint::Imds,
    }
}

fn detect_auto(account_key: Option<String>) -> Source {
    match account_key {
        Some(key) => Source::SharedKey(key),
        None => Source::ManagedIdentity(managed_identity_endpoint_from_env()),
    }
}

/// Resolves and caches [`Credential`]s for one [`super::AzureBlobStorage`]
/// instance. Detection is pure env-var reads and never fails; the actual
/// token fetch (Managed Identity only — Shared Key needs no network) happens
/// lazily on the first [`Self::get`] call and is cached until shortly before
/// expiry.
pub(super) struct AzureCredentialsProvider {
    source: Source,
    client: Client,
    cached: Mutex<Option<(String, u64)>>,
}

impl AzureCredentialsProvider {
    /// Env-var-only detection, zero I/O. `account_key` is `None` when
    /// `AzureBlobConfig`'s corresponding field was empty.
    pub(super) fn detect(account_key: Option<String>) -> Self {
        let source = match std::env::var("RWS_AZURE_CREDENTIAL_SOURCE").ok().as_deref() {
            Some("key") => Source::SharedKey(account_key.unwrap_or_default()),
            Some("managed-identity") => Source::ManagedIdentity(managed_identity_endpoint_from_env()),
            // "auto", unset, or an unrecognized value all fall back to the
            // safe precedence chain rather than hard-failing at construction.
            _ => detect_auto(account_key),
        };
        Self::new(source)
    }

    fn new(source: Source) -> Self {
        AzureCredentialsProvider { source, client: Client::new(), cached: Mutex::new(None) }
    }

    pub(super) fn get(&self) -> Result<Credential, StorageError> {
        match &self.source {
            Source::SharedKey(key) => {
                if key.is_empty() {
                    return Err(StorageError::new(
                        "Shared Key credentials requested but RWS_AZURE_ACCOUNT_KEY is not set",
                    ));
                }
                Ok(Credential::SharedKey(key.clone()))
            }
            Source::ManagedIdentity(endpoint) => {
                let now = epoch_now();
                {
                    let guard = self.cached.lock().unwrap();
                    if let Some((token, expires_at)) = guard.as_ref() {
                        if now + REFRESH_MARGIN_SECS < *expires_at {
                            return Ok(Credential::Bearer(token.clone()));
                        }
                    }
                }
                // Lock intentionally dropped before the network fetch — see
                // the identical rationale in `aws_credentials::CredentialsProvider::get`.
                let (token, expires_at) = self.fetch_token(endpoint)?;
                *self.cached.lock().unwrap() = Some((token.clone(), expires_at));
                Ok(Credential::Bearer(token))
            }
        }
    }

    fn fetch_token(&self, endpoint: &IdentityEndpoint) -> Result<(String, u64), StorageError> {
        match endpoint {
            IdentityEndpoint::AppService { endpoint, header } => fetch_app_service_token(&self.client, endpoint, header),
            IdentityEndpoint::Imds => fetch_imds_token(&self.client, IMDS_HOST),
        }
    }
}

fn epoch_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ── VM / AKS IMDS ────────────────────────────────────────────────────────────

fn fetch_imds_token(client: &Client, imds_base_url: &str) -> Result<(String, u64), StorageError> {
    let url = format!(
        "{imds_base_url}/metadata/identity/oauth2/token?api-version=2018-02-01&resource={}",
        url_search_params::encode_uri_component(STORAGE_RESOURCE)
    );
    let resp = client
        .get(&url)
        .header("Metadata", "true")
        .timeout_ms(IMDS_TIMEOUT_MS)
        .send()
        .map_err(|e| StorageError::new(format!("Azure IMDS token request failed (not running on Azure? {e})")))?;
    if !resp.is_success() {
        return Err(StorageError::new(format!("Azure IMDS token request failed: HTTP {}", resp.status())));
    }
    let body = resp.text().map_err(|e| StorageError::new(format!("reading Azure IMDS token response: {e}")))?;
    parse_token_response(&body)
}

// ── App Service / Container Apps identity endpoint ──────────────────────────

fn fetch_app_service_token(client: &Client, identity_endpoint: &str, identity_header: &str) -> Result<(String, u64), StorageError> {
    let url = format!(
        "{identity_endpoint}?resource={}&api-version=2019-08-01",
        url_search_params::encode_uri_component(STORAGE_RESOURCE)
    );
    let resp = client
        .get(&url)
        .header("X-IDENTITY-HEADER", identity_header)
        .timeout_ms(HTTP_TIMEOUT_MS)
        .send()
        .map_err(|e| StorageError::new(format!("Azure managed identity token request failed: {e}")))?;
    if !resp.is_success() {
        return Err(StorageError::new(format!("Azure managed identity token request failed: HTTP {}", resp.status())));
    }
    let body = resp.text().map_err(|e| StorageError::new(format!("reading Azure managed identity token response: {e}")))?;
    parse_token_response(&body)
}

fn parse_token_response(json: &str) -> Result<(String, u64), StorageError> {
    let access_token =
        extract_json_str_field(json, "access_token").ok_or_else(|| StorageError::new("token response missing access_token"))?;
    // `expires_on` is already Unix epoch seconds as a string — no date
    // parsing needed (unlike AWS's ISO8601 `Expiration`).
    let expires_at = extract_json_str_field(json, "expires_on").and_then(|s| s.parse().ok()).unwrap_or(0);
    Ok((access_token, expires_at))
}

/// Finds `"field":"VALUE"` in `json` and returns `VALUE`. Same hand-rolled
/// idiom as `crate::storage::aws_credentials` (and `crate::auth`) — avoids a
/// JSON parser dependency.
fn extract_json_str_field(json: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let start = json.find(key.as_str())?;
    let rest = json[start + key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

// ── test-only shared env-var lock ───────────────────────────────────────────

/// Guards tests (in this module and in `super::azure_blob::tests`) that
/// read/write `IDENTITY_ENDPOINT`, `IDENTITY_HEADER`, or
/// `RWS_AZURE_CREDENTIAL_SOURCE` — all process-wide env vars shared across
/// the whole `cargo test` binary. See `aws_credentials::credential_env_lock`
/// for the identical rationale (and the race it was added to fix).
#[cfg(test)]
pub(crate) fn credential_env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[cfg(test)]
mod tests;
