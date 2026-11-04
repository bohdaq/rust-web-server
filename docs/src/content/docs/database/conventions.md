---
title: Keys, timestamps, soft delete and locking
description: String and UUID primary keys, created_at/updated_at, soft delete with restore, and optimistic locking with #[version] — maintained by the repository, on SQLite, PostgreSQL and MySQL.
---

Since 18.30.0 `#[derive(Model)]` and the repository handle four things apps otherwise write by hand: keys that aren't auto-increment integers, `created_at` / `updated_at`, soft delete, and optimistic locking. Each is opt-in per model, and none adds a callback or a lazy load: the repository writes the columns in the same statements it already runs.

```rust
use rust_web_server::model::{Timestamp, Uuid};
use rust_web_server::Model;

#[derive(Model, Debug, Clone)]
#[table(name = "articles")]
#[timestamps]   // created_at, updated_at
#[soft_delete]  // deleted_at
pub struct Article {
    #[primary_key]
    pub id: Uuid,                       // Uuid::NIL → save picks a fresh v7
    pub title: String,
    #[version]
    pub lock_version: i64,              // optimistic locking
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub deleted_at: Option<Timestamp>,
}
```

```sql
-- migrations/0001_create_articles.sql (PostgreSQL)
CREATE TABLE articles (
    id           TEXT PRIMARY KEY,
    title        TEXT NOT NULL,
    lock_version BIGINT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL,
    updated_at   TIMESTAMPTZ NOT NULL,
    deleted_at   TIMESTAMPTZ
);
```

The fields keep their names; `#[column(name = "…")]` on them still renames the column. A missing field is a compile error (`#[timestamps] needs a mapped field named updated_at`), and so is a `deleted_at` that isn't an `Option`.

## Primary keys

`ModelRepository<T, ID>` implements `Repository<T, ID>` for `ID` = `i64`, `i32`, `String` and `Uuid`. `T::repository(&pool)` is keyed by `i64` for an integer key of any width (as before 18.30), and by the field's type otherwise; for an `i32` key, `ModelRepository::<T, i32>::new(&pool)` takes `i32` ids.

What `save` does:

| Key | `save` |
|---|---|
| `#[primary_key(auto_increment)]`, value `0` | `INSERT`; the database picks the key |
| `#[primary_key(auto_increment)]`, any other value | `UPDATE` that row |
| `#[primary_key]` (any type) | looks the key up (soft-deleted rows included): `UPDATE` if the row is there, `INSERT` otherwise |
| `Uuid::NIL` (`Uuid::default()`) | `INSERT` with a new `Uuid::now_v7()` |
| empty `String` | an error |

The look-up and the write are two statements, so two processes saving the same new key at the same moment can both insert, and the second gets the database's unique-key error. `repo.insert(&entity)` always inserts.

```rust
let tags = Tag::repository(&pool);           // ModelRepository<Tag, String>
tags.save(&Tag { slug: "rust".into(), label: "Rust".into() }).await?;      // INSERT
tags.save(&Tag { slug: "rust".into(), label: "Rust lang".into() }).await?; // UPDATE
let rust = tags.find_by_id("rust".to_string()).await?;
```

`Uuid` is rws's own 16-byte type (not the `uuid` crate): `Uuid::now_v7()` (time-ordered, so new rows go to the end of the index), `Uuid::new_v4()`, `Uuid::parse(text)`, `Display` as the hyphenated form, `serde` as a string. It is stored as text: use a `TEXT`, `CHAR(36)` or `VARCHAR(36)` column. PostgreSQL `UUID` columns are read, but rws binds text, which PostgreSQL won't compare with a `UUID` column without a cast.

## Timestamps

With `#[timestamps]`, `save` sets `created_at` and `updated_at` to the current UTC time on insert, and `updated_at` on every update; `created_at` is never written again, whatever the entity holds. `QueryBuilder::update` sets `updated_at` too. The fields are [`Timestamp`](/reference/api/) (microseconds since the Unix epoch; `Timestamp::now()`, `parse`, `to_rfc3339`, `serde` as RFC 3339) — or `String`, which gets the stored text.

