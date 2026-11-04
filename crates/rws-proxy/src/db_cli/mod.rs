//! `rws db migrate|rollback|status|seed|schema` (18.30.0, #91): the
//! project's migrations and seeds from the command line — the same
//! operations, and for `status` / `schema` the same JSON, as the dev MCP
//! server's `migrations`, `migrations_run`, `migrations_rollback_last` and
//! `db_schema` tools (they share `dev_mcp::db`).
//!
//! - **Database**: `--database-url <url>`, else `DATABASE_URL`, else the
//!   `RWS_DB_*` variables `DbPool::from_env` reads. URLs are sqlx's:
//!   `sqlite:app.db` (`?mode=rwc` creates the file), `postgres://…`,
//!   `mysql://…`.
//! - **Migrations**: the `migrations` path of the project's `rws.app.toml`
//!   (`migrations/` without one), under `--dir` (default `.`);
//!   `--migrations <dir>` overrides it.
//! - **Seeds**: `seeds/` beside the migrations directory (`migrations/` →
//!   `seeds/`, `db/migrations/` → `db/seeds/`); `--seeds <dir>` overrides
//!   it. Every `*.sql` file runs, in name order, each in its own
//!   transaction, every time — see `DbPool::seed`. An app's own binary can
//!   offer the same command with seeders written in Rust after the SQL
//!   files: [`run_with_seeders`].
//! - **Schema**: `schema` prints every table's columns, primary key,
//!   indexes and foreign keys (`DbPool::schema`); `--json` as the dev MCP
//!   server's `db_schema` answers it.
//!
//! The `rws` binary has the command in every build, but it only reaches a
//! database when built with a `model-*` feature (`cargo install
//! rust-web-server --features model-sqlite,model-postgres,model-mysql`);
//! otherwise it says so and exits with [`EXIT_USAGE`].

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::app_manifest::AppManifest;
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
use crate::model::DbPool;

#[cfg(test)]
mod tests;

/// Done.
pub const EXIT_OK: i32 = 0;
/// The database or a migration failed (connection, SQL error, missing
/// `.down.sql`, unreadable directory): see the message.
pub const EXIT_FAILED: i32 = 1;
/// Bad arguments, or a build without a database driver.
pub const EXIT_USAGE: i32 = 2;

const USAGE: &str = "\
Usage: rws db <command> [--database-url <url>] [--dir <project>]
                        [--migrations <dir>] [--seeds <dir>] [--json]

Commands:
  migrate        apply every pending migration, each in its own transaction
  rollback [N]   undo the last N applied migrations (default 1) with their .down.sql files
  status         list the migrations in order: applied or pending, and whether each has a .down.sql
  seed           run every *.sql seed file, in name order, each in its own transaction
  schema         the tables: columns, primary key, indexes, foreign keys (--json: as data)

