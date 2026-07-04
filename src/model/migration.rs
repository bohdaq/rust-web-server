//! Async migration runner.
//!
//! Reads `*.sql` files from a directory in lexicographic order. Creates
//! `_schema_migrations(version TEXT PRIMARY KEY, applied_at TEXT)` if absent.
//! Runs each unapplied file inside a transaction.
//!
//! Rollback: an up file `NNNN_name.sql` may have a companion down file
//! `NNNN_name.down.sql` in the same directory. [`rollback_last`] and
//! [`rollback`] undo the most recently applied migration(s) — determined by
//! version order, the same invariant `run_migrations` relies on to apply
//! files in order — by running the down file's SQL and deleting the row
//! from `_schema_migrations`, both inside one transaction.

use super::pool::DbPool;
use super::{DbError, Value};

// ── MigrationStatus ───────────────────────────────────────────────────────────

/// Status of a single migration file.
#[derive(Debug, Clone)]
pub struct MigrationStatus {
    /// The filename (without directory prefix), used as the version key.
    pub version: String,
    /// `true` if this migration has been applied.
    pub applied: bool,
    /// `true` if a companion `<version-stem>.down.sql` file exists, meaning
    /// this migration can be undone with [`rollback_last`] / [`rollback`].
    pub has_down: bool,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const CREATE_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS _schema_migrations \
     (version TEXT PRIMARY KEY, applied_at TEXT NOT NULL)";

async fn ensure_migrations_table(pool: &DbPool) -> Result<(), DbError> {
    pool.execute(CREATE_TABLE_SQL, &[]).await?;
    Ok(())
}

async fn applied_versions(pool: &DbPool) -> Result<Vec<String>, DbError> {
    let rows = pool.query_rows("SELECT version FROM _schema_migrations", &[]).await?;
    rows.into_iter().map(|r| r.get::<String>("version")).collect()
}

/// The version of the most recently applied migration, or `None` if none
/// are applied. "Most recent" means highest version string — the same
/// lexicographic order `run_migrations` applies files in.
async fn last_applied_version(pool: &DbPool) -> Result<Option<String>, DbError> {
    let rows = pool
        .query_rows("SELECT version FROM _schema_migrations ORDER BY version DESC LIMIT 1", &[])
        .await?;
    rows.into_iter().next().map(|r| r.get::<String>("version")).transpose()
}

/// Up migration files only — excludes companion `*.down.sql` files, which
/// share the `.sql` extension but are never applied as an "up" step.
fn read_sql_files(dir: &str) -> Result<Vec<(String, String)>, DbError> {
    let mut files: Vec<(String, String)> = Vec::new();
    let entries = std::fs::read_dir(dir)
        .map_err(|e| DbError::new(format!("cannot read migrations directory '{}': {}", dir, e)))?;

    for entry in entries {
        let entry = entry.map_err(|e| DbError::new(e.to_string()))?;
        let path = entry.path();
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_owned();
        if filename.ends_with(".sql") && !filename.ends_with(".down.sql") {
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| DbError::new(format!("cannot read '{}': {}", path.display(), e)))?;
            files.push((filename, contents));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// The companion down filename for up migration `version` (e.g.
/// `0001_x.sql` → `0001_x.down.sql`).
fn down_filename(version: &str) -> String {
    match version.strip_suffix(".sql") {
        Some(stem) => format!("{}.down.sql", stem),
        None => format!("{}.down.sql", version),
    }
}

fn down_file_path(dir: &str, version: &str) -> String {
    format!("{}/{}", dir.trim_end_matches('/'), down_filename(version))
}

fn down_file_exists(dir: &str, version: &str) -> bool {
    std::path::Path::new(&down_file_path(dir, version)).exists()
}

/// Read the down SQL for `version`, or `None` if no companion down file exists.
fn read_down_sql(dir: &str, version: &str) -> Result<Option<String>, DbError> {
    let path = down_file_path(dir, version);
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(DbError::new(format!("cannot read '{}': {}", path, e))),
    }
}

fn now_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, mo, d) = days_to_ymd(secs / 86400);
    let rem = secs % 86400;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, rem / 3600, (rem % 3600) / 60, rem % 60)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let diy = if is_leap(year) { 366 } else { 365 };
        if days < diy { break; }
        days -= diy;
        year += 1;
    }
    let months = [31u64, if is_leap(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    for &dim in &months {
        if days < dim { break; }
        days -= dim;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Run pending migrations from SQL files in `dir`. Called by `DbPool::migrate`.
pub async fn run_migrations(pool: &DbPool, dir: &str) -> Result<(), DbError> {
    ensure_migrations_table(pool).await?;
    let applied = applied_versions(pool).await?;
    let files = read_sql_files(dir)?;

    for (filename, sql) in files {
        if applied.contains(&filename) {
            continue;
        }

        // Execute each statement in the file separately (split on `;`).
        let statements: Vec<&str> = sql.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();
        let version = filename.clone();
        let ts = now_timestamp();
        let insert_sql = format!(
            "INSERT INTO _schema_migrations (version, applied_at) VALUES ({}, {})",
            super::repository::placeholder(1),
            super::repository::placeholder(2),
        );

        let mut tx = pool.begin().await?;
        for stmt in &statements {
            if let Err(e) = tx.execute(stmt, &[]).await {
                let _ = tx.rollback().await;
                return Err(DbError::new(format!("migration '{}' failed: {}", filename, e)));
            }
        }
        if let Err(e) = tx.execute(&insert_sql, &[Value::Text(version), Value::Text(ts)]).await {
            let _ = tx.rollback().await;
            return Err(e);
        }
        tx.commit().await?;
    }
    Ok(())
}

/// Return status (applied / pending) for each SQL file in `dir`. Called by `DbPool::migration_status`.
pub async fn migration_status(pool: &DbPool, dir: &str) -> Result<Vec<MigrationStatus>, DbError> {
    ensure_migrations_table(pool).await?;
    let applied = applied_versions(pool).await?;
    let files = read_sql_files(dir)?;
    Ok(files
        .into_iter()
        .map(|(filename, _)| {
            let is_applied = applied.contains(&filename);
            let has_down = down_file_exists(dir, &filename);
            MigrationStatus { version: filename, applied: is_applied, has_down }
        })
        .collect())
}

/// Roll back the most recently applied migration in `dir`. Called by
/// `DbPool::rollback_last`.
///
/// Requires a companion `<version-stem>.down.sql` file next to the up
/// migration it undoes; returns `Err` if that file is missing. Runs the
/// down file's statements and deletes the `_schema_migrations` row inside
/// one transaction — rolled back together on failure, same as `run_migrations`.
///
/// Returns `Ok(None)` if no migrations are currently applied.
pub async fn rollback_last(pool: &DbPool, dir: &str) -> Result<Option<String>, DbError> {
    ensure_migrations_table(pool).await?;
    let version = match last_applied_version(pool).await? {
        Some(v) => v,
        None => return Ok(None),
    };

    let down_sql = read_down_sql(dir, &version)?.ok_or_else(|| {
        DbError::new(format!(
            "no down migration found for '{}': expected '{}'",
            version,
            down_filename(&version),
        ))
    })?;

    let statements: Vec<&str> = down_sql.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();
    let delete_sql = format!(
        "DELETE FROM _schema_migrations WHERE version = {}",
        super::repository::placeholder(1),
    );

    let mut tx = pool.begin().await?;
    for stmt in &statements {
        if let Err(e) = tx.execute(stmt, &[]).await {
            let _ = tx.rollback().await;
            return Err(DbError::new(format!("rollback of '{}' failed: {}", version, e)));
        }
    }
    if let Err(e) = tx.execute(&delete_sql, &[Value::Text(version.clone())]).await {
        let _ = tx.rollback().await;
        return Err(e);
    }
    tx.commit().await?;
    Ok(Some(version))
}

/// Roll back the last `n` applied migrations, most recently applied first.
/// Called by `DbPool::rollback`.
///
/// Stops early (returning fewer than `n` versions) once nothing is left to
/// roll back. Stops with `Err` at the first missing down file or SQL error;
/// any migrations already rolled back in earlier iterations stay rolled
/// back — each step commits its own transaction independently.
pub async fn rollback(pool: &DbPool, dir: &str, n: usize) -> Result<Vec<String>, DbError> {
    let mut rolled_back = Vec::with_capacity(n);
    for _ in 0..n {
        match rollback_last(pool, dir).await? {
            Some(version) => rolled_back.push(version),
            None => break,
        }
    }
    Ok(rolled_back)
}
