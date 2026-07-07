---
title: SAML 2.0 SSO
description: SAML 2.0 Service Provider — ACS handler, XML signature verification, and attribute mapping, for enterprise IdPs that don't speak OIDC.
---

The `sso-saml` feature (implies `sso`) adds `SamlSp`, a SAML 2.0 Service Provider middleware — the enterprise/B2B alternative to [OAuth2 / OIDC SSO](/features/sso/) for identity providers that only speak SAML (Active Directory Federation Services, Okta SAML, Google Workspace SAML, Keycloak).

```toml
[dependencies]
rust-web-server = { version = "17", features = ["sso-saml"] }
```

## Quick start

```rust
use std::sync::Arc;
use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::session::SessionStore;
use rust_web_server::sso::saml::{SamlSp, SamlConfig, SamlIdpMetadata, AttributeMap};

let saml_sp = SamlSp::new(SamlConfig {
    sp_entity_id: "https://myapp.com/saml/metadata".into(),
    sp_acs_url:   "https://myapp.com/saml/acs".into(),
    idp_metadata: SamlIdpMetadata::from_file("idp-metadata.xml")?,
    sessions:     Arc::new(SessionStore::new(86_400)),
})
.attribute_map(
    AttributeMap::new()
        .map("http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress", "email")
        .map("http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name", "name"),
);

let app = App::new().wrap(saml_sp);
```

`SamlSp` intercepts four paths and passes everything else through:

| Path | Method | Purpose |
|---|---|---|
| `/saml/metadata` | `GET` | SP metadata XML — give this URL to the IdP |
| `/saml/login` | `GET` | Auto-submitting HTML form POSTing an `AuthnRequest` to the IdP |
| `/saml/acs` | `POST` | Assertion Consumer Service — receives and verifies the IdP's `Response` |
| `/saml/logout` | `GET` | Destroys the local session and redirects home |

Any other path checks the session: if a completed login exists, claims are injected as the `X-Rws-Saml-Claims` header (JSON) and the request proceeds; otherwise the browser is redirected to `/saml/login?return_to=<original path>`.

## Reading claims in a handler

```rust
use rust_web_server::sso::saml::SamlSp;
use rust_web_server::request::Request;

fn dashboard(req: &Request) {
    let name_id = SamlSp::name_id(req).unwrap();       // Subject/NameID
    let email   = SamlSp::attr(req, "email");           // mapped via AttributeMap
    let claims  = SamlSp::claims(req).unwrap();          // full SamlClaims { name_id, attributes }
}
```

## Loading IdP metadata

```rust
use rust_web_server::sso::saml::SamlIdpMetadata;

// From a downloaded/exported metadata file:
let meta = SamlIdpMetadata::from_file("idp-metadata.xml")?;

// Or fetch it directly from the IdP's metadata URL:
let meta = SamlIdpMetadata::from_url("https://idp.corp.com/metadata")?;
```

Both parse the same shape: `entityID`, the `SingleSignOnService` endpoint (an `HTTP-POST` binding entry is preferred if the metadata advertises one — see below for why), and the signing certificate under `KeyDescriptor[use=signing]`.

## What assertion validation checks

Every `POST /saml/acs` request runs the full set the SAML spec calls for:

- XML signature verification (`RSA-SHA256` only) over the `Assertion`
- Exactly one `Assertion` in the response (protects against signature-wrapping attacks — see below)
- `Issuer` matches the configured IdP entity ID
- `Conditions/@NotBefore` / `@NotOnOrAfter` time window (60s leeway)
- `AudienceRestriction/Audience` matches the SP's own entity ID
- `InResponseTo` matches the `AuthnRequest` ID stored at `/saml/login` time (anti-replay)
- `SubjectConfirmation/@Method` is `urn:oasis:names:tc:SAML:2.0:cm:bearer`
- `SubjectConfirmationData/@Recipient` matches the configured ACS URL

## Attribute mapping

SAML attributes have IdP-specific names — often long URIs. `AttributeMap` translates them into the field names your application actually wants to read:

```rust
use rust_web_server::sso::saml::AttributeMap;

let map = AttributeMap::new()
    .map("http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress", "email")
    .map("http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name", "name")
    .map("http://schemas.microsoft.com/ws/2008/06/identity/claims/groups", "groups");
```

An attribute with no mapping entry is simply not exposed under a friendly name — read it via its raw SAML name from `SamlClaims::attributes` directly if needed. Multi-valued SAML attributes keep only their first value.

:::note[Why `SamlClaims`, not `OidcClaims`?]
The design for this feature originally sketched mapping SAML attributes into the same [`OidcClaims`](/features/sso/) shape `OidcAuth` uses — but `OidcClaims` was deliberately built (see the OIDC SSO page) without a `groups` field, and this feature's own attribute-mapping example maps exactly a `groups` attribute. Reusing `OidcClaims` would silently drop it. `SamlClaims { name_id, attributes: HashMap<String, String> }` is a plain, free-form map instead — a better fit for SAML's attribute model anyway.
:::

## Scope and deviations from a full SAML implementation

SAML 2.0 is a large, XML-heavy spec. This implementation covers the common enterprise SP flow — receiving and verifying IdP-initiated or SP-initiated bearer assertions — and makes several deliberate, documented scope decisions rather than attempting full spec coverage:

:::caution[Signature verification is byte-exact, not full XML canonicalization]
Correct XML-DSig verification canonicalizes (C14N) the signed XML before checking digests/signatures, so reformatting whitespace or attribute order doesn't invalidate an otherwise-unchanged signature. This module verifies against the **literal bytes as transmitted** instead of implementing C14N (a large undertaking prone to subtle bugs). The practical effect is **fail-closed, not fail-open**: an IdP that reformats XML between signing and transmission would have legitimate logins rejected (loud, immediate, fixable) — never would a forged assertion be accepted. No mainstream IdP (Okta, Azure AD, Google Workspace, Keycloak) reformats between signing and transmission in this flow.
:::

- **No `quick-xml` dependency.** A small, purpose-built XML parser is hand-rolled instead, consistent with this crate's existing philosophy of not taking dependencies for formats it can parse itself (JSON, HTTP, form-urlencoded are all hand-rolled too). It never processes `<!DOCTYPE>` — rejected outright, eliminating XXE by never implementing entity resolution at all.
- **`RSA-SHA256` only** — no SHA-1 (deprecated), no EC-signed assertions.
- **`EncryptedAssertion` is rejected outright**, not silently ignored — no XML decryption is implemented.
- **`AuthnRequest`s use the HTTP-POST binding**, not HTTP-Redirect (which requires DEFLATE compression this crate has no dependency for), and are **never signed** — most IdPs accept unsigned `AuthnRequest`s by default; the flow's security rests on the IdP-signed *assertion*, which is fully verified.
- **Logout is local-only** — `/saml/logout` destroys the SP's own session; there is no SP-initiated `LogoutRequest`/`LogoutResponse` exchange with the IdP. This mirrors [`OidcAuth`'s own logout](/features/sso/), which draws the identical boundary.

None of these are silent gaps — each one either fails closed (rejects rather than accepts something suspicious) or is a compatibility boundary (an unsupported binding/algorithm), never a case where something insecure is accepted as valid.