Database: --database-url (sqlite:app.db, postgres://user:pass@host/db,
mysql://user:pass@host/db), else DATABASE_URL, else RWS_DB_BACKEND, RWS_DB_NAME,
RWS_DB_HOST, RWS_DB_PORT, RWS_DB_USER, RWS_DB_PASSWORD.
Migrations: the `migrations` path of rws.app.toml under --dir (migrations/ by
default). Seeds: seeds/ beside the migrations directory (db/migrations ->
db/seeds). Seeds run again every time: write them so they can.
--json prints the result (or {\"error\": …}) for agents.
Exit codes: 0 done, 1 database or migration error, 2 usage.
";

/// `rws db …` asked for: the arguments after `db`.
pub fn requested(args: &[String]) -> Option<&[String]> {
    (args.get(1).map(String::as_str) == Some("db")).then(|| &args[2..])
}

/// What to do.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Migrate,
    Rollback(usize),
    Status,
    Seed,
    Schema,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Cli {
    command: Command,
    database_url: Option<String>,
    dir: PathBuf,
    migrations: Option<PathBuf>,
    seeds: Option<PathBuf>,
    json: bool,
}

enum Parsed {
    Help,
    Run(Cli),
}

/// Run `rws db <args>` (the arguments after `db`), writing to `out` and
/// `err`; answers the exit code ([`EXIT_OK`], [`EXIT_FAILED`],
/// [`EXIT_USAGE`]).
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    run_seeding(args, Seeding::default(), out, err)
}

/// As [`run`], with `seeders` run after the SQL seed files by `seed`
/// (18.30.0) — for an app's own binary, so `myapp db seed` runs the seeds
/// written in Rust too (the `rws` binary can't link them):
///
/// ```rust,ignore
/// let args: Vec<String> = std::env::args().collect();
/// if let Some(db_args) = rust_web_server::db_cli::requested(&args) {
///     let code = rust_web_server::db_cli::run_with_seeders(db_args, &myapp::seeders(), &mut std::io::stdout(), &mut std::io::stderr());
///     std::process::exit(code);
/// }
/// ```
///
/// With seeders, a missing seeds directory means no SQL seeds rather than
/// an error (`DbPool::seed_with`).
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
pub fn run_with_seeders(args: &[String], seeders: &crate::model::Seeders, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    run_seeding(args, Seeding { seeders: Some(seeders.clone()) }, out, err)
}

/// The Rust seeders `seed` runs, if any.
#[derive(Default)]
struct Seeding {
    #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
    seeders: Option<crate::model::Seeders>,
}

fn run_seeding(args: &[String], seeding: Seeding, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    let cli = match parse(args) {
        Ok(Parsed::Help) => {
            let _ = write!(out, "{USAGE}");
            return EXIT_OK;
        }
        Ok(Parsed::Run(cli)) => cli,
        Err(message) => return fail(&format!("{message}\n(rws db --help lists the commands)"), &message, json, EXIT_USAGE, out, err),
    };
    let (migrations, seeds) = match directories(&cli) {
        Ok(dirs) => dirs,
        Err(message) => return fail(&message, &message, json, EXIT_FAILED, out, err),
    };
    execute(&cli, &migrations, &seeds, seeding, out, err)
}

fn fail(text: &str, message: &str, json: bool, code: i32, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if json {
        let _ = writeln!(out, r#"{{"error":{}}}"#, quoted(message));
    } else {
        let _ = writeln!(err, "rws db: {text}");
    }
    code
}

fn quoted(s: &str) -> String {
    format!("\"{}\"", crate::core::json::escape(s))
}

fn parse(args: &[String]) -> Result<Parsed, String> {
    let mut positional: Vec<String> = Vec::new();
    let mut cli = Cli { command: Command::Status, database_url: None, dir: PathBuf::from("."), migrations: None, seeds: None, json: false };
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        i += 1;
        if arg == "-h" || arg == "--help" || (arg == "help" && positional.is_empty()) {
            return Ok(Parsed::Help);
        }
        if !arg.starts_with("--") {
            positional.push(arg.to_string());
            continue;
        }
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) => (f, Some(v.to_string())),
            None => (arg, None),
        };
        let mut value = |name: &str| -> Result<String, String> {
            if let Some(v) = inline.clone() {
                return Ok(v);
            }
            let v = args.get(i).cloned().ok_or_else(|| format!("{name} needs a value"))?;
            i += 1;
            Ok(v)
        };
        match flag {
            "--json" => cli.json = true,
            "--database-url" => cli.database_url = Some(value("--database-url")?),
            "--dir" => cli.dir = PathBuf::from(value("--dir")?),
            "--migrations" => cli.migrations = Some(PathBuf::from(value("--migrations")?)),
            "--seeds" => cli.seeds = Some(PathBuf::from(value("--seeds")?)),
            other => return Err(format!("unknown option {other}")),
        }
    }
    let Some(command) = positional.first() else {
        return Err("name a command: migrate, rollback, status, seed or schema".to_string());
    };
    let extra = |at: usize| positional.get(at).map(|extra| format!("rws db {command} takes no `{extra}`"));
    cli.command = match command.as_str() {
        "migrate" | "status" | "seed" | "schema" => {
            if let Some(e) = extra(1) {
                return Err(e);
            }
            match command.as_str() {
                "migrate" => Command::Migrate,
                "status" => Command::Status,
                "schema" => Command::Schema,
                _ => Command::Seed,
            }
        }
        "rollback" => {
            if let Some(e) = extra(2) {
                return Err(e);
            }
            let n = match positional.get(1) {
                None => 1,
                Some(n) => match n.parse::<usize>() {
                    Ok(n) if n > 0 => n,
                    _ => return Err(format!("rws db rollback {n}: give how many migrations to undo, a number from 1")),
                },
            };
            Command::Rollback(n)
        }
        other => return Err(format!("unknown command `{other}`: use migrate, rollback, status, seed or schema")),
    };
    Ok(Parsed::Run(cli))
}

