//! Async migration runner.
//!
//! Reads `*.sql` files from a directory in lexicographic order. Creates
//! `_schema_migrations(version TEXT PRIMARY KEY, applied_at TEXT)` if absent.
//! Runs each unapplied file inside a transaction.

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

fn read_sql_files(dir: &str) -> Result<Vec<(String, String)>, DbError> {
    let mut files: Vec<(String, String)> = Vec::new();
    let entries = std::fs::read_dir(dir)
        .map_err(|e| DbError::new(format!("cannot read migrations directory '{}': {}", dir, e)))?;

    for entry in entries {
        let entry = entry.map_err(|e| DbError::new(e.to_string()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("sql") {
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_owned();
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| DbError::new(format!("cannot read '{}': {}", path.display(), e)))?;
            files.push((filename, contents));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
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
            MigrationStatus { version: filename, applied: is_applied }
        })
        .collect())
}
