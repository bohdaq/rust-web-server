---
title: Database Overview
description: Async SQLite, PostgreSQL, and/or MySQL — even more than one at once — with a built-in connection pool via sqlx.
---

The database layer in `rust-web-server` is a JPA/Hibernate-style async ORM backed by [`sqlx`](https://github.com/launchbadge/sqlx). All database operations are `async fn` and require a tokio runtime, which is automatically included when any `model-*` feature is enabled (they all imply `http2`).

## Feature flags

| Flag | Driver | Notes |
|---|---|---|
| `model-sqlite` | `sqlx/sqlite` | Best for development and embedded use; implies `http2` |
| `model-postgres` | `sqlx/postgres` | Standard PostgreSQL driver; implies `http2` |
| `model-mysql` | `sqlx/mysql` | MySQL and MariaDB; implies `http2` |

Add the flag to `Cargo.toml`:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["model-sqlite"] }
```

Unlike most feature pairs in this crate, `model-sqlite`, `model-postgres`, and `model-mysql` are **not** mutually exclusive — enable more than one and each gets its own [`DbPool`](#multiple-backends-in-one-binary) variant, so a single binary can hold pools to more than one backend (e.g. SQLite for hot-path data, PostgreSQL for an analytics tier).

## Environment variables

`DbConfig::from_env()` reads the following variables:

| Variable | Default | Notes |
|---|---|---|
| `RWS_DB_BACKEND` | Inferred if exactly one `model-*` feature is compiled in | `"sqlite"` / `"postgres"` / `"mysql"`; **required** if more than one `model-*` feature is compiled in |
| `RWS_DB_HOST` | `localhost` | Ignored for SQLite |
| `RWS_DB_PORT` | `5432` | Ignored for SQLite |
| `RWS_DB_USER` | — | Ignored for SQLite |
| `RWS_DB_PASSWORD` | — | Ignored for SQLite |
| `RWS_DB_NAME` | **required** | File path for SQLite; use `:memory:` for in-memory |
| `RWS_DB_POOL_SIZE` | `10` | Maximum number of connections in the pool |

## Quick start

The typical startup sequence is: read config, create pool, run migrations, then serve requests. All pool operations are `async fn`.

```rust
use rust_web_server::model::{DbConfig, DbPool, Value};

#[tokio::main]
async fn main() {
    // 1. Build config from environment variables.
    let config = DbConfig::from_env().expect("database config");

    // 2. Create the async connection pool.
    let pool = DbPool::new(config).await.expect("connection pool");

    // 3. Run pending migrations.
    pool.migrate("migrations/").await.expect("migrations");

    // 4. Use the pool in request handlers (pool is Clone).
    let users: Vec<User> = pool
        .query("SELECT * FROM users WHERE active = ?", &[Value::Bool(true)])
        .await
        .expect("query");

    println!("active users: {}", users.len());
}
```

`DbPool` wraps a `sqlx::Pool` and is cheap to clone — pass it by value into your `AppWithState` state.

## Manual config

If you prefer not to use environment variables, build `DbConfig` directly. `backend` selects which compiled-in driver this config is for — it only needs to be explicit when more than one `model-*` feature is enabled; with just one, `DbConfig::from_env()` (above) infers it for you, but a manually-built `DbConfig` always needs it stated:

```rust
use rust_web_server::model::{Backend, DbConfig};

let config = DbConfig {
    backend: Backend::Postgres,
    host: "localhost".into(),
    port: 5432,
    user: "app".into(),
    password: "secret".into(),
    database: "myapp".into(),
    pool_size: 5,
};
```

For SQLite, only `database` (and `backend`) matter — `host`/`port`/`user`/`password` are ignored, so any placeholder value works:

```rust
let config = DbConfig {
    backend: Backend::Sqlite,
    database: "app.db".into(),   // or ":memory:"
    pool_size: 5,
    host: String::new(),
    port: 0,
    user: String::new(),
    password: String::new(),
};
```

## Multiple backends in one binary

Enable more than one `model-*` feature and each gets its own [`DbPool`](#pool-behaviour) variant — build one `DbConfig`/`DbPool` per backend, each with its own `backend`:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["model-sqlite", "model-postgres"] }
```

```rust
use rust_web_server::model::{Backend, DbConfig, DbPool};

// Hot-path data in local SQLite:
let hot = DbPool::new(DbConfig {
    backend: Backend::Sqlite,
    database: "hot.db".into(),
    pool_size: 5,
    host: String::new(), port: 0, user: String::new(), password: String::new(),
}).await.expect("sqlite pool");

// Analytics tier in PostgreSQL:
let analytics = DbPool::new(DbConfig {
    backend: Backend::Postgres,
    host: "analytics-db.internal".into(),
    port: 5432,
    user: "app".into(),
    password: "secret".into(),
    database: "analytics".into(),
    pool_size: 10,
}).await.expect("postgres pool");
```

Both pools are ordinary `DbPool`s — pass whichever one a given handler needs through your app state (e.g. as separate fields, or in a small struct: `struct Pools { hot: DbPool, analytics: DbPool }`). `pool.backend()` reports which backend a given `DbPool` is talking to, if you ever need to branch on it at runtime.

When reading config from the environment instead, set `RWS_DB_BACKEND` before each call to `DbConfig::from_env()` (or build two separate env namespaces and read them independently) — with more than one `model-*` feature compiled in, `from_env()` returns `Err` rather than guessing which backend you meant.

## Pool behaviour

`DbPool::new(config).await` creates a `sqlx::Pool` with `max_connections = config.pool_size` for whichever backend `config.backend` names. sqlx manages the connection lifecycle internally — connections are acquired on demand and returned to the pool automatically after each `await` point. There is no `get()`/`PooledConnection` API; just `pool.execute(...)` and `pool.query_rows(...)` directly.

Under the hood, `DbPool` is an enum with one variant per compiled-in backend (`Sqlite`/`Postgres`/`MySql`) — `execute`, `query_rows`, `begin`, and every other method dispatch to the matching variant internally, so calling code never needs to match on it itself unless it specifically wants to know which backend it's holding (`pool.backend()`).

## What's next

- [#[derive(Model)]](/database/model-derive/) — map structs to tables
- [Repository](/database/repository/) — CRUD operations
- [Query Builder](/database/query-builder/) — filtered queries
- [Raw SQL](/database/raw-sql/) — full control
- [Transactions](/database/transactions/) — atomic operations
- [Migrations](/database/migrations/) — schema versioning
- [Relations](/database/relations/) — HasMany, HasOne, BelongsTo
