---
title: Migrations
description: Manage schema changes with versioned SQL files that run once, are tracked in the database, and can be rolled back.
---

The migration runner reads `*.sql` files from a directory in lexicographic order, executes any that have not already been applied, and records each run in a `_schema_migrations` table. No external tools or frameworks are required.

## How it works

1. `pool.migrate("migrations/").await` creates `_schema_migrations(version VARCHAR(255) PRIMARY KEY, applied_at VARCHAR(64) NOT NULL)` if it does not already exist (`VARCHAR` since 18.30: MySQL can't key a `TEXT` column; a table created as `TEXT` by an earlier release keeps working).
2. It reads every `*.sql` file in the directory sorted lexicographically by filename.
3. Files whose name is already in `_schema_migrations` are skipped.
4. Each unapplied file is executed inside a `BEGIN` / `COMMIT` transaction, one statement at a time — the file is split on `;`, so a `;` inside a string literal or a trigger/function body breaks it. If the file fails, the transaction is rolled back and `migrate` returns `Err` immediately — subsequent files are not attempted.

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

## Seeds

`pool.seed("seeds/").await` runs every `*.sql` file in the directory, in name order, each in its own transaction (split at `;`, like migrations), and answers the files it ran. Nothing records that a seed ran: it runs again every time, so write seeds that can — `INSERT … ON CONFLICT DO NOTHING` (PostgreSQL, SQLite), `INSERT OR IGNORE` (SQLite), `INSERT IGNORE` (MySQL). A failing file is rolled back and stops the run.

```sql
-- seeds/01_roles.sql
INSERT INTO roles (name) VALUES ('admin') ON CONFLICT DO NOTHING;
INSERT INTO roles (name) VALUES ('member') ON CONFLICT DO NOTHING;
```

The convention is a `seeds/` directory beside the migrations directory: `migrations/` → `seeds/`, `db/migrations/` → `db/seeds/`.

## From the command line: `rws db`

`rws db` runs the same operations on the project in the working directory (or `--dir`), with the migrations directory from its [`rws.app.toml`](/building-apps/project-layout/) and the seeds beside it:

```bash
rws db status                    # applied / pending, and which have a .down.sql
rws db migrate                   # apply every pending migration
rws db rollback                  # undo the last one; `rws db rollback 3` the last three
rws db seed                      # run seeds/*.sql
rws db migrate --database-url postgres://app:secret@db/app --json
```

The database is `--database-url`, else `DATABASE_URL`, else the `RWS_DB_*` variables (see [the overview](/database/overview/#environment-variables)); URLs are sqlx's — `sqlite:app.db` (`sqlite:app.db?mode=rwc` creates the file), `postgres://user:password@host:5432/db`, `mysql://user:password@host:3306/db`. `--migrations <dir>` and `--seeds <dir>` override the directories. `--json` prints the result for agents: `status` is the dev MCP server's `migrations` answer (`{"directory":…,"migrations":[{"version":…,"applied":…,"has_down":…}]}`), `migrate` adds `"applied"` (the versions this run applied), `rollback` answers `{"directory":…,"rolled_back":[…]}`, `seed` `{"directory":…,"seeded":[…]}`, and a failure `{"error":…}`. Exit codes: `0` done, `1` a database or migration error (including a missing `.down.sql`), `2` bad arguments.

The command needs a `rws` built with a database driver — `cargo install rust-web-server --features model-sqlite,model-postgres,model-mysql`; the release binaries have none, and say so.

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

    let (listener, thread_pool) = Server::setup().expect("server setup");
    Server::run_tls(listener, thread_pool, app).await;
}
```

:::note[One migration per file]
Each file should contain a single logical change. Splitting changes across many small files makes it easy to roll back a specific step and keeps the history readable.
:::

:::note[Idempotent DDL]
Prefer `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` so that rerunning a migration file manually (during development) does not error. The runner itself skips already-applied files, but the guard costs nothing and prevents accidents.
:::
