# AGENTS.md

A full-stack app on [rust-web-server](https://rws8.tech) (rws): lit frontend,
JSON API, PostgreSQL, deployed to kube-dc. For coding agents working in this
project.

## Layout

The conventional rws layout, named in `rws.app.toml` (the dev MCP
endpoint's `layout` tool reads it). Put new code where it says:

- `src/main.rs`: wiring only (store, static directory, `dev_mcp`,
  `Server::run_tls`).
- `src/lib.rs`: `app()` (the routes) and `dev_tools()` (what the dev MCP
  endpoint shows; register each new model there with `.model::<T>()`).
- `src/routes/<resource>.rs`: a resource's typed handlers and its
  `register(app)`; a new resource is a new module, added in
  `routes/mod.rs` and `app()`.
- `src/models/<table>.rs`: `#[derive(Model)]` types, one per table.
- `src/store.rs`: the `Store` (PostgreSQL or in memory).
- `src/accounts.rs`: `Accounts` — rws's `AuthFlows` (sign-up, login,
  logout, password reset, email confirmation under `/auth`), the welcome
  email sent by a background `JobQueue`, and the mail transport (SMTP with
  `RWS_SMTP_HOST`, else in memory). `site()` in `src/lib.rs` puts `app()`
  behind it, `FlashLayer` and `ErrorPages`.
- `migrations/`: SQL files applied in order at startup
  (`NNNN_name.sql`, with `NNNN_name.down.sql` to roll back; PostgreSQL
  dialect, `$1` placeholders). Never rename an applied file.
- `tests/`: integration tests against `app()`; `tests/dev_mcp.rs` checks
  every dev MCP tool answers (CI runs it).

  New models, routes, migrations and tests come from `rws gen` (the `rws`
  binary of the same release): `rws gen model Post title:String` (a
  PostgreSQL migration, from the `model-postgres` feature), `rws gen route
  posts --prefix /api`, `rws gen test posts`, `rws gen migration <name>`.
  It writes against this release's API and never overwrites a file;
  `--dry-run` shows the plan. https://rws8.tech/building-apps/generators/
- `frontend/`: lit 3 + `@lit-material` components, built by Vite into
  `frontend/dist`, which rws serves as static files.
- `Dockerfile`: frontend → API → distroless image on port 7878.
- `deploy/`: `app.yaml` (Deployment + HTTPS Service), `deploy.sh`,
  `kubeconfig.sh`.
- `.github/workflows/`: `ci` (PRs), `deploy` (main → production project),
  `preview` (one copy per PR, removed on close).

## Commands

- `cargo test`: socket-free tests (`TestClient`); with `PGHOST` etc. set it
  also runs against PostgreSQL.
- `cargo run`: http://127.0.0.1:7878, serving `frontend/dist` (build it first:
  `cd frontend && npm ci && npm run build`).
- `cd frontend && npm run dev`: frontend with live reload on :5173, `/api`
  proxied to `cargo run`.
- `RWS_ENV=dev cargo run`: also the dev MCP endpoint at `/mcp`
  (`claude mcp add --transport http app-dev http://127.0.0.1:7878/mcp`):
  `routes`, `models`, `layout`, `config`, `recent_errors`, and with
  PostgreSQL `db_schema`, `migrations`, `migrations_run`,
  `migrations_rollback_last`; mail previews at `/_mail`; error pages
  with backtraces. Outside `RWS_ENV=dev` only with `MCP_TOKEN`
  set (and requires it); never set `RWS_ENV=dev` in `deploy/`.

## rws in brief

Docs: https://rws8.tech, one file: https://rws8.tech/llms-full.txt.

- Typed handlers: `#[delete("/api/notes/{id}")] async fn remove(Path(id):
  Path<i64>, State(store): State<Store>) -> Result<Json<Value>, AppError>`,
  registered with `App::with_async_state(store).route(remove)`. Arguments
  are extractors (`Path`, `Json`, `Query`, `State`, …); the attribute checks
  `{id}` against `Path` at compile time.
- Accounts: a route that needs a signed-in user takes a `CurrentUser`
  argument, or goes in a group with `.guard(signed_in)` (see
  `src/routes/account.rs`). Settings: `AUTH_SECRET` (32+ bytes; random
  without it, so sessions end on restart), `APP_URL` (links in emails).
  https://rws8.tech/features/auth-flows/
- Errors: `AppError::BadRequest(msg)` & co. answer problem details
  (`application/problem+json`, the message in `detail`, which the frontend
  shows).
- Models: `#[derive(Model)]` with `#[table(name = "…")]`,
  `#[primary_key(auto_increment)]`; `Note::query(&pool)` (query builder),
  `Note::repository(&pool)` (`save`, `find_by_id`, …).
- Requests that match no route fall through to static files from the working
  directory, which `main` sets to `STATIC_DIR` (default `frontend/dist`) —
  never the source tree. `RWS_CONFIG_SPA_FALLBACK=index.html` serves the app
  for client-side routes; `/api` and `/healthz` are excluded.
- Database: `DbPool` (`query_rows`, `execute`, `migrate`). The pool is built
  from the standard `PG*` variables, so TLS settings (`PGSSLMODE`,
  `PGSSLROOTCERT`) and any password characters work.

## kube-dc

- The database is a managed service, never a pod. Its binding Secret
  `<app>-db-owner` (keys `host`, `port`, `dbname`, `username`, `password`,
  `ca.crt`) is mapped to `PG*` in `deploy/app.yaml`; connections are TLS
  with `verify-full`.
- HTTPS comes from the ClusterIP Service annotated
  `service.nlb.kube-dc.com/expose-route: https`; the hostname appears in the
  annotation `service.nlb.kube-dc.com/route-hostname-status` once the
  certificate is issued. Never use `type: LoadBalancer` for web traffic.
- With the kube-dc MCP connector: check status with `list kind=services`,
  `get_logs`, `list kind=events`; create the database with
  `create_managed_service` (show the price before confirming).
- Pod exec is not available on kube-dc; one-off tasks run as Jobs.
- Shipping normally goes through CI: push to `main` (or open a pull request
  for a preview) and the workflows deploy and report the result to GitHub.
- When you deploy directly with the MCP tool `deploy_app` instead, report the
  result to GitHub yourself, so the commit shows it like a CI deploy. After
  `rollout_status` answers `ready` (or `failed`), with the GitHub CLI:

  ```sh
  id=$(gh api repos/{owner}/{repo}/deployments -f ref="$(git rev-parse HEAD)" \
    -f environment=production -F auto_merge=false -F required_contexts[] --jq .id)
  gh api repos/{owner}/{repo}/deployments/$id/statuses \
    -f state=success -f environment_url="https://<hostname from rollout_status>"
  ```

  Use `state=failure` (and no URL) when the rollout failed, and the
  environment the workflows use (`production`, or `preview` for a pull
  request). Skip this when GitHub access is not available, and say so.
