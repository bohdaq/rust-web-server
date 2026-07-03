//! Webhook signature verification for common providers, gated behind the
//! `webhook` Cargo feature (`hmac` + `sha2` — the same RustCrypto crates
//! already used by [`crate::auth`] and [`crate::crypto`]).
//!
//! Every webhook-receiving handler needs to verify that a request actually
//! came from the provider, not an attacker who guessed (or found leaked) the
//! endpoint URL. All three schemes below are HMAC-SHA256-based, but the
//! header format, signed-payload construction, and encoding differ per
//! provider — [`verify_webhook_signature`] and its per-provider siblings
//! encode the exact convention each one uses, so handlers don't have to
//! re-derive it from provider docs.
//!
//! # Example
//!
//! ```rust,no_run
//! # #[cfg(feature = "webhook")]
//! # fn example() {
//! use rust_web_server::webhook::{verify_webhook_signature, WebhookProvider};
//!
//! fn handle_github_webhook(body: &[u8], signature_header: &str, secret: &[u8]) -> bool {
//!     verify_webhook_signature(WebhookProvider::GitHub, body, secret, signature_header)
//! }
//! # }
//! ```
//!
//! # Which header to read
//!
//! | Provider | Header | Format |
//! |---|---|---|
//! | GitHub | `X-Hub-Signature-256` | `sha256=<hex>` |
//! | Shopify | `X-Shopify-Hmac-Sha256` | `<base64>` |
//! | Stripe | `Stripe-Signature` | `t=<unix_ts>,v1=<hex>[,v1=<hex>...]` |

#[cfg(test)]
mod tests;

use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Default timestamp tolerance for [`verify_stripe_signature`] — 300 seconds
/// (5 minutes), matching Stripe's own recommended default. Requests signed
/// further in the past (or future, guarding against clock skew abuse) than
/// this are rejected as a replay-protection measure.
pub const STRIPE_DEFAULT_TOLERANCE_SECS: u64 = 300;

/// Which provider's signature convention to verify a webhook request against.
///
/// Each variant corresponds to one specific header format, HMAC payload
/// construction, and encoding — see the module docs table and the
/// per-provider function docs for the exact convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookProvider {
    /// GitHub — see [`verify_github_signature`].
    GitHub,
    /// Shopify — see [`verify_shopify_signature`].
    Shopify,
    /// Stripe — see [`verify_stripe_signature`]. Uses the default
    /// [`STRIPE_DEFAULT_TOLERANCE_SECS`] timestamp tolerance; call
    /// [`verify_stripe_signature_with_tolerance`] directly to customize it.
    Stripe,
}

/// Verifies `header_value` against `body` and `secret` using `provider`'s
/// signature convention. Returns `false` on any failure: wrong secret,
/// tampered body, malformed header, or (Stripe only) a timestamp outside the
/// default tolerance window.
///
/// `body` must be the exact raw request bytes — re-serializing a parsed JSON
/// body before verifying will not reproduce the byte sequence the provider
/// signed and will always fail verification.
pub fn verify_webhook_signature(
    provider: WebhookProvider,
    body: &[u8],
    secret: &[u8],
    header_value: &str,
) -> bool {
    match provider {
        WebhookProvider::GitHub => verify_github_signature(body, secret, header_value),
        WebhookProvider::Shopify => verify_shopify_signature(body, secret, header_value),
        WebhookProvider::Stripe => verify_stripe_signature(body, secret, header_value),
    }
}

/// Verifies a GitHub webhook's `X-Hub-Signature-256` header value —
/// `"sha256=<hex-encoded-hmac-sha256-of-body>"`, keyed by the webhook's
/// configured secret.
///
/// GitHub also sends the older `X-Hub-Signature` header (HMAC-**SHA1**) for
/// backward compatibility. This function deliberately does not support it —
/// SHA-1 is cryptographically broken, this crate has no SHA-1 dependency,
/// and GitHub recommends `X-Hub-Signature-256` for all new integrations.
pub fn verify_github_signature(body: &[u8], secret: &[u8], header_value: &str) -> bool {
    let Some(hex_sig) = header_value.strip_prefix("sha256=") else {
        return false;
    };
    let Some(expected) = from_hex(hex_sig) else {
        return false;
    };
    hmac_verify(secret, body, &expected)
}

/// Verifies a Shopify webhook's `X-Shopify-Hmac-Sha256` header value — the
/// standard base64 encoding of an HMAC-SHA256 digest of the raw body, keyed
/// by the app's configured secret.
pub fn verify_shopify_signature(body: &[u8], secret: &[u8], header_value: &str) -> bool {
    let Some(expected) = base64_decode(header_value) else {
        return false;
    };
    hmac_verify(secret, body, &expected)
}

/// Verifies a Stripe webhook's `Stripe-Signature` header using the default
/// [`STRIPE_DEFAULT_TOLERANCE_SECS`] timestamp tolerance. See
/// [`verify_stripe_signature_with_tolerance`] for the full format and to
/// customize the tolerance window.
pub fn verify_stripe_signature(body: &[u8], secret: &[u8], header_value: &str) -> bool {
    verify_stripe_signature_with_tolerance(body, secret, header_value, STRIPE_DEFAULT_TOLERANCE_SECS)
}

/// Verifies a Stripe webhook's `Stripe-Signature` header value:
/// `"t=<unix_timestamp>,v1=<hex_hmac_sha256>[,v1=<hex_hmac_sha256>...]"`.
///
/// The signed payload Stripe actually hashes is `"{timestamp}.{body}"`
/// (timestamp as a decimal string, a literal `.`, then the raw body bytes),
/// HMAC-SHA256 keyed by the webhook's signing secret.
///
/// Stripe may send multiple `v1=` entries while a secret rotation is in
/// progress — this accepts the header if *any* entry matches. Rejects the
/// header if `timestamp` is more than `tolerance_secs` away (in either
/// direction) from the current system time, which guards against replaying
/// a captured request long after the fact.
pub fn verify_stripe_signature_with_tolerance(
    body: &[u8],
    secret: &[u8],
    header_value: &str,
    tolerance_secs: u64,
) -> bool {
    let mut timestamp: Option<u64> = None;
    let mut signatures: Vec<&str> = Vec::new();

    for part in header_value.split(',') {
        let part = part.trim();
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = t.parse().ok();
        } else if let Some(v1) = part.strip_prefix("v1=") {
            signatures.push(v1);
        }
    }

    let Some(timestamp) = timestamp else {
        return false;
    };
    if signatures.is_empty() {
        return false;
    }

    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return false;
    };
    if now.as_secs().abs_diff(timestamp) > tolerance_secs {
        return false;
    }

    let signed_payload = [timestamp.to_string().as_bytes(), b".", body].concat();

    signatures
        .iter()
        .any(|hex_sig| from_hex(hex_sig).is_some_and(|expected| hmac_verify(secret, &signed_payload, &expected)))
}

fn hmac_verify(secret: &[u8], message: &[u8], expected: &[u8]) -> bool {
    let Ok(mut mac) = <HmacSha256 as Mac>::new_from_slice(secret) else {
        return false;
    };
    mac.update(message);
    mac.verify_slice(expected).is_ok()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() || s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

// Decodes standard base64 (RFC 4648, `+`/`/` alphabet). Padding characters
// ('=') are stripped before decoding; Shopify emits padded output but this
// tolerates unpadded input too.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let bytes: Vec<u8> = input.bytes().filter(|&b| b != b'=').collect();
    if bytes.is_empty() || bytes.len() % 4 == 1 {
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
