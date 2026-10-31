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

## Relationships

What a caller may do is answered from **relationship tuples** and a
**schema**, Zanzibar style. A tuple says a subject has a relation to an
object:

```text
doc:42#owner@user:olga                 olga owns doc 42
group:eng#member@user:gina             gina is in eng
doc:42#editor@group:eng#member         every member of eng edits doc 42
doc:42#parent@folder:specs             doc 42 is in folder specs
```

The schema says, per object type, what each relation is made of — terms
joined by `|`:

| Term | Means |
|---|---|
| `user` | tuples naming a `user:…` directly |
| `group#member` | tuples naming `group:…#member`: those members |
| `owner` | whoever has `owner` on the same object |
| `parent->viewer` | the viewers of whatever this object's `parent` is |

```rust
use rust_web_server::authz::{Authorizer, MemoryStore, Schema};

let schema = Schema::new()
    .relation("group", "member", "user | group#member")?   // groups in groups
    .relation("folder", "viewer", "user | group#member")?
    .relation("doc", "parent", "folder")?
    .relation("doc", "owner", "user")?
    .relation("doc", "editor", "user | group#member | owner")?
    .relation("doc", "viewer", "user | editor | parent->viewer")?;
let authz = Authorizer::new(schema, MemoryStore::new())?;

authz.write_str("doc:42#owner@user:olga")?;
authz.check("user:olga", "viewer", "doc:42")?;            // true: owner → editor → viewer
authz.explain("user:olga", "viewer", "doc:42")?;          // the path that allowed it
```

- A bare name is a type when no relation of that type has it, otherwise a
  relation on the same object. Types used only as subjects (`user`) need no
  definition. `Authorizer::new` validates the schema: every `group#member`,
  `parent->viewer` and relation must exist.
- Writes are checked against it: `doc:42#viewer@folder:x` is refused when
  `viewer` doesn't take folders.
- A check walks the graph without recursion, ignores cycles, and stops with
  `AuthzError::Limit` after 25 relations deep or 1000 tuple reads — treat
  that as a "no".
- Names are letters, digits, `_`, `-` and `.`; ids anything but `#`, `@`,
  spaces and control characters; both up to 256 characters.

Tuples live in a `TupleStore`: `MemoryStore` here (lost on restart); a SQL
store, config-file routes and admin API endpoints follow in later 18.14
work.
