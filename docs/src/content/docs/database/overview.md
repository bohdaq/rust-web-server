---
title: Database Overview
description: Async SQLite, PostgreSQL, or MySQL with a built-in connection pool via sqlx.
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

Only one of `model-sqlite`, `model-postgres`, or `model-mysql` should be enabled at a time.

## Environment variables

`DbConfig::from_env()` reads the following variables:

| Variable | Default | Notes |
|---|---|---|
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

If you prefer not to use environment variables, build `DbConfig` directly:

```rust
use rust_web_server::model::DbConfig;

let config = DbConfig {
    host: "localhost".into(),
    port: 5432,
    user: "app".into(),
    password: "secret".into(),
    database: "myapp".into(),
    pool_size: 5,
};
```

For SQLite, only `database` matters:

```rust
let config = DbConfig {
    database: "app.db".into(),   // or ":memory:"
    pool_size: 5,
    ..Default::default()          // host/port/user/password are ignored
};
```

## Pool behaviour

`DbPool::new(config).await` creates a `sqlx::Pool` with `max_connections = config.pool_size`. sqlx manages the connection lifecycle internally — connections are acquired on demand and returned to the pool automatically after each `await` point. There is no `get()`/`PooledConnection` API; just `pool.execute(...)` and `pool.query_rows(...)` directly.

## What's next

- [#[derive(Model)]](/database/model-derive/) — map structs to tables
- [Repository](/database/repository/) — CRUD operations
- [Query Builder](/database/query-builder/) — filtered queries
- [Raw SQL](/database/raw-sql/) — full control
- [Transactions](/database/transactions/) — atomic operations
- [Migrations](/database/migrations/) — schema versioning
- [Relations](/database/relations/) — HasMany, HasOne, BelongsTo
