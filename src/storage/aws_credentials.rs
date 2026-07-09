//! AWS workload-identity credential providers for `S3Storage` — no AWS SDK.
//!
//! When `RWS_S3_ACCESS_KEY`/`RWS_S3_SECRET_KEY` are not both set,
//! [`CredentialsProvider`] auto-detects short-lived credentials from the
//! environment instead, mirroring the precedence AWS's own SDKs use:
//!
//! 1. EKS IRSA — `AWS_ROLE_ARN` + `AWS_WEB_IDENTITY_TOKEN_FILE` (injected by
//!    the EKS pod-identity webhook), via STS `AssumeRoleWithWebIdentity`.
//! 2. ECS task role — `AWS_CONTAINER_CREDENTIALS_FULL_URI` or
//!    `_RELATIVE_URI` (injected by the ECS agent).
//! 3. EC2 IMDSv2 — last resort; each request uses a short timeout so a
//!    non-EC2 host (local dev, CI, GCP/Azure) fails fast instead of hanging.
//!
//! Set `RWS_S3_CREDENTIAL_SOURCE=static|irsa|ecs|imds` to force a source and
//! skip detection entirely. Credentials are cached in memory and refreshed
//! shortly before they expire.
//!
//! No XML/JSON parser dependency: STS returns XML, IMDS/ECS return JSON, and
//! both are read with small hand-rolled tag/field extractors — the same
//! approach already used for JWT claims in `crate::auth`.

use super::StorageError;
use crate::http_client::Client;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const REFRESH_MARGIN_SECS: u64 = 120;
const IMDS_HOST: &str = "http://169.254.169.254";
const ECS_HOST: &str = "http://169.254.170.2";
const HTTP_TIMEOUT_MS: u64 = 5_000;
const IMDS_TIMEOUT_MS: u64 = 400;

// ── Credentials ──────────────────────────────────────────────────────────────

/// A set of AWS credentials — either static long-lived keys, or short-lived
/// credentials obtained from IRSA/ECS/IMDS.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Credentials {
    pub access_key: String,
    pub secret_key: String,
    pub session_token: Option<String>,
    /// Unix epoch seconds at which these credentials expire. `None` for
    /// static keys (never refreshed). A source that returns an
    /// unparseable/missing `Expiration` gets `Some(0)` (already expired)
    /// rather than `None`, so a bad expiration format fails safe by forcing
    /// a refetch on every call instead of caching forever.
    pub expires_at_epoch_secs: Option<u64>,
}

impl Credentials {
    fn is_fresh(&self, now_epoch_secs: u64, refresh_margin_secs: u64) -> bool {
        match self.expires_at_epoch_secs {
            None => true,
            Some(exp) => now_epoch_secs + refresh_margin_secs < exp,
        }
    }
}

// ── Source ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Source {
    Static(Credentials),
    Irsa { role_arn: String, token_file: String },
    EcsRelative { path: String },
    EcsFull { url: String, auth_token: Option<String> },
    Imds,
}

fn detect_auto(static_access_key: Option<String>, static_secret_key: Option<String>) -> Source {
    if let (Some(access_key), Some(secret_key)) = (static_access_key, static_secret_key) {
        return Source::Static(Credentials {
            access_key,
            secret_key,
            session_token: None,
            expires_at_epoch_secs: None,
        });
    }
    let role_arn = std::env::var("AWS_ROLE_ARN").ok();
    let token_file = std::env::var("AWS_WEB_IDENTITY_TOKEN_FILE").ok();
    if let (Some(role_arn), Some(token_file)) = (role_arn, token_file) {
        return Source::Irsa { role_arn, token_file };
    }
    if let Ok(url) = std::env::var("AWS_CONTAINER_CREDENTIALS_FULL_URI") {
        return Source::EcsFull { url, auth_token: std::env::var("AWS_CONTAINER_AUTHORIZATION_TOKEN").ok() };
    }
    if let Ok(path) = std::env::var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI") {
        return Source::EcsRelative { path };
    }
    Source::Imds
}

