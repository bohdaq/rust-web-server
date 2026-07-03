use super::*;

fn github_header(body: &[u8], secret: &[u8]) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret).unwrap();
    mac.update(body);
    format!("sha256={}", to_hex(&mac.finalize().into_bytes()))
}

fn shopify_header(body: &[u8], secret: &[u8]) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret).unwrap();
    mac.update(body);
    base64_encode(&mac.finalize().into_bytes())
}

fn stripe_header(body: &[u8], secret: &[u8], timestamp: u64) -> String {
    let signed_payload = [timestamp.to_string().as_bytes(), b".", body].concat();
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret).unwrap();
    mac.update(&signed_payload);
    format!("t={},v1={}", timestamp, to_hex(&mac.finalize().into_bytes()))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
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

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

// ── GitHub ────────────────────────────────────────────────────────────────

#[test]
fn github_valid_signature_passes() {
    let body = br#"{"action":"opened"}"#;
    let secret = b"my-webhook-secret";
    let header = github_header(body, secret);
    assert!(verify_github_signature(body, secret, &header));
}

#[test]
fn github_wrong_secret_fails() {
    let body = br#"{"action":"opened"}"#;
    let header = github_header(body, b"correct-secret");
    assert!(!verify_github_signature(body, b"wrong-secret", &header));
}

#[test]
fn github_tampered_body_fails() {
    let secret = b"my-webhook-secret";
    let header = github_header(b"original body", secret);
    assert!(!verify_github_signature(b"tampered body", secret, &header));
}

#[test]
fn github_missing_prefix_fails() {
    let body = b"payload";
    let secret = b"secret";
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret).unwrap();
    mac.update(body);
    let hex_only = to_hex(&mac.finalize().into_bytes());
    assert!(!verify_github_signature(body, secret, &hex_only));
}

#[test]
fn github_malformed_hex_fails() {
    assert!(!verify_github_signature(b"body", b"secret", "sha256=not-hex!!"));
}

#[test]
fn github_empty_header_fails() {
    assert!(!verify_github_signature(b"body", b"secret", ""));
}

// ── Shopify ───────────────────────────────────────────────────────────────

#[test]
fn shopify_valid_signature_passes() {
    let body = br#"{"id":12345}"#;
    let secret = b"shpss_secret";
    let header = shopify_header(body, secret);
    assert!(verify_shopify_signature(body, secret, &header));
}

#[test]
fn shopify_wrong_secret_fails() {
    let body = br#"{"id":12345}"#;
    let header = shopify_header(body, b"correct-secret");
    assert!(!verify_shopify_signature(body, b"wrong-secret", &header));
}

#[test]
fn shopify_tampered_body_fails() {
    let secret = b"secret";
    let header = shopify_header(b"original", secret);
    assert!(!verify_shopify_signature(b"tampered", secret, &header));
}

#[test]
fn shopify_malformed_base64_fails() {
    assert!(!verify_shopify_signature(b"body", b"secret", "not valid base64!!"));
}

// ── Stripe ────────────────────────────────────────────────────────────────

#[test]
fn stripe_valid_signature_passes() {
    let body = br#"{"type":"payment_intent.succeeded"}"#;
    let secret = b"whsec_test_secret";
    let header = stripe_header(body, secret, now_secs());
    assert!(verify_stripe_signature(body, secret, &header));
}

#[test]
fn stripe_wrong_secret_fails() {
    let body = b"payload";
    let header = stripe_header(body, b"correct-secret", now_secs());
    assert!(!verify_stripe_signature(body, b"wrong-secret", &header));
}

#[test]
fn stripe_tampered_body_fails() {
    let secret = b"secret";
    let header = stripe_header(b"original", secret, now_secs());
    assert!(!verify_stripe_signature(b"tampered", secret, &header));
}

#[test]
fn stripe_timestamp_outside_tolerance_fails() {
    let body = b"payload";
    let secret = b"secret";
    let old_timestamp = now_secs() - 10_000;
    let header = stripe_header(body, secret, old_timestamp);
    assert!(!verify_stripe_signature(body, secret, &header));
}

#[test]
fn stripe_timestamp_within_custom_tolerance_passes() {
    let body = b"payload";
    let secret = b"secret";
    let old_timestamp = now_secs() - 10_000;
    let header = stripe_header(body, secret, old_timestamp);
    assert!(verify_stripe_signature_with_tolerance(body, secret, &header, 20_000));
}

#[test]
fn stripe_multiple_v1_entries_matches_any() {
    let body = b"payload";
    let secret = b"current-secret";
    let timestamp = now_secs();
    let signed_payload = [timestamp.to_string().as_bytes(), b".", body.as_ref()].concat();
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret).unwrap();
    mac.update(&signed_payload);
    let current_sig = to_hex(&mac.finalize().into_bytes());

    // Simulates a secret-rotation window: an old (invalid) v1 signature listed
    // alongside the current one — only the second must match to pass.
    let header = format!("t={},v1=deadbeef,v1={}", timestamp, current_sig);
    assert!(verify_stripe_signature(body, secret, &header));
}

#[test]
fn stripe_missing_timestamp_fails() {
    assert!(!verify_stripe_signature(b"body", b"secret", "v1=abcd1234"));
}

#[test]
fn stripe_missing_v1_fails() {
    let timestamp = now_secs();
    assert!(!verify_stripe_signature(b"body", b"secret", &format!("t={}", timestamp)));
}

#[test]
fn stripe_empty_header_fails() {
    assert!(!verify_stripe_signature(b"body", b"secret", ""));
}

// ── Dispatcher ────────────────────────────────────────────────────────────

#[test]
fn verify_webhook_signature_dispatches_to_github() {
    let body = b"payload";
    let secret = b"secret";
    let header = github_header(body, secret);
    assert!(verify_webhook_signature(WebhookProvider::GitHub, body, secret, &header));
}

#[test]
fn verify_webhook_signature_dispatches_to_shopify() {
    let body = b"payload";
    let secret = b"secret";
    let header = shopify_header(body, secret);
    assert!(verify_webhook_signature(WebhookProvider::Shopify, body, secret, &header));
}

#[test]
fn verify_webhook_signature_dispatches_to_stripe() {
    let body = b"payload";
    let secret = b"secret";
    let header = stripe_header(body, secret, now_secs());
    assert!(verify_webhook_signature(WebhookProvider::Stripe, body, secret, &header));
}

#[test]
fn verify_webhook_signature_wrong_provider_fails() {
    let body = b"payload";
    let secret = b"secret";
    let header = github_header(body, secret);
    // A GitHub-shaped header ("sha256=...") isn't valid base64 in a way that
    // would accidentally match Shopify's raw-base64 convention.
    assert!(!verify_webhook_signature(WebhookProvider::Shopify, body, secret, &header));
}
