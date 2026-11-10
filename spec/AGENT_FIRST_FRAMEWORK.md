# Agent-first framework (18.28–18.31)

Goal: make rws a full application framework in the spirit of Rails and Spring
Boot, **built agent-first** — for coding agents that write rws apps, and for
apps that agents inspect and operate. Written 2026-11-04 from a review of the
tree at 18.27.2 against Rails and Spring (advisor review, spot-checked).
Roadmap items: #85–#95 in [ROADMAP.md](ROADMAP.md).

## Principles

Agent-first is the primary lens; the project's ethos stays.

1. **The running app explains itself.** Routes, middleware, models, schema,
   migrations, config, jobs and recent errors are readable as MCP tools and
   resources (and JSON), in the library — not only in the `rws` binary.
2. **Mistakes fail at compile time, or at build time of the app.** Agents get
   the API wrong most often (see [WEBSITE.md](WEBSITE.md)); a typed signature or a
   macro error is feedback they act on, a runtime 404 is not.
3. **One way, in one place.** A conventional layout an agent can rely on
   without reading the project first.
4. **Actionable errors.** Every error says what was wrong and what to do
   (problem+json by default, startup reports for apps like `ConfigReport`
   for the config file).
5. **Deterministic over prose.** Where an agent would otherwise copy an
   example, a version-pinned generator emits the code against the exact API.
6. **No runtime magic, no hot-path cost.** Macros generate adapters to the
   existing handler signature; nothing new on the request path unless used.

## What exists (18.27.2)

Agent side: `agents/skills/rws/SKILL.md` + generated Cursor/Copilot files
(`ci/gen_agents.py`), per-release `llms.txt`/`llms-full.txt`
(`ci/gen_llms.py`), starters with `AGENTS.md` (`starters/`, CI-built), the docs
MCP server (`site/mcp`), 25 ops MCP tools — **binary only** (`src/main.rs`),
`McpServer::openapi_tools` + `AppWithState::openapi_spec` (routes → MCP tools,
`crates/rws-server/src/mcp/openapi_tools.rs`), `typed_tool` +
`#[derive(JsonSchema)]`, A2A, `rws ctl --json`, `rws --check` JSON.

Framework side: `AppWithState` / `AsyncAppWithState` / `Router`
(`crates/rws-server/src/state/`, `router/`), extractors
(`crates/rws-http/src/extract/`), `#[derive(Validate)]`, Tera templates
(`src/template/`), `#[derive(Model)]` + repository + query builder + explicit
relations + SQL migrations (`crates/rws-server/src/model/`), `JobQueue` /
`PersistentJobQueue` (`src/jobs/`), `scheduler`, mailer (`src/mailer/`),
sessions, CSRF, Argon2id (`crypto`), JWT/OIDC/SAML/`AuthServer`, ReBAC
(`authz/` — ahead of Rails), storage backends, i18n, `di::Container`,
`#[derive(Config)]`, `TestClient`.

## Gaps

