use super::aws_sigv4;
use super::{Storage, StorageError};
use crate::http_client::{Client, Response};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

/// Connection details for an S3-compatible bucket.
#[derive(Clone, Debug)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    /// Scheme + host, e.g. `https://s3.us-east-1.amazonaws.com`. Point this
    /// at a custom host to use Cloudflare R2, MinIO, or any other
    /// S3-compatible provider.
    pub endpoint: String,
}

impl S3Config {
    /// Read configuration from environment variables.
    ///
    /// | Variable | Default |
    /// |---|---|
    /// | `RWS_S3_BUCKET` | **(required)** |
    /// | `RWS_S3_REGION` | `us-east-1` |
    /// | `RWS_S3_ACCESS_KEY` | **(required)** |
    /// | `RWS_S3_SECRET_KEY` | **(required)** |
    /// | `RWS_S3_ENDPOINT` | `https://s3.{region}.amazonaws.com` |
    pub fn from_env() -> Result<Self, StorageError> {
        let bucket = env::var("RWS_S3_BUCKET")
            .map_err(|_| StorageError::new("RWS_S3_BUCKET environment variable is required"))?;
        let region = env::var("RWS_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let access_key = env::var("RWS_S3_ACCESS_KEY")
            .map_err(|_| StorageError::new("RWS_S3_ACCESS_KEY environment variable is required"))?;
        let secret_key = env::var("RWS_S3_SECRET_KEY")
            .map_err(|_| StorageError::new("RWS_S3_SECRET_KEY environment variable is required"))?;
        let endpoint = env::var("RWS_S3_ENDPOINT")
            .unwrap_or_else(|_| format!("https://s3.{region}.amazonaws.com"));
        Ok(S3Config { bucket, region, access_key, secret_key, endpoint })
    }
}

/// S3-compatible object storage (AWS S3, Cloudflare R2, MinIO, ...).
///
/// Signs every request with AWS Signature Version 4 using the outbound HTTP
/// client (`crate::http_client::Client`) — no AWS SDK dependency. Uses
/// path-style addressing (`{endpoint}/{bucket}/{key}`), which every
/// S3-compatible provider supports; virtual-hosted-style (`{bucket}.{host}`)
/// is not used since custom endpoints for R2/MinIO don't reliably support it.
pub struct S3Storage {
    config: S3Config,
    client: Client,
}

impl S3Storage {
    pub fn new(config: S3Config) -> Self {
        S3Storage { config, client: Client::new() }
    }

    /// Shortcut for `S3Storage::new(S3Config::from_env()?)`.
    pub fn from_env() -> Result<Self, StorageError> {
        Ok(S3Storage::new(S3Config::from_env()?))
    }

    /// Hostname only (no scheme, no port) — `http_client::Client` writes the
    /// wire `Host:` header from just the hostname, so the signature must be
    /// computed against exactly that value to validate on the S3 side.
    fn host(&self) -> String {
        let without_scheme = self
            .config
            .endpoint
            .strip_prefix("https://")
            .or_else(|| self.config.endpoint.strip_prefix("http://"))
            .unwrap_or(&self.config.endpoint);
        without_scheme
            .split('/')
            .next()
            .unwrap_or(without_scheme)
            .split(':')
            .next()
            .unwrap_or(without_scheme)
            .to_string()
    }

    /// Percent-encoded `/{bucket}/{key}` — used both as the request path and
    /// as the canonical URI signed by SigV4, so the two always match.
    fn canonical_path(&self, key: &str) -> String {
        aws_sigv4::uri_encode_path(&format!("/{}/{}", self.config.bucket, key))
    }

    fn request_url(&self, key: &str) -> String {
        format!("{}{}", self.config.endpoint.trim_end_matches('/'), self.canonical_path(key))
    }

    fn signed_headers(&self, method: &str, key: &str, payload: &[u8]) -> Vec<(String, String)> {
        let epoch_secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        aws_sigv4::sign(
            method,
            &self.host(),
            &self.canonical_path(key),
            payload,
            &self.config.region,
            &self.config.access_key,
            &self.config.secret_key,
            epoch_secs,
        )
    }

    fn error_from_response(action: &str, key: &str, resp: &Response) -> StorageError {
        StorageError::new(format!(
            "S3 {action} '{key}' failed: HTTP {} {}",
            resp.status(),
            resp.text().unwrap_or_default()
        ))
    }
}

impl Storage for S3Storage {
    fn put(&self, key: &str, data: &[u8], content_type: &str) -> Result<String, StorageError> {
        let url = self.request_url(key);
        let mut builder = self.client.put(&url).header("Content-Type", content_type);
        for (name, value) in self.signed_headers("PUT", key, data) {
            // The `host` entry documents what was signed; the actual `Host:`
            // header is already sent by `Client`, derived from `url`.
            if name.eq_ignore_ascii_case("host") {
                continue;
            }
            builder = builder.header(&name, &value);
        }
        let resp = builder
            .body(data.to_vec())
            .send()
            .map_err(|e| StorageError::new(format!("S3 PUT '{key}' failed: {e}")))?;
        if !resp.is_success() {
            return Err(Self::error_from_response("PUT", key, &resp));
        }
        Ok(key.to_string())
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let url = self.request_url(key);
        let mut builder = self.client.get(&url);
        for (name, value) in self.signed_headers("GET", key, b"") {
            if name.eq_ignore_ascii_case("host") {
                continue;
            }
            builder = builder.header(&name, &value);
        }
        let resp = builder.send().map_err(|e| StorageError::new(format!("S3 GET '{key}' failed: {e}")))?;
        if !resp.is_success() {
            return Err(Self::error_from_response("GET", key, &resp));
        }
        Ok(resp.bytes().to_vec())
    }

    fn delete(&self, key: &str) -> Result<(), StorageError> {
        let url = self.request_url(key);
        let mut builder = self.client.delete(&url);
        for (name, value) in self.signed_headers("DELETE", key, b"") {
            if name.eq_ignore_ascii_case("host") {
                continue;
            }
            builder = builder.header(&name, &value);
        }
        let resp = builder.send().map_err(|e| StorageError::new(format!("S3 DELETE '{key}' failed: {e}")))?;
        // S3 returns 204 whether or not the key existed — matches
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
