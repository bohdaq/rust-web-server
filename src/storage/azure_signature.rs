//! Azure Storage Shared Key request signing for `AzureBlobStorage` — no
//! Azure SDK. Implements the "current"/augmented Shared Key signature format
//! (x-ms-version 2009-09-19 and later) for single-blob `PUT`/`GET`/`DELETE`.
//! Reference: <https://learn.microsoft.com/en-us/rest/api/storageservices/authorize-with-shared-key>

use super::StorageError;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTH_NAMES: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Formats an RFC 1123 date (`Sun, 06 Nov 1994 08:49:37 GMT`) for the
/// `x-ms-date` header — required on every request regardless of auth scheme.
pub(super) fn rfc1123_date(epoch_secs: u64) -> String {
    let (sec, min, hour, day, month, dow) = crate::scheduler::cron::epoch_to_datetime(epoch_secs);
    let (year, _, _) = crate::scheduler::cron::days_to_ymd(epoch_secs / 86400);
    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
        DAY_NAMES[dow as usize], day, MONTH_NAMES[(month - 1) as usize], year, hour, min, sec
    )
}

/// Signs a Blob Shared Key request and returns the `Authorization` header
/// value (`SharedKey {account}:{signature}`).
///
/// `canonical_path` must be `/{container}/{key}`. `x_ms_headers` must be
/// exactly the `x-ms-*` headers the caller is about to send on the wire
/// (e.g. `x-ms-date`, `x-ms-version`, and `x-ms-blob-type` on `PUT`) — every
/// one of them participates in the signature, so a mismatch between what's
/// signed here and what's actually sent breaks the signature.
pub(super) fn sign(
    method: &str,
    account: &str,
    canonical_path: &str,
    content_type: &str,
    content_length: usize,
    account_key_base64: &str,
    x_ms_headers: &[(String, String)],
) -> Result<String, StorageError> {
    let content_length_str = if content_length == 0 { String::new() } else { content_length.to_string() };

    let fields = [
        method,
        "",                       // Content-Encoding
        "",                       // Content-Language
        content_length_str.as_str(),
        "",                       // Content-MD5
        content_type,
        "",                       // Date (x-ms-date is used instead)
        "",                       // If-Modified-Since
        "",                       // If-Match
        "",                       // If-None-Match
        "",                       // If-Unmodified-Since
        "",                       // Range
    ];

    let canonicalized_headers = canonicalize_headers(x_ms_headers);
    let canonicalized_resource = format!("/{account}{canonical_path}");

    let string_to_sign = format!("{}\n{}{}", fields.join("\n"), canonicalized_headers, canonicalized_resource);

    let key = base64_decode(account_key_base64)
        .ok_or_else(|| StorageError::new("RWS_AZURE_ACCOUNT_KEY is not valid base64"))?;
    let signature = base64_encode(&hmac_sha256(&key, string_to_sign.as_bytes()));

    Ok(format!("SharedKey {account}:{signature}"))
}

/// Retrieves every `x-ms-*` header, lowercases the name, sorts
/// lexicographically, and joins as `name:value\n` per header — per the
/// `CanonicalizedHeaders` construction rules.
fn canonicalize_headers(headers: &[(String, String)]) -> String {
    let mut ms_headers: Vec<(String, String)> = headers
        .iter()
        .filter(|(k, _)| k.to_ascii_lowercase().starts_with("x-ms-"))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
        .collect();
    ms_headers.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::new();
    for (k, v) in ms_headers {
        out.push_str(&k);
        out.push(':');
        out.push_str(&v);
        out.push('\n');
    }
    out
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

// ── Standard base64 (with padding) — decodes the account key, encodes the
// resulting signature. Kept local rather than shared with `crate::auth`'s
// base64 helpers, matching this crate's convention of not coupling
// independently feature-gated signing modules (see `aws_sigv4`'s own
// self-contained HMAC helper). ──────────────────────────────────────────────

fn b64_val(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let bytes: Vec<u8> = input.bytes().filter(|&b| b != b'=').collect();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let a = b64_val(chunk[0])?;
        let b = b64_val(chunk[1])?;
        out.push((a << 2) | (b >> 4));
        if chunk.len() > 2 {
            let c = b64_val(chunk[2])?;
            out.push((b << 4) | (c >> 2));
            if chunk.len() > 3 {
                let d = b64_val(chunk[3])?;
                out.push((c << 6) | d);
            }
        }
    }
    Some(out)
}

fn base64_encode(input: &[u8]) -> String {
    const C: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        out.push(C[b0 >> 2] as char);
        out.push(C[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(if chunk.len() > 1 { C[((b1 & 0xf) << 2) | (b2 >> 6)] as char } else { '=' });
        out.push(if chunk.len() > 2 { C[b2 & 0x3f] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests;
