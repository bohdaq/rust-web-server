---
title: Pagination
description: Paginate query results with Page<T> (offset) and CursorPage<T> (keyset), plus a built-in Link header builder.
---

`QueryBuilder<T>` has two pagination terminals so list endpoints don't need to hand-roll `LIMIT`/`OFFSET` math or a separate `COUNT(*)` query every time: `.paginate()` for classic page-number pagination, and `.paginate_after()` for cursor (keyset) pagination on large or frequently-changing tables.

## Offset pagination — `.paginate(page, per_page)`

```rust
use rust_web_server::model::{Order, Page};

let page: Page<User> = User::query(&pool)
    .where_eq("active", true)
    .order_by("created_at", Order::Desc)
    .paginate(2, 20).await?;

page.items;        // Vec<User> — this page's rows
page.page;         // 2
page.per_page;     // 20
page.total_items;  // total matching rows (COUNT(*) with the same filters)
page.total_pages;  // ceil(total_items / per_page)
page.has_next();
page.has_prev();
```

`.paginate()` runs two queries — a `COUNT(*)` with the same filters (no limit/offset), and a `SELECT … LIMIT … OFFSET …` — then wraps both into a `Page<T>`. Any `.limit()`/`.offset()` set earlier in the chain is overridden; `page`/`per_page` are authoritative. `page` and `per_page` are clamped to a minimum of `1`.

## Cursor (keyset) pagination — `.paginate_after(cursor, per_page)`

Offset pagination gets slower page by page on large tables, since the database still has to scan and discard every skipped row before the offset. Keyset pagination avoids that by filtering on "rows after the last one you saw," at the cost of losing `total_items`/`total_pages` and the ability to jump to an arbitrary page.

```rust
use rust_web_server::model::CursorPage;

// First page: no cursor
let page: CursorPage<User> = User::query(&pool)
    .where_eq("active", true)
    .paginate_after(None, 20).await?;

page.items;         // Vec<User>
page.next_cursor;   // Some("123") if there's a next page, None if this was the last

// Next page: pass the previous page's cursor back
let next: CursorPage<User> = User::query(&pool)
    .where_eq("active", true)
    .paginate_after(page.next_cursor.as_deref(), 20).await?;
```

`.paginate_after()` orders by the primary key ascending — overriding any `.order_by()` set earlier, since keyset pagination requires ordering by the cursor column — and fetches `per_page + 1` rows in a single query to cheaply detect whether there's a next page, with no separate `COUNT(*)`. `next_cursor` is the last row's primary key, as a string; the primary key must be numeric, and `paginate_after` returns `Err` if a supplied cursor isn't a valid integer.

:::note[When to use which]
Use `.paginate()` for admin/dashboard UIs that show page numbers or a jump-to-page control. Use `.paginate_after()` for infinite-scroll feeds, high-volume APIs, or any table where frequent inserts make offset pagination's per-page scan cost (and the risk of skipping or repeating rows as data shifts underneath a multi-page fetch) a real problem.
:::

## `Link` response header (RFC 8288)

Both `Page<T>` and `CursorPage<T>` can build a standard `Link` header value, so a list endpoint can tell the client how to fetch the next/previous page without inventing its own pagination envelope:

```rust
use rust_web_server::response::Response;
use rust_web_server::header::Header;

let page: Page<User> = User::query(&pool).paginate(2, 20).await?;

let mut response = Response::new();
if let Some(link) = page.link_header("https://api.example.com/users") {
    response.headers.push(Header { name: "Link".to_string(), value: link });
}
// Link: <https://api.example.com/users?page=1&per_page=20>; rel="first",
//       <https://api.example.com/users?page=1&per_page=20>; rel="prev",
//       <https://api.example.com/users?page=3&per_page=20>; rel="next",
//       <https://api.example.com/users?page=5&per_page=20>; rel="last"
```

`link_header` adds (or overwrites) `page`/`per_page` query parameters on the URL you pass in, preserving any other existing query parameters (e.g. `?active=true`). It omits `rel="first"`/`"prev"` on the first page and `rel="next"`/`"last"` on the last page, and returns `None` if there's only one page or the URL fails to parse.

`CursorPage::link_header(base_url, cursor_param)` builds a single `rel="next"` entry:

```rust
let page: CursorPage<User> = User::query(&pool).paginate_after(None, 20).await?;
let link = page.link_header("https://api.example.com/users", "cursor");
// Some(r#"<https://api.example.com/users?cursor=142>; rel="next""#)
```

## Mapping to a DTO

`.map()` transforms `items` while keeping all pagination metadata — handy for converting a DB row type into an API response type right before serializing:

```rust
let page: Page<User> = User::query(&pool).paginate(1, 20).await?;
let dto_page: Page<UserDto> = page.map(UserDto::from);
```

## Using outside the model layer

`Page<T>` and `CursorPage<T>` aren't tied to `QueryBuilder` or any `model-*` feature — construct one by hand if your data source is something else (an external API, an in-memory `Vec`):

```rust
use rust_web_server::pagination::Page;

let all_items = vec!["a", "b", "c", "d", "e"];
let page = Page::new(all_items[2..4].to_vec(), 2, 2, all_items.len() as u64);
```
