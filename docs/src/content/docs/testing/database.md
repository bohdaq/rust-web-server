---
title: Test databases and factories
description: TestDb gives each test a database that leaves nothing behind; #[derive(Factory)] builds models with default values.
---

Two helpers for tests that touch the database (since 18.29.0, `model-*` features): `TestDb`, a database per test that leaves nothing behind, and `#[derive(Factory)]`, which builds `#[derive(Model)]` structs with a default for every field.

## TestDb

```rust
use rust_web_server::test_client::TestDb;

#[tokio::test]
async fn notes_start_empty() {
    let db = TestDb::sqlite().migrations("migrations").await.unwrap();
    db.execute("INSERT INTO notes (text) VALUES ('hi')", &[]).await.unwrap();
    // db derefs to DbPool; db.pool() is a clone for the app's state
}
```

`TestDb::sqlite()` (and the other constructors) return a builder: `.migrations(dir)` adds a directory of `*.sql` migrations (call it again for more; relative to the package root under `cargo test`), and `.await` — or `.build().await` — connects and migrates. A failing migration is the `Err`.

| Backend | Constructor | How the test is isolated |
|---|---|---|
| SQLite | `TestDb::sqlite()` | a fresh in-memory database per `TestDb`, migrations applied; gone with the last clone of its pool. Tests run in parallel |
| PostgreSQL | `TestDb::postgres(url)`, `TestDb::postgres_from_env()` | migrations applied for real (once per process and directory); then a pool of **one connection** that starts a transaction as it connects. Nothing is committed: `db.rollback().await` rolls it back, and dropping the `TestDb` closes the connection, which the server rolls back |
| MySQL | `TestDb::mysql(url)`, `TestDb::mysql_from_env()` | the same as PostgreSQL |

The `_from_env` forms read `RWS_TEST_POSTGRES_URL` / `RWS_TEST_MYSQL_URL` (sqlx URLs, e.g. `postgres://user:password@localhost/app_test`). When the variable isn't set they print `skipped: …` and return `None`, so a test runs where a server is available and returns early elsewhere:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn orders_against_postgres() {
    let Some(db) = TestDb::postgres_from_env() else { return };
    let db = db.migrations("migrations").await.unwrap();
    // …
}
```

Limits of the transaction, the usual ones for transactional tests:

- Give tests a database of their own: the schema (migrations) persists, only data is rolled back.
- One connection: code holding two connections at once (two transactions alive together) waits for itself. `DbPool::begin`/`transaction` do work — sqlx makes them savepoints inside the test's transaction, so their `commit` commits nothing.
- Raw `BEGIN`/`COMMIT` statements end the test's transaction, and so does DDL on MySQL (`CREATE`, `ALTER`, `DROP`, `TRUNCATE` commit implicitly). Keep schema changes in migrations.
- On PostgreSQL a failed statement aborts the transaction, and every later statement in the test fails; run statements that are expected to fail inside `DbPool::transaction` (a savepoint).

### An app on a TestDb

Hand `db.pool()` to the app's state and drive it with `TestClient` as usual. For an `App::with_async_state` app use `send_async().await`, which awaits the handler on the test's runtime:

```rust
use rust_web_server::app::App;
use rust_web_server::test_client::{TestClient, TestDb};

#[tokio::test]
async fn counts_notes() {
    let db = TestDb::sqlite().migrations("migrations").await.unwrap();
    let client = TestClient::new(my_app::app(db.pool()));
    let response = client.get("/notes").send_async().await;
    assert_eq!(200, response.status());
}
```

`send()` would run the async handler on a runtime made for that request and dropped after it, as the server's blocking pool does. Tasks the handler leaves behind never run there — sqlx returns a pool connection with such a task, so the connection is closed instead, taking a `TestDb`'s in-memory database with it — and a single-threaded test runtime waiting in `send()` can't run its own tasks either.

## Factories

Add `Factory` next to `Model`:

```rust
use rust_web_server::{Factory, Model};

#[derive(Model, Factory, Debug, Clone)]
#[table(name = "users")]
pub struct User {
    #[primary_key(auto_increment)]
    pub id: i64,
    pub name: String,      // "name-1", "name-2", …
    pub email: String,     // "email-1@example.com", …
    pub age: Option<i32>,  // None
    pub admin: bool,       // false
}
```

`User::factory()` returns a `ModelFactory<User>`:

```rust
let unsaved = User::factory().build();
let ann = User::factory().with(|u| u.name = "Ann".into()).create(&db).await?;
assert!(ann.id > 0); // read back after the INSERT
let admins = User::factory().with(|u| u.admin = true).create_many(3, &db).await?;
let five = User::factory().build_many(5);
```

| Method | What it does |
|---|---|
| `.with(\|m\| …)` | change each built instance (after the defaults and earlier `with`s) |
| `.build()` / `.build_many(n)` | instances, not saved |
| `.create(&pool).await` | one instance `INSERT`ed (whatever its key) and read back with its id |
| `.create_many(n, &pool).await` | `n` of them |

Defaults per field type: numbers `0`, `bool` `false`, `Option` `None`, `String` the field's name and a sequence number (with `@example.com` for a field whose name contains `email`), so unique columns stay unique. An `auto_increment` key is `0` (the database picks it); any other key is the sequence number. `#[ignore]` fields, and field types with no `FactoryValue` impl, get `Default::default()` — implement `test_client::factory::FactoryValue` for your own column types to choose their default.

The sequence is per model type and shared by every test in the process, in whatever order they run: assert on the values a factory returns rather than on `name-1`.

`#[derive(Factory)]` needs `rws-macros` 17.30 (the `macros` feature) and reads the same attributes as `#[derive(Model)]`.
