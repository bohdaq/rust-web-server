---
title: Query Builder
description: Use QueryBuilder to construct typed async SELECT, COUNT, DELETE, and UPDATE queries without writing SQL.
---

`QueryBuilder<T>` is obtained from `T::query(&pool)` and provides a fluent API for building queries against the table mapped to `T`. All builder methods consume `self` and return a new `QueryBuilder`. Terminal methods (`fetch_all`, `fetch_one`, `count`, `delete`, `update`) are `async fn`.

## Obtaining a builder

```rust
use rust_web_server::model::DbPool;

let pool = DbPool::from_env().await?;

// QueryBuilder<User> — tied to the `users` table
let qb = User::query(&pool);
```

## Filtering

### Equality filter

`where_eq(col, val)` adds a `col = ?` condition. The placeholder is `?` for SQLite and MySQL, `$N` for PostgreSQL — the builder handles the substitution automatically.

```rust
let admins: Vec<User> = User::query(&pool)
    .where_eq("role", "admin")
    .fetch_all().await?;
```

Chain multiple calls to AND conditions together:

```rust
let result = User::query(&pool)
    .where_eq("role", "admin")
    .where_eq("active", true)
    .fetch_all().await?;
```

### Raw filter

`filter(expr, params)` accepts a raw SQL fragment and a `Vec<Value>`. Use `?` as the placeholder regardless of backend — the builder converts to `$N` for PostgreSQL automatically.

```rust
use rust_web_server::model::Value;

let adults = User::query(&pool)
    .filter("age >= ?", vec![Value::Int(18)])
    .fetch_all().await?;
```

## Ordering

```rust
use rust_web_server::model::Order;

let recent = User::query(&pool)
    .order_by("created_at", Order::Desc)
    .fetch_all().await?;

let alphabetical = User::query(&pool)
    .order_by("name", Order::Asc)
    .fetch_all().await?;
```

## Pagination

`.limit(n)` and `.offset(n)` map directly to SQL `LIMIT` and `OFFSET`, for full manual control:

```rust
let page = 2u64;
let page_size = 20u64;

let users = User::query(&pool)
    .order_by("id", Order::Asc)
    .limit(page_size)
    .offset((page - 1) * page_size)
    .fetch_all().await?;
```

For most list endpoints, `.paginate(page, per_page)` (offset-based, with total counts) and `.paginate_after(cursor, per_page)` (cursor/keyset-based, for large tables) do the `COUNT(*)` + `LIMIT`/`OFFSET` bookkeeping above for you and return a `Page<T>`/`CursorPage<T>` — see [Pagination](/database/pagination/) for both, plus a built-in `Link` response header builder.

## Fetching results

| Method | SQL | Return type |
|---|---|---|
| `fetch_all().await` | `SELECT * FROM … WHERE … ORDER BY … LIMIT … OFFSET …` | `Result<Vec<T>, DbError>` |
| `fetch_one().await` | same with `LIMIT 1` | `Result<Option<T>, DbError>` |
| `count().await` | `SELECT COUNT(*) FROM … WHERE …` | `Result<i64, DbError>` |

```rust
// all matching rows
let users: Vec<User> = User::query(&pool)
    .where_eq("active", true)
    .fetch_all().await?;

// first match only
let user: Option<User> = User::query(&pool)
    .where_eq("email", "alice@example.com")
    .fetch_one().await?;

// count without loading rows
let total: i64 = User::query(&pool)
    .where_eq("role", "admin")
    .count().await?;
```

## Mutation

### Delete matching rows

```rust
User::query(&pool)
    .where_eq("active", false)
    .delete().await?;
```

### Update a single column

`update(col, val).await` issues `UPDATE table SET col = ? WHERE …`. Combine with filters to scope the update.

```rust
User::query(&pool)
    .where_eq("id", 42i64)
    .update("role", "moderator").await?;
```

## Placeholder rules

The builder transparently converts between placeholder styles:

- **SQLite / MySQL** — `?`
- **PostgreSQL** — `$1`, `$2`, … (auto-numbered from left to right)

You never need to pick a style; write `?` in raw `.filter()` expressions and the builder takes care of the rest.

## Complete example: paginated list endpoint

```rust
use rust_web_server::header::Header;
use rust_web_server::model::{DbPool, Order, Page};
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};
use std::sync::Arc;

async fn list_users(pool: Arc<DbPool>, page: u64, per_page: u64) -> Response {
    let per_page = per_page.min(100);

    let page: Page<User> = User::query(&pool)
        .where_eq("active", true)
        .order_by("created_at", Order::Desc)
        .paginate(page, per_page).await
        .unwrap_or_else(|_| Page::new(vec![], page, per_page, 0));

    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    if let Some(link) = page.link_header("https://api.example.com/users") {
        r.headers.push(Header { name: "Link".to_string(), value: link });
    }
    // serialize `page.items`, `page.total_items`, `page.total_pages` into the response body
    r
}
```

See [Pagination](/database/pagination/) for `.paginate_after()` (cursor/keyset pagination) and more on the `Link` header.

:::note[Zero rows is not an error]
`fetch_all().await` returns an empty `Vec` when no rows match — it only returns `Err` on a real database error. Similarly, `fetch_one().await` returns `Ok(None)` for no match.
:::
