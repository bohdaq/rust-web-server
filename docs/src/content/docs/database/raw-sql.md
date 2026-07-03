---
title: Raw SQL
description: Execute hand-written SQL queries and map results to typed structs or untyped rows.
---

The query builder covers most common queries, but sometimes you need raw SQL — complex joins, CTEs, window functions, or database-specific syntax. `DbPool` exposes three async methods for this.

## Typed query: `query::<T>`

`pool.query::<T>(sql, params).await` executes a SQL statement and deserialises each row into `T` using `T::from_row`. `T` must implement `Model` (typically via `#[derive(Model)]`).

```rust
use rust_web_server::model::{DbPool, Value};

let pool = DbPool::from_env().await?;

let users: Vec<User> = pool.query::<User>(
    "SELECT * FROM users WHERE role = ? AND active = ?",
    &[Value::Text("admin".into()), Value::Bool(true)],
).await?;
```

For PostgreSQL replace `?` with `$1`, `$2`, etc.:

```rust
let users: Vec<User> = pool.query::<User>(
    "SELECT * FROM users WHERE role = $1 AND active = $2",
    &[Value::Text("admin".into()), Value::Bool(true)],
).await?;
```

## Untyped query: `query_raw`

`pool.query_raw(sql, params).await` returns `Vec<ModelRow>` — column-name/value pairs without deserialisation. Use this for ad-hoc queries, reporting, or when no matching `Model` type exists.

```rust
let rows = pool.query_raw(
    "SELECT u.name, COUNT(p.id) AS post_count \
     FROM users u \
     LEFT JOIN posts p ON p.user_id = u.id \
     GROUP BY u.id, u.name \
     ORDER BY post_count DESC",
    &[],
).await?;

for row in &rows {
    let name: String = row.get::<String>("name")?;
    let count: i64  = row.get::<i64>("post_count")?;
    println!("{name}: {count} posts");
}
```

## Execute: `execute`

`pool.execute(sql, params).await` runs INSERT, UPDATE, DELETE, or DDL statements and returns the number of rows affected.

```rust
let affected: u64 = pool.execute(
    "UPDATE users SET last_login = ? WHERE id = ?",
    &[Value::Text("2026-07-02T10:00:00Z".into()), Value::Int(42)],
).await?;
println!("{affected} row(s) updated");
```

## Extracting columns from `ModelRow`

`ModelRow::get::<T>(col)` performs typed column extraction by name (case-insensitive). It returns `Result<T, DbError>`.

```rust
let name: String       = row.get::<String>("name")?;
let age: i64           = row.get::<i64>("age")?;
let score: f64         = row.get::<f64>("score")?;
let verified: bool     = row.get::<bool>("verified")?;
let avatar: Vec<u8>    = row.get::<Vec<u8>>("avatar")?;
let bio: Option<String> = row.get::<Option<String>>("bio")?;
```

## The `Value` enum

All parameters and raw column values are represented as `Value`:

| Variant | Rust type | Use for |
|---|---|---|
| `Value::Null` | — | SQL NULL |
| `Value::Bool(bool)` | `bool` | boolean columns |
| `Value::Int(i64)` | integer types | `INTEGER`, `BIGINT`, `SMALLINT` |
| `Value::Float(f64)` | float types | `REAL`, `DOUBLE PRECISION` |
| `Value::Text(String)` | `String` / `&str` | `TEXT`, `VARCHAR`, dates as strings |
| `Value::Bytes(Vec<u8>)` | `Vec<u8>` | `BLOB`, `BYTEA` |

```rust
use rust_web_server::model::Value;

let params = &[
    Value::Text("alice@example.com".into()),
    Value::Int(30),
    Value::Bool(true),
    Value::Null,                               // e.g. an optional field
];
```

## The `FromColumn` trait

Built-in `FromColumn` implementations cover `i16`, `i32`, `i64`, `u32`, `u64`, `f32`, `f64`, `bool`, `String`, `Vec<u8>`, and `Option<T>` for any `T: FromColumn`.

Implement it for custom types to enable `row.get::<MyType>("col")`:

```rust
use rust_web_server::model::{FromColumn, Value, DbError};

#[derive(Debug)]
pub enum UserRole { Admin, Member, Guest }

impl FromColumn for UserRole {
    fn from_column(v: &Value) -> Result<Self, DbError> {
        match v {
            Value::Text(s) => match s.as_str() {
                "admin"  => Ok(UserRole::Admin),
                "member" => Ok(UserRole::Member),
                "guest"  => Ok(UserRole::Guest),
                other    => Err(DbError::new(format!("unknown role: {other}"))),
            },
            other => Err(DbError::new(format!("expected Text for UserRole, got {other:?}"))),
        }
    }
}

// Usage
let role: UserRole = row.get::<UserRole>("role")?;
```

## When to use raw SQL vs QueryBuilder

| Situation | Prefer |
|---|---|
| Simple equality filters, pagination, ordering | `QueryBuilder` |
| Multi-table joins | Raw SQL |
| CTEs (`WITH …`) | Raw SQL |
| Window functions (`OVER PARTITION BY …`) | Raw SQL |
| Database-specific functions (`DATE_TRUNC`, `JSON_EXTRACT`, …) | Raw SQL |
| INSERT / UPDATE / DELETE with complex expressions | Raw SQL via `execute` |
| Schema mutations (DDL) | Raw SQL via `execute` |

:::note[Parameterise always]
Never interpolate user input directly into a SQL string. Always pass untrusted values through the `params` slice so the driver can prepare and bind them safely.
:::
