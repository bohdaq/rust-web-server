//! AWS Secrets Manager secret resolution — `aws-sm://secret-name` (returns
//! the whole `SecretString`) or `aws-sm://secret-name#field` (parses
//! `SecretString` as JSON and extracts one field — Secrets Manager
//! conventionally stores e.g. `{"username":"admin","password":"..."}` as a
//! single secret).
//!
//! Reuses `storage::aws_sigv4`'s SigV4 signer and `storage::aws_credentials`'s
//! IRSA/ECS/IMDSv2 workload-identity credential chain — the exact same AWS
//! authentication story `storage-s3` already implements, just against a
//! different service endpoint and wire format (a JSON-RPC-style POST instead
//! of S3's REST API). Duplicating either would just be two copies of the
//! same canonical-request/credential-chain logic to keep in sync.
//!
//! Region comes from `AWS_REGION` or `AWS_DEFAULT_REGION` (the standard AWS
//! SDK env vars — required, no default; there's no universally-correct
//! default region). Static credentials come from the standard
//! `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` — intentionally not
//! `storage-s3`'s `RWS_S3_*` names, since Secrets Manager access is commonly
//! a different IAM principal than the one used for object storage. Falls
//! back to IRSA/ECS/IMDSv2 auto-detection when the static keys aren't set,
//! exactly like `storage::aws_credentials::CredentialsProvider::detect`.
//!
//! `AWS_ENDPOINT_URL_SECRETSMANAGER` overrides the default
//! `secretsmanager.{region}.amazonaws.com` host — the same env var name the
//! official AWS SDKs use for LocalStack/VPC-endpoint overrides, reused here
//! (rather than an rws-specific name) for this module's own tests too.

use super::SecretsError;
use crate::http_client::Client;
use crate::service_discovery::json_lite::{self, JsonValue};
use crate::storage::{aws_credentials::CredentialsProvider, aws_sigv4};

pub(super) fn resolve(rest: &str) -> Result<String, SecretsError> {
    let (secret_id, field) = match rest.split_once('#') {
        Some((id, f)) => (id, Some(f)),
        None => (rest, None),
    };
    if secret_id.is_empty() {
        return Err(SecretsError::new("aws-sm:// reference is missing a secret name"));
    }

    let region = std::env::var("AWS_REGION")
        .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
        .map_err(|_| SecretsError::new("AWS_REGION or AWS_DEFAULT_REGION must be set to resolve an aws-sm:// reference"))?;

    let access_key = std::env::var("AWS_ACCESS_KEY_ID").ok();
    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
    let provider = CredentialsProvider::detect(&region, access_key, secret_key);
    let creds = provider.get()?;

    // `AWS_ENDPOINT_URL_SECRETSMANAGER` mirrors the real AWS SDKs' endpoint
    // override convention (LocalStack, VPC endpoints, integration tests) —
    // not a made-up rws-specific var name.
    let (scheme_and_host, host) = match std::env::var("AWS_ENDPOINT_URL_SECRETSMANAGER") {
        Ok(url) => {
            let host = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(&url).to_string();
            (url.trim_end_matches('/').to_string(), host)
        }
        Err(_) => {
            let host = format!("secretsmanager.{region}.amazonaws.com");
            (format!("https://{host}"), host)
        }
    };
    let body = format!(r#"{{"SecretId":"{}"}}"#, json_escape(secret_id));
    let epoch_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let signed_headers = aws_sigv4::sign(
        "POST",
        &host,
        "/",
        body.as_bytes(),
        &region,
        &creds.access_key,
        &creds.secret_key,
        creds.session_token.as_deref(),
        epoch_secs,
        "secretsmanager",
    );

    let url = format!("{scheme_and_host}/");
    let client = Client::new();
    let mut request = client
        .post(&url)
        .header("Content-Type", "application/x-amz-json-1.1")
        .header("X-Amz-Target", "secretsmanager.GetSecretValue")
        .body(body.into_bytes());
    for (name, value) in &signed_headers {
        // The Host header is set by the client itself from the URL.
        if name.eq_ignore_ascii_case("host") {
            continue;
        }
        request = request.header(name, value);
    }

    let response = request
        .send()
        .map_err(|e| SecretsError::new(format!("Secrets Manager request for '{secret_id}' failed: {e}")))?;

    if !response.is_success() {
        return Err(SecretsError::new(format!(
            "Secrets Manager returned status {} for '{secret_id}': {}",
            response.status(),
            response.text().unwrap_or_default()
        )));
    }

    let resp_body = response
        .text()
        .map_err(|e| SecretsError::new(format!("Secrets Manager response for '{secret_id}' was not valid UTF-8: {e}")))?;
    let parsed = json_lite::parse(&resp_body)
        .map_err(|e| SecretsError::new(format!("failed to parse Secrets Manager response for '{secret_id}': {e}")))?;

    let secret_string = parsed
        .get("SecretString")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| SecretsError::new(format!("Secrets Manager secret '{secret_id}' has no SecretString (binary secrets are not supported)")))?;

    match field {
        None => Ok(secret_string.to_string()),
        Some(field) => {
            let inner = json_lite::parse(secret_string).map_err(|e| {
                SecretsError::new(format!("SecretString for '{secret_id}' is not valid JSON, but field '{field}' was requested: {e}"))
            })?;
            inner
                .get(field)
                .and_then(JsonValue::as_str)
                .map(str::to_string)
                .ok_or_else(|| SecretsError::new(format!("Secrets Manager secret '{secret_id}' JSON has no field '{field}'")))
        }
    }
}

/// Minimal JSON string escaping for `secret_id` inside a hand-built request
/// body — secret names are rarely anything but `[A-Za-z0-9/_+=.@-]` (AWS's
/// own allowed character set), but escape defensively rather than assume.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}
