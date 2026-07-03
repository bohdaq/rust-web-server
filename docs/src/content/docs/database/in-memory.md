---
title: In-memory SQLite
description: Use DbPool::memory() for zero-config isolated SQLite databases ideal for tests and prototyping.
---

The `model-sqlite` feature ships an ergonomic constructor that opens a SQLite `":memory:"` database without a file path, hostname, or credentials. All operations are `async fn`.

## DbPool::memory() — isolated in-memory database

`DbPool::memory().await` creates a `sqlx::Pool` with `max_connections = 1` connected to `sqlite::memory:`. Each call returns a **separate, independent** in-memory database — ideal for tests that must not share state.

```rust
use rust_web_server::model::{DbPool, Value};

let pool = DbPool::memory().await.unwrap();

pool.execute(
    "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)",
    &[],
).await.unwrap();

pool.execute(
    "INSERT INTO items (name) VALUES (?)",
    &[Value::Text("apple".into())],
).await.unwrap();

let rows = pool.query_rows("SELECT name FROM items", &[]).await.unwrap();
assert_eq!(1, rows.len());
let name: String = rows[0].get("name").unwrap();
assert_eq!("apple", name);
```

## Test isolation

Each `DbPool::memory().await` call is a new, empty database. Two pools have no shared state:

```rust
async fn test_db() -> DbPool {
    let pool = DbPool::memory().await.unwrap();
    pool.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)", &[]).await.unwrap();
    pool
}

// Test A
let pool_a = test_db().await;
pool_a.execute("INSERT INTO users (name) VALUES (?)", &[Value::Text("Alice".into())]).await.unwrap();

// Test B — completely isolated
let pool_b = test_db().await;
let rows = pool_b.query_rows("SELECT * FROM users", &[]).await.unwrap();
assert!(rows.is_empty());  // Alice is only in pool_a's database
```

Use `#[tokio::test]` for async tests:

```rust
#[tokio::test]
async fn test_example() {
    let pool = DbPool::memory().await.unwrap();
    pool.execute("CREATE TABLE t (v TEXT)", &[]).await.unwrap();
    let rows = pool.query_rows("SELECT * FROM t", &[]).await.unwrap();
    assert!(rows.is_empty());
}
```

## Feature requirement

`DbPool::memory()` requires the `model-sqlite` feature, which also implies `http2` (tokio runtime):

```toml
[dependencies]
rust-web-server = { version = "17", features = ["model-sqlite"] }
```
