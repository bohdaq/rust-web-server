---
title: Database Overview
description: Connect to SQLite, PostgreSQL, or MySQL with a built-in connection pool and zero third-party ORM dependencies.
---

The database layer in `rust-web-server` is a JPA/Hibernate-style ORM implemented from scratch. There are no third-party ORM dependencies. A single feature flag selects the backend driver; exactly one driver can be active per compilation unit.

## Feature flags

| Flag | Driver | Notes |
|---|---|---|
| `model-sqlite` | `rusqlite` (bundles libsqlite3) | Best for development and embedded use |
| `model-postgres` | `postgres` crate | Standard PostgreSQL driver |
| `model-mysql` | `mysql` crate | MySQL and MariaDB |

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
| `RWS_DB_POOL_SIZE` | `10` | Number of connections pre-created in the pool |

## Quick start

The typical startup sequence is: read config, create pool, run migrations, then serve requests.

```rust
use rust_web_server::model::{DbConfig, DbPool};

fn main() {
    // 1. Build config from environment variables.
    let config = DbConfig::from_env().expect("database config");

    // 2. Create the connection pool (opens pool_size connections eagerly).
    let pool = DbPool::new(config).expect("connection pool");

    // 3. Check out a connection and run pending migrations.
    {
        let mut conn = pool.get().expect("pool connection");
        conn.migrate("migrations/").expect("migrations");
    }

    // 4. Use the pool in request handlers.
    let mut conn = pool.get().expect("pool connection");
    let users: Vec<User> = conn
        .query("SELECT * FROM users WHERE active = ?", &[Value::Bool(true)])
        .expect("query");

    println!("active users: {}", users.len());
}
```

The `PooledConnection` guard is returned to the pool automatically when it goes out of scope — no explicit `release` call is needed.

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

`DbPool::new(config)` opens exactly `config.pool_size` connections at construction time. `pool.get()` pops a connection from an internal `Mutex<Vec<DbConnection>>`. If the pool is empty (all connections are checked out), a new connection is opened on demand. The connection is returned to the pool when the `PooledConnection` is dropped.

```rust
{
    let mut conn = pool.get()?;   // checked out
    // ... use conn ...
}                                  // returned to pool here
```

## What's next

- [#[derive(Model)]](/database/model-derive/) — map structs to tables
- [Repository](/database/repository/) — CRUD operations
- [Query Builder](/database/query-builder/) — filtered queries
- [Raw SQL](/database/raw-sql/) — full control
- [Transactions](/database/transactions/) — atomic operations
- [Migrations](/database/migrations/) — schema versioning
- [Relations](/database/relations/) — HasMany, HasOne, BelongsTo