fn ecs_source_from_env() -> Source {
    if let Ok(url) = std::env::var("AWS_CONTAINER_CREDENTIALS_FULL_URI") {
        Source::EcsFull { url, auth_token: std::env::var("AWS_CONTAINER_AUTHORIZATION_TOKEN").ok() }
    } else {
        Source::EcsRelative { path: std::env::var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI").unwrap_or_default() }
    }
}

// ── CredentialsProvider ──────────────────────────────────────────────────────

/// Resolves and caches [`Credentials`] for one [`super::S3Storage`] instance.
///
/// Detection (which `Source` to use) is pure env-var reads and never fails.
/// The actual network fetch happens lazily on the first [`Self::get`] call
/// and is cached until shortly before expiry — no I/O happens at
/// construction time, matching every other constructor in this crate.
pub(crate) struct CredentialsProvider {
    source: Source,
    region: String,
    client: Client,
    cached: Mutex<Option<Credentials>>,
}

impl CredentialsProvider {
    /// Env-var-only detection, zero I/O. `static_access_key`/`static_secret_key`
    /// are `None` when `S3Config`'s corresponding field was empty.
    pub(crate) fn detect(region: &str, static_access_key: Option<String>, static_secret_key: Option<String>) -> Self {
        let source = match std::env::var("RWS_S3_CREDENTIAL_SOURCE").ok().as_deref() {
            Some("static") => Source::Static(Credentials {
                access_key: static_access_key.unwrap_or_default(),
                secret_key: static_secret_key.unwrap_or_default(),
                session_token: None,
                expires_at_epoch_secs: None,
            }),
            Some("irsa") => Source::Irsa {
                role_arn: std::env::var("AWS_ROLE_ARN").unwrap_or_default(),
                token_file: std::env::var("AWS_WEB_IDENTITY_TOKEN_FILE").unwrap_or_default(),
            },
            Some("ecs") => ecs_source_from_env(),
            Some("imds") => Source::Imds,
            // "auto", unset, or an unrecognized value all fall back to the
            // safe precedence chain rather than hard-failing at construction.
            _ => detect_auto(static_access_key, static_secret_key),
        };
        Self::new(source, region)
    }

    fn new(source: Source, region: &str) -> Self {
        CredentialsProvider { source, region: region.to_string(), client: Client::new(), cached: Mutex::new(None) }
    }

    /// Returns cached credentials if fresh, else fetches, caches, and
    /// returns new ones.
    pub(crate) fn get(&self) -> Result<Credentials, StorageError> {
        let now = epoch_now();
        {
            let guard = self.cached.lock().unwrap();
            if let Some(c) = guard.as_ref() {
                if c.is_fresh(now, REFRESH_MARGIN_SECS) {
                    return Ok(c.clone());
                }
            }
        }
        // Lock is intentionally dropped before the network fetch: two
        // threads racing past an expired cache may both fetch concurrently.
        // Both succeed independently and the cache just keeps the
        // last-written value — simpler than a wait-for-in-flight-fetch state
        // machine, and harmless at the request rate this is used at.
        let fresh = self.fetch()?;
        *self.cached.lock().unwrap() = Some(fresh.clone());
        Ok(fresh)
    }

    fn fetch(&self) -> Result<Credentials, StorageError> {
        match &self.source {
            Source::Static(creds) => Ok(creds.clone()),
            Source::Irsa { role_arn, token_file } => {
                let sts_base_url = format!("https://sts.{}.amazonaws.com", self.region);
                fetch_irsa_credentials(&self.client, &sts_base_url, role_arn, token_file)
            }
            Source::EcsRelative { path } => {
                if path.is_empty() {
                    return Err(StorageError::new(
                        "ECS task role credentials requested but neither AWS_CONTAINER_CREDENTIALS_RELATIVE_URI nor _FULL_URI is set",
                    ));
                }
                fetch_ecs_credentials(&self.client, &format!("{ECS_HOST}{path}"), None)
            }
            Source::EcsFull { url, auth_token } => fetch_ecs_credentials(&self.client, url, auth_token.as_deref()),
            Source::Imds => fetch_imds_credentials(&self.client, IMDS_HOST),
        }
    }
}

fn epoch_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ── IRSA (EKS) — STS AssumeRoleWithWebIdentity ──────────────────────────────

fn fetch_irsa_credentials(client: &Client, sts_base_url: &str, role_arn: &str, token_file: &str) -> Result<Credentials, StorageError> {
    if role_arn.is_empty() || token_file.is_empty() {
        return Err(StorageError::new(
            "IRSA credentials requested but AWS_ROLE_ARN and/or AWS_WEB_IDENTITY_TOKEN_FILE is not set",
        ));
    }
    let token = std::fs::read_to_string(token_file)
        .map_err(|e| StorageError::new(format!("reading AWS_WEB_IDENTITY_TOKEN_FILE '{token_file}': {e}")))?;
    let token = token.trim();

    let url = format!(
        "{sts_base_url}/?Action=AssumeRoleWithWebIdentity&Version=2011-06-15&RoleArn={}&WebIdentityToken={}&RoleSessionName=rws-s3",
        url_search_params::encode_uri_component(role_arn),
        url_search_params::encode_uri_component(token),
    );
    let resp = client
        .get(&url)
        .timeout_ms(HTTP_TIMEOUT_MS)
        .send()
        .map_err(|e| StorageError::new(format!("STS AssumeRoleWithWebIdentity request failed: {e}")))?;
    if !resp.is_success() {
        return Err(StorageError::new(format!(
            "STS AssumeRoleWithWebIdentity failed: HTTP {} {}",
            resp.status(),
            resp.text().unwrap_or_default()
        )));
    }
    let body = resp.text().map_err(|e| StorageError::new(format!("reading STS response: {e}")))?;
    parse_sts_response(&body)
}

fn parse_sts_response(xml: &str) -> Result<Credentials, StorageError> {
    let access_key =
        extract_tag(xml, "AccessKeyId").ok_or_else(|| StorageError::new("STS AssumeRoleWithWebIdentity response missing AccessKeyId"))?;
    let secret_key = extract_tag(xml, "SecretAccessKey")
        .ok_or_else(|| StorageError::new("STS AssumeRoleWithWebIdentity response missing SecretAccessKey"))?;
    let session_token = extract_tag(xml, "SessionToken")
        .ok_or_else(|| StorageError::new("STS AssumeRoleWithWebIdentity response missing SessionToken"))?;
    let expires_at_epoch_secs = Some(extract_tag(xml, "Expiration").and_then(|s| parse_iso8601_epoch(&s)).unwrap_or(0));
    Ok(Credentials { access_key, secret_key, session_token: Some(session_token), expires_at_epoch_secs })
}

// ── EC2 IMDSv2 ───────────────────────────────────────────────────────────────

fn fetch_imds_credentials(client: &Client, imds_base_url: &str) -> Result<Credentials, StorageError> {
    let token_resp = client
        .put(&format!("{imds_base_url}/latest/api/token"))
        .header("X-aws-ec2-metadata-token-ttl-seconds", "21600")
        .timeout_ms(IMDS_TIMEOUT_MS)
        .send()
        .map_err(|e| StorageError::new(format!("IMDSv2 token request failed (not running on EC2? {e})")))?;
    if !token_resp.is_success() {
        return Err(StorageError::new(format!("IMDSv2 token request failed: HTTP {}", token_resp.status())));
    }
    let token = token_resp.text().map_err(|e| StorageError::new(format!("reading IMDSv2 token: {e}")))?;
    let token = token.trim();

    let role_list_resp = client
        .get(&format!("{imds_base_url}/latest/meta-data/iam/security-credentials/"))
        .header("X-aws-ec2-metadata-token", token)
        .timeout_ms(IMDS_TIMEOUT_MS)
        .send()
        .map_err(|e| StorageError::new(format!("IMDSv2 role list request failed: {e}")))?;
    if !role_list_resp.is_success() {
        return Err(StorageError::new(format!("IMDSv2 role list request failed: HTTP {}", role_list_resp.status())));
    }
    let role_list = role_list_resp.text().map_err(|e| StorageError::new(format!("reading IMDSv2 role list: {e}")))?;
    let role = role_list
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| StorageError::new("IMDSv2: no IAM role attached to this instance"))?;

    let creds_resp = client
        .get(&format!("{imds_base_url}/latest/meta-data/iam/security-credentials/{role}"))
        .header("X-aws-ec2-metadata-token", token)
        .timeout_ms(IMDS_TIMEOUT_MS)
        .send()
        .map_err(|e| StorageError::new(format!("IMDSv2 credentials request failed: {e}")))?;
    if !creds_resp.is_success() {
        return Err(StorageError::new(format!("IMDSv2 credentials request failed: HTTP {}", creds_resp.status())));
    }
    let body = creds_resp.text().map_err(|e| StorageError::new(format!("reading IMDSv2 credentials: {e}")))?;
    parse_json_credentials(&body)
}

