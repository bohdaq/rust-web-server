---
title: Project layout and rws.app.toml
description: The conventional layout of an rws application — where routes, models, jobs, migrations, templates and tests go — and the rws.app.toml manifest that names the app and any path that differs.
---

Since 18.28.0 an rws application has one conventional layout, so a person or
a coding agent can find (and add) a route, a model or a migration without
reading the project first. The [starters](/getting-started/ai-agents/#start-from-a-starter)
use it; the [dev MCP endpoint](/mcp/dev-tools/)'s `layout` tool reports it.

```text
my-app/
├── rws.app.toml          the manifest: the app's name, paths that differ
├── Cargo.toml
├── src/
│   ├── main.rs           wiring only: settings, dev_mcp, Server::run_tls
│   ├── lib.rs            app(): the routes and middleware, built in a function
│   ├── routes/           typed handlers (#[get] …), one module per resource
│   │   ├── mod.rs
│   │   └── posts.rs
│   ├── models/           #[derive(Model)] types, one module per table
│   └── jobs/             background jobs and scheduled tasks
├── migrations/           0001_create_posts.sql, 0001_create_posts.down.sql, …
├── templates/            Tera templates
├── tests/                integration tests against app() (TestClient)
└── static/               files served as they are
```

| Role | Path | What goes there |
|---|---|---|
| `main` | `src/main.rs` | Wiring only: `Server::setup()`, `app().dev_mcp_with(…)`, `Server::run_tls`. Nothing a test needs. |
| `lib` | `src/lib.rs` | `pub fn app() -> …` building every route and layer, so `tests/` can call it; the state type; `dev_tools()` |
| `routes` | `src/routes/` | One module per resource: its [typed handlers](/building-apps/routing/#typed-handlers-get-with-extractors) and a `register(app) -> app` that adds them (`app.route(list).route(show)`) |
| `models` | `src/models/` | [`#[derive(Model)]`](/database/model-derive/) types, one module per table |
| `jobs` | `src/jobs/` | [Jobs](/features/jobs/) and scheduled tasks |
| `migrations` | `migrations/` | `NNNN_name.sql`, applied in order, each with an optional `NNNN_name.down.sql` for rollback ([migrations](/database/migrations/)) |
| `templates` | `templates/` | [Tera templates](/building-apps/templates/) |
| `tests` | `tests/` | Integration tests: `TestClient::new(my_app::app())` |
| `static` | `static/` | Static files |

A binary-only crate can't be tested from `tests/`, hence `src/lib.rs`: the
binary is a few lines that serve what the library builds.

```rust
// src/main.rs
use rust_web_server::dev_mcp::DevMcpExt;
use rust_web_server::server::Server;

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().expect("server setup");
    let app = my_app::app().dev_mcp_with(my_app::dev_tools());
    Server::run_tls(listener, pool, app).await;
}
```

```rust
// src/lib.rs
pub mod routes;

use rust_web_server::dev_mcp::DevTools;
use rust_web_server::prelude::*;
use rust_web_server::state::AppWithState;

#[derive(Clone, Default)] // the server clones the app per worker
pub struct Db {
    pub posts: std::collections::HashMap<u64, String>, // or a DbPool
}

pub fn app() -> AppWithState<Db> {
    routes::posts::register(App::with_state(Db::default()))
}

pub fn dev_tools() -> DevTools {
    DevTools::new().app_dir(env!("CARGO_MANIFEST_DIR"))
}
```

```rust
// src/routes/posts.rs
use rust_web_server::error::AppError;
use rust_web_server::handler::{Json, Path};
use rust_web_server::state::AppWithState;
use rust_web_server::get;

use crate::Db;

#[get("/posts/{id}")]
pub fn show(Path(id): Path<u64>, db: &Db) -> Result<Json<String>, AppError> {
    db.posts.get(&id).cloned().map(Json).ok_or_else(|| AppError::NotFound(format!("no post {id}")))
}

pub fn register(app: AppWithState<Db>) -> AppWithState<Db> {
    app.route(show)
}
```

## rws.app.toml

The manifest sits at the project root. It names the app and says where the
project differs from the layout; every key is optional, so an empty file —
or none — means "conventional":

```toml
[app]
name = "blog"              # default: Cargo.toml's [package] name, else the directory's

[paths]
static = "frontend/dist"   # only what differs; the rest keep their defaults
```

| Key | Default |
|---|---|
| `[app] name` | `[package] name` from `Cargo.toml`, else the directory's name |
| `[paths] main` | `src/main.rs` |
| `[paths] lib` | `src/lib.rs` |
| `[paths] routes` | `src/routes` |
| `[paths] models` | `src/models` |
| `[paths] jobs` | `src/jobs` |
| `[paths] migrations` | `migrations` |
| `[paths] templates` | `templates` |
| `[paths] tests` | `tests` |
| `[paths] static` | `static` |

- Values are TOML strings (`"…"` or `'…'`); `# comments` are allowed;
  `paths.static = "…"` at the top level is the same as the key in `[paths]`.
- Paths are relative to the project root and stay inside it: a leading `/`,
  a drive letter or a `..` is an error, as is a key set twice. Errors name
  the line: ``rws.app.toml line 3: `routes` = "/srv/x" is absolute; paths
  are relative to the project root``.
- Unknown keys and tables are warnings, not errors, so a manifest written
  for a newer release still loads.
- The manifest describes the project; rws doesn't move files or change
  routing because of it. The `mcp` starter, whose "routes" are MCP tools,
  says `routes = "src/tools"`.

Read it in code with `app_manifest::AppManifest`:

```rust
use rust_web_server::app_manifest::AppManifest;

let manifest = AppManifest::load(env!("CARGO_MANIFEST_DIR"))?; // conventional when absent
println!("{} keeps its models in {}", manifest.name, manifest.paths.models);
for entry in manifest.layout(env!("CARGO_MANIFEST_DIR")) {
    println!("{:<10} {:<14} {}", entry.role, entry.path, if entry.exists { "ok" } else { "missing" });
}
```

`AppManifest::parse(text)` (or `text.parse::<AppManifest>()`) reads a
manifest from a string; files over 64 KiB are refused.
