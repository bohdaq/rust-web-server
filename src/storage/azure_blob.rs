use super::azure_credentials::{AzureCredentialsProvider, Credential};
use super::azure_signature;
use super::{Storage, StorageError};
use crate::http_client::{Client, Response};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

/// `x-ms-version` sent on every request, regardless of auth scheme. Any
/// reasonably recent stable version works for basic Put/Get/Delete Blob;
/// bump only if a newer feature is needed.
const API_VERSION: &str = "2021-08-06";

/// Connection details for an Azure Blob Storage container.
#[derive(Clone, Debug)]
pub struct AzureBlobConfig {
    pub account: String,
    pub container: String,
    /// Empty string means "no static key — auto-detect Managed Identity
    /// (VM/AKS IMDS, or App Service/Container Apps identity endpoint)
    /// instead". See [`AzureBlobStorage::new`].
    pub account_key: String,
    /// Scheme + host, e.g. `https://myaccount.blob.core.windows.net`.
    /// Override for the Azurite local emulator or a private endpoint.
    pub endpoint: String,
}

impl AzureBlobConfig {
    /// Read configuration from environment variables.
    ///
    /// | Variable | Default |
    /// |---|---|
    /// | `RWS_AZURE_ACCOUNT` | **(required)** |
    /// | `RWS_AZURE_CONTAINER` | **(required)** |
    /// | `RWS_AZURE_ACCOUNT_KEY` | optional — falls back to Managed Identity when unset |
    /// | `RWS_AZURE_ENDPOINT` | `https://{account}.blob.core.windows.net` |
    pub fn from_env() -> Result<Self, StorageError> {
        let account = env::var("RWS_AZURE_ACCOUNT")
            .map_err(|_| StorageError::new("RWS_AZURE_ACCOUNT environment variable is required"))?;
        let container = env::var("RWS_AZURE_CONTAINER")
            .map_err(|_| StorageError::new("RWS_AZURE_CONTAINER environment variable is required"))?;
        let account_key = env::var("RWS_AZURE_ACCOUNT_KEY").unwrap_or_default();
        let endpoint =
            env::var("RWS_AZURE_ENDPOINT").unwrap_or_else(|_| format!("https://{account}.blob.core.windows.net"));
        Ok(AzureBlobConfig { account, container, account_key, endpoint })
    }
}

/// Azure Blob Storage object storage.
///
/// Signs every request with the Shared Key HMAC-SHA256 scheme using the
/// outbound HTTP client (`crate::http_client::Client`) — no Azure SDK
/// dependency. Credentials come from `config.account_key` when non-empty;
/// otherwise they're auto-detected from Managed Identity (App
/// Service/Container Apps identity endpoint, or VM/AKS IMDS as a last
/// resort), cached, and refreshed shortly before expiry. See
/// `crate::storage::azure_credentials` and `crate::storage::azure_signature`.
pub struct AzureBlobStorage {
    config: AzureBlobConfig,
    client: Client,
    credentials: AzureCredentialsProvider,
}

impl AzureBlobStorage {
    pub fn new(config: AzureBlobConfig) -> Self {
        let account_key = non_empty(&config.account_key);
        let credentials = AzureCredentialsProvider::detect(account_key);
        AzureBlobStorage { config, client: Client::new(), credentials }
    }

    /// Shortcut for `AzureBlobStorage::new(AzureBlobConfig::from_env()?)`.
    pub fn from_env() -> Result<Self, StorageError> {
        Ok(AzureBlobStorage::new(AzureBlobConfig::from_env()?))
    }

    /// Percent-encoded `/{container}/{key}` — used both as the request path
    /// and as the canonical resource signed by Shared Key, so the two always
    /// match. Not shared with `aws_sigv4::uri_encode_path`: that helper only
    /// compiles under the independently-gated `storage-s3` feature.
    fn canonical_path(&self, key: &str) -> String {
        format!("/{}/{}", self.config.container, uri_encode_key(key))
    }

    fn request_url(&self, key: &str) -> String {
        format!("{}{}", self.config.endpoint.trim_end_matches('/'), self.canonical_path(key))
    }

