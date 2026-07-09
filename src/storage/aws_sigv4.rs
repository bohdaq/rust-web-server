//! AWS Signature Version 4 request signing for `S3Storage` — no AWS SDK.
//!
//! Implements just enough of the SigV4 spec to sign single-object
//! `PUT`/`GET`/`DELETE` requests with header-based auth (no presigned URLs,
//! no query-string signing, no multipart/chunked upload signing).
//! Reference: <https://docs.aws.amazon.com/general/latest/gr/sigv4-create-canonical-request.html>

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Percent-encode a full object path (e.g. `/bucket/key with spaces.png`),
/// preserving `/` as a path separator but encoding everything else outside
/// the unreserved set (`A-Z a-z 0-9 - . _ ~`). The same encoded string must
/// be used both in the request line and in the canonical request signed
/// here — encoding it once in `S3Storage::object_url` and reusing the result
/// for signing keeps the two in sync.
pub(super) fn uri_encode_path(path: &str) -> String {
    path.split('/').map(uri_encode_segment).collect::<Vec<_>>().join("/")
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

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// `(date, datetime)` as `YYYYMMDD` and `YYYYMMDDTHHMMSSZ`, both required by
/// SigV4. Built from `crate::scheduler::cron::days_to_ymd` (already
/// implements Gregorian calendar decomposition) rather than a new date
/// dependency.
fn amz_date_and_datetime(epoch_secs: u64) -> (String, String) {
    let days = epoch_secs / 86400;
    let secs_in_day = epoch_secs % 86400;
    let hour = secs_in_day / 3600;
    let min = (secs_in_day % 3600) / 60;
    let sec = secs_in_day % 60;
    let (y, m, d) = crate::scheduler::cron::days_to_ymd(days);
    let date = format!("{:04}{:02}{:02}", y, m, d);
    let datetime = format!("{date}T{:02}{:02}{:02}Z", hour, min, sec);
    (date, datetime)
}

/// Signs a single AWS request (any service — `service` picks the signing
/// scope, e.g. `"s3"` for `S3Storage`, `"secretsmanager"` for
/// `secrets::aws_secrets_manager`). `canonical_path` must already be
/// percent-encoded (see [`uri_encode_path`]) and must be byte-identical to
/// the path sent on the wire; pass `"/"` for a JSON-RPC-style POST API like
/// Secrets Manager's, which has no per-resource path. Returns the headers to
/// attach to the request, in addition to any headers the caller adds itself
/// (e.g. `Content-Type`).
///
/// `session_token` is `Some` when signing with temporary credentials (EKS
/// IRSA, ECS task role, EC2 IMDSv2) — it adds `x-amz-security-token` to the
/// canonical request and signed-headers list, sorted last alphabetically so
/// it never needs interleaving with the three fixed headers. `None`
/// reproduces byte-identical output to signing without a token.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sign(
    method: &str,
    host: &str,
    canonical_path: &str,
    payload: &[u8],
    region: &str,
    access_key: &str,
    secret_key: &str,
    session_token: Option<&str>,
    epoch_secs: u64,
    service: &str,
) -> Vec<(String, String)> {
    let (date, datetime) = amz_date_and_datetime(epoch_secs);
    let payload_hash = to_hex(&Sha256::digest(payload));

    let canonical_headers = match session_token {
        Some(token) => format!(
            "host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{datetime}\nx-amz-security-token:{token}\n"
        ),
        None => format!(
            "host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{datetime}\n"
        ),
    };
    let signed_headers = if session_token.is_some() {
        "host;x-amz-content-sha256;x-amz-date;x-amz-security-token"
    } else {
        "host;x-amz-content-sha256;x-amz-date"
    };

    let canonical_request = format!(
        "{method}\n{canonical_path}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );
    let hashed_canonical_request = to_hex(&Sha256::digest(canonical_request.as_bytes()));

    let credential_scope = format!("{date}/{region}/{service}/aws4_request");
    let string_to_sign =
        format!("AWS4-HMAC-SHA256\n{datetime}\n{credential_scope}\n{hashed_canonical_request}");

    let k_date = hmac(format!("AWS4{secret_key}").as_bytes(), date.as_bytes());
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, service.as_bytes());
    let k_signing = hmac(&k_service, b"aws4_request");
    let signature = to_hex(&hmac(&k_signing, string_to_sign.as_bytes()));

    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );

    let mut headers = vec![
        ("host".to_string(), host.to_string()),
        ("x-amz-content-sha256".to_string(), payload_hash),
        ("x-amz-date".to_string(), datetime),
    ];
    if let Some(token) = session_token {
        headers.push(("x-amz-security-token".to_string(), token.to_string()));
    }
    headers.push(("Authorization".to_string(), authorization));
    headers
}

#[cfg(test)]
mod tests;
