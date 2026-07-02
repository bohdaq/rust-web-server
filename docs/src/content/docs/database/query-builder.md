---
title: Query Builder
description: Use QueryBuilder to construct typed SELECT, COUNT, DELETE, and UPDATE queries without writing SQL.
---

`QueryBuilder<T>` is obtained from `T::query(&mut conn)` and provides a fluent API for building queries against the table mapped to `T`. All methods consume `self` and return a new `QueryBuilder`, making the chain easy to compose.

## Obtaining a builder

```rust
use rust_web_server::model::{DbConfig, DbConnection};

let config = DbConfig::from_env()?;
let mut conn = DbConnection::open(&config)?;

// QueryBuilder<User> — tied to the `users` table
let qb = User::query(&mut conn);
```

## Filtering

### Equality filter

`where_eq(col, val)` adds a `col = ?` condition. The placeholder is `?` for SQLite and MySQL, `$N` for PostgreSQL — the builder handles the substitution automatically.

```rust
let admins: Vec<User> = User::query(&mut conn)
    .where_eq("role", "admin")
    .fetch_all()?;
```

Chain multiple calls to AND conditions together:

```rust
let result = User::query(&mut conn)
    .where_eq("role", "admin")
    .where_eq("active", true)
    .fetch_all()?;
```

### Raw filter

`filter(expr, params)` accepts a raw SQL fragment and a `Vec<Value>`. Use `?` as the placeholder regardless of backend — the builder converts to `$N` for PostgreSQL automatically.

```rust
use rust_web_server::model::Value;

let adults = User::query(&mut conn)
    .filter("age >= ?", vec![Value::Int(18)])
    .fetch_all()?;
```

## Ordering

```rust
use rust_web_server::model::Order;

let recent = User::query(&mut conn)
    .order_by("created_at", Order::Desc)
    .fetch_all()?;

let alphabetical = User::query(&mut conn)
    .order_by("name", Order::Asc)
    .fetch_all()?;
```

## Pagination

`.limit(n)` and `.offset(n)` map directly to SQL `LIMIT` and `OFFSET`.

```rust
let page = 2u64;
let page_size = 20u64;

let users = User::query(&mut conn)
    .order_by("id", Order::Asc)
    .limit(page_size)
    .offset((page - 1) * page_size)
    .fetch_all()?;
```

## Fetching results

| Method | SQL | Return type |
|---|---|---|
| `fetch_all()` | `SELECT * FROM … WHERE … ORDER BY … LIMIT … OFFSET …` | `Result<Vec<T>, DbError>` |
| `fetch_one()` | same with `LIMIT 1` | `Result<Option<T>, DbError>` |
| `count()` | `SELECT COUNT(*) FROM … WHERE …` | `Result<i64, DbError>` |

```rust
// all matching rows
let users: Vec<User> = User::query(&mut conn)
    .where_eq("active", true)
    .fetch_all()?;

// first match only
let user: Option<User> = User::query(&mut conn)
    .where_eq("email", "alice@example.com")
    .fetch_one()?;

// count without loading rows
let total: i64 = User::query(&mut conn)
    .where_eq("role", "admin")
    .count()?;
```

## Mutation

### Delete matching rows

```rust
User::query(&mut conn)
    .where_eq("active", false)
    .delete()?;
```

### Update a single column

`update(col, val)` issues `UPDATE table SET col = ? WHERE …`. Combine with filters to scope the update.

```rust
User::query(&mut conn)
    .where_eq("id", 42i64)
    .update("role", "moderator")?;
```

## Placeholder rules

The builder transparently converts between placeholder styles:

- **SQLite / MySQL** — `?`
- **PostgreSQL** — `$1`, `$2`, … (auto-numbered from left to right)

You never need to pick a style; write `?` in raw `.filter()` expressions and the builder takes care of the rest.

## Complete example: paginated list endpoint

```rust
use rust_web_server::app::App;
use rust_web_server::model::{DbConfig, DbConnection, Order};
use rust_web_server::request::Request;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use rust_web_server::extract::Query;
use rust_web_server::routes;
use std::collections::HashMap;
use std::sync::Arc;

fn list_users(
    req: &Request,
    _params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &Arc<()>,
) -> Response {
    // Parse ?page=N&per_page=N from query string
    let qs: HashMap<String, String> = req.request_uri
        .split_once('?')
        .map(|(_, q)| url_decode_pairs(q))
        .unwrap_or_default();

    let page: u64 = qs.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
    let per_page: u64 = qs.get("per_page").and_then(|s| s.parse().ok()).unwrap_or(20).min(100);

    let config = DbConfig::from_env().unwrap();
    let mut db = DbConnection::open(&config).unwrap();

    let total = User::query(&mut db)
        .where_eq("active", true)
        .count()
        .unwrap_or(0);

    let users = User::query(&mut db)
        .where_eq("active", true)
        .order_by("created_at", Order::Desc)
        .limit(per_page)
        .offset((page - 1) * per_page)
        .fetch_all()
        .unwrap_or_default();

    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    // serialize `users` and `total` into the response body as needed
    r
}
```

:::note[Zero rows is not an error]
`fetch_all()` returns an empty `Vec` when no rows match — it only returns `Err` on a real database error. Similarly, `fetch_one()` returns `Ok(None)` for no match.
:::
