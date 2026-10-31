---
title: Authorization
description: Who the caller is, from any authenticator — and, as relationship-based access control, what they may do.
---

Authentication says a request is allowed in; authorization says what its
caller may do. rws gives every authenticator's caller one shape,
`authz::Identity`, which the rest of the request can read. Since 18.14.0.

## Identity

Each authenticator runs the rest of the request as the caller it
authenticated:

| Authenticator | Subject | Notes |
|---|---|---|
| `JwtLayer` | `user:<sub>` | `.identity_claim("email")` to use another claim; a valid token without it has no identity |
| `OidcAuth` | `user:<sub>` | from the session's ID token claims |
| `SamlSp` | `user:<NameID>` | |
| `BasicAuthLayer` | `user:<name>` | also from an htpasswd file |
| `ForwardAuthLayer` | `user:<value>` | `.identity_header("X-User-Id")`: a header in the auth service's 2xx answer |
| MCP (`require_bearer_tokens`, `require_oauth`) | `user:<sub>` | tool handlers included |

A value that already has a `type:` prefix (`service:billing`) keeps it.
Handlers read the caller with `Identity::current()`:

```rust
use rust_web_server::app::App;
use rust_web_server::auth::JwtLayer;
use rust_web_server::authz::Identity;
use rust_web_server::core::New;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

let app = App::with_state(())
    .get("/me", |_, _, _, _| match Identity::current() {
        Some(me) => Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, me.subject()),
        None => Response::text(STATUS_CODE_REASON_PHRASE.n401_unauthorized, "who are you?"),
    })
    .wrap(JwtLayer::new(b"secret"));
```

`Identity` has `subject()` (`user:alice`), `subject_type()`, `subject_id()`,
`source()` (`jwt`, `oidc`, `saml`, `basic`, `forward`, `mcp`), `claims()`
(the raw JSON, when there were claims) and `claim(name)`.

**It is not a header.** The authenticator runs the rest of the request
inside `authz::with_identity(identity, || …)` — a value tied to the thread
(or, with `authz::scope_identity`, to the async task) for that call — so a
client can't send one, and nothing leaks into the next request. Your own
authenticator can do the same:

```rust
use rust_web_server::authz::{with_identity, Identity};

// inside a Middleware::handle, after checking credentials
with_identity(Identity::new("service:billing", "api-key"), || next.execute(request, connection))
```

Work handed to another thread (a spawned thread, an MCP task) doesn't carry
the identity along; pass it explicitly.
