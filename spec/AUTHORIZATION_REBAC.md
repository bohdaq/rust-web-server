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
| 2 | **done 2026-10-31**: `authz::rebac` — `ObjectRef` / `Subject` / `Tuple` (parse + Display; names `[A-Za-z0-9_.-]`, ids no `#@`/space/control, ≤ 256), `parse_expression` (`|`-separated `user`, `group#member`, `owner`, `parent->viewer`; ≤ 4096 chars), `Schema` (`relation`, `object_type`, `validate`: a bare name = relation if the type has it and it isn't a type, else a subject type; `group#member`, `parent` (direct types only) and `parent->viewer` (on every target type) checked), `Schema::allows` for writes, `TupleStore` (write/delete/read/list + `TupleFilter`), `MemoryStore`, `Authorizer` (`write`/`write_str`/`delete`, `check`, `explain` → path). Walk: explicit stack + visited set + parent links for the path; `MAX_DEPTH` 25 and `MAX_READS` 1000 → `AuthzError::Limit`. Tests: 8 (parsing, expressions, schema errors, writes vs schema, direct/computed/nested groups/parent walk + explain path, cycle ends with no, depth limit, read budget, list filters) + 2 doc tests + stack-depth case (5000-group cycle, 500-term expression on a 256 KB stack) + fuzz `authz_parse` (556k runs, clean). Docs: Relationships section, api.md row, CHANGELOG. 2224 default / 2032 http1 |
| 3 | **done 2026-10-31**: `authz::ObjectTemplate` (`{path:N}` from 1, `{query:k}`, `{header:H}`; must start with a literal `type:`; renders to an `ObjectRef` or `None`) and `AuthzLayer` (`new(authorizer, relation, template)` checks the relation exists; `with(authorizer, pick)`; no caller → 401, no object / no / `Limit` (logged) → 403, JSON error body). Config: `ProxyConfig.authz` (`[authz]` `store`, `tuples`; `[authz.type.<t>]` keys = relations), `[route.middleware.authz] relation / object` (counts in `is_empty`, so such routes leave the async forwarder); `builder::config_authorizer` builds per compile over the process-wide memory store (`authz::config`, kept across reloads; seeds written each time) and publishes it (`authz::config::current`), the layer sits inside auth; an unbuildable `[authz]` or a missing one → `AuthMisconfigured` 503. `rws --check`: schema/seed errors via a throwaway store, unknown keys (relation keys free), route without `[authz]`, bad template, undefined `type#relation`, `store` other than memory, a warning for authz without auth. Also: the proxy parser now joins multi-line arrays (`logical_lines`; quote-aware bracket counting, comments, trailing comma) — the e2e's tuple list needed it; the top-level settings reader still reads one line per key (it's built for wasm32, `proxy_config` isn't). Tests: templates, layer 200/403/401, config route with JWT + a runtime tuple surviving `reload_from_str`, 8 check cases, parser. `ci/e2e/authz.sh` (CI `authz`): JWT tokens from Python, owner/group/folder reads, editors write, 403/401, same after SIGHUP. Docs: Protecting routes + In the config file, config-driven + config-file sections, api.md row, CHANGELOG. 2228 default / 2437 auth+sso / 2036 http1 |
| 4 | **done 2026-10-31**: `authz::SqlStore` (any `model-*` feature): own runtime `rws-authz-sql` (sqlx connections belong to the runtime that opened them; `block_on_isolated` builds a runtime per call — too slow per read and it would strand the pool), queries spawned there and awaited over a std channel. `open(DbConfig)` / `from_env()` make the pool on that runtime; `new(pool)` takes one. Table `rws_authz_tuples` (`tuple_key` = SHA-256 hex of the tuple text as PK — six VARCHAR(256) columns exceed MySQL's 3072-byte key; index on object type/id/relation, prefix lengths on MySQL), insert-or-ignore per database, placeholders `$n` on Postgres; read cache 1 s, cleared by this process's writes, bounded at 10k keys. `[authz] store = "sql"` → process-wide store from `RWS_DB_*`; `rws --check` validates with a throwaway memory store (never connects or writes) and errors on `sql` in a build without a model feature. Tests (SQLite, 4): write/dup/read/list/delete + cache clear, Authorizer check over a file DB reopened (persistence), cache staleness ≤ 1 s, config `store = "sql"` via env. CI: `model-sqlite` added to the `tls + auth + sso + serde` test job (2769 there locally); Postgres/MySQL compiled by the all-features MSRV build. Also removed an unused `h3_upstream::default_tls`. 2228 default / 2036 http1 |
| 5 | **done 2026-10-31**: `admin::ops::register_authorizer` / `authorizer()` (a registered one, else the config file's), `authz_schema_json`, `authz_tuples_json` (object = `type:id` or a type; relation; subject; limit 1–1000), `authz_write` (schema-checked), `authz_delete`, `authz_check` (with the explaining path). Admin API `GET /authz`, `GET`/`POST`/`DELETE /authz/tuples` (delete by `?tuple=` or body), `POST /authz/check`; 404 when nothing is configured, 400 for bad input, 405 with `Allow` (`wrong_method` became a function). MCP tools `authz_check`, `authz_tuples` (read-only), `authz_write`, `authz_delete` (changes) — 24 tools now (docs + CLAUDE.md). Admin UI: Controls → Authorization (schema summary, add field, tuple table with Remove, check form with Allowed/Denied and the path); a 404 hides it. Tests: admin API end to end in-process; `ci/e2e/authz.sh` adds grant → access, explain path, list, survives SIGHUP, delete → 403; `ci/e2e/admin_ui.sh` adds a tuple and checks from the page. 2229 default / 2770 CI features + model-sqlite / 2037 http1 |
| 6 | **review fixes before the tag (2026-10-31)**: (1) a token whose `sub` was `group:eng#member` became that userset and inherited the group's grants — identities from outside are now `Identity::user(id)` (type always `user`, id validated: no `#`/space/control; `Identity::new` keeps `type:id` for code), `AuthzLayer` checks `check_subject(&caller.as_subject(), …)` so an id is never re-parsed; ids may contain `@` (emails) — tuples parse object up to `#`, relation up to the next `@`; (2) path confusion: `/docs/42/../7` rendered `doc:42` while a proxy forwards the raw path — `ObjectTemplate::render` decodes each segment once (`percent_decode_path`; `URL::percent_decode` left `%2e` alone) and names nothing for `.`/`..`/encoded `/` or `\\`; (3) `block_on_isolated` carries the identity like the otel context (async handlers); (4) `rws --check` warns that a global `[middleware.authz]` covers /healthz, /readyz, /metrics. Also found: docs and a test had the wrap order backwards — the first `.wrap` is outermost, so the authenticator is wrapped first. Regression tests for all of it. Bumped to 18.14.0; tag after CI is green |
