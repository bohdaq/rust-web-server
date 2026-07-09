//! Azure Key Vault secret resolution — `azkv://vault-name/secret-name`.
//!
//! Two auth modes, tried in this order:
//!
//! 1. **Service principal (client-credentials grant)** — used when
//!    `AZURE_KEY_VAULT_TENANT_ID`, `AZURE_KEY_VAULT_CLIENT_ID`, and
//!    `AZURE_KEY_VAULT_CLIENT_SECRET` are all set. POSTs to Azure AD's
//!    `/oauth2/v2.0/token` endpoint for a Key-Vault-scoped bearer token —
//!    this is the standalone "app registration" flow, independent of the
//!    host's own identity.
//! 2. **Managed Identity** — the same App Service/Container Apps
//!    (`IDENTITY_ENDPOINT`+`IDENTITY_HEADER`) or VM/AKS IMDS detection
//!    `storage::azure_credentials` already implements for Blob Storage,
//!    reused here via its now-`pub(crate)` `fetch_app_service_token`/
//!    `fetch_imds_token`/`managed_identity_endpoint_from_env`, just
//!    requesting a token for Key Vault's resource URI instead of Storage's.
//!
//! Then `GET https://{vault-name}.vault.azure.net/secrets/{secret-name}?api-version=7.4`
//! with `Authorization: Bearer {token}`.
//!
//! `AZURE_KEY_VAULT_ENDPOINT_OVERRIDE` and `AZURE_AD_LOGIN_ENDPOINT_OVERRIDE`
//! replace the Key Vault and Azure AD hosts respectively — for a Key Vault
//! emulator, or for this module's own tests.

use super::SecretsError;
use crate::http_client::Client;
use crate::service_discovery::json_lite::{self, JsonValue};
use crate::storage::azure_credentials::{self, IdentityEndpoint};

const KEY_VAULT_RESOURCE: &str = "https://vault.azure.net";
const API_VERSION: &str = "7.4";

pub(super) fn resolve(rest: &str) -> Result<String, SecretsError> {
    let (vault_name, secret_name) = rest
        .split_once('/')
        .ok_or_else(|| SecretsError::new(format!("azkv:// reference 'azkv://{rest}' must be 'vault-name/secret-name'")))?;
    if vault_name.is_empty() || secret_name.is_empty() {
        return Err(SecretsError::new(format!(
            "azkv:// reference 'azkv://{rest}' has an empty vault name or secret name"
        )));
    }

    let client = Client::new();
    let token = fetch_token(&client)?;

    // Test-only escape hatch (also handy against a Key Vault emulator) — the
    // real vault name is still part of every `azkv://` reference, just
    // unused for routing when this is set.
    let base = std::env::var("AZURE_KEY_VAULT_ENDPOINT_OVERRIDE")
        .unwrap_or_else(|_| format!("https://{vault_name}.vault.azure.net"));
    let url = format!("{}/secrets/{secret_name}?api-version={API_VERSION}", base.trim_end_matches('/'));
    let response = client
        .get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .map_err(|e| SecretsError::new(format!("Key Vault request for '{secret_name}' failed: {e}")))?;

    if !response.is_success() {
        return Err(SecretsError::new(format!(
            "Key Vault returned status {} for '{secret_name}': {}",
            response.status(),
            response.text().unwrap_or_default()
        )));
    }

    let body = response
        .text()
        .map_err(|e| SecretsError::new(format!("Key Vault response for '{secret_name}' was not valid UTF-8: {e}")))?;
    let parsed =
        json_lite::parse(&body).map_err(|e| SecretsError::new(format!("failed to parse Key Vault response for '{secret_name}': {e}")))?;

    parsed
        .get("value")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| SecretsError::new(format!("Key Vault secret '{secret_name}' response has no 'value' field")))
}

fn fetch_token(client: &Client) -> Result<String, SecretsError> {
    let tenant_id = std::env::var("AZURE_KEY_VAULT_TENANT_ID").ok();
    let client_id = std::env::var("AZURE_KEY_VAULT_CLIENT_ID").ok();
    let client_secret = std::env::var("AZURE_KEY_VAULT_CLIENT_SECRET").ok();

    if let (Some(tenant_id), Some(client_id), Some(client_secret)) = (tenant_id, client_id, client_secret) {
        return fetch_service_principal_token(client, &tenant_id, &client_id, &client_secret);
    }

    let (token, _expires_at) = match azure_credentials::managed_identity_endpoint_from_env() {
        IdentityEndpoint::AppService { endpoint, header } => {
            azure_credentials::fetch_app_service_token(client, &endpoint, &header, KEY_VAULT_RESOURCE)
        }
        IdentityEndpoint::Imds => azure_credentials::fetch_imds_token(client, "http://169.254.169.254", KEY_VAULT_RESOURCE),
    }
    .map_err(|e| SecretsError::new(format!("failed to acquire a Key Vault access token: {e}")))?;
    Ok(token)
}

fn fetch_service_principal_token(client: &Client, tenant_id: &str, client_id: &str, client_secret: &str) -> Result<String, SecretsError> {
    // Test-only escape hatch, same idea as `AZURE_KEY_VAULT_ENDPOINT_OVERRIDE`.
    let authority =
        std::env::var("AZURE_AD_LOGIN_ENDPOINT_OVERRIDE").unwrap_or_else(|_| "https://login.microsoftonline.com".to_string());
    let url = format!("{}/{tenant_id}/oauth2/v2.0/token", authority.trim_end_matches('/'));
    let scope = format!("{KEY_VAULT_RESOURCE}/.default");
    let response = client
        .post(&url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("scope", &scope),
        ])
        .send()
        .map_err(|e| SecretsError::new(format!("Azure AD token request failed: {e}")))?;

    if !response.is_success() {
        return Err(SecretsError::new(format!(
            "Azure AD token request failed: HTTP {}: {}",
            response.status(),
            response.text().unwrap_or_default()
        )));
    }

    let body = response
        .text()
        .map_err(|e| SecretsError::new(format!("Azure AD token response was not valid UTF-8: {e}")))?;
    let parsed = json_lite::parse(&body).map_err(|e| SecretsError::new(format!("failed to parse Azure AD token response: {e}")))?;
    parsed
        .get("access_token")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| SecretsError::new("Azure AD token response has no access_token field"))
}