| Backend | Column type | What rws writes |
|---|---|---|
| SQLite | `TEXT` | `2026-11-04T12:34:56.123456Z` (RFC 3339, sorts as text) |
| PostgreSQL | `TIMESTAMPTZ` (or `TIMESTAMP`) | the same text, `CAST(… AS TIMESTAMPTZ)` |
| MySQL | `DATETIME(6)` | `2026-11-04 12:34:56.123456` (UTC, no zone) |

Reading gives the same RFC 3339 text on every backend (rws decodes PostgreSQL `TIMESTAMPTZ`/`TIMESTAMP`/`DATE` and MySQL `DATETIME`/`TIMESTAMP`/`DATE` itself), so `Timestamp` fields round-trip. MySQL `TIMESTAMP` columns convert to the session time zone; use `DATETIME(6)`, which stores what it is given.

In your own queries, bind a `Timestamp` with `ts.to_value(pool.backend())` — the MySQL form differs.

## Soft delete

With `#[soft_delete]` and a `deleted_at: Option<Timestamp>` field:

- `repo.delete_by_id(id)` and `QueryBuilder::delete()` set `deleted_at` instead of deleting (a row deleted already keeps its time);
- `find_by_id`, `find_all`, `count`, `exists_by_id`, every `QueryBuilder` operation and `HasMany` / `HasOne` / `BelongsTo` leave out rows with `deleted_at` set;
- `QueryBuilder::with_deleted()` includes them, `only_deleted()` lists only them;
- `repo.restore(id)` (`true` when the row was deleted) and `QueryBuilder::restore()` clear `deleted_at`;
- `repo.force_delete_by_id(id)` and `QueryBuilder::force_delete()` delete for real;
- `repo.find_by_id_with_deleted(id)` reads a row either way.

```rust
let repo = Article::repository(&pool);
repo.delete_by_id(article.id).await?;                     // soft
assert!(repo.find_by_id(article.id).await?.is_none());
let trash = Article::query(&pool).only_deleted().fetch_all().await?;
repo.restore(article.id).await?;
Article::query(&pool).only_deleted().force_delete().await?; // empty the trash
```

Raw SQL (`pool.query`, `pool.execute`) sees every row; add `WHERE deleted_at IS NULL` yourself.

## Optimistic locking

A `#[version]` integer field is `1` after the insert and one more after each update. `save` updates only while the row still has the version the entity was read with (`UPDATE … SET …, lock_version = lock_version + 1 WHERE id = ? AND lock_version = ?`); otherwise nothing is written and it fails with `DbErrorKind::StaleVersion`:

```rust
use rust_web_server::model::DbErrorKind;

match repo.save(&edited).await {
    Ok(saved) => saved,
    Err(e) if e.kind() == DbErrorKind::StaleVersion => {
        // Someone saved it since `edited` was read: reload and apply the change again,
        // or answer 409 Conflict.
        return conflict(&e.to_string());
    }
    Err(e) => return Err(e.into()),
}
```

A row that is gone gives `DbErrorKind::NotFound` instead. `QueryBuilder::update` increments the version as well (it doesn't check it), so copies read before a bulk update are stale afterwards.

## Tested on every backend

The model tests run on SQLite, PostgreSQL 16 and MySQL 8.4 in CI (the `model-databases` job). To run them against your own servers, set `RWS_TEST_POSTGRES_URL` / `RWS_TEST_MYSQL_URL` — each test makes a schema (PostgreSQL) or database (MySQL) of its own and drops it, so the MySQL user needs `CREATE` and `DROP`:

```bash
RWS_TEST_POSTGRES_URL=postgres://rws:rws@127.0.0.1/rws_test \
RWS_TEST_MYSQL_URL=mysql://root:rws@127.0.0.1/rws_test \
cargo test -p rws-server --features "http2 test-support macros model-sqlite model-postgres model-mysql" --lib -- model
```
