# AGENTS.md

A full-stack app on [rust-web-server](https://rws8.tech) (rws): lit frontend,
JSON API, PostgreSQL, deployed to kube-dc. For coding agents working in this
project.

## Layout

- `src/main.rs`: the API (`app()`), the `Store` (PostgreSQL or in memory),
  `main`, tests. Build the app in a function, test the function.
- `migrations/`: SQL files applied in order at startup (`NNN_name.sql`,
  PostgreSQL dialect, `$1` placeholders). Never rename an applied file.
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

## rws in brief

Docs: https://rws8.tech, one file: https://rws8.tech/llms-full.txt.

- `App::with_async_state(state)` with `.get/.post/.put/.patch/.delete(pattern,
  |request, params, conn, state| async move { … })`. Patterns `/notes/{id}`;
  `params.get("id")`. State is `Arc`-shared and cloned per worker.
- Responses: `Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, bytes)`.
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
