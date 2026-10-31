# Authorization: relationship-based access control (#32)

Written 2026-10-31. Roadmap item #32 ([ROADMAP.md](ROADMAP.md)). Built
tranche by tranche; this file is updated as each lands. Target release:
**18.14.0**.

## What exists

- **Authentication, scattered identity**: `JwtLayer` (HS256/RS256/ES256)
  verifies a token and drops the claims; `OidcAuth` and `SamlSp` put claims
  in their own request headers (`X-Rws-Oidc-Claims`, `X-Rws-Saml-Claims`);
  `BasicAuthLayer` knows the user name only inside its closure;
  `ForwardAuthLayer` copies chosen headers from the auth service; MCP keeps
  `auth_claims` in `McpContext`; config-file `bearer` auth has no identity.
- **No authorization** beyond "authenticated or not" (and MCP's per-identity
  tool allow lists in the gateway).
- A model layer (`DbPool`, sqlx: SQLite / Postgres / MySQL features) and an
  admin API + MCP tools (`admin::ops`, names are a contract).

## Decisions

User, 2026-10-31:
- **Relationship-based (ReBAC)**, Zanzibar style: relationship tuples
  (`doc:42#editor@user:alice`) and a schema saying how relations derive
  from each other; `check(subject, relation, object)` walks the graph.
- **Schema in config and code**: `[authz]` in `rws.config.toml` (validated
  by `rws --check`, reloaded on SIGHUP) or built in Rust. Not editable at
  runtime.
- **Identity from every authenticator**: JwtLayer, OidcAuth, SamlSp,
  BasicAuthLayer, ForwardAuthLayer, MCP bearer tokens and the config file's
  auth all hand one `Identity` to authorization. No identity → `401`, not
  allowed → `403`.
- **Tuples in memory or SQL**: a `TupleStore` trait; in-memory by default
  (tests, small apps, seeded from the config file), SQL on the model layer
  (an `rws_authz_tuples` table), reads cached briefly.
- **Tuples written from code and the admin API**: `authz.write` / `delete`
  in Rust; admin API endpoints to list, add, remove and check; matching MCP
  tools on the binary.

Implementation (defaults):
- **Identity is not a header.** An authenticator runs `next` inside
  `authz::with_identity(identity, ..)` (a thread-local, and a task-local for
  async code, as `early_hints` does), so a client can't send one;
  `Identity::current()` gives handlers the caller. `Identity { subject:
  "user:alice", source, claims: Option<String> (raw JSON) }`; the subject's
  type and the claim it comes from are configurable (`sub` by default; Basic
  → the user name; forward auth → a named header from the auth service).
- **Schema language** (one string per relation, hand-parsed, fuzzed, depth-
  free loops):
  ```toml
  [authz]
  store = "memory"                     # or "sql" (RWS_DB_* / the app's DbPool)
  tuples = ["folder:root#owner@user:alice", "doc:42#parent@folder:root"]

  [authz.type.group]
  member = "user"
  [authz.type.folder]
  owner  = "user"
  viewer = "user | group#member | owner"
  [authz.type.doc]
  parent = "folder"
  owner  = "user"
  editor = "user | owner"
  viewer = "user | group#member | editor | parent->viewer"
  ```
  Terms: a subject type (`user`), a userset (`group#member`), another
  relation on the same object (`owner`), or a tuple-to-userset
  (`parent->viewer`: the viewers of whatever `parent` points at). Union
  only in the first version (`&` / `-` later if asked).
- **Check**: iterative graph walk with a visited set, depth limit 25 and a
  read budget per check; a cycle or an exhausted budget is a deny, logged.
  `Authorizer::check(subject, relation, object) -> Result<bool, AuthzError>`;
  `explain` returns the path that allowed it (for the admin API's check).
- **Routes**: `[route.middleware.authz] relation = "viewer"`, `object =
  "doc:{path:2}"` — templates take `{path:N}` (Nth path segment), `{query:k}`,
  `{header:Name}`; `rws --check` checks types, relations and templates
  against the schema. Library: `AuthzLayer::new(authorizer, |request| (relation,
  object))`.
- **SQL store**: one table (`object_type, object_id, relation, subject_type,
  subject_id, subject_relation`, unique), created if missing; async `DbPool`
  driven from the sync check through `async_bridge::block_on_isolated`;
  per-check memo plus a 1 s read cache, cleared by this process's writes.
- **Admin API**: `GET /authz/tuples?object=&subject=`, `POST /authz/tuples`,
  `DELETE /authz/tuples`, `POST /authz/check` (with the explaining path);
  MCP tools `authz_check`, `authz_tuples`, `authz_write`, `authz_delete`.

## Tranches

| # | Tranche | Done when |
|---|---|---|
| 1 | Identity | `authz::Identity`, `with_identity` / `Identity::current()` (thread + task local); set by JwtLayer, OidcAuth, SamlSp, BasicAuthLayer, ForwardAuthLayer, MCP bearer/OAuth and config-file auth; subject mapping options; tests per authenticator and a spoofing test |
| 2 | Schema + check | relation-expression parser (fuzz target, stack-depth case), schema types, `MemoryStore`, `Authorizer::check` / `explain` with limits; tests on a Zanzibar-style folder/doc/group model incl. cycles and budgets |
| 3 | Routes + config | `[authz]` (schema, seed tuples), `[route.middleware.authz]` with templates, `AuthzLayer`, 401/403, `rws --check`, reload; docs page `features/authorization.md`; e2e |
| 4 | SQL store | `SqlStore` on `DbPool` (SQLite tests; Postgres/MySQL compiled in CI), read cache; `store = "sql"` |
| 5 | Admin API + MCP | tuple list/add/remove/check endpoints, 4 MCP tools, admin UI section; e2e |
| 6 | Release | CHANGELOG, api.md rows, 18.14.0 |

Every tranche: tests, docs, CHANGELOG (Unreleased), `gen_llms.py`; default
and `http1`-only test runs before pushing.

## Status

| # | State |
|---|---|
| 1 | **done 2026-10-31**: `authz::Identity` (`subject` `type:id`, a bare id → `user:`; `source`; raw `claims`; `claim()`; `from_claims` reads a top-level string member via `json_rpc::object_members`), `with_identity` / crate-private `enter` guard (thread-local, restored on drop/panic) and `scope_identity` (task-local, http2), `Identity::current()` (task first). Set by `JwtLayer` (`sub`, `.identity_claim()`; a valid token without the claim → no identity), `BasicAuthLayer` (user), `OidcAuth` (`sub`), `SamlSp` (`name_id`), `ForwardAuthLayer` (`.identity_header()`, only on 2xx, control chars refused), MCP (`require_bearer_tokens` / `require_oauth` claims `sub`, a guard over the rest of `execute`). Config-file auth: `jwt` and `basic` get it through those layers; `bearer` (one shared token) has none. Tests: 7 in `authz` (typing, claims, restore on panic, async scope, spoofed headers ignored, JWT/Basic, MCP tool sees `user:alice via mcp`) + forward, OIDC, SAML. Docs: new `features/authorization.md` (Identity section; sidebar), auth page notes, api.md row, CHANGELOG. 2215 default / 2423 auth+sso / 2023 http1 |
| 2 | — |
| 3 | — |
| 4 | — |
| 5 | — |
| 6 | — |
