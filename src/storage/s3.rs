use super::aws_credentials::CredentialsProvider;
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
    /// Empty string means "no static key — auto-detect workload identity
    /// (EKS IRSA, ECS task role, EC2 IMDSv2) instead". See
    /// [`S3Storage::new`].
    pub access_key: String,
    /// Empty string means "no static key — auto-detect workload identity".
    /// See [`S3Storage::new`].
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
    /// | `RWS_S3_ACCESS_KEY` | optional — falls back to workload identity (IRSA/ECS/IMDS) when unset |
    /// | `RWS_S3_SECRET_KEY` | optional — falls back to workload identity (IRSA/ECS/IMDS) when unset |
    /// | `RWS_S3_ENDPOINT` | `https://s3.{region}.amazonaws.com` |
    pub fn from_env() -> Result<Self, StorageError> {
        let bucket = env::var("RWS_S3_BUCKET")
            .map_err(|_| StorageError::new("RWS_S3_BUCKET environment variable is required"))?;
        let region = env::var("RWS_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let access_key = env::var("RWS_S3_ACCESS_KEY").unwrap_or_default();
        let secret_key = env::var("RWS_S3_SECRET_KEY").unwrap_or_default();
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
///
/// Credentials come from `config.access_key`/`config.secret_key` when both
/// are non-empty; otherwise they're auto-detected from EKS IRSA, ECS task
/// role, or EC2 IMDSv2 (in that order), cached, and refreshed shortly before
/// expiry. See `crate::storage::aws_credentials`.
pub struct S3Storage {
    config: S3Config,
    client: Client,
    credentials: CredentialsProvider,
}

impl S3Storage {
    pub fn new(config: S3Config) -> Self {
        let access_key = non_empty(&config.access_key);
        let secret_key = non_empty(&config.secret_key);
        let credentials = CredentialsProvider::detect(&config.region, access_key, secret_key);
        S3Storage { config, client: Client::new(), credentials }
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

    fn signed_headers(&self, method: &str, key: &str, payload: &[u8]) -> Result<Vec<(String, String)>, StorageError> {
        let creds = self.credentials.get()?;
        let epoch_secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        Ok(aws_sigv4::sign(
            method,
            &self.host(),
            &self.canonical_path(key),
            payload,
            &self.config.region,
            &creds.access_key,
            &creds.secret_key,
            creds.session_token.as_deref(),
            epoch_secs,
            "s3",
        ))
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
        for (name, value) in self.signed_headers("PUT", key, data)? {
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
        for (name, value) in self.signed_headers("GET", key, b"")? {
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
        for (name, value) in self.signed_headers("DELETE", key, b"")? {
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

    /// S3 multipart upload: parts of [`S3_PART_SIZE`] read and sent one at
    /// a time, so memory stays at one part however large the object. An
    /// object that fits in one part is a plain `PUT`. Any failure aborts the
    /// upload, so S3 keeps no orphaned parts.
    fn put_stream(&self, key: &str, reader: &mut dyn std::io::Read, content_type: &str) -> Result<String, StorageError> {
        let mut part = read_part(reader, key)?;
        if part.len() < S3_PART_SIZE {
            return self.put(key, &part, content_type);
        }
        let upload_id = self.create_multipart_upload(key, content_type)?;
        let mut etags: Vec<String> = Vec::new();
        let uploaded = (|| -> Result<(), StorageError> {
            loop {
                let number = (etags.len() + 1).to_string();
                etags.push(self.upload_part(key, &upload_id, &number, &part)?);
                part = read_part(reader, key)?;
                if part.is_empty() {
                    return Ok(());
                }
            }
        })()
        .and_then(|_| self.complete_multipart_upload(key, &upload_id, &etags));
        if let Err(e) = uploaded {
            let _ = self.abort_multipart_upload(key, &upload_id);
            return Err(e);
        }
        Ok(key.to_string())
    }
}

/// Size of each part of an S3 multipart upload (S3's minimum is 5 MiB,
/// except for the last part). 10 000 parts — S3's maximum — make objects of
/// up to ~78 GiB.
pub const S3_PART_SIZE: usize = 8 * 1024 * 1024;

/// Up to [`S3_PART_SIZE`] bytes from `reader` (fewer only at the end).
fn read_part(reader: &mut dyn std::io::Read, key: &str) -> Result<Vec<u8>, StorageError> {
    let mut part = vec![0u8; S3_PART_SIZE];
    let mut filled = 0;
    while filled < part.len() {
        match reader.read(&mut part[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(StorageError::new(format!("failed to read the data for '{key}': {e}"))),
        }
    }
    part.truncate(filled);
    Ok(part)
}

impl S3Storage {
    /// Send a request to `/{bucket}/{key}?{params}`, signed with the query.
    fn send_with_query(&self, method: &str, key: &str, params: &[(&str, &str)], body: Vec<u8>, content_type: Option<&str>) -> Result<Response, StorageError> {
        let query = aws_sigv4::canonical_query(params);
        let url = format!("{}?{}", self.request_url(key), query);
        let creds = self.credentials.get()?;
        let epoch_secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let headers = aws_sigv4::sign_with_query(
            method, &self.host(), &self.canonical_path(key), params, &body, &self.config.region,
            &creds.access_key, &creds.secret_key, creds.session_token.as_deref(), epoch_secs, "s3",
        );
        let mut builder = self.client.request(method, &url);
        if let Some(content_type) = content_type {
            builder = builder.header("Content-Type", content_type);
        }
        for (name, value) in headers {
            if !name.eq_ignore_ascii_case("host") {
                builder = builder.header(&name, &value);
            }
        }
        builder.body(body).send().map_err(|e| StorageError::new(format!("S3 {method} '{key}' failed: {e}")))
    }

    fn create_multipart_upload(&self, key: &str, content_type: &str) -> Result<String, StorageError> {
        let resp = self.send_with_query("POST", key, &[("uploads", "")], Vec::new(), Some(content_type))?;
        if !resp.is_success() {
            return Err(Self::error_from_response("CreateMultipartUpload", key, &resp));
        }
        xml_element(&resp.text().unwrap_or_default(), "UploadId")
            .ok_or_else(|| StorageError::new(format!("S3 CreateMultipartUpload '{key}': no UploadId in the response")))
    }

    fn upload_part(&self, key: &str, upload_id: &str, number: &str, data: &[u8]) -> Result<String, StorageError> {
        let resp = self.send_with_query("PUT", key, &[("partNumber", number), ("uploadId", upload_id)], data.to_vec(), None)?;
        if !resp.is_success() {
            return Err(Self::error_from_response("UploadPart", key, &resp));
        }
        resp.header("etag").map(str::to_string)
            .ok_or_else(|| StorageError::new(format!("S3 UploadPart '{key}' part {number}: no ETag in the response")))
    }

    fn complete_multipart_upload(&self, key: &str, upload_id: &str, etags: &[String]) -> Result<(), StorageError> {
        let mut body = String::from("<CompleteMultipartUpload>");
        for (i, etag) in etags.iter().enumerate() {
            body.push_str(&format!("<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>", i + 1, xml_escape(etag)));
        }
        body.push_str("</CompleteMultipartUpload>");
        let resp = self.send_with_query("POST", key, &[("uploadId", upload_id)], body.into_bytes(), Some("application/xml"))?;
        // S3 can answer 200 and still report an error in the body.
        let text = resp.text().unwrap_or_default();
        if !resp.is_success() || text.contains("<Error>") {
            return Err(StorageError::new(format!("S3 CompleteMultipartUpload '{key}' failed: HTTP {} {text}", resp.status())));
        }
        Ok(())
    }

    fn abort_multipart_upload(&self, key: &str, upload_id: &str) -> Result<(), StorageError> {
        let resp = self.send_with_query("DELETE", key, &[("uploadId", upload_id)], Vec::new(), None)?;
        if !resp.is_success() {
            return Err(Self::error_from_response("AbortMultipartUpload", key, &resp));
        }
        Ok(())
    }
}

/// The text of the first `<name>…</name>` in `xml`.
fn xml_element(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{name}>"))?;
    Some(xml[start..start + end].to_string())
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() { None } else { Some(s.to_string()) }
}

#[cfg(test)]
mod tests;
