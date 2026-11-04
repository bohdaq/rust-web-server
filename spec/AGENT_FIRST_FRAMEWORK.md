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
| Layout | each starter is one `src/main.rs` | conventional layout + app manifest | S |
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
  *Status (2026-11-04):* landed in `rws-proxy` (`dev_mcp`), not `rws-server`
  — the ops tools call `admin::ops`, which needs `cluster`, `traffic` and
  `proxy_config`, all in `rws-proxy`; the facade re-exports it as
  `rust_web_server::dev_mcp`. Done: `routes` (the middleware is the app's
  stack, `Middleware::layer_name()`; rws has no per-route middleware),
  `config`, `recent_requests`/`recent_errors`, `db_schema`, `migrations`,
  `migrations_run`, `migrations_rollback_last`, `RWS_ENV`, the ops tools
  (`ops_tools`, used by the binary). Open: `models` (needs a model
  registry — with #87's manifest), `jobs`, MCP resources, a starter
  answering every tool in CI.
- **#86 Compile-checked routes** — `#[get]`/`#[post]`/… generate the adapter;
  extractors as arguments; path parameters checked at compile time; duplicate
  and shadowed routes reported when the app is built (panic in debug, startup
  error in release).
- **#87 Conventional layout + `rws.app.toml` manifest** — `src/routes/`,
  `src/models/`, `src/jobs/`, `migrations/`, `templates/`, `tests/`; starters
  adopt it; `dev_mcp` and `rws gen` read it.

Done when: a starter app answers every `dev_mcp` tool in a CI test; a wrong
`{id}` and a duplicate route fail a compile-fail (trybuild) test; the starters
use the layout and `ci/e2e/starters.sh` passes; SKILL.md and llms files
describe the new API.

### 18.29.0 — Generators and test loop

- **#88 `rws gen model|route|migration|test`** — CLI and `dev_mcp` tools,
  output pinned to the release; idempotent (refuses to overwrite, prints a
  diff).
- **#89 Test ergonomics** — `TestDb` (a transaction per test, rolled back),
  factories from `#[derive(Model)]`, WebSocket/SSE in `TestClient`, an OpenAPI
  snapshot assertion; machine-readable test failures for agents.

Done when: every generator's output compiles and passes its generated test
in CI, for each starter.

### 18.30.0 — Data

- **#90 ORM** — generic/UUID primary keys, `created_at`/`updated_at`, eager
  `with()`, many-to-many, scopes, soft delete, optimistic locking, seeds, read
  replicas, schema dump as JSON.
- **#91 `rws db migrate|rollback|status|seed`** — the same operations as the
  `dev_mcp` tools.

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
