---
title: Migrations
description: Manage schema changes with versioned SQL files that run once, are tracked in the database, and can be rolled back.
---

The migration runner reads `*.sql` files from a directory in lexicographic order, executes any that have not already been applied, and records each run in a `_schema_migrations` table. No external tools or frameworks are required.

## How it works

1. `pool.migrate("migrations/").await` creates `_schema_migrations(version TEXT PRIMARY KEY, applied_at TEXT)` if it does not already exist.
2. It reads every `*.sql` file in the directory sorted lexicographically by filename.
3. Files whose name is already in `_schema_migrations` are skipped.
4. Each unapplied file is executed inside a `BEGIN` / `COMMIT` transaction. If the file fails, the transaction is rolled back and `migrate` returns `Err` immediately — subsequent files are not attempted.

## File naming convention

Prefix files with a zero-padded sequence number so lexicographic order matches execution order:

```
migrations/
  001_create_users.sql
  002_add_email_index.sql
  003_create_posts.sql
  004_add_posts_status_column.sql
```

The full filename (without the directory prefix) is used as the version key, so renaming a file that has already been applied will cause it to run again. Never rename applied migration files.

## Example migration files

```sql
-- migrations/001_create_users.sql
CREATE TABLE users (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    name    TEXT    NOT NULL,
    email   TEXT    NOT NULL UNIQUE,
    active  INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);
```

```sql
-- migrations/002_add_email_index.sql
CREATE INDEX idx_users_email ON users (email);
```

```sql
-- migrations/003_create_posts.sql
CREATE TABLE posts (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER NOT NULL REFERENCES users(id),
    title      TEXT    NOT NULL,
    body       TEXT    NOT NULL,
    status     TEXT    NOT NULL DEFAULT 'draft',
    created_at TEXT    NOT NULL
);
```

```sql
-- migrations/004_add_posts_status_column.sql
ALTER TABLE posts ADD COLUMN published_at TEXT;
```

## Running migrations

Call `migrate` once at server startup, before the application begins accepting requests:

```rust
use rust_web_server::model::{DbConfig, DbPool};

#[tokio::main]
async fn main() {
    let config = DbConfig::from_env().expect("database config");
    let pool = DbPool::new(config).await.expect("connection pool");

    pool.migrate("migrations/").await.expect("migrations failed");

    // start the server ...
}
```

If all migrations have already been applied, `migrate` is a no-op and returns immediately.

## Checking migration status

`pool.migration_status(dir).await` returns a `Vec<MigrationStatus>` — one entry per SQL file in the directory — without executing anything. Use it for health checks, admin endpoints, or CLI tooling.

```rust
use rust_web_server::model::MigrationStatus;

let statuses: Vec<MigrationStatus> = pool.migration_status("migrations/").await?;

for s in &statuses {
    let state = if s.applied { "applied" } else { "pending" };
    println!("{}: {}", s.version, state);
}

// Check if any migrations are pending
let pending = statuses.iter().any(|s| !s.applied);
if pending {
    eprintln!("Warning: unapplied migrations exist");
}
```

`MigrationStatus` fields:

| Field | Type | Description |
|---|---|---|
| `version` | `String` | Filename used as the version key |
| `applied` | `bool` | `true` if the migration has been run |
| `has_down` | `bool` | `true` if a companion `.down.sql` file exists, meaning this migration can be rolled back |

## Rolling back

Add a companion **down file** next to any up migration you want to be reversible: take the up file's name and replace its `.sql` extension with `.down.sql`.

```
migrations/
  001_create_users.sql
  001_create_users.down.sql
  002_add_email_index.sql
  002_add_email_index.down.sql
```

```sql
-- migrations/001_create_users.down.sql
DROP TABLE users;
```

```sql
-- migrations/002_add_email_index.down.sql
DROP INDEX idx_users_email;
```

`pool.rollback_last(dir).await` undoes the single most recently applied migration: it runs the down file's SQL and deletes the migration's row from `_schema_migrations`, both inside one transaction — rolled back together if the down SQL fails. "Most recently applied" is the highest version string among applied migrations, the same lexicographic order `migrate` uses to apply them.

```rust
use rust_web_server::model::DbPool;

let pool = DbPool::from_env().await.expect("connection pool");

match pool.rollback_last("migrations/").await {
    Ok(Some(version)) => println!("rolled back {}", version),
    Ok(None) => println!("nothing to roll back"),
    Err(e) => eprintln!("rollback failed: {}", e),
}
```

`pool.rollback(dir, n).await` rolls back up to the last `n` applied migrations, most recent first, stopping early (without error) once nothing is left to undo:

```rust
// Roll back the last 3 migrations, in reverse order they were applied.
let rolled_back = pool.rollback("migrations/", 3).await.expect("rollback failed");
for version in &rolled_back {
    println!("rolled back {}", version);
}
```

Both methods return `Err` if the migration they're trying to undo has no companion `.down.sql` file — rollback is opt-in per migration, not automatic. Migrations already rolled back before such a failure stay rolled back; each step commits independently.

:::caution[Down migrations are not automatically the inverse]
`rust-web-server` does not generate down SQL for you — you write it, the same way you write the up file. Get it right by hand: `DROP TABLE` undoes `CREATE TABLE`, `DROP COLUMN` undoes `ADD COLUMN`, and so on. A down file that doesn't actually reverse its up file will leave the schema in an inconsistent state relative to `_schema_migrations`.
:::

## Startup pattern

A typical async server startup sequence:

```rust
use rust_web_server::model::{DbConfig, DbPool};
use rust_web_server::app::App;
use rust_web_server::server::Server;

#[tokio::main]
async fn main() {
    let config = DbConfig::from_env().expect("db config");

    // Create pool and run migrations
    let pool = DbPool::new(config).await.expect("connection pool");
    pool.migrate("migrations/").await.expect("migrations");

    // Pass the pool (Clone) into your app state
    let app = App::with_async_state(std::sync::Arc::new(pool))
        // register routes ...
        ;

    Server::new().run_tls(app).await;
}
```

:::note[One migration per file]
Each file should contain a single logical change. Splitting changes across many small files makes it easy to roll back a specific step and keeps the history readable.
:::

:::note[Idempotent DDL]
Prefer `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` so that rerunning a migration file manually (during development) does not error. The runner itself skips already-applied files, but the guard costs nothing and prevents accidents.
:::