| Area | Today | Missing | Size |
|---|---|---|---|
| Introspection for library apps | ops tools in `src/main.rs` only; `route_entries()`, OpenAPI | dev MCP server in `rws-server`: routes + middleware, models, DB schema, migration status/run, config, jobs, recent errors | M |
| Handlers | one signature `fn(&Request, &PathParams, &ConnectionInfo, &S)`; `#[get]`/`#[route]` only add a doc comment (`rws-macros/src/lib.rs` `annotate`) | extractors as fn args (`Path<T>`, `Query<T>`, `Form<T>`, `Json<T>`, `State<S>`, `Identity`); `#[get("/posts/{id}")]` that generates the adapter and checks `{id}` against the arguments; duplicate/shadowed route detection | M |
| Routing | verbs, `{param}`, `with_host` | `resource("/posts", …)`, `nest`/`group` with shared middleware, route guards (`before_action`), typed URL builders | M |
| Layout | each starter is one `src/main.rs` (done on main: #87) | conventional layout + app manifest | S |
| Generators | none (18.7.0: "no `rws new` / `cargo-rws`") | `rws gen model|route|migration|test`, also as MCP tools | M |
| Forms / views | raw form maps; Tera | `Form<T>` binding, flash messages (none exist), view helpers (CSRF tag, form, URL), Vite manifest helper | S–M |
| ORM | i64 keys only (`Repository<T, i64>`), explicit `.load()` | generic/UUID keys, timestamps, eager `with()`, many-to-many, scopes, soft delete, optimistic locking, seeds, read replicas, `rws db migrate|rollback|status|seed`, schema dump as JSON | L |
| Jobs / mail / events | persistent queue with retries; in-memory cron; SMTP | `run_at` enqueue, named queues/priority, unique jobs, dead-letter, cron on the persistent queue with a lock across replicas; templated mail, previews, `deliver_later`, attachments; typed `EventBus` | M |
| Realtime | WebSocket, SSE | channels / pub-sub over them | M |
| Attachments | `Storage` trait (local/S3/Azure/GCS) | model ↔ blob binding, signed URLs | M |
| Auth flows | Argon2id, sessions, JWT/OIDC/SAML | sign-up, login, password reset, email confirmation (JSON and Tera variants) | M |
| Profiles, cache, DI | env < toml < CLI; response cache; `Container` | `RWS_ENV` (dev/test/prod defaults), `fetch_or` cache store, DI lifecycle hooks | S each |
| Errors | `AppError`; `error/problem.rs` not the default | problem+json by default in apps; dev error page with backtrace; app build report ("route X shadows Y") | S |
| Testing | `TestClient` (HTTP only), in-memory SQLite | `TestDb` (transaction per test), factories, WebSocket/SSE in `TestClient`, OpenAPI snapshot assertion | M |

Fine as they are: authorization, i18n, transactions (`pool.transaction`),
health and metrics.

## Decisions

- **The no-scaffolder decision (18.7.0, #42) is revisited in part.** Still no
  `rws new` — starters + cargo-generate cover new projects. But generators for
  code *inside* a project (model, route, migration, test) are added, as CLI and
  MCP tools: their output is pinned to the release's exact API, which is more
  reliable for agents than prose context, and they enforce the layout (#87).
  Only those four; no generators for every artifact.
- **The dev MCP server is the console.** Rust has no `rails console`; the
  agent gets the same reach through tools, gated to `RWS_ENV=dev` or a token.
- **Macros compile down to today's signature**: no boxing or extraction cost
  added to handlers that don't use them.

## Not copied from Rails / Spring

Runtime auto-wiring DI and classpath auto-configuration; AOP proxies for
`@Transactional` / `@Cacheable` (closures do it); implicit lazy loading and
callback-heavy ActiveRecord (keep explicit `.load()`, add eager `with()`); YAML
profile overlays (one `RWS_ENV` over the existing layering); a REPL; Hotwire /
Turbo coupling (a Vite manifest helper only); a full ActionCable server with
Redis adapters (thin channels over WebSocket/SSE); generators for every
artifact; GraphQL (stays in [IDEAS.md](IDEAS.md)); monkey-patched core
extensions.

## Releases

### 18.28.0 — Agent-first foundations

- **#85 `app.dev_mcp()`** — move the binary's ops tools into `rws-server` (the
  binary keeps using them) and add `routes` (with middleware per route),
  `models`, `db_schema`, `migrations` (status, run, rollback), `config`,
  `jobs`, `recent_errors`; also as MCP resources. Off unless `RWS_ENV=dev` or a
  bearer token is set. Covers the IDEAS item "MCP tools generated from routes,
  ORM models or jobs".
  *Shipped on main (2026-11-04):* in `rws-proxy` (`dev_mcp`), not `rws-server`
  — the ops tools call `admin::ops`, which needs `cluster`, `traffic` and
  `proxy_config`, all in `rws-proxy`; the facade re-exports it as
  `rust_web_server::dev_mcp`. Done: `routes` (the middleware is the app's
  stack, `Middleware::layer_name()`; rws has no per-route middleware),
  `config`, `recent_requests`/`recent_errors`, `db_schema`, `migrations`,
  `migrations_run`, `migrations_rollback_last`, `RWS_ENV`, the ops tools
  (`ops_tools`, used by the binary); with #87 `layout` and `models` — an
  explicit registry, `DevTools::model::<T>()` (`model::ModelInfo`), since
  Rust can't enumerate `#[derive(Model)]` types, compared with the
  database's tables when one is configured — and every starter's
  `tests/dev_mcp.rs` answering every listed tool, run by
  `ci/e2e/starters.sh`. Open (moved to later releases): `jobs`, the same
  data as MCP resources.
- **#86 Compile-checked routes** — `#[get]`/`#[post]`/… generate the adapter;
  extractors as arguments; path parameters checked at compile time; duplicate
  and shadowed routes reported when the app is built. *Shipped on main
  (unreleased):*
  the attribute turns a typed function into a unit struct implementing
  `handler::Handler`/`AsyncHandler`, registered with `.route(f)` (actix's
  shape, so the pattern served is the one checked); conflicts are a one-time
  stderr warning plus `route_conflicts()`, not a panic or startup error —
  18.x is additive and a panic would break apps that build today. The
  compile-fail tests (trybuild, `rws-macros/tests/ui/`) cover wrong and
  missing path parameters and arity; a duplicate route is a runtime
  report, since routes are registered at run time.
- **#87 Conventional layout + `rws.app.toml` manifest** — `src/routes/`,
  `src/models/`, `src/jobs/`, `migrations/`, `templates/`, `tests/`; starters
  adopt it; `dev_mcp` and `rws gen` read it. *Shipped on main (unreleased):*
  the layout adds `src/lib.rs` (`app()`) beside a wiring-only `src/main.rs`
  — `tests/` can't reach a binary-only crate — and `static/`. The manifest
  (`[app] name`, `[paths] main|lib|routes|models|jobs|migrations|templates|tests|static`,
  every key optional, absent file = conventional) is parsed by
  `app_manifest::AppManifest` in `rws-http` (the lowest crate with the
  TOML string helpers): line-based, 64 KiB cap, paths kept inside the
  project, unknown keys as warnings; fuzz target and stack-depth case. It
  describes the project — nothing is moved or routed by it. `dev_mcp`'s
  `layout` reads it on every call; `rws gen` (#88) will. The starters
  split into `main.rs` + `lib.rs` + `routes/` (`tools/` for the MCP
  starter, said in its manifest) + `models/` + `tests/`, use `#[get]` +
  `.route(f)`, mount `dev_mcp` under `RWS_ENV=dev`, and ask for 18.28.

Done when: a starter app answers every `dev_mcp` tool in a CI test; a wrong
`{id}` and a duplicate route fail a compile-fail (trybuild) test; the starters
use the layout and `ci/e2e/starters.sh` passes; SKILL.md and llms files
describe the new API.

### 18.29.0 — Generators and test loop

- **#88 `rws gen model|route|migration|test`** — CLI and `dev_mcp` tools,
  output pinned to the release; idempotent (refuses to overwrite, prints a
  diff). *Shipped on main (unreleased):* `generator` in `rws-proxy` (beside
  `dev_mcp`, which mounts it; re-exported as `rust_web_server::generator`):
  `plan(dir, &Generate)` reads the project (paths from `rws.app.toml`) and
  answers a `Plan` of file changes with unified diffs, `Plan::apply` writes
  it; `rws gen` (exit 0/1/2, `--dry-run`, `--json`, `--dir`) and the
  `gen_model`/`gen_route`/`gen_migration`/`gen_test` tools (`GEN_TOOLS`,
  listed unless read-only, `dry_run`) are front ends. Files it creates are
  never replaced (identical = unchanged, else a conflict and nothing of the
  plan is written); existing files only get a line (`pub mod …;`, the
  `model-<db>` feature, the new `register` call wrapped around `app()`'s
  existing one), or a note says what to add by hand. Decisions taken: the
  generated routes are stateless typed handlers over a module-level
  in-memory store (they compile against any `AppWithState<S>` /
  `AsyncAppWithState<S>`; the app type and `S` are read from `app()`'s
  signature), not wired to the model — the starters' stores differ too much
  to generate against; an app without such an `app()` (the `mcp` starter's
  `McpServer`) is refused for `gen route`/`gen test`; the model's own test
  round-trips through SQLite in memory when the backend is SQLite; the empty
  migration is `SELECT 1;` (the migrator splits at `;` and runs each part).
  `ci/e2e/starters.sh` runs the generators in every starter (twice, then a
  conflicting model) and `cargo test`. Found on the way: `tools/call` took
  the tool name from `arguments.name` when a client sent `arguments` first
  (fixed in `McpServer::call_tool`).
- **#89 Test ergonomics** — `TestDb` (a transaction per test, rolled back),
  factories from `#[derive(Model)]`, WebSocket/SSE in `TestClient`, an OpenAPI
  snapshot assertion; machine-readable test failures for agents.

Done when: every generator's output compiles and passes its generated test
in CI, for each starter.

### 18.30.0 — Data

- **#90 ORM** — generic/UUID primary keys, `created_at`/`updated_at`, eager
  `with()`, many-to-many, scopes, soft delete, optimistic locking, seeds, read
  replicas, schema dump as JSON.
  *Part 1 on main (unreleased):* `Repository` for `i64`/`i32`/`String`/
  `model::Uuid` keys (own 16-byte type: `uuid` isn't in the tree; v7 for
  generated keys) — a non-auto key `save`s as an insert when it isn't in the
  table, else an update; `#[timestamps]` / `#[soft_delete]` / `#[version]`
  on `#[derive(Model)]`, written by the repository in the statements it
  already runs (no callbacks; `Model::created_at_column()` …
  `version_column()` default to `None`, so hand-written impls compile);
  `model::Timestamp` (UTC µs; TEXT / TIMESTAMPTZ via `CAST` / DATETIME(6));
  PostgreSQL and MySQL temporal and UUID columns decoded from wire bytes
  rather than sqlx's chrono/uuid features; `DbErrorKind::StaleVersion` by a
  message prefix, since `DbError(pub String)` stays until 19.0. The model
  tests run per backend (`model/backends.rs`: a schema or database per
  test), which found four PostgreSQL/MySQL bugs (see CHANGELOG). Part 2:
  eager `with()`, many-to-many, scopes, Rust-factory seeds, read replicas,
  schema dump.
- **#91 `rws db migrate|rollback|status|seed`** — the same operations as the
  `dev_mcp` tools. *On main (unreleased):* `db_cli` in `rws-proxy` (beside
  `generator`), on `dev_mcp::db`'s runtime and status JSON; seeds are
  `seeds/` beside the migrations directory, plain SQL, run every time.

Done when: SQLite, PostgreSQL and MySQL pass the new model tests in CI.

### 18.31.0 — App building blocks

- **#92 Auth flows** — sign-up, login, logout, password reset, email
  confirmation on sessions + Argon2id + mailer; JSON and Tera variants.
- **#93 Jobs, mail and events** — `run_at`, queues/priority, unique jobs,
  dead-letter, persistent cron with a cross-replica lock; `Mailer::render`
  (Tera), previews, `deliver_later`, attachments; a typed in-process
  `EventBus` (also feeds MCP `notify`).
- **#94 Web layer** — `resource()` routing, `nest`/`group`, route guards,
  `Form<T>`, flash messages, view helpers, Vite manifest helper, channels over
  WebSocket/SSE, attachments on models.
- **#95 Defaults** — `RWS_ENV` profiles, problem+json by default, dev error
  page, `fetch_or` cache, DI lifecycle hooks.

Done when: `starters/fullstack` uses the auth flows, a job, a mail and the
web layer, and passes in CI.

*Status (2026-11-10, branch `framework-18-31`, to ship as 18.35.0):* all
four done — #94 `group`/`resource`/named routes/`flash`/view helpers/`vite`/
`channels`/`attachment`; #93 `EnqueueOptions`, dead-letter, cross-replica
`schedule`, `Mail`/`Deliver`/`TestMailbox`/`deliver_later`/`MailPreviews`,
`EventBus`; #92 `auth_flows::AuthFlows`; #95 `RwsEnv` profile helpers,
`ErrorPages`, `ValueCache`, `di::Lifecycle` (problem+json was already the
default). `starters/fullstack` has accounts, a welcome mail sent by a job,
`FlashLayer`, `ErrorPages` and the model admin. Beyond the plan: nested
eager loading, `rws gen migration --from-models` (#108), `ModelAdmin` (#109).
Layer order matters: `FlashLayer`/`CsrfLayer`/`ErrorPages` wrap before
(outside) `AuthFlows` and `ModelAdmin`, which answer their own pages.
