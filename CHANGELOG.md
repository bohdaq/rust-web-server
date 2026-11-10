# Changelog

Notable changes per release. Versions follow [semver](https://semver.org/):
a major version means a breaking change — see
[Migrating to 18.0](https://rws8.tech/reference/migrating-18/) for the next one.
Releases and binaries: [GitHub releases](https://github.com/bohdaq/rust-web-server/releases).

## Unreleased

### Added

- **Traffic: browsers from hosting networks are bots.** A browser
  `User-Agent` from AWS, Google Cloud, Azure, OVHcloud, Hetzner,
  DigitalOcean or another cloud/server host (a link scanner, preview or
  headless crawler — they load assets and even run the script) now counts
  as a bot of kind `hosting`, named after the provider, not as a person.
  The networks are a ~60 KB table built from DB-IP's ASN Lite
  (`ci/gen_hosting.py`); consumer VPN and CDN egress is left out.
  `[traffic] hosting_networks = false` turns it off.
- **`starters/operator`**: a Kubernetes operator — a `Greeting` custom
  resource, a controller (owned ConfigMap, status, finalizer, leader
  election), validating and mutating webhooks, `--manifests`, a Dockerfile;
  `ci/e2e/operator.sh` deploys it into kind (CI job `kubernetes`) and checks
  the API server calls its webhooks over TLS.
- **`WebhookCertificate::shared`** (one certificate for all replicas, in a
  Secret), `to_secret` / `from_secret`; **`k8s::client::offline()`**.

- **Health checks** (`health`): `health::readiness` / `liveness` /
  `startup` (and `register_async`, `database(name, pool)`) register checks
  that `/readyz`, `/livez` (`/healthz`) and the new `/startupz` run —
  concurrently, with a timeout, off the event loop; `?verbose` lists each
  check. Without checks `/healthz` and `/readyz` answer as before. The Helm
  chart has an optional `startupProbe`.
- **Kubernetes framework** (`k8s` feature, new crate `rws-k8s`,
  `spec/K8S_FRAMEWORK.md`), on kube-rs:
  - `k8s::client`, `ConfigWatch` (ConfigMaps/Secrets kept current,
    `apply_to_env`), `k8s://ns/name#key` secrets, service discovery from
    EndpointSlices into a `BackendPool`, and a public `LeaderElector`.
  - Operators: `Controller` (reconcile, owns, finalizers, error backoff),
    `Operator` (several controllers, leader election, readiness check,
    `rws_controller_*` metrics), `Context` (`patch_status`, `apply_owned`,
    `event`).
  - Webhooks: validating, mutating (JSON patch computed) and conversion, as
    an rws app; `WebhookCertificate` (CA + serving cert, `patch_ca_bundle`).
  - Manifests from the code: CRDs, RBAC (rules merged; `Operator::rules`),
    webhook configurations, `--manifests`.
  - `testing::TestCluster`: tests against a real API server
    (`RWS_TEST_KUBECONFIG`), a namespace per test; CI job `kubernetes`
    runs them on kind.
- **`rws gen deploy --image <image>`**: `deploy/k8s/` with a Deployment
  wired to the probes and graceful shutdown, Service, PDB, HPA,
  ServiceAccount and kustomization.
- **Graceful drain**: after SIGTERM's delay the async servers stop
  accepting, then wait up to `shutdown_timeout_secs`
  (`RWS_CONFIG_SHUTDOWN_TIMEOUT_SECS`, default 25) for handlers still
  running; before, those were cut off when the process exited.

### Added

- **Web layer** (roadmap #94, `spec/AGENT_FIRST_FRAMEWORK.md`):
  - **Route groups and guards**: `app.group("/admin", |g| g.guard(signed_in).wrap(layer).get(…))`
    on `AppWithState` and `AsyncAppWithState` (async groups: guards, no
    per-group middleware); groups nest, guards run outer first.
  - **Resources**: `app.resource("/posts", |r| r.index(…).show(…)…)` — the
    seven conventional routes (`new` before `{id}`, `update` on PUT and
    PATCH), each named `posts.<action>`; `extra` for more.
  - **Named routes**: `.name("post")` after a route, `router::url_for(name, params)`.
  - **Flash messages** (`flash`): `Redirect::to(path).notice(…)` (303),
    `Flash` (also a handler argument), `FlashLayer` clears it once shown.
  - **View helpers**: the `url_for`, `vite` and `vite_asset` template
    functions and `template::page_context(request)` (flash, `csrf_field`,
    path). The `tera` feature now turns on `serde`.
  - **Vite manifest helper** (`vite`, `serde`): tags for an entry from
    `.vite/manifest.json` or the dev server (`VITE_DEV_SERVER`).
  - **Channels** (`channels::Hub`): named topics pushed to SSE and
    WebSocket clients.
  - **Attachments on models** (`attachment`, any `storage-*` feature): a
    file in a `Storage` backend as one `TEXT` column, from a multipart
    upload with `Attachment::from_multipart`.
- **Sign-up and login** (`auth_flows`, roadmap #92; `crypto` + `mailer` +
  `serde`): `AuthFlows`, a middleware with sign-up, login, logout, password
  reset and email confirmation as JSON or HTML forms (built-in pages or
  your Tera templates); signed stateless sessions (a password change ends
  them) and single-use reset links; rate limits per IP; `CurrentUser`,
  `signed_in` and `signed_in_or_redirect`; `UserStore` with
  `MemoryUserStore` and `DbUserStore` (`rws_users`).
- **Jobs, mail and events** (roadmap #93):
  - **`PersistentJobQueue`**: `enqueue_with(type, payload, EnqueueOptions)` —
    `run_at`/`run_in`, named `queue`s (`queues(&[…])` limits a process),
    `priority`, `unique(key)`; the dead-letter list (`failed_jobs`, `retry`,
    `delete`, `on_dead`, `jobs`); `schedule(name, cron, type, payload)`
    enqueues on a cron schedule once per due time across replicas
    (`rws_job_schedules`). `CronSchedule::next_after`.
  - **Mail**: `Mail` (an `Email` with `MailAttachment`s, sent as
    `multipart/mixed` by `Mailer::send_mail`); the `Deliver` trait and
    `TestMailbox`; `deliver_later` on a `JobQueue` and `Mail::enqueue` +
    `register_delivery` on a `PersistentJobQueue`; `EmailBuilder::render`
    from `<name>.txt`/`.html` templates (`tera`); `MailPreviews` at `/_mail`.
  - **`events::EventBus`**: typed in-process events, forwarded to MCP
    notifications or channel topics.

- **Defaults** (roadmap #95):
  - **`RWS_ENV` profiles**: `RwsEnv::shows_error_details()` (dev, test) and
    `reloads_templates()` (dev); `template::render` re-reads templates
    before each render in dev.
  - **`error_pages::ErrorPages`**: panics and app errors as a page with the
    message, location, request and backtrace (or problem details with
    `location`/`backtrace`) in dev and test, a bare `500` in prod.
  - **`cache::ValueCache`**: `fetch_or` / `try_fetch_or` / `fetch_or_async`
    with a TTL per entry.
  - **Container lifecycle**: the `Lifecycle` trait, `register_managed`,
    `on_start` / `on_stop`, `start` / `stop`.

- **ORM**: nested eager loading — `with_nested(relation, |n| n.with(…))`
  on `QueryBuilder`/`EagerQuery`, read with `Loaded::related_loaded` (one
  query per relation per level); `rws gen migration <name> --from-models`
  (`generator::plan_model_migration`) writes the additive migration from
  the `#[derive(Model)]` structs to the database (`CREATE TABLE`, `ADD
  COLUMN`; anything else is a note).
- **Model admin** (`model_admin::ModelAdmin`, roadmap #109; a `model-*`
  feature): pages to list, search, show, create, edit and delete rows of
  registered tables, read from the live schema; denied unless `allow` says
  otherwise.
- **`AsyncAppWithState::wrap`**, as `AppWithState::wrap`.
- **`AuthFlows::on_signup`**, and `starters/fullstack` now has accounts
  (`/auth`, a guarded `/api/account`), a welcome email sent by a job,
  `FlashLayer`, `ErrorPages` and mail previews in dev.

### Fixed

- **Traffic: the site picker.** The first 16 `Host` names seen took every
  slot, so raw IPs, `(none)` and mail names crowded out real sites, which
  then counted as `other`. Every site a `[[route]]` matches by `host` now
  gets its own counts (read at start and on reload) and is listed first,
  under *Sites*, then other names seen.
- **Traffic: picking a site shows only that site.** Visitors, AI agents,
  bots and referring sites were all sites' together; pages matched by
  prefix (`example.com` also took `example.com.evil`) and included the
  shared `(other)` row. Days counted before the upgrade keep their
  all-site lists, shown under *All sites*.
- **`/readyz` of library apps answered `503` for good**: only the `rws`
  binary marked the server ready. `Server::run`, `run_plain` and `run_tls`
  now do as they start serving.
- **`k8s::manifests::to_yaml` output crashed `kubectl apply`**: a stream
  starting with `{` is read as JSON, and kubectl panics when a document over
  2 KiB (a CRD) is followed by `---`. Each document now starts with `---`.
- The Gateway API controller's leader election is `rws_k8s`'s
  `LeaderElector` (the `leader` feature of rws-k8s); its behaviour and API
  are unchanged.

- **`PersistentJobQueue` on PostgreSQL and MySQL**: its SQL used `?`
  placeholders (PostgreSQL wants `$1`…) and a `TEXT` primary key (MySQL
  refuses one), so it only worked on SQLite. CI now runs it on all three.

### Changed

- **Admin Traffic page: world map** is now `@lit-material/world-map`: one
  tab stop, the arrow keys read countries highest first and Enter picks one
  (again clears the filter), a legend for the shading, countries too small
  to draw (Singapore, Malta…) listed under the map, and a screen-reader
  table. The shapes it ships are 12 kB gzipped instead of 27 kB;
  `ci/gen_world_map.py` is gone.

## 18.34.1 — 2026-11-06

Docs and website only; no code changes.

### Changed

- **rws vs nginx vs Caddy**: a new comparison page in the reference section,
  with the nginx and Caddy facts checked against their docs (sources on the
  page).
- **Answer-first openings** on the home page, Quick start, Automatic HTTPS
  and the proxy overview, plus new home page FAQ entries.

## 18.34.0 — 2026-11-06

### Added

- **`.md` is served as `text/markdown`** (and compressed): Markdown copies of
  pages for AI agents no longer download as `application/octet-stream`.
- **rws8.tech for search engines and AI answers**: `robots.txt` (AI crawlers
  explicitly welcome) and `sitemap.xml` (with each docs page's last change
  from git) are generated; docs pages carry `TechArticle` and
  `BreadcrumbList` structured data, a visible "Last updated" date and a
  Markdown copy (`/docs/<page>.md`, also linked from the page); the home page
  adds `Organization` and `WebSite`; every page has `og:type`, `og:url` and a
  Twitter card; `/favicon.ico` exists. `llms.txt` links the Markdown copies
  under `/docs/` (it linked old paths that redirected).

### Fixed

- **The home page (`/` → `./index.html`) gets the static `Cache-Control`**
  (`static_cache_control`, default `no-cache`) like every other static file,
  not the dynamic `no-store`, which kept browsers from caching it and out of
  their back/forward cache.
- **A config `static` route answers a miss with its root's `404.html`**
  (status 404) when there is one, as the default static server does; it
  sent an empty body.

## 18.33.0 — 2026-11-05

### Added

- **Traffic page: Humans / AI agents / Bots / All**: a switch in the top
  bar. AI agents and Bots get their own numbers (with the change from the
  period before), chart and lists — agents with what they're for and the
  pages they read; bots by kind and name, and 404s — and All charts the
  three groups together with their share of requests. Replaces the AI &
  bots tab.
- **Traffic page: world map** of people's countries (Natural Earth 1:110m,
  public domain, generated by `ci/gen_world_map.py` into the page; no
  outside requests); clicking a country filters by it.

### Fixed

- Traffic page: a long page name no longer pushes a card's numbers into the
  next card; the site picker matches the other controls.

## 18.32.0 — 2026-11-05

### Changed

- **The admin UI's Traffic page, redesigned**: a sticky bar (Today / 7 days
  / 30 days / a custom range, site, compare with the period before, people
  online), key numbers that choose what the chart shows, cards of top
  pages, sources, countries and devices, and tabs for Pages, Sources,
  Audience, Behaviour and AI & bots instead of one long page. Click a row to
  filter by it (any dimension with the traffic database). Page names drop
  the site when there's one.
- `GET /admin/api/v1/traffic` takes `from`/`to` days, and its answer gains
  `pages` per bucket, `previous_series` and `days`. Views over two days now
  cover whole UTC days (a "week" is today and the six days before), so
  per-day counts line up with the period; the site list leaves out hosts
  with no counted traffic.
- **rws8.tech** loads the first-party traffic script on every page.

## 18.31.0 — 2026-11-05

### Added

- **Bundled traffic tables refreshed**: AI agents and crawlers
  (`ci/gen_bot_lists.py --update`) and DB-IP Lite countries 2026-11.

- **Traffic page: sources and audience** (`spec/TRAFFIC_ANALYTICS.md`,
  tranche 1): how people arrived (direct, search, social, AI assistants,
  other sites, UTM campaigns with `utm_source`/`utm_medium`/`utm_campaign`
  tables) and who they are (country, browser, operating system, device
  class), each person counted once a day — in the admin UI, `GET
  /admin/api/v1/traffic` (`channels`, `countries`, `browsers`, `os`,
  `devices`, `utm_*`, `geo_source`) and the saved file. Countries come from
  a DB-IP Lite table built in (new default feature `geo`, ~700 KB;
  `ci/gen_geo.py`; `[traffic] geo_db` for a newer one); the lookup stays on
  the server and no address is kept. Chromium client hints are preferred
  to the `User-Agent`. No cookies or browser storage. Same per-request cost
  as before (A/B ~165 ns). Fuzz target `traffic_geo`.
- **Traffic page: sessions** (tranche 2): sessions (page views ≤ 30
  minutes apart), bounce rate, session length, pages per session — with
  the change from the previous period — and entry and exit pages; `GET
  /traffic` gains `sessions`, `previous_sessions`, `entry_pages`,
  `exit_pages`. Sources now count once per session, from its first page
  view. Older saved files load unchanged.
- **Traffic page: right now** (tranche 3): people on the site in the last
  5 minutes, page views per minute for the last 30, the pages being read
  and where the readers are — `GET /admin/api/v1/traffic/live`, MCP
  `traffic_summary {"live": true}`, a "Right now" tile on the Traffic page
  (which now refreshes every 10 s). Kept in memory only (≤ 4096 recent page
  views per shard).
- **Traffic page: the optional first-party script** (tranche 4): `[traffic]
  script = true` makes rws serve `/_rws/t.js` (~1.6 KB, no cookie or
  browser storage) and take its reports at `/_rws/t` (same origin, ≤ 2 KB,
  only for visitors counted as people, ≤ 500 per session). New Behaviour
  section: time on page, scroll depth, screen sizes, single-page-app route
  changes (counted as page views), `rws.track(name, props)` events, and
  **goals** (`[traffic] goals`: paths, `prefix*`, `event:<name>`) with
  conversions per session. `TrafficScriptLayer` is wrapped into the
  binary's static and proxy apps and passes other requests through (proxy
  routes keep the async forwarder). `/_rws/` is reserved.
- **Traffic database (PostgreSQL, opt-in)** (tranche 5): `[traffic]
  database_url_env` names the variable holding a `postgres://` URL; rws
  creates `rws_traffic_pageviews` and `rws_traffic_reports` and writes every
  person's page view and script report there in batches (no IP or
  User-Agent; bounded queue, dropped rather than blocking). `GET
  /admin/api/v1/traffic/query` (and MCP `traffic_summary` with `from`/`to`)
  answers any date range and filter combination; the Traffic page gets an
  Explore section. **Release binaries now include `model-postgres`** (sqlx),
  except the FIPS build.

- **ORM, part 2** (roadmap #90):
  - Eager loading: `Post::query(&pool).with(Post::comments()).with(Post::author()).fetch_all()`
    loads each relation for the whole result set in one more query
    (`WHERE fk IN (…)`, 500 keys per query), not one per row. A
    `model::Relation<O, R>` describes it — `has_many(fk)`, `has_one(fk)`,
    `belongs_to(fk)`, `belongs_to_many(join, owner_key, related_key)` — and
    loads one owner's (`load`) or many owners' (`load_for`) rows too.
    `with()` returns an `EagerQuery` whose `fetch_all` / `fetch_one` /
    `paginate` answer `Loaded<T>` (derefs to the model; `related(&rel)`,
    `one(&rel)`, `take`, `is_loaded`). Soft-deleted related rows are left
    out. See [Eager loading](https://rws8.tech/database/relations/#eager-loading-with-with).
  - Many-to-many: `BelongsToMany<R>` — `load`, `ids`, `attach`, `detach`,
    `detach_all`, `sync` (answers `SyncChanges`), through a join table.
  - Scopes: `QueryBuilder::scope(f)` applies a reusable query function;
    `where_in`, `where_null`, `where_not_null`. See
    [Scopes](https://rws8.tech/database/query-builder/#scopes).
  - Seeds in Rust: `model::Seeders` (`add(name, |pool| async move { … })`,
    or the `Seeder` trait) run after `seeds/*.sql` by
    `DbPool::seed_with(dir, &seeders)`; factories make the rows.
    `db_cli::run_with_seeders` gives an app's own binary `db seed` with
    them. See [Seeds in Rust](https://rws8.tech/database/migrations/#seeds-in-rust).
  - Read replicas: `DbPool::with_replicas(primary, replicas)` (or
    `RWS_DB_REPLICA_URLS` with `DbPool::from_env`) sends `query*` — so the
    query builder's and repository's reads and relation loads — to the
    replicas in turn, and writes, transactions, migrations, seeds, `save`
    and `schema` to the primary; `primary()`, `replica()`, `replicas()`,
    `close()`. `DbPool` has a new variant, `Replicated`: code matching on
    its variants needs an arm for it (`close()` replaces the usual match
    to close a pool). See [Read replicas](https://rws8.tech/database/overview/#read-replicas).
  - The schema as data: `DbPool::schema()` → `DbSchema` (tables, columns
    with type / nullability / default, primary key, indexes, foreign keys)
    on SQLite, PostgreSQL and MySQL, `to_json()`; `rws db schema [--json]`,
    and the dev MCP server's `db_schema` now lists keys, indexes and
    foreign keys too. See [The schema as data](https://rws8.tech/database/migrations/#the-schema-as-data).
  - `rws gen model … --timestamps --soft-delete` (and the `gen_model`
    tool's `timestamps` / `soft_delete`): `#[timestamps]` with
    `created_at` / `updated_at`, `#[soft_delete]` with `deleted_at`, the
    columns in the migration; `Generate::Model` has the two new fields.

## 18.30.0 — 2026-11-04

### Added

- **SPA fallback per path prefix**: `spa_fallback_routes = ["/cloud=cloud/index.html"]`
  (`RWS_CONFIG_SPA_FALLBACK_ROUTES`) gives each single-page app on a server its own fallback file,
  so a site can host apps under `/cloud` or `/admin` while its other misses stay `404`. Longest
  prefix wins; `spa_fallback` still covers paths under no prefix; `rws --check` reports missing
  files and malformed entries. See
  [Several apps on one server](https://rws8.tech/features/static-files/#several-apps-on-one-server).
- **ORM, part 1** (roadmap #90; rws-macros 17.31.0): see
  [Keys, timestamps, soft delete and locking](https://rws8.tech/database/conventions/).
  - Keys beyond `i64`: `ModelRepository<T, ID>` implements `Repository` for
    `i32`, `String` and `model::Uuid` too (`PrimaryKey`). For a key that
    isn't auto-increment, `save` inserts when the key isn't in the table
    and updates otherwise; a `Uuid::NIL` key gets a fresh `Uuid::now_v7()`;
    `repo.insert(&entity)` always inserts. `T::repository(&pool)` is keyed
    by the key's type for `String`/`Uuid` keys (still `i64` for integers).
    `Uuid` is rws's own (v4, v7, parse, `serde`), stored as text.
  - `#[timestamps]`: `save` writes `created_at` (insert only) and
    `updated_at` (`model::Timestamp`, UTC, microseconds) — `TEXT` on SQLite,
    `TIMESTAMPTZ` on PostgreSQL, `DATETIME(6)` on MySQL.
  - `#[soft_delete]` (`deleted_at: Option<Timestamp>`): `delete_by_id` and
    `QueryBuilder::delete` set it; the repository, the query builder and
    relations skip such rows unless `with_deleted()` / `only_deleted()`;
    `restore`, `force_delete_by_id` / `force_delete`,
    `find_by_id_with_deleted`.
  - `#[version]`: optimistic locking — `save` of a stale copy changes
    nothing and fails with `DbErrorKind::StaleVersion` (`DbError::kind()`,
    `is_stale_version()`; also `NotFound`).
  - `DbPool::seed(dir)` and `DbPool::connect(url)`; `FromColumn for Value`.
  - PostgreSQL `TIMESTAMPTZ`/`TIMESTAMP`/`DATE`/`UUID` and MySQL
    `DATETIME`/`TIMESTAMP`/`DATE` columns read as text (they were `NULL`).
- **`rws db migrate|rollback [N]|status|seed`** (roadmap #91): the
  project's migrations (`rws.app.toml`'s path) and seeds (`seeds/` beside
  them) from the command line, on `--database-url`, `DATABASE_URL` or
  `RWS_DB_*`; `--json` (the `status` JSON is the dev MCP server's
  `migrations` answer), exit codes 0/1/2. Needs a `rws` built with a
  `model-*` feature. See [`rws db`](https://rws8.tech/database/migrations/#from-the-command-line-rws-db).
- **CI runs the model, `TestDb` and `rws db` tests on PostgreSQL 16 and
  MySQL 8.4** (job `model-databases`), as well as SQLite.
- **rws-macros 17.31.0** must be published before rust-web-server.

### Fixed

- **The model layer on PostgreSQL and MySQL**, found by running its tests
  there for the first time: `INTEGER`/`SMALLINT`/`REAL` columns read as
  `NULL` on PostgreSQL (only `BIGINT`/`DOUBLE PRECISION` decoded), and so
  did an `INSERT … RETURNING` of a `SERIAL` key; `save` of a `None` field
  failed on PostgreSQL ("column is of type integer but expression is of type
  text" — NULLs are now written as literals); `QueryBuilder::count` (and so
  `paginate`) after `order_by` failed on PostgreSQL (the `ORDER BY` is left
  out); and migrations failed on MySQL, which can't key the
  `_schema_migrations` table's `TEXT` column (now `VARCHAR(255)`; existing
  tables keep working). On SQLite a column declared with a type sqlx
  doesn't read as text (`DATETIME`, `DATE`) is read as it is stored rather
  than as `NULL`. `save` of a missing row with a non-zero key is a
  `DbErrorKind::NotFound` error instead of "updated row not found".

## 18.29.0 — 2026-11-04

### Added

- **Test helpers** (roadmap #89), all in `test_client` (nothing on the
  request path changes):
  - `client.websocket(path).connect()` → `TestWebSocket`: runs a route's
    `WebSocket::upgrade` handler over an in-memory duplex — `send_text`,
    `send_binary`, `recv`, `recv_text`, `close` (masking, pings, fragments
    and the closing handshake handled); a refused upgrade is the `Err`
    response. See [WebSocket and SSE in tests](https://rws8.tech/testing/websocket-sse/).
  - `response.sse()` / `response.sse_events(n, timeout)` read a
    `text/event-stream` body, streamed or buffered, into `SseFrame`s
    (`event`, `data`, `id`, `retry`); `SseParser` is the bounded parser
    underneath (fuzz target `sse_stream`). `TestResponse` keeps a streamed
    body (`is_streamed`, `take_stream`) and is `Debug`.
  - `assert_openapi_snapshot(&client, "tests/snapshots/openapi.json")` and
    `assert_json_snapshot`: normalized JSON (sorted keys) compared with a
    committed file, a line diff on a mismatch; `RWS_UPDATE_SNAPSHOTS=1`
    writes it. See [OpenAPI snapshots](https://rws8.tech/testing/snapshots/).
  - `TestDb` (`model-*` features): `TestDb::sqlite().migrations("migrations").await`
    is a fresh in-memory database per test; `TestDb::postgres(url)` /
    `mysql(url)` (and `_from_env`, reading `RWS_TEST_POSTGRES_URL` /
    `RWS_TEST_MYSQL_URL` and skipping when unset) migrate once, then pin the
    pool to one connection inside a transaction that is never committed.
  - `#[derive(Factory)]` (rws-macros 17.30.0) beside `#[derive(Model)]`:
    `User::factory().with(|u| …).build()` / `.create(&pool).await` /
    `*_many(n)`, with per-type defaults (`name-3`, `email-3@example.com`,
    `0`, `false`, `None`). See [Test databases and factories](https://rws8.tech/testing/database/).
  - `TestRequest::send_async().await` for `App::with_async_state` apps:
    awaits the handler instead of blocking the test's thread (`send`
    does), for pools made on a single-threaded test runtime.
- **rws-macros 17.30.0** must be published before rust-web-server.

- **`rws gen model|route|migration|test`** (roadmap #88): code generators for
  a project in the conventional layout (`rws.app.toml`). `gen model Post
  title:String …` writes a `#[derive(Model)]` type (`id` auto-increment, a
  test — with SQLite a round trip through the migrations) and
  `migrations/NNNN_create_posts.sql` + `.down.sql` for SQLite, PostgreSQL or
  MySQL (`--db`, else the `model-*` feature in `Cargo.toml`, which is added
  when missing); `gen route posts` typed handlers (`#[get]` …, list, show,
  create, update, remove) with `register(app)`, registered in `app()`;
  `gen migration <name>` an empty up/down pair; `gen test posts`
  `TestClient` tests for those routes. Output is pinned to the release's API —
  CI generates into every starter and runs the generated tests. Nothing is
  overwritten: a file in the way is a conflict shown as a unified diff (exit
  1), so a second run changes nothing; `--dry-run`, `--json`, `--dir`. Also
  the dev MCP endpoint's `gen_model`, `gen_route`, `gen_migration`,
  `gen_test` tools (`dev_mcp::GEN_TOOLS`; left out by `.read_only()`;
  `dry_run`) and the `rust_web_server::generator` module. See
  [Generators](https://rws8.tech/building-apps/generators/).

### Fixed

- **A config change before the server was up ended the process.** Fleet's
  first pull, the config watcher and the admin API ask for a reload by
  raising `SIGHUP`; one that came after startup marked the server ready but
  before the accept loop had installed its handler hit the signal's default
  action. `config_reload::request_reload` now reloads in place until a
  handler is installed (seen in `ci/e2e/fleet.sh`'s S3 step).

- **`App::with_async_state` handlers lost their `DbPool` connection after
  every request.** Each request's handler ran on a tokio runtime made for it
  and dropped afterwards (`async_bridge::block_on_isolated`); sqlx gives a
  connection back to its pool from a task spawned on that runtime, which
  never ran, so the connection was closed: a reconnect per request for
  PostgreSQL/MySQL, and `DbPool::memory()` lost its database between
  requests. This was every request to an async route — the async servers
  never awaited the handler directly — and `McpServer::async_tool` calls.
  Handlers now run on long-lived runtimes: the async servers await them
  through `Application::execute_async` (now offered for matching routes) on
  the connection's runtime, or on a process-wide `rws-async` runtime under
  per-core runtimes; synchronous callers (behind a middleware layer, the
  HTTP/1.1-only build, `TestClient::send`) wait while it runs on
  `rws-async`. Also no thread and runtime built per request, and requests no
  async route matches go to the fallback `App` without any runtime. The
  caller's span, `Identity` and early-hints destination stay current in the
  handler. See [Where handlers run](https://rws8.tech/building-apps/async-handlers/#where-handlers-run).

- **MCP: `tools/call` read the tool's name from its arguments** when an
  argument was called `name` and the client sent `arguments` before `name`
  (as clients that sort keys do): the call went to the wrong tool or failed
  with "Unknown tool". The name and the arguments are now `params`' own
  members.

- **Proxy: no 502 for a POST after a backend closed an idle connection.** The
  upstream pools kept idle connections for 60 s, while an rws backend closes
  them after 30 s; a non-idempotent request on such a connection could not be
  retried and got a 502 (seen with Stripe webhooks behind a config-file
  route). A pooled connection is now checked before reuse (a non-blocking
  peek, plain and TLS, sync and async forwarders) and skipped when the backend
  has closed it, and the pools keep idle connections 25 s. A request that the
  backend drops after receiving it is still not replayed unless idempotent.

## 18.28.0 — 2026-11-04

### Added

- **`app.dev_mcp()`** (roadmap #85): an MCP endpoint a coding agent can use
  to inspect the app it is working on — `routes` (method, pattern and the
  middleware stack around them), `config` (`RWS_ENV`, features and every
  `RWS_*`/`OTEL_*`/`MCP_*` setting, secrets and URL passwords hidden),
  `recent_requests`, `recent_errors`, and with a `model-*` feature
  `db_schema` (SQLite, PostgreSQL, MySQL) and `migrations` /
  `migrations_run` / `migrations_rollback_last`. On every `Application`
  through `rust_web_server::dev_mcp::DevMcpExt`; `DevTools` chooses the
  tools (`.database`, `.migrations`, `.with_ops`, `.read_only`, `.token`,
  `.at`). Mounted only with `RWS_ENV=dev` (then without authentication) or a
  bearer token (`MCP_TOKEN`); otherwise the app is served alone. See
  [Let a coding agent inspect your app](https://rws8.tech/mcp/dev-tools/).
- **`RWS_ENV`** (`dev`, `test`, `prod` — the default) and
  `entry_point::RwsEnv`.
- The `rws` binary's 25 MCP operations tools moved into the library as
  `dev_mcp::ops_tools` (the binary uses it; names, order and behaviour
  unchanged), with `dev_mcp::OPS_WRITE_TOOLS`.
- `Application::registered_routes()` / `Application::middleware_names()` and
  `Middleware::layer_name()`, default methods that let tools describe an app;
  `WithMiddleware`, `McpServer` and `TimeoutLayer` forward them.
- **Conventional app layout and `rws.app.toml`** (roadmap #87): one layout
  for rws applications — `src/main.rs` (wiring only), `src/lib.rs`
  (`app()`), `src/routes/`, `src/models/`, `src/jobs/`, `migrations/`,
  `templates/`, `tests/`, `static/` — and a manifest at the project root
  naming the app and any path that differs (every key optional; absent
  means conventional). `app_manifest::AppManifest` (`load(dir)`, `parse`,
  `layout(dir)`), `AppPaths`, `LayoutEntry`, `EntryKind`, `ManifestError`
  (line-numbered, never a panic; unknown keys are warnings). Fuzz target
  `app_manifest`. See
  [Project layout](https://rws8.tech/building-apps/project-layout/).
- `dev_mcp` tools **`layout`** (the manifest and which conventional paths
  exist, with their files) and **`models`** (`model-*`): the types named with
  `DevTools::model::<T>()` — table, columns, primary key — compared with the
  database's tables when one is configured (`missing_columns`,
  `unmapped_columns`). `DevTools::app_dir(dir)` sets the project root.
  `model::ModelInfo::of::<T>()` describes a `Model` type.
- **Starters on the layout**: `starters/api`, `mcp` and `fullstack` split
  into `main.rs` + `lib.rs` + `routes/` (`tools/` for MCP) / `models/` +
  `tests/`, ship an `rws.app.toml`, use typed handlers (`#[get]` +
  `.route(f)`; errors as problem details), and mount `dev_mcp` under
  `RWS_ENV=dev`. Each has a `tests/dev_mcp.rs` that calls every dev MCP
  tool it lists; `ci/e2e/starters.sh` runs them, fills `{{crate_name}}`,
  patches `rws-macros`, and fails on stray `{{`/`{%` text cargo-generate
  would misread. The starters now ask for `rust-web-server` 18.28;
  `fullstack`'s migration is `0001_create_notes.sql` with a `.down.sql`.

- **Logs for security monitoring** (#102). All opt-in; with none of it set
  the access log is byte for byte what it was.
  - **IP anonymisation**: `access_log_anonymize_ip = "truncate"` writes
    client addresses as their IPv4 /24 or IPv6 /48, `"hash"` as a keyed
    HMAC-SHA256 pseudonym (`access_log_ip_hash_secret`) — in JSON and
    combined lines (also for addresses from `X-Forwarded-For`), OTLP log
    records and `recent_requests`.
  - **Redaction**: `log_redact_query = ["token", "api_key"]` logs those
    query values as `REDACTED` in paths and `Referer`s; `log_redact_headers`
    does the same for header values. `Authorization`, `Proxy-Authorization`,
    `Cookie` and `Set-Cookie` are never logged (the access log writes only
    `Host`, `User-Agent` and `Referer`; `Log::request_response` now shows
    them as `REDACTED`).
  - **Syslog**: `access_log_syslog = "udp://host:514"`, `tcp://…` (RFC 6587
    octet counting) or `unix:///dev/log` sends every line as an RFC 5424
    message (facility `access_log_syslog_facility`, default `local0`), from
    a background thread.
  - **OTLP logs**: with `OTEL_LOGS_EXPORTER=otlp` access-log records go to
    the collector's `/v1/logs` (OTLP/HTTP JSON), carrying the request's
    trace and span ids when it was traced; batched per thread and sent off
    the request path (`otel::logs`). `ci/e2e/otlp.sh` checks them.
  - **Tamper-evident audit log**: lines appended to `[admin] audit_log`
    carry `prev` (SHA-256 of the line before) and `hash`;
    `GET /admin/api/v1/audit/verify` and `rws ctl audit --verify [FILE]`
    report the first edited, removed or inserted entry. Files from before
    keep working (their lines start the chain).
  - `rws --check` validates the new settings.

- **Compile-checked routes (#86).** `#[get("/posts/{id}")]` & co. on a
  function whose arguments are extractors — `Path<T>`, `Query<T>`,
  `Form<T>` (`serde`), `Json<T>`, `State<S>` or `&S`, `Body`, `BodyText`,
  `RequestHeaders`, `&Request`, `&ConnectionInfo`, any `FromRequest` type —
  and whose return type is anything `IntoResponse` generates the handler
  adapter: register it with `.route(show_post)` on `AppWithState`, `Router`
  or `AsyncAppWithState` (which also takes `async fn` handlers). The route's
  parameters are checked against the `Path` argument at compile time (a
  wrong name, a tuple of the wrong arity, a parameter nobody takes, a
  `#[derive(FromPath)]` struct whose fields don't match). A failed
  extraction answers with its rejection (`400`/`415`/`422`, problem
  details). Classic handlers keep the doc-comment-only behaviour. New:
  module `handler` (`Handler`, `AsyncHandler`, `RouteSpec`,
  `FromRequestParts`, `RequestParts`, `Path`, `FromPath`, `State`, `Query`,
  `Form`, `HandlerFuture`) and `#[derive(FromPath)]`. See
  [Routing](https://rws8.tech/building-apps/routing/#typed-handlers-get-with-extractors).
  **rws-macros 17.29.0 must be published before rust-web-server.**
- **Duplicate and shadowed routes are reported.** `route_conflicts()` on
  `Router`, `AppWithState` and `AsyncAppWithState` lists routes that never
  run because an earlier one with the same method matches every path they
  do (`/users/:id` and `/users/{uid}`; `/users/:id` before `/users/me`);
  each is printed once to stderr when it's registered. Dispatch doesn't
  change.
- `IntoResponse` for `Result<T, E>`, `String`, `&'static str`, `Json<T>` and
  `(&StatusCodeReasonPhrase, T)`.

- **Feature presets** (#96): `web` (templates, cookies and CSRF, password
  hashing, SQLite ORM, jobs, mail), `api` (serde, macros, OpenAPI, JWT auth,
  SQLite ORM, jobs), `proxy` (HTTP/3, ACME, compression, route auth, regex
  rewrites, secret references) and `full` (every feature but `fips`,
  `pkcs11`, `http1` and `mimalloc`). They only turn on other features;
  contents and clean build times on
  [Installation](https://rws8.tech/getting-started/installation/#presets).
- **docs.rs shows every feature's items** (#96), each labelled with the
  features it needs (`[package.metadata.docs.rs]` with `--cfg docsrs` and
  `doc_cfg`, in the facade and every crate under `crates/`).

- Database TLS: `RWS_DB_SSL_MODE` (`disable`, `prefer`, `require`,
  `verify-ca`, `verify-full`), `RWS_DB_SSL_ROOT_CERT` and
  `RWS_DB_SSL_CLIENT_CERT`/`_KEY`, read by `DbPool::from_env()` (and the SQL
  authorization store); in code `DbTls` / `DbSslMode` and
  `DbPool::new_with_tls`. A managed PostgreSQL or MySQL with its own CA can
  now be verified; before, rws could only connect without checking the
  certificate. `DbPool::new` keeps `prefer`.
- `http_client::Client::ca_file(path)` trusts a private CA instead of the
  public roots; `S3Storage::with_ca_file` and `RWS_S3_CA_FILE` use it for an
  S3 endpoint with a private certificate.

### Fixed

- Config-file routes with `[route.middleware.authz]` take the authorizer of
  the compile that builds them. They used to read the process-wide one back
  after setting it, so two configs compiled at once (parallel tests) could
  hand a route the other config's authorizer, or none, so it answered `503`.
  This was the cause of a flaky `server_tests::authz` test.

- PostgreSQL and MySQL pools panicked at their first TLS connection
  (rustls: no process-level crypto provider) in an app that doesn't serve
  TLS itself, as behind a load balancer; rws now installs its provider
  first. The user and password are passed to the driver as they are, not in
  a URL, so `@`, `:`, `/` and `#` in them work.
- Behind a load balancer that terminates TLS, the URLs rws builds for the
  client said `http://`: the A2A agent card's interface (gateway routes and
  `A2aServer`), `OidcAuth`'s redirect URI when none is configured, and its
  cookies had no `Secure`. They now take the scheme from a trusted proxy's
  `X-Forwarded-Proto` (`ip_filter::request_scheme`). `ForwardAuthLayer`
  sends that scheme and the client's address (not the load balancer's) as
  `X-Forwarded-Proto` / `X-Forwarded-For`. Found by the stage suite (S7.5).
- S3: an endpoint on a port other than the scheme's default (MinIO's
  `:9000`) got `SignatureDoesNotMatch` for every request: the signed `Host`
  left the port out, the one sent had it.

- FastCGI: `SCRIPT_FILENAME` and `DOCUMENT_ROOT` are the paths under the
  configured `root`, with symlinks left in (nginx's
  `$document_root$fastcgi_script_name`), not resolved. php-fpm in another
  container, with the same files mounted at the same path, answered
  `File not found.` for every script when symlink targets differed there,
  as in a Kubernetes ConfigMap volume. Found by the stage suite (S9.4).
  Symlinked deploy directories (`current` → `releases/N`) now show PHP the
  `current` path.

## 18.27.2 — 2026-11-03

### Fixed

- **Streamed proxy bodies ended only at the backend's read timeout.** A
  large (> 1 MB) upstream response with a `Content-Length` was read past its
  last byte on the backend's keep-alive connection until the read timeout:
  HTTP/2 and HTTP/3 clients got the whole body, then `RST_STREAM`
  (`INTERNAL_ERROR`) 30 s later — Git LFS retried for ever — and HTTP/1.1
  clients waited 30 s (18.27.1 hid it there). The body now ends at its
  length (a shorter one is an error), in both forwarders. A chunked upstream
  body over HTTP/1.1 is decoded and framed again, so it ends at the last
  chunk instead of the same timeout; its trailers are no longer passed on.
- An HTTPS backend that stalled or failed mid-body ended a streamed body as
  if complete; with a `Content-Length` that is now an error.

## 18.27.1 — 2026-11-03

### Fixed

- **Large proxied downloads over HTTP/1.1** (an upstream `Content-Length`
  over 1 MB, e.g. Git LFS objects behind rws). The HTTP/1.1 writer dropped
  the length and re-chunked the body in 8 KiB pieces of three writes each:
  1.8 MB/s where HTTP/2 did 90 MB/s, and a backend that failed mid-body got
  a final chunk, so the client received a short body that looked complete.
  Now the `Content-Length` is kept and exactly that many bytes are copied;
  reads and writes are 64 KiB (one write per re-encoded chunk too); a read
  error or an early end closes the connection without ending the body.

## 18.27.0 — 2026-11-03

### Added

- Helm chart: `configFile` — the `rws.config.toml` as a value, mounted from
  a ConfigMap with `RWS_CONFIG_FILE` set (until now the chart had no way to
  give rws routes, upstreams or middleware); `extraVolumes` /
  `extraVolumeMounts`. Found by the stage end-to-end suite.
- `ip_filter::client_ip(request, peer)` — the client address an app should
  use (below).

### Changed

- **The client behind a trusted proxy.** When the peer is in
  `trusted_proxies`, the client address is the first untrusted one from the
  right of `X-Forwarded-For` (else `Forwarded: for=`), as nginx's
  `real_ip_recursive`. The rate limits, `allow_ips` / `deny_ips`, blocklist,
  WAF IP rules, `ip_hash`, access and request logs, FastCGI `REMOTE_ADDR`,
  admin audit and Wasm plugin input use it; until now behind a load balancer
  they all saw the balancer (every client shared one rate-limit bucket).
  Breaking for an IP allowlist written against the balancer's address:
  `client_ip = "peer"` (`RWS_CONFIG_CLIENT_IP`) restores it. Found by the
  stage end-to-end suite.
- A backend that doesn't answer within `read_timeout_ms` (or connect in
  time) gets `504 Gateway Timeout` instead of `502 Bad Gateway`, as nginx
  answers; `Proxy-Status` already said `http_response_timeout`. Found by the
  stage end-to-end suite.
- `[cors]` reaches `[[route]]` responses too, as documented ("every
  response"): an allowed origin's preflight is answered by rws, and a
  route's response gets the headers unless its backend sent its own
  `Access-Control-Allow-Origin`. Until now only static files had them.
  Found by the stage end-to-end suite.
- Static-file ETags come from a hash of the file's bytes instead of its
  modification time, so every replica gives the same ETag and
  `If-None-Match` gets a `304` from any of them (behind a load balancer it
  was 1 pod in 3). Worked out once per file version; files over 256 MiB keep
  the modification time. Cached ETags from before revalidate once with a
  `200`. Found by the stage end-to-end suite.
- On SIGTERM rws keeps accepting for `shutdown_delay_secs` (default 5,
  `RWS_CONFIG_SHUTDOWN_DELAY_SECS`) with `/readyz` already `503`, then
  drains: a load balancer still sending to the pod in the moment after the
  stop got errors (a `503` in a rolling restart behind the kube-dc gateway).
  `0` restores the immediate drain; Ctrl+C has no delay. Found by the stage
  end-to-end suite.
- `Forwarded` behind a proxy that sends only `X-Forwarded-*` (a TLS
  terminator, most load balancers) starts with its hops, the client's with
  the `X-Forwarded-Proto`/`-Host` it reported: it said only this hop's
  `for=<proxy>;proto=http` while `X-Forwarded-Proto` said `https`. Found by
  the stage end-to-end suite.
- **`/metrics` only for scrapers.** `GET /metrics` answers clients in
  `metrics_allow` (`RWS_CONFIG_METRICS_ALLOW`; loopback and private ranges by
  default) and `404`s the rest: it was public, upstream names and backend
  addresses included. `"*"` restores it. Found by the stage end-to-end
  suite.
- Basic auth answers wrong credentials with `WWW-Authenticate` too (only
  missing ones had it), so a browser asks again (RFC 9110 §11.6.1). Found by
  the stage end-to-end suite.

## 18.26.0 — 2026-11-03

### Changed

- rust-web-server is now a workspace of crates published together at one
  version (`spec/WORKSPACE_SPLIT.md`). Every `rust_web_server::…` path is
  unchanged and the public API is identical — keep depending on
  `rust-web-server`; the other crates are internal:
  - `rws-base` — `core`, `ext`, `http`, `mime_type`, `null`, `symbol`, `url`;
  - `rws-http` — requests, responses, headers, ranges, bodies, JSON,
    extractors, compression, CORS, errors, settings;
  - `rws-server` — the servers, TLS, `App`, routing, middleware, MCP,
    OpenAPI, SSO, sessions, metrics, OpenTelemetry, the HTTP client, the
    model layer;
  - `rws-proxy` — proxy, config routes, admin API, cluster, fleet, WAF, LLM
    and A2A gateways, FastCGI, traffic insights;
  - `rws-acme`, `rws-storage`, `rws-tower`, `rws-wasm-plugin`,
    `rws-gateway-api` — built only with the feature that enables each.
- ACME, the proxy, the WAF, the LLM gateway, traffic insights and templates
  plug into the server through registration hooks instead of being called
  from it.
- Contributors: a change in one layer rebuilds and retests that layer only
  (an `mcp` change: 3 s to rebuild its tests, 21 s before).

## 18.25.0 — 2026-11-02

### Changed

- CI checks the public API (`public API` job, `ci/public_api.sh`) against
  `ci/public-api.txt`, so a release can't drop or change a public item by
  accident; additions update the listing in the same commit.

- PKCS#11 keys: a handshake's signature no longer stalls the worker it runs
  on — on the shared runtime the worker's other connections move to the rest
  of the pool while the token works (`block_in_place`). The `rws` binary
  therefore keeps the shared runtime when a TLS key is a `pkcs11:` URI and
  `runtime` isn't set (per-core was the default since 18.0.0); an explicit
  `runtime = "per-core"` still wins, with a startup warning.

- One base64 decoder: `core::base64::Decoder` (standard, URL-safe or
  either alphabet; optional whitespace; padding optional, concatenated
  padded segments allowed) replaces seven private decoders (JWT, PKCE,
  PEM, webhook secrets, Azure keys, etcd values, MCP content).
  `Base64::decode` was quadratic — a 200 KB input took minutes — and is now
  linear; `Base64::encode` no longer allocates per 3 bytes. Input that was
  never valid base64 (a lone trailing character, data after too much
  padding) is now refused everywhere instead of being decoded to whatever
  bits were left.

## 18.24.0 — 2026-11-01

### Changed

- Releases are gated (#83): the release workflow refuses a tag whose commit
  isn't on `main` or a `<major>.<minor>.x` line or whose CI run isn't green;
  CI builds with `--locked`, lints the workflow files (actionlint) and checks
  the release's Windows features on Windows. The `18.20.x` LTS branch exists.
- **Proxied requests: client-sent forwarding headers are believed only from
  `trusted_proxies`** (#84). An incoming `X-Forwarded-For/Proto/Host` or
  `Forwarded` is kept when the peer is loopback or in a private range
  (10/8, 172.16/12, 192.168/16, 100.64/10, fc00::/7) by default; from anyone
  else rws replaces them, so a client can't claim `https` or another host to
  your app. **Behind a public CDN (Cloudflare, Fastly…) list its ranges in
  `trusted_proxies`, or set `trusted_proxies = "*"` for the old behaviour.**

### Security

- Fleet config pull (#80): signatures now carry a serial (`rws-fleet-sig v2
  <serial> <sig>`, from `rws fleet sign`), and a config whose serial isn't
  above the applied one is refused, so an older signed file can't roll an
  instance back; a fetched config can't change the `[fleet]` settings; when
  the reload refuses a fetched config the previous file is restored. 18.23
  signature files still verify; re-sign to get rollback protection.
- WAF (#77): MySQL's executable comments (`union/*!select*/`) no longer hide
  SQL from rule 942120; command injection after `;` or `|` needs a
  shell-like argument, so text like "tired; head hurts" isn't blocked;
  `rws --check` warns when a WAF route has no body size limit.
- OIDC sign-in (#73): a `return_to` with a tab, newline, space or other
  control character is replaced by `/` (browsers drop tabs from URLs, so
  `/%09/evil.example` was an open redirect after login); query values are
  decoded as UTF-8.
- PKCS#11 (#79): a `pin-value=` in a key URI is hidden in the startup output
  and `GET /admin/api/v1/config`; URIs are percent-decoded per RFC 3986.

### Fixed

- Cluster sync (#76): an admin change no longer waits on Redis — it's
  queued for the sync thread, which sends it at once; a Redis connect gives
  up after 2 s per address instead of the OS's long retry.
- FastCGI routes serve `/.well-known/` (security.txt, app links); other
  dot-files stay refused.
- The JSON escaping in the OAuth server, AWS Secrets Manager client and ACME
  shares one implementation (`core::json::escape`); the first two left
  control characters unescaped.
- WebSocket upgrades through proxy routes sent the client's
  `X-Forwarded-For` and rws's own as two headers; they're one chain now.
- PKCS#11: handshakes no longer wait on one shared session per key — up to
  8 sessions sign in parallel.

## 18.23.2 — 2026-11-01

### Fixed

- ACME: a chunked reply from the CA is decoded properly. The decoder skipped
  the first chunk-size line and read data as a size, so the body came back
  empty; Let's Encrypt chunks large orders (12 names here), and rws then waited
  for a status it never saw ("order timed out with status ''"). The framing now
  comes from the Transfer-Encoding header.

## 18.23.1 — 2026-11-01

### Fixed

- The Windows release binary didn't build in 18.23.0 (the PKCS#11 signer
  converted a `u64` where Windows' `CK_ULONG` is 32 bits), so 18.23.0 has
  no Windows download; 18.23.1 does.

## 18.23.0 — 2026-11-01

### Added

- FIPS build (#78): the `fips` feature runs all TLS on aws-lc's FIPS 140-3
  module with FIPS-approved algorithms only, checks every TLS config at
  startup, and says `(FIPS)` in `--version`; a
  `rws-<version>-x86_64-unknown-linux-gnu-fips.tar.gz` release artifact.
  Every rustls config now names `crypto_provider()` explicitly.
- TLS keys in an HSM (#79): with the `pkcs11` feature `tls_key_file` takes a
  `pkcs11:` URI (RFC 7512: token, object/id, module-path, pin-source) and
  handshakes are signed on the token — ECDSA P-256/P-384, RSA-PSS and
  PKCS#1; `rws --check` opens the key. In the macOS, Windows and glibc FIPS
  release binaries.
- Fleet config pull (#80): `[fleet] source` (HTTPS with ETags, `s3://`,
  `git+…#branch:path`, `{node}`) fetched every `interval_secs`; an Ed25519
  signature (`rws fleet keygen`, `rws fleet sign`), `rws --check` and the
  reload must all pass before the config replaces the running one; outcomes
  POSTed to `report_url` and shown in `status`. Release binaries now include
  `storage-s3`.

## 18.22.0 — 2026-11-01

### Added

- Admin users, roles and audit log (#75): `[admin] users =
  ["name:ENV_VAR:role"]` adds named tokens with `viewer`, `operator` or
  `admin` roles; every change through the API, UI, `rws ctl` or MCP tools
  (and every refused attempt) goes to an audit log — `GET /audit`,
  `rws ctl audit`, an Audit page, and `audit_log` for a JSON-lines file;
  `GET /whoami`. Single sign-on for the admin UI with `oidc_issuer` /
  `oidc_provider` and `oidc_roles` (groups → roles), cookie changes guarded
  by an `X-Rws-Admin` header.
- Runtime state across replicas (#76): with `[cluster] redis` (or
  `[acme] redis`) the IP blocklist, maintenance mode, drained backends,
  feature flags and cache purges changed on one replica apply on all of
  them within a second; Redis outages are ridden out (changes queue, an
  empty Redis is re-seeded from the replicas); `status` lists the nodes.
- Web application firewall (#77): `[middleware.waf]` / `[route.middleware.waf]`
  with rws's own rules — SQL injection, XSS, path traversal and sensitive
  files, inclusion wrappers, command injection, cloud-metadata SSRF,
  Log4Shell, scanners — over decoded paths, arguments, form/JSON/multipart
  fields, cookies and headers; anomaly scoring, paranoia 1–3, `block` or
  `detect`, exclusions by rule and path, custom `deny`/`allow`/`log`/`score`
  rules; `403` problem+json, `[waf]` log lines, `rws_waf_*` metrics;
  `waf::WafLayer` for code.
- Support policy (#81): long-term-support lines (one minor every six months,
  security and critical fixes for twelve months; 18.20 is the first, until
  2027-11-01), security response targets and coordinated disclosure in
  `SECURITY.md` and a Support policy docs page.

## 18.21.0 — 2026-11-01

### Security

- `ForwardAuthLayer`: a header named in `copy_header` now only ever comes
  from the auth service — a client's own copy is dropped even when the
  service doesn't send one (it used to pass through). The auth service also
  gets `X-Forwarded-Method/Proto/Host/Uri/For` set by rws, not the client's.
- `OidcAuth` removes `X-Rws-Oidc-Claims` and `X-Auth-Request-*` headers a
  client sends, on excluded paths too; `return_to` must be a local path.
- An `[route.middleware.auth]` table with an unknown `type` makes the route
  answer `503` (and `rws --check` fail) instead of leaving it unprotected.

### Added

- Local HTTPS (#71): `tls = "internal"` (`--tls=internal`) makes rws its
  own certificate authority and serves a certificate for `localhost`,
  `127.0.0.1`, `::1`, the listen address, the virtual hosts and
  `tls_internal_names`, re-issued weekly without a restart. `rws trust`
  puts the CA in the system trust store (Linux, macOS, Windows, and the
  NSS database browsers use on Linux); `rws untrust` removes it.
- FastCGI (#72): `type = "fastcgi"` routes run PHP through php-fpm (TCP or
  Unix socket, kept-alive connections) with front-controller routing,
  `PATH_INFO`, static files served by rws, scripts confined to the root
  and httpoxy-safe variables; `rust_web_server::fastcgi::FastCgi` for code.
- Sign-in in the config file (#73): `[route.middleware.auth] type = "oidc"`
  (an issuer's discovery, or `provider = "google" | "microsoft" | "github"`)
  with `allowed_emails` / `allowed_domains` / `allowed_groups`, signed-cookie
  sessions that survive restarts and work across replicas, and
  `X-Auth-Request-User/Email/Groups` upstream; `type = "forward"` asks an
  auth service (Authelia, Authentik, oauth2-proxy). `OidcAuth` gains
  `signed_cookies`, `allow_emails`, `allow_domains`, `allow_groups`,
  `forward_headers`, `session_ttl`, and an empty `redirect_uri` follows the
  request's host. Release binaries now include the `sso` feature.
- WebSockets through proxy routes (#82): a `type = "proxy"` route relays
  `Connection: upgrade` requests to a plain-HTTP backend (Action Cable,
  LiveView, Socket.IO, hot reload) on the main listener; the relay moved
  from the Gateway API controller to `proxy::upgrade`.
- Cookbook (#74): complete configurations for a static site, a single-page
  app with an API, Node, Python/Ruby and PHP apps, internal tools behind
  sign-in, an API gateway and local development — each checked by
  `rws --check` in CI (`cookbook_configs_pass_check`).

### Fixed

- The outbound HTTP client sends the port in `Host` when it isn't the
  scheme's default (OIDC discovery behind Keycloak on `:8080` built URLs
  without it).
- The admin API's IP blocklist (and `rws ctl block`) now applies in proxy
  mode too; it only covered the static-file server. Clients that aren't
  blocked still take the non-blocking proxy path.

### Tests

- `ci/e2e/mtls_crl.sh`: CRLs end to end with an openssl CA and curl —
  revoked client certificates refused after SIGHUP and with `watch = true`,
  DER CRLs, `rws --check` on a junk CRL.

## 18.20.0 — 2026-11-01

### Added

- systemd integration (#67), without libsystemd: with `Type=notify` rws says
  `READY=1` once it serves (listeners bound, certificates loaded),
  `RELOADING=1` / `READY=1` around a reload, `STOPPING=1` at shutdown, and
  pings `WATCHDOG=1` from its event loop at half `WatchdogSec`. Socket
  activation: listeners systemd passes (`rws.socket`, matched by
  `FileDescriptorName` or port) are used instead of binding — restarts
  refuse nothing — including by every per-core runtime. The packages' unit
  is `Type=notify` with `WatchdogSec=30` and they install `rws.socket`.
- CRLs for inbound mTLS (#68): `tls_client_crl_file` (PEM or DER,
  comma-separated) makes revoked client certificates fail the handshake;
  re-read on reload and by `watch`, parsed by `rws --check`;
  `tls_client_crl_strict = true` also refuses stale CRLs and certificates no
  CRL covers.
- Key rotation (#69): `JwtLayer::secrets([new, old])` and
  `secret_with_kid(kid, secret)` verify HS256 tokens signed with any of
  several secrets (a token's `kid` picks one); the config file's jwt auth
  takes `previous_secret_env`. htpasswd files are re-read when they change
  (adding or removing a user needs no restart), and `htpasswd_file` may
  list several.
- `rws ctl` (#70): the admin API from the command line — `status`, `reload`
  (exit 1 with the errors on a bad config), `drain`, `maintenance`,
  `block`/`unblock`, `purge`, `caches`, `upstreams`, `certs [renew]`,
  `logs [-f]`, `traffic`; tables for people, `--json` for scripts. Address
  and token from `[admin]` in the config file, `--admin`, `--token-file` or
  `RWS_ADMIN_TOKEN` (never an argument). The deb/rpm packages install an
  `rwsctl` symlink.

### Changed

- Proxied requests carry RFC 7239 `Forwarded` (`for=…;proto=…;host=…`)
  beside the `X-Forwarded-*` headers (#68), extending an incoming one.
  Backends that read `Forwarded` (Spring's `ForwardedHeaderFilter`, Rails)
  now see it, with the values `X-Forwarded-*` already carry.
  `forwarded_header = "off"` (`RWS_CONFIG_FORWARDED_HEADER`) leaves it out.

## 18.19.0 — 2026-11-01

### Changed

- Licensed under MIT only. `LICENSE` is now a single MIT file; earlier
  releases also offered Apache-2.0, CC BY 4.0, ISC and LGPL-3.0, and those
  releases keep that choice. `CONTRIBUTING.md` says contributions are accepted
  under MIT.

### Added

- `mcp::openapi_tools` (#66): OpenAPI 3.0/3.1 operations as MCP tools —
  input schemas from path, query and header parameters and the JSON body,
  output schemas from object responses, annotations from the method, and
  `request(arguments)` building each call's HTTP request. Only `GET`
  operations unless included by operationId; local `$ref`s inlined with
  depth, cycle and size limits. Fuzz target `openapi_to_mcp`.
- The MCP gateway takes HTTP APIs as sources: `[[route.action.mcp.openapi]]`
  (`McpGateway::openapi`) turns an OpenAPI document's operations into
  `<prefix>__<operation>` tools, each call an HTTP request to `base_url`
  (no redirects) with the API's credentials from the environment.
- `McpServer::openapi_tools(app, spec, options)`: an app's own operations
  as MCP tools, each call run in-process through the app (the caller's
  identity in `X-Rws-Mcp-Caller`); `AppWithState::openapi_spec(&config)`
  (`openapi` feature) returns the document `openapi()` serves.
- Config proxy routes choose the upstream `Host`: `preserve_host = true`
  sends the client's, `upstream_host = "…"` a fixed name (`:authority` over
  HTTP/3 upstreams). It was always the backend's address, and a `header_set`
  rewrite of `Host` was silently dropped — `rws --check` now warns about one.

### Fixed

- Admin UI: long config values are shortened in the middle (the full value on
  hover) instead of widening the page, nothing scrolls sideways on a phone any
  more (the layout's column, the seven-section bottom bar and the header fit),
  and sizes and uptime are formatted for the reader's locale (1023.97 KiB shows
  as 1 MiB, not "1024.0 KiB").

- Config-file strings (`rws.config.toml` settings and proxy sections) are
  TOML strings: a basic string's escapes (`\n`, `\r`, `\"`, `\\`, `\uXXXX`, …)
  are decoded, a literal string (`'…'`) is taken as written. They used to lose
  only their quotes, so `body = "a\r\nb"` answered with a literal `\r\n` and
  the documented `body = "{\"ok\":true}"` with its backslashes. A backslash
  before anything else is still kept as written (`"^/v\d+/"`). Note: a value
  that relied on the old reading of a real escape — say a Windows path
  `"C:\new"` — now changes; write it as `'C:\new'` or `"C:\\new"`.
- An escaped quote no longer ends a string for comment, array and inline-table
  splitting (`"a\"#b"` lost everything after `#`), and inline tables keep
  non-ASCII text (it was rebuilt byte by byte and garbled).

## 18.18.0 — 2026-11-01

### Added

- A2A push notifications: webhooks registered with `SendMessage` or
  `CreateTaskPushNotificationConfig` (and Get/List/Delete) get every task
  update as `application/a2a+json`, with the config's `Authorization` and
  `X-A2A-Notification-Token`; only URLs under the prefixes given to
  `A2aServer::push_notifications` are called, without redirects, three
  attempts each. `TaskHandle::artifact_chunk` streams an artifact in
  pieces. CI runs the official A2A compatibility kit (a2a-tck) against
  `examples/a2a_tck.rs`. `A2aServer::extended_card` serves `GetExtendedAgentCard` to
  authenticated callers.

- `[route.action] type = "a2a"`: an A2A gateway route. It serves the
  upstream agent's card with its JSON-RPC interface pointing at the route,
  forwards JSON-RPC calls (streams included) with headers from the
  environment instead of the caller's `Authorization`, and writes an audit
  line per call.

### Fixed

- `http_client` tries every address a name resolves to, not just the first:
  `localhost` (often `::1` first) now reaches a server listening on IPv4 only,
  and so do hosts whose IPv6 address doesn't answer.
- A2A: a task a client's follow-up message moves on (an `A2aReply::Task`
  for a continued task) now notifies its streams and webhooks; only
  `TaskHandle` updates did.

## 18.17.0 — 2026-10-31

### Changed

- Errors rws produces itself (proxy `502`/`503`/`504`, `413`, `431`,
  `408`, `401`/`403` from the auth and filter layers, `429`, maintenance,
  misconfigured routes, `AppError`) answer with RFC 9457 problem details,
  `application/problem+json`
  (`{"type":"about:blank","title":"Bad Gateway","status":502}`), instead of
  a plain-text line. Status codes and headers are unchanged. Set
  `error_format = "text"` (`RWS_CONFIG_ERROR_FORMAT`) for the old bodies.
  [Error responses](https://rws8.tech/reference/errors/) lists them.
- Proxied responses carry `Proxy-Status` (RFC 9209): `rws; received-status=200`,
  or on failure the reason, `rws; error=connection_refused` (DNS, refused,
  timeouts, TLS, terminated, incomplete, protocol errors, no backend in
  rotation). Config-file routes, `ReverseProxy`, HTTP/2 and HTTP/3 reverse
  proxies, canary, ingress, gRPC routes and the Gateway API router.
  `proxy_status = "errors"` or `"off"` limits it, `proxy_name` names the
  hop, `proxy_status_next_hop = true` adds the backend address and error
  text (off by default: they reveal internal addresses).
- Responses through a response cache (`CacheLayer`, memory or disk, and
  `[route.middleware.cache]`) carry `Cache-Status` (RFC 9211):
  `rws; hit; ttl=42`, `rws; fwd=uri-miss; stored`, `fwd=stale`,
  `fwd=request`, `fwd=method`, `fwd=bypass; detail=private`. An upstream's
  `Cache-Status` is kept. `cache_status = false` leaves it out.

### Added

- `a2a::A2aServer` (#65): an agent other AI agents call over A2A 1.0
  (JSON-RPC binding): the Agent Card at `/.well-known/agent-card.json`,
  `SendMessage` (a direct message or a stored task; continuing a task that
  waits for input), `GetTask`, `ListTasks`, `CancelTask`, `A2A-Version`
  checks and the A2A error codes; long-running tasks updated from other
  threads through a `TaskHandle` (`SendMessage` waits for them, or answers
  at once with `returnImmediately`); `SendStreamingMessage` and
  `SubscribeToTask` as server-sent events; bearer tokens with per-caller
  tasks, listed on the card. Fuzz target `a2a_request`.

### Fixed

- `install.sh` falls back to the `releases/latest` redirect when the GitHub
  API answers nothing (its 60 requests an hour per address run out on shared
  CI runners and office networks), instead of failing with "could not find
  the latest release".
- `service_discovery::json_lite` (used for Vault, AWS Secrets Manager,
  Consul, etcd, Docker and GCS responses) refuses JSON nested deeper than
  128 levels instead of recursing until the stack overflows.
- Host routing (`[route.match] host` and `Router::with_host`) goes by the
  request's `Host` header (`:authority` over HTTP/2 and HTTP/3), falling back
  to SNI only without one. It went by SNI first, so a browser reusing its
  connection to one name on a shared certificate for another name got the
  first name's routes (seen on rws8.tech: the site served Gitea after a visit
  to git.rws8.tech). HTTP/3 requests now carry `:authority` as `Host`, as
  HTTP/2 ones did. `Router::with_host` also ignores case and a port it doesn't
  name, as config routes already did.

## 18.16.0 — 2026-10-31

### Added

- Translations (`i18n` module, #33): message catalogs per language
  (`<language>.toml`, a flat TOML subset), `Accept-Language` parsing and
  RFC 4647 negotiation (`AcceptLanguage`, also an extractor), fallback from
  regional to base to default language, `{name}` placeholders, plural forms
  for the common language families (`plural_category`), and
  `Translator::localize` for `Content-Language` + `Vary`. No new
  dependency. Fuzz targets `accept_language`, `i18n_catalog`.
- `GcsStorage` (`storage-gcs` feature): Google Cloud Storage for the
  `Storage` trait, over the JSON API with no Google SDK. Credentials as
  Google's client libraries find them: `RWS_GCS_ACCESS_TOKEN`, a service
  account key or `authorized_user` file (`GOOGLE_APPLICATION_CREDENTIALS`
  or gcloud's default file), or the metadata server (Compute Engine, GKE,
  Cloud Run); `RWS_GCS_CREDENTIAL_SOURCE=none` for emulators. CI now runs
  every storage backend's tests.

### Fixed

- The `wasm32-wasip2` build (`rws-wasm-shim`) compiles again: WebSocket
  routes, upstream metrics and the MCP gateway are native-only there. CI
  now builds the component and serves pages, a 404 and a streamed file
  through `wasmtime serve` (`ci/e2e/wasm_shim.sh`), so it can't break
  unnoticed again.

## 18.15.0 — 2026-10-31

### Added

- `traffic::classify(ip, user_agent, path)`: people (browsers), AI agents
  (training, search, assistants) and bots (search engines, SEO, monitoring,
  link previews, scanners, tools), from bundled lists — ai.robots.txt and
  crawler-user-agents (MIT, `src/traffic/NOTICE.md`) — and the IP ranges
  Google, Bing, Apple, DuckDuckGo, OpenAI, Perplexity and Common Crawl
  publish; a User-Agent claiming one of those crawlers from elsewhere is
  marked spoofed. `ci/gen_bot_lists.py` regenerates the tables (`--check`
  in CI). Fuzz target `traffic_classify`.
- Traffic counting (`[traffic]`, on whenever the admin API is): every
  request — logged or not — lands in hourly counts per host and group
  (people, AI, bots), daily unique visitors (hashed with a secret that's
  never stored; no IPs or User-Agents kept) and daily top lists (pages,
  AI agents, bot kinds, referrers, 404s). A browser counts as a person
  only once it loads a stylesheet, script or font. Saved every 5 minutes
  to `traffic.json` beside the certificate (or `persist`), 30 days by
  default, bounded to a few MB at any load. About 160 ns per request. The
  file is readable by its owner only (0600).
- Traffic page in the admin UI: visitors, page views and requests for
  people, AI agents and bots over a day, a week or 30 days, change from the
  period before, a chart, and tables of the pages people read, AI agents
  (verified by network or not), the pages they read, bots by kind,
  referrers and 404s; a button per site. Admin API `GET /traffic` and MCP
  tool `traffic_summary` (the binary has 25 tools). Docs:
  [Traffic insights](https://rws8.tech/deployment/traffic/).

- A static path that names nothing but ends with punctuation copied along
  with a link (`/docs/http2/):` from an llms.txt line, `/docs/install.`)
  redirects (301) to the path without it when that exists — those requests
  were 404s.

- `min_free_disk` (`RWS_CONFIG_MIN_FREE_DISK`, default the larger of 1% and
  1 GiB): below it rws stops writing the access-log file and disk-cache
  entries (dropped and counted) until space returns — a full disk costs
  log lines, not the server. `rws_disk_free_bytes`, `rws_disk_low`,
  `rws_access_log_dropped_lines_total`, `rws_cache_skipped_writes_total`;
  `disk` in the admin `/status` and a warning on the admin UI's Overview.
  Access-log write errors are logged at most once a minute. A "Disk
  safety" section covers journald, Docker and Kubernetes log limits.

### Changed

- The admin UI registers its Phosphor icons with `@lit-material/icon`'s
  `registerIcons()` and renders them as `lit-material-icon` instead of
  its own name-to-SVG helper; it looks the same.
- `starters/fullstack`'s frontend uses lit-material components rather than
  raw `<input>`/`<ul>`: an outlined text field, a list with a delete icon
  button per note, an empty state, and a snackbar for failed requests (it
  used to print them inline). Agents that copy the starter now start from
  those components.
- The access-log file rotates at 100 MiB by default (`access_log_max_size`
  was `0`, never) — at most 600 MiB with the 5 kept files. Set it to `0` to
  keep rotating with logrotate.
- The JSON access log adds `host`, `user_agent` and `referer` after `bytes`,
  each only when the request sent that header — who visited, for which
  site, from where. Existing fields and their order are unchanged.

## 18.14.0 — 2026-10-31

### Added

- `authz::Identity`: every authenticator hands its caller to the rest of
  the request — `JwtLayer` (`sub`, or `.identity_claim(..)`), `OidcAuth`,
  `SamlSp`, `BasicAuthLayer`, `ForwardAuthLayer` (`.identity_header(..)`)
  and MCP bearer / OAuth tokens. Read it with `Identity::current()`; set
  your own with `authz::with_identity` / `scope_identity`. Not a header, so
  clients can't forge it. A value from a token, header or login is always a
  user id (`Identity::user`): it can't name another type or a set such as
  `group:eng#member`. Async handlers see the caller too.
- Relationship-based access control (Zanzibar style): `authz::Schema`
  (relations as unions of `user`, `group#member`, another relation,
  `parent->viewer`), tuples (`doc:42#editor@user:alice`), `TupleStore` with
  `MemoryStore`, and `Authorizer::check` / `explain` — an iterative walk
  with a visited set, 25 relations deep and 1000 reads at most. Fuzz target
  `authz_parse`.
- `AuthzLayer` (`401` without a caller, `403` when not allowed) with object
  templates (`doc:{path:2}`, `{query:k}`, `{header:H}`; path segments
  percent-decoded, and a path with `.`/`..` or an encoded `/` names nothing,
  so `/docs/42/../7` can't pass a check on doc 42), and in the config
  file `[authz]` (schema per `[authz.type.<name>]`, seed `tuples`) plus
  `[route.middleware.authz] relation / object`; `rws --check` validates
  them; runtime tuples survive a reload. `ci/e2e/authz.sh`.
- `authz::SqlStore`: tuples in SQLite, Postgres or MySQL (a model
  feature), with a 1 s read cache; `[authz] store = "sql"` (`RWS_DB_*`).
- Authorization at runtime: admin API `GET /authz`, `GET`/`POST`/`DELETE
  /authz/tuples`, `POST /authz/check` (with the path that allowed it); an
  Authorization section in the admin UI; MCP tools `authz_check`,
  `authz_tuples`, `authz_write`, `authz_delete` (the binary has 24 tools).
  `admin::ops::register_authorizer` for library apps.
- The proxy config parser accepts arrays spanning several lines (comments
  and a trailing comma allowed).

## 18.13.0 — 2026-10-31

### Added

- HTTP/3 to upstreams: `[[upstream]] protocol = "h3"` sends requests over
  QUIC to `https://` backends (one multiplexed connection per backend,
  streaming both ways, trailers), on both forwarding paths. A backend QUIC
  can't reach is served over TCP and kept there for 5 minutes,
  counted by `rws_upstream_h3_fallbacks_total`. `rws --check` validates
  `protocol`. `proxy::H3ReverseProxy` does the same in code.
  `ci/e2e/h3_upstream.sh`: rws to rws over QUIC, and the TCP fallback.

## 18.12.0 — 2026-10-31

### Added

- 103 Early Hints: `early_hints::send(links)` from any handler or middleware,
  and `EarlyHintsLayer` for a fixed list, write a `103` head with `Link`
  values before the response. HTTP/2 and HTTP/3 by default; HTTP/1.1 with
  `early_hints_http1 = true` (`RWS_CONFIG_EARLY_HINTS_HTTP1`); never
  HTTP/1.0. At most 4 per request.
- Early hints from the config file: `[route.middleware] early_hints =
  ["</app.css>; rel=preload; as=style"]` on any route (proxy routes keep the
  async forwarder) or in the global `[middleware]` table, and proxies pass a backend's own `103` on (its `Link`
  values) instead of dropping it. `rws --check` checks the values;
  `rws_early_hints_total{source}` on `/metrics`; `ci/e2e/early_hints.sh`.
- Dictionary compression codec (RFC 9842): `compression::dictionary` encodes
  and decodes `dcz` (zstd, the dictionary as a raw prefix) and `dcb` (brotli
  custom dictionary), parses `Available-Dictionary` and negotiates the
  coding. Output checked against the `zstd` CLI. Fuzz target
  `available_dictionary`.
- Dictionary compression for static files: `compression_dictionaries =
  ["/js/app.*.js"]` (`RWS_CONFIG_COMPRESSION_DICTIONARIES`) offers matching
  files as dictionaries (`Use-As-Dictionary`), keeps their bytes (32 MiB,
  oldest out) and sends later versions as `dcz` / `dcb` deltas to browsers
  that hold an earlier one. `ci/e2e/dictionary.sh`: Chromium decodes the
  delta; the `zstd` CLI and libbrotlidec decode each coding.

## 18.11.0 — 2026-10-31

### Added

- Cache purge: `CacheLayer::purge(&Purge::{All, Exact, Prefix})` for memory
  and disk caches; HTTP `PURGE` (a trailing `*` purges a prefix) with a
  bearer token from `.purge_token()` / `[route.middleware.cache]
  purge_token_env`; `cache::registry` of named caches (`app`,
  `route:<name>`); admin API `GET /caches` and `DELETE
  /caches/<name>?path=&prefix=` (`/cache` unchanged), a purge field in the
  admin UI, and `path` / `prefix` on the `cache_clear` MCP tool. `PURGE` is
  now an accepted request method (it was a `400`).

- Request mirroring: `[route.middleware.mirror]` (`upstream` or `backends`,
  `percent`) copies a share of a route's requests to other backends and
  drops their answers; fire-and-forget through a bounded queue, so a slow or
  down mirror never affects responses. `proxy::mirror::Mirror` /
  `MirrorLayer` in code, `rws_mirror_requests_total` on `/metrics`,
  `ci/e2e/mirror.sh`.

- Disk cache: `CacheLayer::disk(dir, max_bytes)` and `[route.middleware.cache]
  dir` / `max_size` keep cached responses in files, bounded by total size
  and kept across restarts. Fuzz target `cache_entry`.

### Changed

- The response cache honours the response's `Cache-Control: s-maxage`, else
  `max-age`, when shorter than the layer's `ttl`; `max-age=0` responses are
  no longer stored. Before, every cacheable response was kept for the full
  `ttl`.

## 18.10.2 — 2026-10-30

### Added

- `Response::with_status(status)` and `impl Default for Response`: build a
  response without a struct literal.

### Changed

- Header writing and upstream-response parsing use byte searches
  (`memchr`) for CR/LF checks and line splits, and a cheaper blank-line
  test. Output is unchanged; a same-run benchmark showed no measurable
  throughput change.

## 18.10.1 — 2026-10-30

### Added

- `starters/fullstack`: a lit (`@lit-material`) frontend served by rws as
  static files with the SPA fallback, a JSON API under `/api` with
  `App::with_async_state`, PostgreSQL through the standard `PG*` variables
  (in memory without `PGHOST`), a distroless multi-stage `Dockerfile`, and
  GitHub Actions workflows that test, push to GHCR and deploy to a kube-dc
  project, with a preview per pull request removed when it closes.

### Changed

- Admin UI built on the new lit-material components (status, stat,
  empty-state, log-viewer, table, chart, adaptive navigation): one navigation
  that is a drawer, rail or bottom bar by window width; the log stops
  following while you scroll up, with a "new lines" button; the overview
  adds a requests-per-second sparkline.

### Fixed

- Admin API `GET /status` and `GET /build` (and the `feature_flags` MCP tool)
  list the `wasm-plugins` feature.

## 18.10.0 — 2026-10-30

### Added

- Admin API on a listener of its own (`[admin] listen`, off by default):
  bearer token from the environment variable `token_env` names (required,
  on loopback too; without it the listener doesn't start and `rws --check`
  says so), browser origin checks, `GET /admin/api/v1/status`.
  `admin::AdminApi` / `AdminConfig` / `start` for library users.
  `ci/e2e/admin.sh`.
- Admin API: effective config (secrets hidden), reload with its outcome
  (`config_reload::last_reload`, `report_error`), counters, build
  features, rate limits, CORS, maintenance, blocklist, feature flags,
  response cache, routes, recent requests and errors. `admin::ops` holds the
  shared code; the binary's MCP tools call it, so `server_config` now lists
  every `RWS_CONFIG_*` setting with secrets hidden.
- Admin API: upstreams with each backend's weight, health, circuit-breaker
  state and drain state; drain and undrain a backend (`PUT
  /admin/api/v1/upstreams/<name>/drain`; all drained → `503`); certificate
  inventory with expiry and the ACME renewal loop's last outcome; `POST
  /admin/api/v1/certificates/renew` orders a certificate now
  (`acme::request_renewal`, `acme::renewal_status`).
- Admin API: `GET /admin/api/v1/logs` (the last access-log lines, up to
  500) and `GET /admin/api/v1/logs/stream` (a live tail as server-sent
  events), fed from the access-log sink's writes (`log::tail`).
- Admin UI at `/admin/` on the admin listener: overview with a live
  requests-per-second chart, upstreams with drain switches, certificates
  and renew, controls (maintenance, reload, cache, blocklist, flags),
  config and a live log. Built with lit-material from `admin-ui/` and
  embedded in the binary; strict CSP; `ci/e2e/admin_ui.sh` drives it in
  headless Chromium.

- WebAssembly plugins (feature `wasm-plugins`, on in the release binaries):
  `wasm_plugin::WasmLayer` and `[route.middleware.wasm]` run a core module
  on each request (pass, edit headers or target, or answer) and response
  (edit status and headers). No imports (no WASI); CPU bounded by fuel,
  memory by `memory_mb`; a fresh instance per request; failures answer
  `500` and count in `rws_wasm_plugin_errors_total`. A route whose plugin
  can't load answers `503`. Example plugin in `examples/wasm_plugin`, fuzz
  target `wasm_plugin_output`, `ci/e2e/wasm_plugin.sh`.

### Changed

- The `rws` binary's maintenance mode now also covers config-file proxy
  routes. `MaintenanceLayer` passes requests through while it is off, so
  proxy routes keep their non-blocking forwarder.

## 18.9.1 — 2026-10-29

### Fixed

- rws8.tech: the site build failed when an older docs version's tag has
  `docs/sidebar.json` but no `docs/astro.config.mjs` (18.8), so 18.9.0's
  site was not published; the LLM proxy docs page's front matter didn't
  parse. No changes to the library or the binary.

## 18.9.0 — 2026-10-29

### Added

- LLM proxy (`llm::LlmProxy`, and `[route.action] type = "llm"` in the
  config file): one endpoint in front of OpenAI-compatible and Anthropic
  providers. Virtual keys per caller (the provider's key is swapped in),
  fallback to the next provider on no answer, `429` or `5xx` before the
  first byte, token usage from JSON bodies and event streams (OpenAI
  streams get `stream_options.include_usage`), token budgets per identity
  in memory or shared through Redis, `rws_llm_*` metrics (requests, tokens
  per identity, time to first token, fallbacks, budget rejections) and an
  optional audit line per request. `llm::UsageParser` reads usage out of a
  provider response on its own. Fuzz target `llm_usage`,
  `ci/e2e/llm_proxy.sh`, `examples/llm_proxy.rs`.
- Proxied requests carry `X-Forwarded-Proto` (`https` or `http`, as the
  client connected) unless the client already sent one: the HTTP/1.1 and
  HTTP/2 upstream forwarders, config-file proxy routes, Gateway API routes,
  mirrors and upgrades.
- `McpServer::before_tool_call(|ctx, tool| …)`: refuse a call before it runs
  with a message the client gets as a tool error.
- `McpServer::after_tool_call(|ctx, call| …)`: every finished tool call with
  its outcome (`ToolCallOutcome`: ok, tool error, refused, denied) and
  duration.
- MCP gateway from the config file: `[route.action] type = "mcp"` with
  `[[route.action.mcp.upstream]]`, `caller`, `allow` and `limit` tables,
  secrets from environment variables, checked by `rws --check`;
  `ci/e2e/mcp_gateway.sh` runs it against two upstreams.
- `McpServer::resource_filter` and `prompt_filter`, like `tool_filter` for
  resources and prompts.
- MCP gateway: `Upstream::header_for(name, |claims| …)` sends a per-caller
  header on tool calls (credentials injected server-side per user).
  `Upstream`'s `Debug` no longer prints header values.
- MCP gateway: upstream prompts (`<prefix>__<name>`) and resources (their own
  URIs) are federated and filtered by the same `allow` rules as tools.
- MCP gateway: `McpGateway::audit_log(true)`, one JSON line per tool call in
  the access log (time, caller, tool, upstream, outcome, duration).
- MCP gateway: `McpGateway::limit(tools, max_calls, window_secs)`, per caller
  and tool; a refused call doesn't reach the upstream.
- MCP gateway: `McpGateway::refresh_every` (default 5 minutes) re-reads
  upstream tool lists and applies the difference, with
  `notifications/tools/list_changed` to clients; tools were listed only at
  `build`.
- MCP gateway: an upstream that only speaks MCP 2026-07-28 (it refuses
  `initialize`) is reached statelessly, after `server/discover`, with the
  version in `_meta` and the `Mcp-Method`/`Mcp-Name` headers; 2026-only
  result fields are dropped so 2025 clients can call its tools.

### API changes

- `ConnectionInfo` has a new public field, `tls: bool` (the client connected
  over TLS). Code that builds a `ConnectionInfo` with a struct literal must
  add it; `ConnectionInfo::new(client, server)` and `Default` now exist, so
  `ConnectionInfo { client, server, ..Default::default() }` keeps working
  when fields are added later.

## 18.8.4 — 2026-10-29

### Added

- `directory_listing = false` (`RWS_CONFIG_DIRECTORY_LISTING`): a directory
  without an `index.html` answers 404 instead of listing its files. Default
  `true`, as before.
- `install.sh` (`curl -fsSL https://rws8.tech/install.sh | sh`): installs the
  release binary for Linux or macOS after checking its `.sha256`, into
  `/usr/local/bin` or `~/.local/bin`; `--version` and `--dir` pin a release
  or a directory. The site's install panel offers it, and CI runs it on
  Linux and macOS.
- Proxied requests carry `X-Forwarded-Host` (the `Host` the client sent)
  unless the client already sent one: the HTTP/1.1 and HTTP/2 upstream
  forwarders and Gateway API upgrades.

- `config_check::check_str_offline`: `rws --check`'s checks on configuration
  text without reading this machine's files or environment, for text from
  someone else.
- The docs MCP server at `https://rws8.tech/mcp` (`site/mcp`): `versions`,
  `search_docs`, `get_page`, `get_example`, `api_item`, `setting`,
  `check_config` and `changes` for every minor release of the current major,
  page resources and a `start_rws_app` prompt. Releases attach its binary;
  the site build writes its data (`/mcp/<minor>.json`).

### Changed

- Startup no longer prints the full usage text (about 150 lines) before
  the lines that matter; `rws --help` still does. With HTTP/3 the app is
  built once instead of once per listener, so its startup notes ("MCP tools
  not mounted") print once.

## 18.8.3 — 2026-10-29

### Fixed

- Automatic HTTPS against Let's Encrypt: the ACME client closed its side of
  the connection before reading the newNonce reply, and Let's Encrypt's front
  end answers nothing to such a HEAD, so every order failed with "Replay-Nonce
  missing from newNonce response" (Pebble, which CI tests against, tolerated
  it). The request is no longer half-closed.
- `watch = true` with `[acme]`: the watcher started before the first
  certificate was ordered, saw the certificate and key being written and sent
  `SIGHUP` before the handler was installed, which ended the process. It now
  starts once ACME has finished, before the servers run.
- Adding a name to `[acme] domains` now gets a certificate for it: the stored
  certificate counted as current while it was valid, even when it didn't
  cover every configured name, so the new name kept the old certificate
  until renewal. A certificate missing a configured name is ordered again.

### Website

- rws8.tech is a new site built in `site/` (lit-material, prerendered with
  Lit SSR) and served by rws itself: a landing page, benchmarks charts, a
  config playground whose every output passes `rws --check`, and the docs
  for every minor release of the current major under `/docs/<minor>/`. Old
  doc URLs redirect to `/docs/…`. Starlight and the GitHub Pages deploy are
  gone; each release attaches `rws8-site-<tag>.tar.gz`, which the server
  installs.

### Documentation

- Config file reference: the rewrite rule types are `header_set`,
  `header_remove`, `uri_set`, `uri_strip_prefix`, `uri_add_prefix`, `status`
  and `body_replace`; the page listed names (`set_header`, `set_status`, ...)
  that `rws --check` rejects.

## 18.8.2 — 2026-10-28

### Documentation

- Gateway API benchmark results (docs/deployment/kubernetes-gateway-api):
  rws against Envoy Gateway, NGINX Gateway Fabric, Traefik, kgateway and
  Istio over HTTP/1.1, HTTPS, HTTP/2, 1 MiB bodies and a filter-heavy route;
  the harness (`bench/gateway/run.sh`, workflow `gateway-bench.yml`) covers
  those scenarios. No code changes.

## 18.8.1 — 2026-10-28

### Changed

- Gateway API router: requests are forwarded on the connection's task
  through the async upstream forwarder (plain and BackendTLSPolicy backends;
  pools of up to 1024 idle connections per endpoint) instead of holding a
  blocking-pool thread each. At 256 connections the blocking path fell to
  15k req/s with 119 ms p99 and some failed requests in the gateway
  benchmark. Upgrades and h2c backends keep the blocking path.
- Helm chart: with `gatewayAPI.enabled` the pods get no CPU limit (the
  `resources.limits.cpu` default of `500m` is left out; memory and requests
  apply as set). A Gateway API data plane serves every Gateway's traffic, and
  half a CPU throttled it into high tail latency in the gateway benchmark.
  Deployments without `gatewayAPI.enabled` are unchanged.

## 18.8.0 — 2026-10-28

### Added

- Kubernetes Gateway API controller (#19, `gateway-api` feature; see
  docs/deployment/kubernetes-gateway-api): conformant with Gateway API v1.6.3
  (standard channel) for the GATEWAY-HTTP, GATEWAY-GRPC, GATEWAY-TLS,
  GATEWAY-TCP and GATEWAY-UDP profiles with every standard-channel extended
  feature, in shared and provisioned (a Deployment per Gateway) modes. HTTPRoute
  (matching, the filters, timeouts, mirroring, CORS, h2c and WebSocket
  backends), GRPCRoute, TLSRoute (passthrough and terminate), TCPRoute,
  UDPRoute, ListenerSet, ReferenceGrant, BackendTLSPolicy (with SAN checks),
  client-certificate validation, static addresses; a Service and EndpointSlice
  per Gateway, Lease leader election, Helm values (`gatewayAPI.*`), and a CI
  job running the official suite in kind in both modes.
  The Linux release binaries and the container image include it (off unless
  enabled; it doubles the binary, so macOS and Windows builds don't).

### Fixed

- HTTP/2 requests carry their `:authority` as the `Host` header when the
  client sent no `host` field (RFC 9113 §8.3.1): applications, routers with
  `.with_host`, and the Gateway API router saw no host for HTTP/2 requests.

### Changed

- The minimum supported Rust version is 1.99 (was 1.88): kube 4.2, under the
  `gateway-api` feature, and its dependencies need 1.89+, and the project
  now tracks the latest stable release.

## 18.7.0 — 2026-10-27

### Added

- OpenAPI request and response schemas (#22): `OpenApiConfig::operation(method,
  pattern, Operation)` describes an operation's summary, `operationId`, tags,
  JSON request body (`.body::<T>()`), responses (`.response::<T>(status, …)`,
  `.status(…)`), query parameters (`.query::<T>()`) and path parameter types,
  with schemas from `JsonSchema` — the derivation MCP tools use, so serde
  renames are followed. `examples/mcp_inventory.rs` describes its REST API;
  `ci/e2e/openapi.sh` validates the document with openapi-spec-validator.

- Tower interop, opt-in (#23, `tower` feature): `tower::mount` / `TowerApp`
  serve a Tower service (a tonic gRPC server, tower-http's `ServeDir`, an axum
  `Router`) from an rws app, bodies streaming both ways with gRPC trailers;
  `TowerLayer` uses a Tower layer (tower-http) as rws middleware;
  `TowerService` serves an rws app from hyper or axum. Tower futures run on a
  dedicated `rws-tower` runtime. rws's own request path is unchanged, and the
  default build gets no new dependency. `examples/tower.rs` (tonic-health +
  tower-http); `ci/e2e/tower.sh` calls it with grpcurl over h2c.

- `{param}` route syntax (#42): `/users/{id}` and `/files/{*path}` beside
  `/users/:id` and `/files/*path` — what axum 0.8 and OpenAPI use — in
  `Router`, `AppWithState`, `AsyncAppWithState` and WebSocket routes; `{{`
  and `}}` are literal braces. OpenAPI operations match routes in either
  spelling.

- Starters (#42): `starters/api` (JSON API + OpenAPI 3.1) and
  `starters/mcp` (MCP server), standalone projects with their own `AGENTS.md`,
  copyable with `cargo generate`; `ci/e2e/starters.sh` builds and tests them
  against the checkout on every push.
- An agent pack (#42): `agents/skills/rws/SKILL.md` (Agent Skills: Claude
  Code, Codex, …) with a Cursor rule and Copilot instructions generated from
  it (`ci/gen_agents.py`), published at `rws8.tech/agents/`.
- `llms.txt` / `llms-full.txt` on the site, at the root and per release under
  `/<major.minor>/` (they were only in the repository: the site's links to them
  were broken). Docs page: getting-started/ai-agents.

### Changed

- `/openapi.json` is OpenAPI 3.1.0 (was 3.0.3): `JsonSchema` emits JSON Schema
  2020-12, which 3.1 uses as is. `OpenApiConfig` has a new public field,
  `operations`; code that builds it with a struct literal must set it
  (`Vec::new()`), or use `OpenApiConfig::new`.
- `examples/mcp_inventory.rs` needs the `openapi` feature too.
- A route pattern segment written `{name}` is now a parameter; before 18.7.0
  it matched the literal text `{name}`. Write `{{name}}` for literal braces.

## 18.6.0 — 2026-10-27

### Added

- Config-file proxy parity (#21): an `[[upstream]]` takes `weights`
  (smooth weighted round-robin; proportional `random` and
  `least_connections`), `[upstream.sticky]` (a `HttpOnly` cookie naming the
  backend by a hash of its URL), `[upstream.circuit_breaker]` (passive health
  checks: failing backends leave rotation for `recovery_secs`; `503` +
  `Retry-After` when all are out) and `[upstream.retry]` (idempotent requests
  with a buffered body go to another backend on no response or a listed
  status). All checked by `rws --check`.
- Per-upstream metrics on `/metrics`: `rws_upstream_requests_total`,
  `rws_upstream_request_duration_seconds`, `rws_upstream_retries_total`,
  `rws_upstream_backend_up`, `rws_upstream_circuit_breaker_state`.
- `ci/e2e/proxy_policies.sh` runs these against real backends.
- `http_server_request_duration_seconds` (#25): the OpenTelemetry
  `http.server.request.duration` metric, recorded by the server core for every
  request on HTTP/1.1, HTTP/2 and HTTP/3, labelled by method, status, scheme
  and protocol version.
- Prometheus native histograms (#25): `/metrics` answers in the protobuf
  format when the scraper asks for it (Prometheus with
  `scrape_native_histograms`), with exponential buckets for
  `http_server_request_duration_seconds`. `ci/e2e/native_histograms.sh`
  checks it against Prometheus 3.15.
- OTLP metrics export (#25): `otel::metrics::start` / `start_from_env` push
  `http.server.request.duration` to an OpenTelemetry collector over
  OTLP/HTTP (JSON, cumulative) every `OTEL_METRIC_EXPORT_INTERVAL`, with a
  last export on shutdown. The `rws` binary exports metrics and traces (wrapping
  everything in `OtelLayer`) when `OTEL_EXPORTER_OTLP_ENDPOINT` is set.
  `OTEL_EXPORTER_OTLP_{METRICS,TRACES}_ENDPOINT` and `…_HEADERS` are honoured.
  `ci/e2e/otlp.sh` checks it against otelcol-contrib 0.162.
- Automatic client spans and W3C Baggage (#25): inside a request traced by
  `OtelLayer`, `http_client::Client`/`AsyncClient`, the HTTP/1.1 proxies and
  `DbPool`/`DbTransaction` statements get client spans, and outbound calls carry
  `traceparent` and `baggage` (`otel::Baggage`, `current_baggage`,
  `set_baggage`). `otel::Context` carries a span across threads (used for
  `AsyncAppWithState` handlers); `otel::detached_span` is a `Send` span for
  async code. `ci/e2e/otlp.sh` follows a proxied request into the collector.
  New fuzz target `otel_baggage`.
- TLS metrics (#25): `rws_tls_handshakes_total{tls_protocol_version,tls_cipher,tls_group}`,
  `rws_tls_handshake_failures_total` and
  `rws_tls_certificate_expiry_timestamp_seconds{name}` (default certificate,
  virtual hosts, on-demand names).

### Fixed

- HTTP/2 over cleartext (h2c) no longer sends `Strict-Transport-Security`.
- Trace and span IDs are random: they were a per-process counter (plus the
  start second), so two rws instances in one trace minted the same span IDs.
- OTLP span export no longer runs on the request thread when a batch fills
  (a background thread sends it; partial batches go out after 5 s), honours
  the endpoint's path and `https://`, and escapes span names and attribute
  values in its JSON.

### Changed

- rustls is built with `prefer-post-quantum` (its own default, which rws's
  `default-features = false` had dropped): the server now picks the
  X25519MLKEM768 hybrid key exchange when a client offers it (current Chrome,
  Firefox, Safari, curl with OpenSSL 3.5), instead of plain X25519, and
  outbound connections (proxy, ACME, HTTP client) offer it first.

- `proxy_config::UpstreamConfig` has new public fields (`weights`, `sticky`,
  `circuit_breaker`, `retry`); code that builds it with a struct literal must
  set them (`Vec::new()` / `None`).

## 18.5.0 — 2026-10-27

### Added

- MCP 2026-07-28 beside the session-based versions, on one endpoint (#15):
  stateless requests (`_meta` version and capabilities, `MCP-Protocol-Version`
  / `Mcp-Method` / `Mcp-Name` checked against the body: `-32602`, `-32020`,
  `-32022`, all `400`), `server/discover`, `resultType` and `serverInfo` on
  every result, `ttlMs`/`cacheScope` on cacheable ones (`.cache_hints`),
  `subscriptions/listen` (acknowledged, filtered, tagged), request-scoped
  progress and log messages (`ctx.log`, only with a `logLevel`), and multi
  round-trip requests: `McpContent::input_required(InputRequired::new()
  .elicit(..).sample(..).list_roots(..).state(..))`, `PromptReply` with
  `.prompt_with_context`, `ctx.input_response` / `elicitation_content` /
  `request_state` (HMAC-signed, bound to method, name, caller and 10 min;
  `.request_state_key` for replicas) / `client_supports`; a request for an
  undeclared capability is `-32021`. Removed methods are `404 -32601` there.
  Official conformance suite 0.2.0-alpha.12 in CI: every scenario 2026-07-28
  requires passes, and 2025-11-25 stays at its baseline (#16).
- The MCP Tasks extension (`io.modelcontextprotocol/tasks`, #15): a tool
  returns `ctx.start_task(|task| ...)` — a task handle for clients that
  declared the extension (`tasks/get`, `tasks/update`, `tasks/cancel`,
  `notifications/tasks` on `subscriptions/listen`), the work run inline for
  any other. `TaskContext::input` parks the task as `input_required` until
  `tasks/update` answers; `TaskError` maps to completed-with-error, failed or
  cancelled; `.task_required(name)` refuses non-declaring clients (`-32021`);
  `.task_ttl`, `.max_running_tasks` (256: each running task holds a thread).
  Tasks belong to their caller.
- `Mcp-Param-*` headers (MCP 2026-07-28, SEP-2243): tool parameters marked
  `x-mcp-header` must be mirrored in matching headers on `tools/call`
  (`400 -32020` otherwise); base64 header values must be canonical. The
  conformance suite's 2026-07-28 run now passes every check, 196, including
  the unscored ones.
- `resources/templates/list`: resources registered with a URI template are
  listed there; `resources/list` holds only concrete resources (it listed
  templates as resources before, which the suite flags).
- `crate::core::sha256`: SHA-256 and HMAC-SHA256, no dependency.
- Streaming Server-Sent Events (#24): `Sse::stream()` returns the response
  and an `SseSender`; events reach the client as they are sent, over
  HTTP/1.1, HTTP/2 and HTTP/3. `send` waits for room (bounded memory),
  `try_send` drops, `send_async` for async code; `Err(SseClosed)` once the
  client is gone, found by a keep-alive comment every 15 s. `SseStream`
  sets the buffer and interval. The MCP server's `GET /mcp` stream now uses
  it.
- WebSocket routes (#24): `AppWithState::websocket` / `Router::websocket`,
  or `WebSocket::upgrade(request, handler)` in any handler, hand the
  upgraded connection to the handler on a thread of its own.
  `WebSocketConnection::recv` returns whole messages (fragments joined,
  pings answered, the closing handshake done, protocol errors closed with
  1002/1007/1009); `WebSocketSender` sends from other threads. HTTP/1.1,
  plain and TLS, async and `http1` builds. `examples/realtime.rs`; e2e with
  Node's WebSocket and fetch (`ci/e2e/realtime.sh`).

### Fixed

- `SseEvent` encoding: a lone `\r` in `data`, or a line break in `id` or
  the event type, could add SSE fields of its own; `data` is now split at
  every line end and `id`/event type keep one line.

## 18.4.0 — 2026-10-27

### Fixed

- Per-core runtimes: startup returns only once every runtime's socket is
  bound. Between the setup listener closing and those sockets opening, a
  connection could be refused or reset; the gap is now microseconds, and
  "Started N per-core runtimes" is printed once the port is served again.
- A WebSocket Close frame whose reason isn't UTF-8 now fails the frame, as
  RFC 6455 §7.1.6 requires (text frames already did). It was decoded
  lossily, which could make the reason up to three times the frame payload
  limit (found by the `websocket_frame` fuzz target).

### Added

- TLS-ALPN-01 (RFC 8737, #17): `[acme] challenge = "tls-alpn-01"` gets and
  renews certificates on the HTTPS port alone — no port 80. At startup a
  temporary acceptor answers on the HTTPS socket; afterwards the TLS
  listener answers `acme-tls/1` handshakes with the challenge certificate
  (and only those). Tested against Pebble, first issuance and renewal.
- DNS-01 and wildcard certificates (#17): `[acme] challenge = "dns-01"`
  publishes `_acme-challenge` TXT records through `dns_provider = "rfc2136"`
  (a DNS UPDATE to the zone's primary server, signed with TSIG
  HMAC-SHA256/512 — BIND, Knot, PowerDNS, …) or `"exec"` (a program, for any
  DNS API), waits until the record is visible, and removes it afterwards;
  `*.example.com` in `domains` gets a wildcard certificate. Refusals are
  explained (REFUSED, NOTAUTH, TSIG BADSIG/BADKEY/BADTIME). Tested against
  Pebble and BIND (`ci/e2e/acme_dns01.sh`); the DNS reply parser — now one
  module, `dns`, shared with SRV service discovery — is fuzzed
  (`fuzz/fuzz_targets/dns_message.rs`).
- DNS-01 through the Cloudflare and Route 53 APIs (#17): `dns_provider =
  "cloudflare"` (an API token, `dns_api_token`) or `"route53"` (requests
  signed with AWS Signature V4 from `AWS_ACCESS_KEY_ID` /
  `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN`); the zone by
  `dns_zone_id` or looked up; `dns_api_url` for a proxy or test double.
  Tested against stand-in APIs writing into BIND, signatures checked
  independently (`ci/e2e/acme_dns_api.sh`).
- `challenge = "dns-persist-01"` (#17, draft-ietf-acme-dns-persist): one
  TXT record at `_validation-persist.<domain>` you publish once — no port,
  no DNS credentials on the server, wildcards too. rws prints the exact
  record, checks it's visible before asking the CA, and never changes it.
  Tested against Pebble.
- On-demand TLS (#18): with `[acme] on_demand = true` and an
  `on_demand_ask` endpoint, rws orders a certificate at the first handshake
  for a name the endpoint allows (`GET <ask>?domain=<name>`, 2xx) — for
  customers' own domains. Refused names get the default certificate; one
  order per name, at most four at once; certificates kept under
  `.rws/acme/on-demand/`, loaded at startup and renewed. `rws --check`
  requires the ask endpoint and refuses DNS-01 with it. Tested against
  Pebble (`ci/e2e/acme_on_demand.sh`).
- Replicas sharing certificates through Redis (#18): with `[acme] redis`,
  one replica orders each certificate (a lock in Redis) while the others
  wait and install it, renewals reach every replica, and any replica answers
  HTTP-01 and TLS-ALPN-01 challenges another presented. Redis being down
  isn't fatal. Tested with two replicas against Pebble
  (`ci/e2e/acme_shared.sh`): one order per certificate.
- `[acme] challenge` (`RWS_CONFIG_ACME_CHALLENGE`: `http-01`, `tls-alpn-01`,
  `dns-01`) and the DNS-01 settings (`dns_provider`, `dns_server`,
  `dns_zone`, `dns_tsig_key`, `dns_tsig_secret`, `dns_tsig_algorithm`,
  `dns_exec`, `dns_propagation_secs`); `rws --check` validates them and
  accepts a wildcard domain with `dns-01`.

### Changed

- `acme::AcmeConfig` has new public fields (`challenge`, `dns`): code that
  builds it as a struct literal adds them (`AcmeConfig::from_env` is
  unaffected).
- Startup output no longer prints the values of settings whose names
  contain `SECRET`, `PASSWORD` or `TOKEN` ("(hidden)").

## 18.3.0 — 2026-10-27

### Added

- gRPC and HTTP/2 upstreams at parity with HTTP/1.1 (#13): `H2ReverseProxy`,
  `GrpcProxy` and the config file's `grpc` action keep one pooled,
  multiplexed connection per backend (a new handshake per request before);
  stream response bodies as they arrive and request bodies as they arrive, so
  server-streaming, client-streaming and bidirectional gRPC calls work; and
  pass the backend's trailers (`grpc-status`, `grpc-message`) to HTTP/2 and
  HTTP/3 clients, with `te: trailers` forwarded. A backend failing mid-call
  resets the client's stream.
- `Response::trailers` and `response::Trailers`: trailer fields sent after a
  body over HTTP/2 and HTTP/3 (HTTP/1.1 drops them).
- Upstream client certificates (mutual TLS): `tls_client_cert(cert, key)` on
  `ReverseProxy`, `H2ReverseProxy` and `GrpcProxy`, `[[upstream]]
  tls_client_cert_file` / `tls_client_key_file`, `UpstreamTls::new(ca,
  client)`. `tls_ca_file` now also applies to HTTP/2 and gRPC upstreams
  (`H2ReverseProxy::tls_ca_file`, `GrpcProxy::tls_ca_file`).
- `Middleware::answers_streamed_body`: a layer that answers a request itself
  can take its body as a stream, whatever the inner application says.
- gRPC-web bridge (#14): `GrpcWebLayer` translates `application/grpc-web`
  and `-text` requests to gRPC and the responses back — the status as the
  body's last frame — so browsers can call gRPC services through rws; the
  config file's `grpc` action does it on its own. Checked from headless
  Chromium over HTTP/1.1 and HTTP/2 (`ci/e2e/grpc_web.sh`).
- Inbound PROXY protocol v1 and v2 (#20): `proxy_protocol = true`
  (`RWS_CONFIG_PROXY_PROTOCOL`) takes the client address from the header an
  L4 load balancer (HAProxy, AWS NLB, GCP) prepends to each connection, on
  the plain and TLS listeners; connections without one are closed. Tested
  behind HAProxy (`ci/e2e/proxy_protocol.sh`); parser fuzzed
  (`fuzz/fuzz_targets/proxy_protocol.rs`).
- Cleartext HTTP/2 (h2c, prior knowledge) on the plain listener (#20): a
  connection opening with the HTTP/2 preface is served as HTTP/2 — gRPC
  without TLS, `grpcurl -plaintext`, `curl --http2-prior-knowledge`.
  Anything else is HTTP/1.1 as before.

### Changed

- `Response` and `proxy_config::UpstreamConfig` have new public fields
  (`trailers`; `tls_client_cert_file`, `tls_client_key_file`): code that
  builds them as struct literals adds them (`trailers: None`) — the
  constructors (`Response::new`, `text`, `json`, `get_response`) are
  unaffected. As with `stream_pipe` in 17.110.0.
- The config file's `grpc` action forwards over HTTP/2 (it used HTTP/1.1, so
  gRPC didn't work through it), and `grpc://`, `grpcs://` and `h2s://`
  backends pass `rws --check`.
- HTTP/2 requests keep `te: trailers` for the application (it was validated,
  then dropped).

### Fixed

- An HTTP/1.1 response with no body (a handler returning
  `Response::new()`, a gRPC-web trailers-only error) had no
  `Content-Length`, so a keep-alive client waited for a body until the
  connection closed. It now says `Content-Length: 0` (not for 1xx, 204 and
  304, which never have content, nor for streamed bodies).

## 18.2.0 — 2026-10-26

### Security

- zstd 0.14.3, zstd-safe 8.2.0, zstd-sys 2.1.2 (RUSTSEC-2026-0391, -0398,
  -0403, -0410, published 2026-10-16; taken although newer than the two-week
  rule, being security fixes). rws calls only `zstd::bulk::compress` on its
  own response bodies — a fresh context per call, no decompression, no
  WebAssembly build with zstd — so 18.1.0 isn't known to be exposed; the
  crate now requires zstd ≥ 0.14.3.

### Added

- Runnable examples (#11): `hello`, `rest_api`, `static_site`,
  `reverse_proxy`, `sse_ticker` and `upload` join the MCP examples in
  `examples/`, each with a test `cargo test` runs; listed on the new
  [Examples](https://rws8.tech/getting-started/examples/) page. CI now
  tests with the `serde` and `macros` features too, so `rest_api` and
  `mcp_inventory` run on every push.
- `AGENTS.md`: a guide for coding agents building an app on the crate.
- Helm chart published with each release (#26):
  `helm install my-rws oci://ghcr.io/bohdaq/charts/rws --version <version>`
  (chart version = rws version, image `ghcr.io/bohdaq/rws:<version>` by
  default). New optional `Ingress` and prometheus-operator `ServiceMonitor`;
  `helm/prometheus-adapter-values.yaml` serves the HPA's request-rate metric.
  CI lints the chart, checks it with kubeconform and installs it into kind
  with TLS on and off.
- deb and rpm packages (#27) for x86_64 and arm64 on each release: `/usr/bin/rws`,
  the hardened `rws.service` (now the one on the systemd page; it serves
  `/var/www/rws` and reads `/etc/rws/rws.config.toml`), started on port
  7878 on install, and signed apt and dnf repositories at
  `https://rws8.tech/apt` and `/rpm` (newest release; the rpm files are signed
  too). CI builds the repositories with a throwaway key and installs from
  them and from the files on systemd hosts. The release workflow now pushes
  the rws8.tech `docs` branch as one orphan commit per release (the branch
  is only a deploy target; packages would bloat its history).
- Windows service (#27): `rws --windows-service=<directory>` runs under the
  Service Control Manager; Stop and Shutdown end it gracefully
  (`server::request_shutdown`, which library users can call too). New page
  [Windows](https://rws8.tech/deployment/windows/). Adds the `windows-service`
  dependency on Windows only.
- winget manifests (`packaging/winget/generate.sh`) and a nixpkgs expression
  (`packaging/nix/package.nix`), checked in CI and submitted upstream per
  release (#27).

### Fixed

- Helm chart with `tls.enabled` (its default): liveness/readiness probes and
  the Prometheus scrape annotations used plain HTTP against the TLS port, so
  pods never became ready; they now use HTTPS (the Kubernetes docs' YAML had
  the same mistake). The HPA's custom metric was an `External`
  `rws_route_requests_total` nothing served; it is now the `Pods` metric
  `rws_requests_per_second` from the Prometheus Adapter rule. The chart's
  scrape annotations moved from `podAnnotations` to `metrics.podAnnotations`,
  and the Service port is named `http` (port 80) when TLS is off.
- MCP `ctx.sample()` / `ctx.list_roots()` called right after `initialize`,
  before the client had opened its `GET /mcp` stream, sent the request into
  nothing and timed out; they now wait for the stream within the same
  timeout. SDK clients open that stream in the background, so this made the
  conformance suite's `tools-call-sampling` scenario fail about one run in
  three.
- Release workflow: the docs-site job builds on Node 24 (Astro needs
  ≥ 22.23.4; the runner's Node 22 was older, so 18.1.0's site update failed).

## 18.1.0 — 2026-10-26

### Added

- brotli and zstd compression (#7): responses are compressed in the coding
  the client prefers by `Accept-Encoding` q-values (`br`, `zstd` or `gzip`),
  at the fast levels peers use per response (brotli 4, zstd 3); static files
  are compressed once per version per coding (brotli 6, zstd 6) and cached,
  and precompressed `app.js.br` / `.zst` / `.gz` siblings are served when at
  least as new as the file. `compression::apply_compression`, `negotiate`,
  `encode`, `Encoding`, `Level`; Cargo features `brotli` and `zstd` (default).
  New benchmark scenarios `text-br` and `text-zstd`.
- Reload on file change (#10): `watch = true` (`RWS_CONFIG_WATCH`) reloads
  when `rws.config.toml` or a configured certificate or key changes, with
  the same validation as `SIGHUP`; covered by `ci/e2e/config_reload.sh`.
- Access log to a file (#9): `access_log` (`RWS_CONFIG_ACCESS_LOG`) writes
  access-log lines to a file instead of stdout; `access_log_max_size` rotates
  it by size (`access.log` → `access.log.1` …, keeping `access_log_keep`),
  and `SIGUSR1` reopens it for logrotate (only when a file is configured;
  otherwise `SIGUSR1` keeps its default action). A hardened systemd unit is
  documented on the new [systemd](/deployment/systemd/) page.
- Fuzz targets for WebSocket frames, DNS SRV responses, Redis replies and
  `Cookie` headers, and a request-smuggling seed corpus (CL+TE, conflicting
  or listed `Content-Length`, obfuscated or folded `Transfer-Encoding`,
  bare-LF and oversized chunks) whose handling a test pins (#8).

### Fixed

- **WebSocket frames claiming a huge payload (#8).** `WebSocket::read_frame`
  allocated whatever length a client claimed (up to 2^64 − 1 bytes) before
  reading it, so one 14-byte frame could abort the process. Payloads are now
  read as they arrive and capped at `WebSocket::MAX_FRAME_PAYLOAD` (16 MiB);
  `WebSocket::read_frame_limited` takes another limit.
- **Redis replies (#8).** The RESP reader allocated a bulk string at the
  length the server claimed; bulk strings are now read as they arrive, capped
  at 512 MiB (Redis's own maximum), reply lines at 64 KiB, and a connection
  closed mid-reply is an error instead of reading as `+OK`.
- **DNS SRV discovery (#8).** A response whose SRV record ended inside its
  RDATA panicked while reading priority, weight and port; it is now an error.
- A compressed response kept the uncompressed body's strong `ETag`, so a
  cache could not tell the representations apart; it now carries the coding
  (`"x"` → `"x-gzip"`), static files included.
- An `Accept-Encoding` parameter starting with a multi-byte character
  (`gzip;aé`) panicked while looking for `q=`, ending that connection
  (found by the new `accept_encoding` fuzz target; the same code was in 17.x
  and 18.0.0).

## 18.0.0 — 2026-10-26

### Breaking

- **The `rws` binary runs one runtime per core** (`RWS_CONFIG_RUNTIME=per-core`,
  Unix): one single-threaded runtime per core on its own `SO_REUSEPORT`
  socket, as nginx runs workers. On the fastest benchmark runners that is
  where rws trailed nginx on requests per second; on slower machines it costs
  more CPU per small request. `runtime = "shared"` / `RWS_CONFIG_RUNTIME=shared`
  restores 17.x behaviour; library applications stay `shared` unless they opt in.
- **`Header` fields are `Cow<'static, str>`** instead of `String`. Build with
  `Header::new(name, value)` or `.into()`; read with `&*h.value` /
  `h.value.as_ref()`.
- **Default response headers:** client hints (`Accept-CH`, `Critical-CH`, the
  hints in `Vary`) are opt-in with `RWS_CONFIG_CLIENT_HINTS=true`; `Vary` is
  `Origin` otherwise. Static files send `Cache-Control: no-cache`
  (`RWS_CONFIG_STATIC_CACHE_CONTROL`) instead of `no-store`, plus a standard
  `Last-Modified`; every response has a standard `Date`.
  `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy` and the default
  CSP are opt-in with `RWS_CONFIG_SECURITY_HEADERS=true` (a CSP set with
  `RWS_CONFIG_CSP` is always sent), and `Date-Unix-Epoch-Nanos` /
  `Last-Modified-Unix-Epoch-Nanos` with `RWS_CONFIG_NANOS_HEADERS=true`.
- **`Request.body` is a `request::Body`** (#31), not a `Vec<u8>`: the bytes as before (it derefs to `&[u8]`), or a stream of a body still arriving. Build with `.into()` / `Default::default()`; `.to_vec()` where a `Vec<u8>` is wanted. See the migration page.
- **ACME renewal follows the certificate and the CA** (#1). Certificates are renewed at two-thirds of their lifetime, or earlier when the CA's ACME Renewal Information (RFC 9773) suggests it, instead of 30 days before expiry — which would renew a 6-day certificate at once and leave a 45-day one (Let's Encrypt's default from 2028) a third of its life. Renewal orders name the certificate they replace (`replaces`). `RWS_CONFIG_ACME_PROFILE` orders a certificate profile (`classic`, `tlsserver`, `shortlived`). `RWS_CONFIG_ACME_RENEW_BEFORE_DAYS` has no default now; set it to keep a fixed margin. `AcmeConfig::renew_before_days` is an `Option<i64>` and `AcmeConfig` has a `profile` field. The renewal loop sends `SIGHUP` only after a renewal (it did after every check).
- **MCP sessions are bounded** (#4): idle for an hour (`McpServer::session_ttl`) or past 10 000 (`max_sessions`, least recently used first) a session is evicted with its subscriptions and SSE streams; `DELETE /mcp` ends one; at most 1000 SSE streams (`max_sse_clients`, then `503`). A request carrying an `Mcp-Session-Id` the server doesn't know now gets `404` — the transport's signal to initialize again — instead of running with an empty context.
- **CORS allow-all is off by default** (#59). `RWS_CONFIG_CORS_ALLOW_ALL` defaulted to `true`, which echoed any `Origin` with `Access-Control-Allow-Credentials: true` — any site could read a logged-in user's responses. With no CORS configuration `rws` now adds no CORS headers; list origins in `RWS_CONFIG_CORS_ALLOW_ORIGINS`, or set `RWS_CONFIG_CORS_ALLOW_ALL=true` for the old behaviour.
- **HTTP/2 server:** rws serves HTTP/2 with its own implementation
  (`h2_server`) instead of the `h2` crate; the hidden `h2_handler` module is
  gone. Same conformance (h2spec 146/146), limits and behaviour; the `h2`
  crate stays as the proxies' HTTP/2 client.
- **RSA keys:** `auth::verify_jwt_rs256` and `SamlIdpMetadata.signing_key` take
  `rsa_key::RsaPublicKey` (was `rsa::RsaPublicKey`); RSA keys under 2048 bits
  are rejected. The `rsa` crate (RUSTSEC-2023-0071) is no longer a dependency
  of the auth/SSO features.

### Added

- MCP conformance: the official suite (`@modelcontextprotocol/conformance`
  0.1.16, 2025-11-25 scenarios) runs on every push against
  `examples/mcp_conformance.rs` (`ci/e2e/mcp_conformance.sh`): 28 of 31 pass,
  the three elicitation scenarios are known failures
  (`ci/mcp-conformance-baseline.yml`). For it, `McpContent::audio`,
  `McpContent::blocks` (several items in one tool result) and
  `McpContent::blob` (binary resources, sent as `blob`). A `tools/call` with a
  `progressToken` from a client that accepts `text/event-stream` is answered
  as an event stream — progress notifications, then the result — instead of
  sending progress to the separate `GET /mcp` stream, which a client may not
  have opened yet; progress without such a stream now goes only to the
  caller's session (it went to every connected session).
- MCP gateway (#62): `mcp::gateway::McpGateway` puts one endpoint in front of
  upstream MCP servers — tools as `<prefix>__<tool>`, `allow(identity, tools)`
  rules (deny by default), upstream credentials set per `Upstream` and never
  the caller's; JSON and event-stream replies, paginated `tools/list`, session
  re-initialization; upstream failures are `isError` results.
  `McpServer::require_bearer_tokens` (a token per named caller, seen as
  `{"sub":…}` in `auth_claims`) and `McpServer::tool_filter` (per-request tool
  visibility) work on any MCP server. `examples/mcp_gateway.rs`; docs page
  [MCP gateway](/mcp/gateway/).
- Typed MCP tools (#61): `McpServer::typed_tool(name, description, |ctx, args: In| -> Result<Out, String>)`
  (`serde`) derives `inputSchema`/`outputSchema` from the types, deserializes
  the arguments and returns `structuredContent`. `#[derive(JsonSchema)]`
  (`macros`) and the `schema::JsonSchema` trait produce the schemas, following
  `#[serde]` renames; attributes that would make them disagree are compile
  errors. `examples/mcp_inventory.rs` exposes an app's domain functions as
  REST routes and MCP tools with OAuth and live resource updates; docs page
  [Build agent tools](/mcp/agent-tools/). **rws-macros must be published
  (with the derive) before rust-web-server 18.0.0.**
- `If-Modified-Since` answers `304` for static files (when there's no `If-None-Match`).
- `[[upstream]] tls_ca_file` / `ReverseProxy::tls_ca_file`: trust a private CA for `https://` upstreams.
- Inline tables in `rws.config.toml` settings (`rate_limit = { max_requests = 5 }`).
- `RWS_CONFIG_OPEN_FILE_CACHE_STRICT=false`: reuse a cached file's metadata for the cache window instead of checking it per request.
- `mimalloc` feature: the `rws` binary allocates with mimalloc v3; release binaries and images are built with it — 4–6% less CPU per request, at a larger memory footprint (about 34 MiB under load instead of 9; up to ~120 MiB serving large files over TLS). Build without the feature for the smaller footprint.
- `RWS_CONFIG_RUNTIME=per-core`: one single-threaded runtime per core, each on its own `SO_REUSEPORT` listener (Unix) — more throughput on fast CPUs, more CPU per request on slow ones; the default stays `shared`.
- Supply chain (#5): release binaries are built with `cargo auditable` (`cargo audit bin rws` works on them), releases carry a CycloneDX SBOM, archives and images get GitHub build-provenance attestations, and images are signed keyless with cosign. CI runs `cargo deny check` (advisories, permissive licenses only, crates.io sources only; `deny.toml`).
- Streamed request bodies (#31): an application that answers `true` from `Application::streams_body` (or `AppWithState::stream_body(method, pattern)`) gets `request.body` as a stream it reads while the upload arrives — a 512 MiB upload stays under 64 MiB of memory. HTTP/1.1 (plain and TLS) and HTTP/2 (the stream window is credited back as the handler reads); HTTP/3 and the HTTP/1.1-only build still read the body first. `Middleware::accepts_streamed_body` lets a header-only layer pass it through. `Storage::put_stream(key, reader, content_type)` stores a stream: S3 as a multipart upload in 8 MiB parts (aborted on failure), local disk via a temporary file renamed into place — a 2 GiB HTTP upload streamed to an S3-compatible server grew peak memory by under 96 MiB.
- MCP (#3): structured tool results (`McpContent::structured`, `McpServer::tool_output_schema` → `outputSchema`/`structuredContent`), tool and server titles (`tool_title`, `title`) and icons (`McpIcon`, `tool_icon`, `icon`). `tools/call` arguments are checked against the input schema (an object, `required` properties, property `type`s); a mismatch is an `isError` tool result rather than reaching the handler. With `require_oauth`, the server publishes RFC 9728 protected-resource metadata at `/.well-known/oauth-protected-resource` and its `401` carries `WWW-Authenticate: Bearer resource_metadata=…`; the `/.well-known/oauth-authorization-server` look-alike document is gone. The server speaks protocol versions 2024-11-05 through 2025-11-25 (it advertised 2024-11-05 only and echoed any older string): an unsupported requested version is answered with 2025-11-25, an unsupported `MCP-Protocol-Version` header gets `400`, and JSON-RPC batches are refused for sessions on 2025-06-18 or later.

### Fixed

- MCP (#60): `extract_arg` returns numbers and booleans (it read only strings), so the binary's `set_maintenance {"enabled": true}` and `feature_set` did nothing and `recent_requests`/`recent_errors` ignored `n`; the `reload_config` tool now reloads (it set a flag only the HTTP/1.1-only server reads) via the new `config_reload::request_reload()`. The built-in tools carry read-only/destructive hints (`McpServer::tool_annotations`), and the new [Operate rws from an AI client](https://rws8.tech/mcp/operate/) page has the setup and playbooks.
- Docs (#12): MCP sampling, roots and logging are marked deprecated by the 2026-07-28 MCP specification (they keep working); the MCP SSE stream is documented — and now tested — over HTTP/2 and HTTP/3, not only HTTP/1.1.
- A request head must arrive within 30 s of its first byte, else `408` — a slow-drip client could hold a connection for weeks.
- `H2ReverseProxy::read_timeout_ms` was ignored; a silent HTTP/2 backend hung the request.
- The reverse proxy sent the client's `Content-Length` upstream next to its own (two headers — strict backends reject that, and it's a desync risk). It now sends one, drops headers named in `Connection`, `Trailer` and `Proxy-Connection`, and appends the client address to an incoming `X-Forwarded-For` instead of sending a second one.
- `CacheLayer` no longer stores `no-cache` responses, responses to `Range` requests, or responses that vary on a header outside its key.
- A JWKS key that can't be used is skipped instead of failing verification.
- HTTP/2 (#6): resets the server sends because the client broke a stream's rules (a zero or overflowing `WINDOW_UPDATE`, a bad `PRIORITY`, frames on a closed stream) count toward the rapid-reset budget, closing the "MadeYouReset" hole (CVE-2025-8671) in rws's own HTTP/2 server; and a connection with nothing running that sends nothing for 30 s — a silent client, a half-sent header block or request body, a client that never opens the flow-control window — is closed with `GOAWAY(NO_ERROR)`. It used to stay open forever.
- `If-Modified-Since` (new in 18.0) accepts the obsolete RFC 850 and asctime date forms as well as the IMF-fixdate (RFC 9110 §5.6.7); a client sending them always got the whole file.
- `If-Range` with a date (on top of 17.113.2's entity-tag handling): the `Range` applies when the date equals the file's `Last-Modified` and that is at least a second old (a strong validator), else the whole file is sent.

### Performance

On the benchmark runner, 1 KiB static files went from 49k to 126k requests/s
and the 1 KiB reverse proxy from 28k to 49k: open files, small
file contents and gzip output are cached per file version; one write per
response (`writev` for large bodies, `sendfile` with the head in the same
segment); header lists, dates and numbers built without allocation; request
and upstream-response heads parsed in one pass; HTTPS upstreams forwarded
without blocking a thread; access log buffered per thread (flushed every
100 ms). Files over TLS are read on the connection's task (13% less CPU and
about half the memory for large downloads), HTTP/2 is served by rws's own implementation (20–37% less CPU per request
than with the `h2` crate), and release builds
allocate with mimalloc (4–6% less CPU, more memory). Details:
[Benchmarks](https://rws8.tech/reference/benchmarks/).

## 17.113.2 — HTTP conformance

### Upgrading

Some fixes refuse or change what 17.113.1 accepted:

- An HTTP/1.1 request without `Host`, with two `Host` headers, with a lower-case method (`get`), with a folded header line or with whitespace before a header's colon now gets `400` (#57).
- An HTTP/1.0 connection now closes after each response unless the client sends `Connection: keep-alive` (RFC 9112 §9.3; #57).
- CORS origins match whole: an allow-list entry that relied on matching part of an origin (`example.com` for `https://app.example.com`) now matches nothing — list each origin (#58).
- Request paths are percent-decoded: a file whose name contains a literal `%` is requested as `%25` (#55).

### Fixed

- **gzip no longer rewrites a partial body (#52).** A `Range` request with `Accept-Encoding: gzip` got the requested slice gzipped and `Content-Range` rewritten to the compressed size (`bytes=0-1200` of a 3,700-byte file → `Content-Range: bytes 0-37/37`), which no client can reassemble. A `206` is now sent uncompressed (RFC 9110 §14.4).
- **multipart/byteranges framing (#53).** A multi-range response had neither `Content-Length` nor `Transfer-Encoding`, so a keep-alive client couldn't tell where it ended and waited; the closing delimiter lacked its trailing `--`; and the boundary was the fixed string `String_separator`, which broke the framing of any part that contained it. The response now carries a `Content-Length`, ends with `--boundary--`, and uses a boundary (`Range::multipart_boundary`) that occurs in none of the parts (RFC 9112 §6.3, RFC 2046 §5.1.1). The legacy `Response::_parse_response` reads the boundary from `Content-Type` and no longer loops forever on a body without a closing delimiter.
- **Bodiless responses carry no body or Content-Length (#54).** `1xx`, `204` and `304` responses must have no content (RFC 9110 §6.4.1, §15.3.5): `OPTIONS` on a static file answered `204` with `Content-Length` and, over HTTP/2, a `DATA` frame. Every writer now drops the body (buffered, file or pipe) and `Content-Length`/`Transfer-Encoding` for those statuses (`Response::drop_content_for_status`). And a `200` to `OPTIONS` now sends its body: it used to announce it with `Content-Length` and then drop it, so the client waited for bytes that never came.
- **The static request path is percent-decoded (#55).** Files were looked up by the raw request target, so `GET /my%20file.txt` was a `404` for `my file.txt`, and the directory listing's (correctly encoded) links to such names were dead. The path is now decoded once, before the web-root and hidden-file checks of #43 (so `/%2e%2e/` is refused like `/../`); `%2520` stays a literal `%20`. A `Range` request for a name containing a space, quote, `&`, `|` or `;` no longer gets `416` (the file reader refused such paths), and the general static path finds escaped names' `ETag`.
- **If-None-Match is a list, compared weakly (#56).** Only a header equal to the file's `ETag` gave `304`: a client sending several tags (`"a", "b"`) or the weak form (`W/"…"`) always got the whole file again (RFC 9110 §13.1.2).
- **If-Range is honored (#56).** It was ignored: a client resuming a download with `If-Range` got `206` with bytes of the file as it is now, even after the file changed. A `Range` now applies only while `If-Range` strongly matches the current `ETag`; otherwise the whole file is sent with `200` (RFC 9110 §13.1.5).
- **Range: one satisfiable range is enough; 416 names the length (#56).** `bytes=0-9, 99999-` on a small file was a `416` because of the second range; unsatisfiable ranges are now skipped and `416` is sent only when none can be served — with `Content-Range: bytes */<length>`, which it lacked (RFC 9110 §14.2, §15.5.17).
- **405 with Allow for files (#56).** `POST`/`PUT`/`DELETE` on an existing static file got `404`, which says the resource doesn't exist; it now gets `405 Method Not Allowed` with `Allow: GET, HEAD, OPTIONS` (RFC 9110 §15.5.6).
- **Strict HTTP/1.1 request heads (#57).** Requests without `Host` or with two, a lower-case method, `HTTP/0.9`/`HTTP/2.0` in the request line, folded header lines, whitespace before a header's colon, and header names that aren't tokens are answered `400` — each is a way for `rws` and a proxy in front of it to read the same bytes differently (RFC 9112 §2.2–§3.2, §5.1). `Connection` is evaluated token by token: `Connection: close, TE` used to keep the connection open; an HTTP/1.0 connection now stays open only with `Connection: keep-alive`. `Request::parse` itself stays lenient for library callers; the checks are in the connection loops (`server::framing::validate_head`).
- **CORS: whole-origin matching; exposed headers on the response (#58).** An origin was allowed when it was a substring of the configured list, so with `https://app.example.com` listed, `https://app.example.co` (and `https://pp.example.com`) got CORS headers — with credentials when those are on. Origins are now compared whole (`Cors::origin_is_allowed`). `Access-Control-Expose-Headers` was sent only on the preflight, which browsers ignore; it is now on the actual response.

## 17.113.1 — security patch

Fixes from the 2026-10-25 security review. Upgrade from 17.113.0.

### Security

- **Proxy: interim responses (#48).** A `1xx` from an upstream (`100 Continue`, `103 Early Hints`) was taken as the final response; the real response stayed in the pooled connection and was delivered to the next request on it — possibly another client's. Both forwarders now read past interim responses (at most 16; `101 Switching Protocols` is still final).
- **Static files stay inside the web root (#43).** A request path with `..` segments (`/../../etc/passwd`) was resolved against the working directory and could read any file the process could; such paths now get `404`, and a symlink inside the root that points outside it is not followed. Dot-files and dot-directories (`.git/`, `.env`) are no longer served; set `RWS_CONFIG_SERVE_DOTFILES=true` to serve them again (`.well-known/` is always served).
- **Byte-range limits (#46).** Each range of a multi-range request was read into memory separately: `Range: bytes=0-,0-,0-,…` (thousands of ranges in a 64 KiB head) made one request hold thousands of copies of a file. Overlapping ranges are now merged, and more than 16 ranges are answered with the whole file as one range (RFC 9110 §14.2).
- **No CR/LF in header lines (#47).** The HTTP/1.1 writers copied header names, values and the reason phrase verbatim, so a value carrying user input (a redirect target, an upstream header) could inject headers or a second response; CR and LF now become spaces. The mailer does the same for `Subject`, `To`, `Cc`, `Reply-To` and `From`, and refuses an address containing CR, LF, `<` or `>` (it went into `MAIL FROM` / `RCPT TO` as is). HTTP/2 and HTTP/3 already refused such values.
- **CacheLayer: never cache private responses (#49).** The cache is keyed by URL, so a response built from one user's `Cookie` or `Authorization` was served to the next visitor, a `Set-Cookie` response was replayed to everyone, a `Range` response was served for the whole resource, and a streamed response was replayed with an empty body. Requests with `Authorization`, `Cookie` or `Range` now skip the cache, and responses with `Set-Cookie` or a streamed body aren't stored. The `rws` binary wraps every response in `CacheLayer`.
- **Proxy routes match a normalized path (#50).** Routes matched the raw request path, so `/%61dmin`, `//admin` or `/x/../admin` skipped a route protecting `/admin/*` (auth, IP filter, rate limit) and reached the backend through a catch-all route, which then saw `/admin`. Paths are now normalized before matching and sent upstream normalized (unreserved escapes decoded, a stray `%` encoded, repeated slashes collapsed, dot segments resolved), and `host` matches ignoring case and port.
- **`/mcp` needs a token (#44).** The `rws` binary served its MCP tools — config reload, blocklist, maintenance mode, server introspection — to anyone who could reach the port unless `MCP_TOKEN` was set. Now they're mounted only with a non-empty `MCP_TOKEN`, and `/mcp` sits behind the access log, IP blocklist and global rate limit. In `McpServer`: a request with an `Origin` header not listed in the new `.allowed_origins(...)` gets `403` (browser cross-site requests and DNS rebinding; `MCP_ALLOWED_ORIGINS` in the binary), a `POST` that isn't `application/json` gets `415`, an empty bearer token refuses every request, and tokens are compared in constant time.
- **Private keys out of the web root (#45).** With no paths configured, ACME stored `cert.pem`, `key.pem` and `acme_account.key` in the working directory — the web root — so both private keys could be downloaded. They now default to `.rws/acme/` (keys `0600`, directory `0700`, never served); on upgrade, files at the old default paths are moved there. The static server no longer serves `*.pem`, `*.key` or `rws.config.toml` anywhere. **If you ran ACME without explicit paths, rotate the certificate key and the ACME account key.** The Docker image already kept them in `/data`.

## 17.113.0 — 2026-10-24

- Proxy: pooled keep-alive upstream connections on every HTTP/1.1 path, pooled HTTPS backends, non-blocking forwarding to plain-HTTP upstreams; hot reload of routes, upstreams and `[middleware]` on `SIGHUP`.
- `rws --check` validates a config file; fixed inline middleware tables being ignored (an auth bypass), global `[middleware]`, IPv6 IP filters.
- HTTP/1.1: request heads read across reads (up to 64 KiB), pipelined requests kept, chunked bodies decoded.
- Static files: open-file cache, `sendfile` for plain-TCP downloads, quick requests run on the event loop.
- Compression skips small, incompressible and already-encoded bodies.
- Docker image no longer crashes at startup; published to Docker Hub with a smoke test and a `/data` volume.

## 17.112.0 — 2026-10-24

- RFC 9110 byte ranges; `Content-Range` only on `206`, now over HTTP/2 and HTTP/3 too.
- Every setting works from `rws.config.toml`; unknown keys warn instead of vanishing.
- Config-driven proxy auth fails closed instead of serving the route unauthenticated.
- Automatic HTTPS (ACME) from one config section, verified against Pebble.
- The documented proxy config no longer returns `502`; proxied responses no longer break HTTP/2.

## 17.111.0 — 2026-10-23

- Prebuilt binaries, Homebrew tap, GHCR image, `cargo binstall`; `rws --version` / `--help`.

## 17.110.0 — 2026-10-23

- Directory listing for static directories without `index.html`; opt-in SPA fallback.
- Ingress: TLS to the API server, watch API, `pathType: Exact`, IngressClass filtering.
- Service discovery: Consul, etcd, Docker labels, DNS SRV / weighted DNS.
- Canary: smooth weighted round-robin, live weight updates; circuit breaker wired into `ReverseProxy`.
- Secret references (`vault://`, `aws-sm://`, `azkv://`); `Response::json` / `Response::text`.
- WebAssembly guest component (`wasm32-wasip2`) with outbound HTTP and streaming bodies.
- Helm chart.

## Earlier

See the [GitHub releases](https://github.com/bohdaq/rust-web-server/releases) and the git tags.