/// The migrations and seeds directories for `cli`.
fn directories(cli: &Cli) -> Result<(String, String), String> {
    let migrations = match &cli.migrations {
        Some(dir) => dir.clone(),
        None => {
            let manifest = AppManifest::load(&cli.dir).map_err(|e| e.to_string())?;
            cli.dir.join(&manifest.paths.migrations)
        }
    };
    let seeds = match &cli.seeds {
        Some(dir) => dir.clone(),
        None => seeds_beside(&migrations),
    };
    Ok((path_text(&migrations), path_text(&seeds)))
}

/// `seeds/` in the directory that holds `migrations`.
fn seeds_beside(migrations: &Path) -> PathBuf {
    let trimmed: PathBuf = migrations.components().collect();
    match trimmed.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join("seeds"),
        _ => PathBuf::from("seeds"),
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(not(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql")))]
fn execute(cli: &Cli, _migrations: &str, _seeds: &str, _seeding: Seeding, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let message = "this rws was built without a database driver — install one with them: \
                   cargo install rust-web-server --features model-sqlite,model-postgres,model-mysql";
    fail(message, message, cli.json, EXIT_USAGE, out, err)
}

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
fn execute(cli: &Cli, migrations: &str, seeds: &str, seeding: Seeding, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (command, url) = (cli.command.clone(), cli.database_url.clone().or_else(|| std::env::var("DATABASE_URL").ok().filter(|u| !u.trim().is_empty())));
    let (migrations, seeds) = (migrations.to_string(), seeds.to_string());
    let result = crate::dev_mcp::db::run(async move {
        let pool = match &url {
            Some(url) => DbPool::connect(url).await?,
            None => DbPool::from_env().await.map_err(|e| {
                crate::model::DbError::new(format!("{} (or give --database-url / DATABASE_URL)", e.0))
            })?,
        };
        let answer = answer(&pool, &command, &migrations, &seeds, seeding.seeders.as_ref()).await;
        close(&pool).await;
        answer
    });
    match result {
        Ok(json) => {
            if cli.json {
                let _ = writeln!(out, "{json}");
            } else {
                let _ = write!(out, "{}", render(&cli.command, &json));
            }
            EXIT_OK
        }
        Err(message) => fail(&message, &message, cli.json, EXIT_FAILED, out, err),
    }
}

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
async fn close(pool: &DbPool) {
    pool.close().await;
}

/// The command's result as JSON: `status` is the dev MCP server's
/// `migrations` answer; `migrate` adds `"applied"` (the versions this run
/// applied) to it; `rollback` is `{"rolled_back":[…]}`, `seed`
/// `{"directory":…,"seeded":[…]}` (SQL files, then Rust seeders), `schema`
/// the dev MCP server's `db_schema` answer.
#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
async fn answer(
    pool: &DbPool,
    command: &Command,
    migrations: &str,
    seeds: &str,
    seeders: Option<&crate::model::Seeders>,
) -> Result<String, crate::model::DbError> {
    let list = |items: &[String]| format!("[{}]", items.iter().map(|v| quoted(v)).collect::<Vec<_>>().join(","));
    match command {
        Command::Status => crate::dev_mcp::db::status_json(pool, migrations).await,
        Command::Migrate => {
            let pending: Vec<String> =
                pool.migration_status(migrations).await?.into_iter().filter(|m| !m.applied).map(|m| m.version).collect();
            pool.migrate(migrations).await?;
            let status = crate::dev_mcp::db::status_json(pool, migrations).await?;
            // {"directory":…,"migrations":[…]} → {"applied":[…],"directory":…,…}
            Ok(format!(r#"{{"applied":{},{}"#, list(&pending), &status[1..]))
        }
        Command::Rollback(n) => {
            let undone = pool.rollback(migrations, *n).await?;
            Ok(format!(r#"{{"directory":{},"rolled_back":{}}}"#, quoted(migrations), list(&undone)))
        }
        Command::Seed => {
            let ran = match seeders {
                Some(seeders) => pool.seed_with(seeds, seeders).await?,
                None => pool.seed(seeds).await?,
            };
            Ok(format!(r#"{{"directory":{},"seeded":{}}}"#, quoted(seeds), list(&ran)))
        }
        Command::Schema => crate::dev_mcp::db::schema_json(pool).await,
    }
}

/// Text for a person, from the JSON [`answer`] built.
#[cfg_attr(not(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql")), allow(dead_code))]
fn render(command: &Command, json: &str) -> String {
    use crate::mcp::json_rpc::{extract_raw, extract_str, split_array_elements};
    let elements = |key: &str| extract_raw(json, key).map(|raw| split_array_elements(raw.trim())).unwrap_or_default();
    let strings = |key: &str| -> Vec<String> {
        elements(key)
            .iter()
            .map(|v| v.trim().trim_matches('"').replace("\\\"", "\"").replace("\\\\", "\\"))
            .collect()
    };
    let lines = |items: Vec<String>, none: &str, prefix: &str| -> String {
        if items.is_empty() {
            format!("{none}\n")
        } else {
            items.iter().map(|v| format!("{prefix}{v}\n")).collect()
        }
    };
    match command {
        Command::Migrate => lines(strings("applied"), "nothing to apply: every migration is applied", "applied  "),
        Command::Rollback(_) => lines(strings("rolled_back"), "nothing to roll back: no migration is applied", "rolled back  "),
        Command::Seed => lines(strings("seeded"), "no seed files", "seeded  "),
        Command::Schema => render_schema(json),
        Command::Status => {
            let migrations = elements("migrations");
            if migrations.is_empty() {
                return "no migrations\n".to_string();
            }
            migrations
                .iter()
                .map(|m| {
                    let version = extract_str(m, "version").unwrap_or_default();
                    let applied = m.contains(r#""applied":true"#);
                    let down = if m.contains(r#""has_down":true"#) { "" } else { "  (no .down.sql)" };
                    format!("{}  {version}{down}\n", if applied { "applied" } else { "pending" })
                })
                .collect()
        }
    }
}

/// `schema` for a person: each table, its columns (type, `NOT NULL`,
/// default, `PK`), indexes and foreign keys.
#[cfg_attr(not(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql")), allow(dead_code))]
fn render_schema(json: &str) -> String {
    use crate::mcp::json_rpc::{extract_raw, extract_str, split_array_elements};
    let elements = |json: &str, key: &str| extract_raw(json, key).map(|raw| split_array_elements(raw.trim())).unwrap_or_default();
    let names = |json: &str, key: &str| -> String {
        elements(json, key).iter().map(|v| v.trim().trim_matches('"').to_string()).collect::<Vec<_>>().join(", ")
    };
    let tables = elements(json, "tables");
    if tables.is_empty() {
        return "no tables\n".to_string();
    }
    let mut out = String::new();
    for table in tables {
        out.push_str(&format!("{}\n", extract_str(&table, "name").unwrap_or_default()));
        for column in elements(&table, "columns") {
            let mut line = format!("  {}  {}", extract_str(&column, "name").unwrap_or_default(), extract_str(&column, "type").unwrap_or_default());
            if column.contains(r#""nullable":false"#) {
                line.push_str("  NOT NULL");
            }
            if let Some(default) = extract_str(&column, "default") {
                line.push_str(&format!("  DEFAULT {default}"));
            }
            if column.contains(r#""primary_key":true"#) {
                line.push_str("  PK");
            }
            out.push_str(&line);
            out.push('\n');
        }
        for index in elements(&table, "indexes") {
            let kind = if index.contains(r#""primary":true"#) {
                "primary key"
            } else if index.contains(r#""unique":true"#) {
                "unique index"
            } else {
                "index"
            };
            out.push_str(&format!("  {kind} {} ({})\n", extract_str(&index, "name").unwrap_or_default(), names(&index, "columns")));
        }
        for key in elements(&table, "foreign_keys") {
            let references = extract_raw(&key, "references").unwrap_or_default();
            out.push_str(&format!(
                "  foreign key ({}) references {} ({}) on delete {}\n",
                names(&key, "columns"),
                extract_str(&references, "table").unwrap_or_default(),
                names(&references, "columns"),
                extract_str(&key, "on_delete").unwrap_or_default()
            ));
        }
    }
    out
}