    /// Builds the full header set for a request: `x-ms-date`, `x-ms-version`,
    /// any caller-supplied extra `x-ms-*` headers (e.g. `x-ms-blob-type` on
    /// `PUT`), and `Authorization` — either `SharedKey ...` (computed over
    /// exactly this header set) or `Bearer {token}` for Managed Identity.
    fn auth_headers(
        &self,
        method: &str,
        key: &str,
        content_type: &str,
        content_length: usize,
        extra_x_ms: &[(String, String)],
    ) -> Result<Vec<(String, String)>, StorageError> {
        let epoch_secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let x_ms_date = azure_signature::rfc1123_date(epoch_secs);

        let mut headers = vec![("x-ms-date".to_string(), x_ms_date), ("x-ms-version".to_string(), API_VERSION.to_string())];
        headers.extend(extra_x_ms.iter().cloned());

        let authorization = match self.credentials.get()? {
            Credential::SharedKey(account_key) => azure_signature::sign(
                method,
                &self.config.account,
                &self.canonical_path(key),
                content_type,
                content_length,
                &account_key,
                &headers,
            )?,
            Credential::Bearer(token) => format!("Bearer {token}"),
        };
        headers.push(("Authorization".to_string(), authorization));
        Ok(headers)
    }

    fn error_from_response(action: &str, key: &str, resp: &Response) -> StorageError {
        StorageError::new(format!("Azure Blob {action} '{key}' failed: HTTP {} {}", resp.status(), resp.text().unwrap_or_default()))
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() { None } else { Some(s.to_string()) }
}

/// Percent-encodes a blob key, preserving `/` as a path separator — mirrors
/// `aws_sigv4::uri_encode_path`'s rules (unreserved set: `A-Z a-z 0-9 - . _ ~`).
fn uri_encode_key(key: &str) -> String {
    key.split('/').map(uri_encode_segment).collect::<Vec<_>>().join("/")
}

fn uri_encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

impl Storage for AzureBlobStorage {
    fn put(&self, key: &str, data: &[u8], content_type: &str) -> Result<String, StorageError> {
        let url = self.request_url(key);
        let extra = [("x-ms-blob-type".to_string(), "BlockBlob".to_string())];
        let headers = self.auth_headers("PUT", key, content_type, data.len(), &extra)?;

        let mut builder = self.client.put(&url).header("Content-Type", content_type);
        for (name, value) in headers {
            builder = builder.header(&name, &value);
        }
        let resp = builder
            .body(data.to_vec())
            .send()
            .map_err(|e| StorageError::new(format!("Azure Blob PUT '{key}' failed: {e}")))?;
        if !resp.is_success() {
            return Err(Self::error_from_response("PUT", key, &resp));
        }
        Ok(key.to_string())
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let url = self.request_url(key);
        let headers = self.auth_headers("GET", key, "", 0, &[])?;

        let mut builder = self.client.get(&url);
        for (name, value) in headers {
            builder = builder.header(&name, &value);
        }
        let resp = builder.send().map_err(|e| StorageError::new(format!("Azure Blob GET '{key}' failed: {e}")))?;
        if !resp.is_success() {
            return Err(Self::error_from_response("GET", key, &resp));
        }
        Ok(resp.bytes().to_vec())
    }

    fn delete(&self, key: &str) -> Result<(), StorageError> {
        let url = self.request_url(key);
        let headers = self.auth_headers("DELETE", key, "", 0, &[])?;

        let mut builder = self.client.delete(&url);
        for (name, value) in headers {
            builder = builder.header(&name, &value);
        }
        let resp = builder.send().map_err(|e| StorageError::new(format!("Azure Blob DELETE '{key}' failed: {e}")))?;
        // Azure returns 202 whether or not the blob existed — matches
        // `Storage::delete`'s no-op-on-missing contract.
        if !resp.is_success() {
            return Err(Self::error_from_response("DELETE", key, &resp));
        }
        Ok(())
    }

    fn url(&self, key: &str) -> String {
        self.request_url(key)
    }
}

#[cfg(test)]
mod tests;
