use super::gcs_credentials::{self, GcsCredentialsProvider};
use super::{Storage, StorageError};
use crate::http_client::{Client, RequestBuilder, Response};

/// Connection details for a Google Cloud Storage bucket.
#[derive(Clone, Debug)]
pub struct GcsConfig {
    pub bucket: String,
    /// Scheme + host of the JSON API, `https://storage.googleapis.com` by
    /// default. Override for an emulator (fake-gcs-server) or a private
    /// endpoint.
    pub endpoint: String,
}

impl GcsConfig {
    /// Read configuration from environment variables.
    ///
    /// | Variable | Default |
    /// |---|---|
    /// | `RWS_GCS_BUCKET` | **(required)** |
    /// | `RWS_GCS_ENDPOINT` | `https://storage.googleapis.com` |
    ///
    /// Credentials are found separately — see [`GcsStorage`].
    pub fn from_env() -> Result<Self, StorageError> {
        let bucket = std::env::var("RWS_GCS_BUCKET")
            .ok()
            .filter(|b| !b.trim().is_empty())
            .ok_or_else(|| StorageError::new("RWS_GCS_BUCKET environment variable is required"))?;
        let endpoint = std::env::var("RWS_GCS_ENDPOINT")
            .ok()
            .filter(|e| !e.trim().is_empty())
            .unwrap_or_else(|| "https://storage.googleapis.com".to_string());
        Ok(GcsConfig { bucket, endpoint })
    }
}

/// Google Cloud Storage, through its JSON API and the outbound HTTP client
/// (`crate::http_client::Client`) — no Google SDK.
///
/// Requests carry an OAuth 2.0 access token, found as Google's client
/// libraries find Application Default Credentials: `RWS_GCS_ACCESS_TOKEN`,
/// then the file in `GOOGLE_APPLICATION_CREDENTIALS` (a service account key
/// or `authorized_user`), then gcloud's
/// `application_default_credentials.json`, then the metadata server
/// (Compute Engine, GKE Workload Identity, Cloud Run). Tokens are cached and
/// refreshed before they expire. `RWS_GCS_CREDENTIAL_SOURCE=token|file|metadata|none`
/// picks one; `none` sends no credentials (emulators).
pub struct GcsStorage {
    config: GcsConfig,
    client: Client,
    credentials: GcsCredentialsProvider,
}

impl GcsStorage {
    pub fn new(config: GcsConfig) -> Self {
        GcsStorage::with_source(config, gcs_credentials::detect())
    }

    pub(super) fn with_source(config: GcsConfig, source: gcs_credentials::Source) -> Self {
        GcsStorage { config, client: Client::new(), credentials: GcsCredentialsProvider::new(source) }
    }

    /// Shortcut for `GcsStorage::new(GcsConfig::from_env()?)`.
    pub fn from_env() -> Result<Self, StorageError> {
        Ok(GcsStorage::new(GcsConfig::from_env()?))
    }

    fn endpoint(&self) -> &str {
        self.config.endpoint.trim_end_matches('/')
    }

    /// `/storage/v1/b/{bucket}/o/{object}`: the object name is one path
    /// segment, `/` included, so it's encoded too.
    fn object_url(&self, key: &str) -> String {
        format!("{}/storage/v1/b/{}/o/{}", self.endpoint(), encode(&self.config.bucket), encode(key))
    }

    fn authorize<'a>(&self, builder: RequestBuilder<'a>) -> Result<RequestBuilder<'a>, StorageError> {
        Ok(match self.credentials.authorization()? {
            Some(value) => builder.header("Authorization", &value),
            None => builder,
        })
    }

    fn error(action: &str, key: &str, resp: &Response) -> StorageError {
        StorageError::new(format!("GCS {action} '{key}' failed: HTTP {} {}", resp.status(), resp.text().unwrap_or_default()))
    }
}

/// Percent-encodes everything but the unreserved set (`A-Z a-z 0-9 - . _ ~`).
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

impl Storage for GcsStorage {
    fn put(&self, key: &str, data: &[u8], content_type: &str) -> Result<String, StorageError> {
        let url = format!("{}/upload/storage/v1/b/{}/o?uploadType=media&name={}", self.endpoint(), encode(&self.config.bucket), encode(key));
        let content_type = if content_type.is_empty() { "application/octet-stream" } else { content_type };
        let resp = self
            .authorize(self.client.post(&url).header("Content-Type", content_type))?
            .body(data.to_vec())
            .send()
            .map_err(|e| StorageError::new(format!("GCS PUT '{key}' failed: {e}")))?;
        if !resp.is_success() {
            return Err(Self::error("PUT", key, &resp));
        }
        Ok(key.to_string())
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let url = format!("{}?alt=media", self.object_url(key));
        let resp = self.authorize(self.client.get(&url))?.send().map_err(|e| StorageError::new(format!("GCS GET '{key}' failed: {e}")))?;
        if !resp.is_success() {
            return Err(Self::error("GET", key, &resp));
        }
        Ok(resp.bytes().to_vec())
    }

    fn delete(&self, key: &str) -> Result<(), StorageError> {
        let resp = self
            .authorize(self.client.delete(&self.object_url(key)))?
            .send()
            .map_err(|e| StorageError::new(format!("GCS DELETE '{key}' failed: {e}")))?;
        // A missing object is a no-op, as `Storage::delete` promises.
        if !resp.is_success() && resp.status() != 404 {
            return Err(Self::error("DELETE", key, &resp));
        }
        Ok(())
    }

    /// `{endpoint}/{bucket}/{key}` — the object's public URL
    /// (`https://storage.googleapis.com/...`), which works for objects
    /// readable by `allUsers`; private ones need a signed URL.
    fn url(&self, key: &str) -> String {
        let key = key.split('/').map(encode).collect::<Vec<_>>().join("/");
        format!("{}/{}/{}", self.endpoint(), self.config.bucket, key)
    }
}

#[cfg(test)]
mod tests;
