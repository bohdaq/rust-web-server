---
title: Migrations
description: Manage schema changes with versioned SQL files that run once and are tracked in the database.
---

The migration runner reads `*.sql` files from a directory in lexicographic order, executes any that have not already been applied, and records each run in a `_schema_migrations` table. No external tools or frameworks are required.

## How it works

1. `conn.migrate("migrations/")` creates `_schema_migrations(version TEXT PRIMARY KEY, applied_at TEXT)` if it does not already exist.
2. It reads every `*.sql` file in the directory sorted lexicographically by filename.
3. Files whose name is already in `_schema_migrations` are skipped.
4. Each unapplied file is executed in a `BEGIN` / `COMMIT` transaction. If the file fails, the transaction is rolled back and `migrate` returns `Err` immediately — subsequent files are not attempted.

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
use rust_web_server::model::{DbConfig, DbConnection};

fn main() {
    let config = DbConfig::from_env().expect("database config");
    let mut conn = DbConnection::open(&config).expect("database connection");

    conn.migrate("migrations/").expect("migrations failed");

    // start the server ...
}
```

If all migrations have already been applied, `migrate` is a no-op and returns immediately.

## Checking migration status

`conn.migration_status(dir)` returns a `Vec<MigrationStatus>` — one entry per SQL file in the directory — without executing anything. Use it for health checks, admin endpoints, or CLI tooling.

```rust
use rust_web_server::model::MigrationStatus;

let statuses: Vec<MigrationStatus> = conn.migration_status("migrations/")?;

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

## Startup pattern

A typical server startup sequence:

```rust
use rust_web_server::model::{DbConfig, DbPool};
use rust_web_server::app::App;
use rust_web_server::server::Server;

fn main() {
    let db_config = DbConfig::from_env().expect("db config");

    // Run migrations using a dedicated connection
    {
        let mut conn = rust_web_server::model::DbConnection::open(&db_config)
            .expect("migration connection");
        conn.migrate("migrations/").expect("migrations");
    }

    // Create a pool for request handlers
    let pool = DbPool::new(&db_config, db_config.pool_size)
        .expect("connection pool");

    let app = App::with_state(std::sync::Arc::new(pool))
        // register routes ...
        ;

    Server::new().run(app);
}
```

:::note[One migration per file]
Each file should contain a single logical change. Splitting changes across many small files makes it easy to roll back a specific step and keeps the history readable.
:::

:::note[Idempotent DDL]
Prefer `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` so that rerunning a migration file manually (during development) does not error. The runner itself skips already-applied files, but the guard costs nothing and prevents accidents.
:::
