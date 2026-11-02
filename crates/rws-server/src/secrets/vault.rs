//! HashiCorp Vault KV v2 secret resolution — `vault://path#field`.
//!
//! `path` is the logical KV v2 path exactly as used by `vault kv get path`
//! (e.g. `secret/myapp/db`) — the first path segment is the mount name, and
//! `data/` is inserted right after it to build the actual HTTP API path
//! (`/v1/secret/data/myapp/db`), matching the same convention the Vault CLI
//! itself applies so users don't have to think in terms of the KV v2 API's
//! own path shape.
//!
//! Configured via the same environment variables the official `vault` CLI
//! and client libraries use, so an already-Vault-integrated environment
//! needs no new variables:
//! - `VAULT_ADDR` — defaults to `http://127.0.0.1:8200` (the Vault CLI's own
//!   default, e.g. a local dev server or an in-cluster Vault Agent sidecar).
//! - `VAULT_TOKEN` — required, no default.
//!
//! Only token auth is implemented (a static `X-Vault-Token` header) — no
//! AppRole, Kubernetes auth, or token renewal. A short-lived token from one
//! of those methods can still be placed directly in `VAULT_TOKEN` by
//! whatever process starts `rws`.

use super::SecretsError;
use crate::http_client::Client;
use crate::service_discovery::json_lite::{self, JsonValue};

pub(super) fn resolve(rest: &str) -> Result<String, SecretsError> {
    let (path, field) = rest
        .split_once('#')
        .ok_or_else(|| SecretsError::new(format!("vault:// reference 'vault://{rest}' is missing a '#field' suffix")))?;
    if path.is_empty() || field.is_empty() {
        return Err(SecretsError::new(format!("vault:// reference 'vault://{rest}' has an empty path or field")));
    }

    let mut segments = path.splitn(2, '/');
    let mount = segments.next().filter(|s| !s.is_empty());
    let secret_path = segments.next().filter(|s| !s.is_empty());
    let (mount, secret_path) = match (mount, secret_path) {
        (Some(m), Some(p)) => (m, p),
        _ => {
            return Err(SecretsError::new(format!(
                "vault:// path '{path}' must be at least 'mount/secret-path'"
            )))
        }
    };

    let addr = std::env::var("VAULT_ADDR").unwrap_or_else(|_| "http://127.0.0.1:8200".to_string());
    let token = std::env::var("VAULT_TOKEN").map_err(|_| SecretsError::new("VAULT_TOKEN environment variable is not set"))?;

    let url = format!("{}/v1/{}/data/{}", addr.trim_end_matches('/'), mount, secret_path);

    let response = Client::new()
        .get(&url)
        .header("X-Vault-Token", &token)
        .send()
        .map_err(|e| SecretsError::new(format!("Vault request to {url} failed: {e}")))?;

    if !response.is_success() {
        return Err(SecretsError::new(format!(
            "Vault returned status {} for {url}: {}",
            response.status(),
            response.text().unwrap_or_default()
        )));
    }

    let body = response
        .text()
        .map_err(|e| SecretsError::new(format!("Vault response from {url} was not valid UTF-8: {e}")))?;
    let parsed = json_lite::parse(&body).map_err(|e| SecretsError::new(format!("failed to parse Vault response from {url}: {e}")))?;

    parsed
        .get("data")
        .and_then(|outer| outer.get("data"))
        .and_then(|inner| inner.get(field))
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| SecretsError::new(format!("Vault secret at '{path}' has no field '{field}'")))
}