// ── ECS task role ────────────────────────────────────────────────────────────

fn fetch_ecs_credentials(client: &Client, url: &str, auth_token: Option<&str>) -> Result<Credentials, StorageError> {
    let mut builder = client.get(url).timeout_ms(HTTP_TIMEOUT_MS);
    if let Some(tok) = auth_token {
        builder = builder.header("Authorization", tok);
    }
    let resp = builder.send().map_err(|e| StorageError::new(format!("ECS task role credentials request failed: {e}")))?;
    if !resp.is_success() {
        return Err(StorageError::new(format!("ECS task role credentials request failed: HTTP {}", resp.status())));
    }
    let body = resp.text().map_err(|e| StorageError::new(format!("reading ECS task role credentials: {e}")))?;
    parse_json_credentials(&body)
}

// ── Shared JSON credentials shape (IMDS + ECS) ──────────────────────────────

fn parse_json_credentials(json: &str) -> Result<Credentials, StorageError> {
    let access_key = extract_json_str_field(json, "AccessKeyId").ok_or_else(|| StorageError::new("credentials response missing AccessKeyId"))?;
    let secret_key =
        extract_json_str_field(json, "SecretAccessKey").ok_or_else(|| StorageError::new("credentials response missing SecretAccessKey"))?;
    let token = extract_json_str_field(json, "Token").ok_or_else(|| StorageError::new("credentials response missing Token"))?;
    let expires_at_epoch_secs = Some(extract_json_str_field(json, "Expiration").and_then(|s| parse_iso8601_epoch(&s)).unwrap_or(0));
    Ok(Credentials { access_key, secret_key, session_token: Some(token), expires_at_epoch_secs })
}

