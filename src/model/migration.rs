//! Migration runner.
//!
//! Reads `*.sql` files from a directory in lexicographic order. Creates
//! `_schema_migrations(version TEXT PRIMARY KEY, applied_at TEXT)` if absent.
//! Runs each file whose name is not yet in the table, each wrapped in a transaction.

use super::connection::DbConnection;
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

// ── Internal helpers ──────────────────────────────────────────────────────────

const CREATE_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS _schema_migrations (\
     version TEXT PRIMARY KEY, \
     applied_at TEXT NOT NULL\
     )";

fn ensure_migrations_table(conn: &mut DbConnection) -> Result<(), DbError> {
    conn.execute(CREATE_TABLE_SQL, &[])?;
    Ok(())
}

fn applied_versions(conn: &mut DbConnection) -> Result<Vec<String>, DbError> {
    let rows = conn.query_rows("SELECT version FROM _schema_migrations", &[])?;
    rows.into_iter()
        .map(|r| r.get::<String>("version"))
        .collect()
}

fn read_sql_files(dir: &str) -> Result<Vec<(String, String)>, DbError> {
    let mut files: Vec<(String, String)> = Vec::new();

    let entries = std::fs::read_dir(dir)
        .map_err(|e| DbError::new(format!("cannot read migrations directory '{}': {}", dir, e)))?;

    for entry in entries {
        let entry = entry.map_err(|e| DbError::new(e.to_string()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("sql") {
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_owned();
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| DbError::new(format!("cannot read '{}': {}", path.display(), e)))?;
            files.push((filename, contents));
        }
    }

    // Lexicographic order.
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

fn now_timestamp() -> String {
    // Simple ISO-8601 timestamp without external deps.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Format as YYYY-MM-DDTHH:MM:SSZ (approximate, UTC).
    let s = secs;
    let min_in_sec = 60u64;
    let hour_in_sec = 3600u64;
    let day_in_sec = 86400u64;
    let days = s / day_in_sec;
    let rem = s % day_in_sec;
    let h = rem / hour_in_sec;
    let m = (rem % hour_in_sec) / min_in_sec;
    let sec = rem % min_in_sec;

    // Days since epoch → approximate date.
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, sec)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Simplified Gregorian calendar calculation.
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let months = [31u64, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    for &dim in &months {
        if days < dim {
            break;
        }
        days -= dim;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Run pending migrations from SQL files in `dir`.
pub fn run_migrations(conn: &mut DbConnection, dir: &str) -> Result<(), DbError> {
    ensure_migrations_table(conn)?;
    let applied = applied_versions(conn)?;
    let files = read_sql_files(dir)?;

    for (filename, sql) in files {
        if applied.contains(&filename) {
            continue;
        }
        // Wrap each migration in a transaction.
        let version = filename.clone();
        conn.begin()?;
        match conn.execute(&sql, &[]) {
            Ok(_) => {
                let ts = now_timestamp();
                let insert_sql = format!(
                    "INSERT INTO _schema_migrations (version, applied_at) VALUES ({}, {})",
                    super::repository::placeholder(1),
                    super::repository::placeholder(2)
                );
                match conn.execute(&insert_sql, &[Value::Text(version), Value::Text(ts)]) {
                    Ok(_) => conn.commit()?,
                    Err(e) => {
                        let _ = conn.rollback();
                        return Err(e);
                    }
                }
            }
            Err(e) => {
                let _ = conn.rollback();
                return Err(DbError::new(format!("migration '{}' failed: {}", filename, e)));
            }
        }
    }
    Ok(())
}

/// Return the status (applied/pending) for each SQL file in `dir`.
pub fn migration_status(conn: &mut DbConnection, dir: &str) -> Result<Vec<MigrationStatus>, DbError> {
    ensure_migrations_table(conn)?;
    let applied = applied_versions(conn)?;
    let files = read_sql_files(dir)?;

    Ok(files
        .into_iter()
        .map(|(filename, _)| {
            let is_applied = applied.contains(&filename);
            MigrationStatus {
                version: filename,
                applied: is_applied,
            }
        })
        .collect())
}
