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

### Seeds in Rust

Rows that are easier to make in Rust — with [factories](/testing/database/), computed values, several tables at once — go in seeders (18.30.0). A seeder is a name and an async function of the pool; `pool.seed_with(dir, &seeders)` runs the `*.sql` files first, then the seeders in the order added, and answers what it ran:

```rust
use rust_web_server::model::{DbPool, Seeders};

pub fn seeders() -> Seeders {
    Seeders::new()
        .add("users", |pool: DbPool| async move {
            User::factory().create_many(10, &pool).await?;
            Ok(())
        })
        .add("admin", |pool: DbPool| async move {
            if User::query(&pool).where_eq("email", "admin@example.com").count().await? == 0 {
                User::factory().with(|u| { u.email = "admin@example.com".into(); u.admin = true; }).create(&pool).await?;
            }
            Ok(())
        })
}

let ran = pool.seed_with("seeds", &seeders()).await?; // ["01_roles.sql", "users", "admin"]
```

Like SQL seeds, seeders run every time, so write them to tolerate their rows being there (the `admin` one looks first). A seeder isn't wrapped in a transaction — use `pool.transaction(…)` in it when its rows must go in together — and a failing one stops the run with its name in the error. With seeders, a missing SQL directory means no SQL seeds rather than an error. Implement the `Seeder` trait instead of a closure when a seeder needs state of its own.

The `rws` binary can't run an app's Rust code, so `rws db seed` runs the SQL files only. Give the app's own binary the same command, with its seeders, through `db_cli::run_with_seeders`:

```rust
// src/main.rs
let args: Vec<String> = std::env::args().collect();
if let Some(db_args) = rust_web_server::db_cli::requested(&args) {
    let code = rust_web_server::db_cli::run_with_seeders(db_args, &myapp::seeders(), &mut std::io::stdout(), &mut std::io::stderr());
    std::process::exit(code);
}
```

`cargo run -- db seed` then runs `seeds/*.sql` and the seeders (`--json`: `{"directory":…,"seeded":["01_roles.sql","users","admin"]}`); every other `db` command works as `rws db`'s.

## From the command line: `rws db`

`rws db` runs the same operations on the project in the working directory (or `--dir`), with the migrations directory from its [`rws.app.toml`](/building-apps/project-layout/) and the seeds beside it:

```bash
rws db status                    # applied / pending, and which have a .down.sql
rws db migrate                   # apply every pending migration
rws db rollback                  # undo the last one; `rws db rollback 3` the last three
rws db seed                      # run seeds/*.sql
rws db schema                    # tables: columns, primary key, indexes, foreign keys
rws db migrate --database-url postgres://app:secret@db/app --json
```

The database is `--database-url`, else `DATABASE_URL`, else the `RWS_DB_*` variables (see [the overview](/database/overview/#environment-variables)); URLs are sqlx's — `sqlite:app.db` (`sqlite:app.db?mode=rwc` creates the file), `postgres://user:password@host:5432/db`, `mysql://user:password@host:3306/db`. `--migrations <dir>` and `--seeds <dir>` override the directories. `--json` prints the result for agents: `status` is the dev MCP server's `migrations` answer (`{"directory":…,"migrations":[{"version":…,"applied":…,"has_down":…}]}`), `migrate` adds `"applied"` (the versions this run applied), `rollback` answers `{"directory":…,"rolled_back":[…]}`, `seed` `{"directory":…,"seeded":[…]}`, and a failure `{"error":…}`. Exit codes: `0` done, `1` a database or migration error (including a missing `.down.sql`), `2` bad arguments.

## The schema as data

`pool.schema().await?` reads the connected schema from the database's catalog — SQLite's `sqlite_master` and `pragma_*` functions, PostgreSQL's `information_schema` and `pg_catalog` (the `current_schema()`), MySQL's `information_schema` (the `DATABASE()`) — into a `DbSchema` (18.30.0): every table (views left out, `_schema_migrations` included) with its columns (type as the backend spells it, nullability, default, whether it is in the primary key), the primary key in key order, the indexes (columns, `unique`, `primary`; SQLite has none for an `INTEGER PRIMARY KEY`) and the foreign keys (columns, referenced table and columns, `on_update`, `on_delete`). It reads the primary of a [replicated pool](/database/overview/#read-replicas).

```rust
let schema = pool.schema().await?;
let users = schema.table("users").expect("migrated");
assert_eq!(vec!["id".to_string()], users.primary_key);
for fk in &schema.table("orders").unwrap().foreign_keys {
    println!("{:?} -> {}({:?}) on delete {}", fk.columns, fk.references_table, fk.references_columns, fk.on_delete);
}
println!("{}", schema.to_json());
```

`to_json()` is what `rws db schema --json` prints and the dev MCP server's `db_schema` tool answers:

```json
{"backend":"sqlite","tables":[{"name":"orders",
  "columns":[{"name":"id","type":"INTEGER","nullable":true,"default":null,"primary_key":true},
             {"name":"user_id","type":"INTEGER","nullable":false,"default":null,"primary_key":false}],
  "primary_key":["id"],
  "indexes":[{"name":"orders_by_user","columns":["user_id"],"unique":false,"primary":false}],
  "foreign_keys":[{"name":null,"columns":["user_id"],"references":{"table":"users","columns":["id"]},"on_update":"NO ACTION","on_delete":"CASCADE"}]}]}
```

Without `--json`, `rws db schema` prints a table per block: `  user_id  INTEGER  NOT NULL`, `  index orders_by_user (user_id)`, `  foreign key (user_id) references users (id) on delete CASCADE`.

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
