---
title: Webhook Signature Verification
description: Verify inbound webhook signatures from GitHub, Shopify, and Stripe with built-in HMAC helpers.
---

Requires the `webhook` feature:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["webhook"] }
```

Any handler that accepts webhooks from a third party needs to confirm the request actually came from that provider — otherwise anyone who finds (or guesses) the endpoint URL can send a forged payload. `hmac` and `sha2` are already dependencies elsewhere in `rws` (`auth`, `crypto`, `storage-s3`); this feature just adds the provider-specific conventions on top so you don't re-derive them from each provider's docs.

## Quick start

```rust
use rust_web_server::webhook::{verify_webhook_signature, WebhookProvider};

fn handle_github_webhook(body: &[u8], signature_header: &str, secret: &[u8]) -> bool {
    verify_webhook_signature(WebhookProvider::GitHub, body, secret, signature_header)
}
```

:::caution[Use the raw request body]
`body` must be the exact bytes read off the wire. Re-serializing a parsed `Json<T>` value before verifying will not reproduce the byte sequence the provider actually signed, and verification will always fail. Read it via the `Body` extractor or `request.body` directly.
:::

## Supported providers

| Provider | Header | Format |
|---|---|---|
| GitHub | `X-Hub-Signature-256` | `sha256=<hex-hmac-sha256-of-body>` |
| Shopify | `X-Shopify-Hmac-Sha256` | `<base64-hmac-sha256-of-body>` |
| Stripe | `Stripe-Signature` | `t=<unix_ts>,v1=<hex>[,v1=<hex>...]` |

Call the per-provider function directly when the provider is known at the call site — this is equivalent to `verify_webhook_signature` with the matching `WebhookProvider` variant:

```rust
use rust_web_server::webhook::{verify_github_signature, verify_shopify_signature, verify_stripe_signature};

verify_github_signature(body, secret, header_value);
verify_shopify_signature(body, secret, header_value);
verify_stripe_signature(body, secret, header_value);
```

:::note[GitHub's legacy `X-Hub-Signature` header]
GitHub also sends an older SHA-1-based `X-Hub-Signature` for backward compatibility. `verify_github_signature` only supports `X-Hub-Signature-256` — SHA-1 is cryptographically broken and this crate has no SHA-1 dependency. Always prefer the SHA-256 header; GitHub recommends it for all new integrations.
:::

## Stripe's timestamp tolerance

Stripe signs `"{timestamp}.{body}"`, not the body alone, and expects you to reject requests whose timestamp is too far from the current time — this stops a captured request from being replayed later. `verify_stripe_signature` checks against a default 300-second (5-minute) window, matching Stripe's own recommendation:

```rust
use rust_web_server::webhook::{verify_stripe_signature_with_tolerance, STRIPE_DEFAULT_TOLERANCE_SECS};

// Widen the window to 10 minutes:
verify_stripe_signature_with_tolerance(body, secret, header_value, 600);

// The default constant, if you want to reference it:
assert_eq!(300, STRIPE_DEFAULT_TOLERANCE_SECS);
```

Stripe may list multiple `v1=` entries in the header while a signing-secret rotation is in progress — verification succeeds if *any* entry matches, so you don't need to special-case rotation.

## In a handler

```rust
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::request::Request;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::webhook::{verify_webhook_signature, WebhookProvider};

fn stripe_webhook(request: &Request) -> Response {
    let signature = request
        .get_header("Stripe-Signature".to_string())
        .map(|h| h.value.clone())
        .unwrap_or_default();

    if !verify_webhook_signature(WebhookProvider::Stripe, &request.body, b"whsec_...", &signature) {
        let mut res = Response::new();
        res.status_code = *STATUS_CODE_REASON_PHRASE.n401_unauthorized.status_code;
        return res;
    }

    // Signature verified — safe to process request.body as a trusted Stripe event.
    Response::new()
}
```