// ── Hand-rolled extraction (no XML/JSON parser dependency) ──────────────────

/// Finds the first `<tag>VALUE</tag>` in `xml` and returns `VALUE`. Mirrors
/// `crate::auth`'s hand-rolled JSON claim extraction, applied to XML tags.
fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let rest = &xml[start..];
    let end = rest.find(&close)?;
    Some(rest[..end].to_string())
}

/// Finds `"field":"VALUE"` in `json` and returns `VALUE`. Same idiom as
/// `crate::auth::extract_string_claim` — avoids a JSON parser dependency.
fn extract_json_str_field(json: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let start = json.find(key.as_str())?;
    let rest = json[start + key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

/// Parses `YYYY-MM-DDTHH:MM:SSZ` (the format AWS uses for credential
/// `Expiration` fields) into Unix epoch seconds. `None` on any unexpected
/// format — callers treat that as "already expired" so a parse failure
/// fails safe (forces a refetch) rather than caching forever.
fn parse_iso8601_epoch(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let year: u32 = s.get(0..4)?.parse().ok()?;
    let month: u32 = s.get(5..7)?.parse().ok()?;
    let day: u32 = s.get(8..10)?.parse().ok()?;
    let hour: u64 = s.get(11..13)?.parse().ok()?;
    let min: u64 = s.get(14..16)?.parse().ok()?;
    let sec: u64 = s.get(17..19)?.parse().ok()?;
    let days = crate::scheduler::cron::ymd_to_days(year, month, day);
    Some(days * 86400 + hour * 3600 + min * 60 + sec)
}

// ── test-only shared env-var lock ───────────────────────────────────────────

/// Guards tests (in this module and in `super::s3::tests`) that read/write
/// `AWS_ROLE_ARN`, `AWS_WEB_IDENTITY_TOKEN_FILE`,
/// `AWS_CONTAINER_CREDENTIALS_*`, or `RWS_S3_CREDENTIAL_SOURCE` — all
/// process-wide env vars, shared across the whole `cargo test` binary. A
/// single lock here (rather than one per test file) is required because
/// `s3::tests` also needs to set these vars to exercise dynamic credentials
/// end-to-end, and a separate lock per file wouldn't prevent the two files'
/// tests from interleaving on the same real env vars.
#[cfg(test)]
pub(crate) fn credential_env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[cfg(test)]
mod tests;
