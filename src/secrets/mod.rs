//! Secret-reference resolution for HashiCorp Vault, AWS Secrets Manager, and
//! Azure Key Vault — no vendor SDK for any of the three, matching this
//! crate's existing `storage-s3`/`storage-azure`/`sso` modules.
//!
//! JWT signing keys, DB credentials, and TLS keys are ordinarily just plain
//! `RWS_CONFIG_*`/application env vars. [`resolve`] lets any of those values
//! instead be a *reference* to a secret held in a managed secrets store:
//!
//! | Prefix | Backend | Example |
//! |---|---|---|
//! | `vault://path#field` | HashiCorp Vault (KV v2) | `vault://secret/myapp/db#password` |
//! | `aws-sm://name` or `aws-sm://name#field` | AWS Secrets Manager | `aws-sm://prod/db-password` |
//! | `azkv://vault-name/secret-name` | Azure Key Vault | `azkv://my-kv/db-password` |
//!
//! A value that doesn't start with one of these prefixes is returned
//! unchanged — resolution is purely additive, not a new required step for
//! every existing plain-value config var.
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_web_server::secrets;
//!
//! // VAULT_ADDR / VAULT_TOKEN (Vault), AWS_REGION + credentials (Secrets
//! // Manager), or AZURE_KEY_VAULT_TENANT_ID + co. (Key Vault) must already
//! // be set in the environment for the corresponding prefix to resolve.
//! let db_password = secrets::resolve("vault://secret/myapp/db#password")?;
//! # Ok::<(), secrets::SecretsError>(())
//! ```
//!
//! # Automatic env var resolution
//!
//! [`resolve_env_vars`] scans every currently-set `RWS_`-prefixed environment
//! variable and rewrites in place (via `std::env::set_var`) any whose value
//! matches one of the prefixes above. `Server::setup()` calls this
//! automatically (right after `entry_point::bootstrap()`, so it sees the
//! fully layered config) once this feature is enabled, so **any**
//! `RWS_CONFIG_*` value — `RWS_CONFIG_TLS_KEY_FILE`, a config-driven proxy's
//! `token_env`-referenced variable, anything — can be a secret reference
//! with no code changes, matching the "additive to every existing
//! `RWS_CONFIG_*`/`RWS_*` env var" goal this feature was built for.
//!
//! # Failure mode: fail fast, not fail open
//!
//! A value that *looks* like a secret reference but fails to resolve (wrong
//! token, unreachable backend, missing field, ...) is a startup error, not a
//! silently-ignored one — the alternative would be a server starting up with
//! a JWT signing key or DB password literally equal to the string
//! `"vault://secret/myapp/db#password"`, which is worse than refusing to
//! start at all.

#[cfg(test)]
mod tests;

mod aws_secrets_manager;
mod azure_key_vault;
mod vault;

use std::fmt;

/// Error resolving a secret reference — a network failure, an
/// authentication failure, a missing field, or a malformed reference.
#[derive(Debug)]
pub struct SecretsError(pub String);

impl SecretsError {
    pub fn new(msg: impl Into<String>) -> Self {
        SecretsError(msg.into())
    }
}

impl fmt::Display for SecretsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SecretsError {}

impl From<crate::storage::StorageError> for SecretsError {
    fn from(e: crate::storage::StorageError) -> Self {
        SecretsError(e.0)
    }
}

const VAULT_PREFIX: &str = "vault://";
const AWS_SM_PREFIX: &str = "aws-sm://";
const AZURE_KV_PREFIX: &str = "azkv://";

/// Resolves `value` if it starts with a recognized secret-reference prefix
/// (`vault://`, `aws-sm://`, `azkv://`); otherwise returns it unchanged.
///
/// See the [module docs](self) for the prefix formats and each backend's
/// required environment variables.
pub fn resolve(value: &str) -> Result<String, SecretsError> {
    if let Some(rest) = value.strip_prefix(VAULT_PREFIX) {
        return vault::resolve(rest);
    }
    if let Some(rest) = value.strip_prefix(AWS_SM_PREFIX) {
        return aws_secrets_manager::resolve(rest);
    }
    if let Some(rest) = value.strip_prefix(AZURE_KV_PREFIX) {
        return azure_key_vault::resolve(rest);
    }
    Ok(value.to_string())
}

fn is_secret_ref(value: &str) -> bool {
    value.starts_with(VAULT_PREFIX) || value.starts_with(AWS_SM_PREFIX) || value.starts_with(AZURE_KV_PREFIX)
}

/// Scans every environment variable whose name starts with `RWS_` and
/// rewrites in place (via `std::env::set_var`) any whose *value* matches a
/// recognized secret-reference prefix. Variables that don't match are left
/// untouched — this only ever narrows down to the ones that opted in by
/// using one of the prefixes.
///
/// Called automatically from `entry_point::bootstrap()` when this feature is
/// enabled; exposed publicly so a library user driving their own startup
/// sequence (not calling `bootstrap()`) can call it explicitly instead.
pub fn resolve_env_vars() -> Result<(), SecretsError> {
    let matching: Vec<(String, String)> = std::env::vars()
        .filter(|(key, value)| key.starts_with("RWS_") && is_secret_ref(value))
        .collect();

    for (key, value) in matching {
        let resolved = resolve(&value)
            .map_err(|e| SecretsError::new(format!("resolving secret reference for {key}: {e}")))?;
        std::env::set_var(&key, resolved);
    }
    Ok(())
}
