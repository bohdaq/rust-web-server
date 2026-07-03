---
title: In-memory SQLite
description: Use DbPool::memory() and DbConnection::memory() for zero-config SQLite databases ideal for tests and prototyping.
---

The `model-sqlite` feature ships two ergonomic constructors that open a SQLite
`":memory:"` database without a file path, hostname, or credentials.

## When to use each

| Constructor | Scope | Typical use |
|---|---|---|
| `DbPool::memory()` | Shared — all `pool.get()` calls see the same database | App state shared across handlers; integration tests that need multiple steps on one dataset |
| `DbConnection::memory()` | Isolated — each call is a new, empty database | Unit tests that must not share state; one-shot scripts |

## DbPool::memory() — shared database

`DbPool::memory()` is equivalent to `DbPool::new(DbConfig::memory())`, which creates
`DbConfig { database: ":memory:", pool_size: 1, .. }` and opens one connection.

```rust
use rust_web_server::model::{DbPool, Value};

let pool = DbPool::memory().unwrap();

{
    let mut conn = pool.get().unwrap();
    conn.execute(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)",
        &[],
    ).unwrap();
    conn.execute(
        "INSERT INTO items (name) VALUES (?1)",
        &[Value::Text("apple".into())],
    ).unwrap();
}  // PooledConnection dropped → connection returned to pool

let mut conn = pool.get().unwrap();
let rows = conn.query_rows("SELECT name FROM items", &[]).unwrap();
assert_eq!(1, rows.len());
let name: String = rows[0].get("name").unwrap();
assert_eq!("apple", name);
```

### Pool exhaustion returns an error

When `pool_size = 1` and the single connection is already checked out, `pool.get()`
returns `Err` with a clear message instead of silently opening a new empty database
(which would discard all data written by the held connection).

```rust
let pool = DbPool::memory().unwrap();
let _held = pool.get().unwrap();   // the only connection is now checked out
let err = pool.get();
assert!(err.is_err());
assert!(err.unwrap_err().0.contains("exhausted"));
```

## DbConnection::memory() — isolated database

Every call to `DbConnection::memory()` returns a fresh connection to a brand-new
empty in-memory database. Two connections returned from successive calls have no
shared state.

```rust
use rust_web_server::model::{DbConnection, Value};

fn fresh_db() -> DbConnection {
    let mut conn = DbConnection::memory().unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
        &[],
    ).unwrap();
    conn
}

// Test A
let mut conn_a = fresh_db();
conn_a.execute("INSERT INTO users (name) VALUES (?1)", &[Value::Text("Alice".into())]).unwrap();

// Test B — completely isolated
let mut conn_b = fresh_db();
let rows = conn_b.query_rows("SELECT * FROM users", &[]).unwrap();
assert!(rows.is_empty());  // Alice is only in conn_a's database
```

## DbConfig::memory()

Both helpers ultimately use `DbConfig::memory()`:

```rust
use rust_web_server::model::DbConfig;

let cfg = DbConfig::memory();
// cfg.database == ":memory:"
// cfg.pool_size == 1
```

Build it directly when you need to inspect or pass the config separately, then
hand it to `DbPool::new(cfg)`.

## Feature requirement

All three constructors require the `model-sqlite` feature:

```toml
[dependencies]
rust-web-server = { version = "17", features = ["model-sqlite"] }
```
