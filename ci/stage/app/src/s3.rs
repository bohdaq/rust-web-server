//! S3 (`storage-s3`): rws's `S3Storage` for PUT/GET/DELETE, plus a SigV4
//! query-string presigner — rws has no presigned URLs (its `aws_sigv4`
//! signs headers only).

use hmac::{Hmac, Mac};
use rust_web_server::storage::{S3Config, S3Storage};
use sha2::{Digest, Sha256};

use crate::common::env;

pub struct S3 {
    pub storage: S3Storage,
    pub config: S3Config,
}

impl S3 {
    /// `S3_ENDPOINT`, `S3_BUCKET`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
    /// `S3_REGION` (default `us-east-1`). `Err` names the first one missing.
    pub fn from_env() -> Result<S3, String> {
        let get = |name: &str| env(name).ok_or_else(|| name.to_string());
        let config = S3Config {
            endpoint: get("S3_ENDPOINT")?.trim_end_matches('/').to_string(),
            bucket: get("S3_BUCKET")?,
            access_key: get("AWS_ACCESS_KEY_ID")?,
            secret_key: get("AWS_SECRET_ACCESS_KEY")?,
            region: env("S3_REGION").unwrap_or_else(|| "us-east-1".to_string()),
        };
        Ok(S3 { storage: S3Storage::new(config.clone()), config })
    }

    /// A presigned `GET` URL for `key`, valid `expires_secs` (path-style,
    /// like `S3Storage`).
    pub fn presign_get(&self, key: &str, expires_secs: u64, epoch_secs: u64) -> String {
        presign(&self.config, "GET", key, expires_secs, epoch_secs)
    }
}

const UNRESERVED: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

fn encode(s: &str, keep_slash: bool) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if UNRESERVED.contains(&b) || (keep_slash && b == b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// `(YYYYMMDD, YYYYMMDDTHHMMSSZ)` for a Unix time.
fn amz_dates(epoch: u64) -> (String, String) {
    let days = (epoch / 86_400) as i64;
    let rem = epoch % 86_400;
    // Civil from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    let date = format!("{y:04}{m:02}{d:02}");
    let time = format!("{date}T{:02}{:02}{:02}Z", rem / 3600, (rem % 3600) / 60, rem % 60);
    (date, time)
}

/// The `Host` a client sends for `endpoint`: with the port when it isn't
/// the scheme's default.
fn host_of(endpoint: &str) -> String {
    let (default_port, rest) = if let Some(r) = endpoint.strip_prefix("https://") {
        ("443", r)
    } else if let Some(r) = endpoint.strip_prefix("http://") {
        ("80", r)
    } else {
        ("443", endpoint)
    };
    let authority = rest.split('/').next().unwrap_or(rest);
    match authority.rsplit_once(':') {
        Some((host, port)) if port == default_port => host.to_string(),
        _ => authority.to_string(),
    }
}

pub fn presign(config: &S3Config, method: &str, key: &str, expires_secs: u64, epoch_secs: u64) -> String {
    let path = encode(&format!("/{}/{}", config.bucket, key), true);
    format!("{}{}", config.endpoint, sign_query(config, method, &path, expires_secs, epoch_secs))
}

/// `{path}?{query}` with the SigV4 query signature (`UNSIGNED-PAYLOAD`,
/// only `host` signed). `path` is already URI-encoded.
fn sign_query(config: &S3Config, method: &str, path: &str, expires_secs: u64, epoch_secs: u64) -> String {
    let (date, datetime) = amz_dates(epoch_secs);
    let host = host_of(&config.endpoint);
    let scope = format!("{date}/{}/s3/aws4_request", config.region);
    let credential = format!("{}/{scope}", config.access_key);
    // Already in sorted order.
    let params = [
        ("X-Amz-Algorithm", "AWS4-HMAC-SHA256".to_string()),
        ("X-Amz-Credential", credential),
        ("X-Amz-Date", datetime.clone()),
        ("X-Amz-Expires", expires_secs.to_string()),
        ("X-Amz-SignedHeaders", "host".to_string()),
    ];
    let query = params.iter().map(|(k, v)| format!("{}={}", encode(k, false), encode(v, false))).collect::<Vec<_>>().join("&");
    let canonical = format!("{method}\n{path}\n{query}\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD");
    let to_sign = format!("AWS4-HMAC-SHA256\n{datetime}\n{scope}\n{}", hex(&Sha256::digest(canonical.as_bytes())));
    let k = hmac(format!("AWS4{}", config.secret_key).as_bytes(), date.as_bytes());
    let k = hmac(&k, config.region.as_bytes());
    let k = hmac(&k, b"s3");
    let k = hmac(&k, b"aws4_request");
    format!("{path}?{query}&X-Amz-Signature={}", hex(&hmac(&k, to_sign.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AWS's documented example ("Authenticating Requests: Using Query
    /// Parameters", GET /test.txt, 2013-05-24, 86400 s).
    #[test]
    fn presign_matches_the_aws_example() {
        let config = S3Config {
            bucket: "examplebucket".into(),
            region: "us-east-1".into(),
            access_key: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
            endpoint: "https://examplebucket.s3.amazonaws.com".into(),
        };
        // AWS's example is virtual-hosted: the path is the key alone.
        let url = sign_query(&config, "GET", "/test.txt", 86_400, 1_369_353_600);
        assert!(url.ends_with("X-Amz-Signature=aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404"), "{url}");
    }

    #[test]
    fn dates_and_hosts() {
        assert_eq!(("20130524".to_string(), "20130524T000000Z".to_string()), amz_dates(1_369_353_600));
        assert_eq!("minio:9000", host_of("http://minio:9000"));
        assert_eq!("s3.example.com", host_of("https://s3.example.com:443/"));
    }

    #[test]
    fn presigned_url_is_path_style() {
        let config = S3Config {
            bucket: "b".into(),
            region: "us-east-1".into(),
            access_key: "AK".into(),
            secret_key: "SK".into(),
            endpoint: "https://s3.example.com".into(),
        };
        let url = presign(&config, "GET", "a b.txt", 300, 1_700_000_000);
        assert!(url.starts_with("https://s3.example.com/b/a%20b.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&"), "{url}");
        assert!(url.contains("X-Amz-Expires=300"));
    }
}
