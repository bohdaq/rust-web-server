//! The database tools (`model-*` features): `db_schema`, and with a
//! migrations directory `migrations`, `migrations_run`,
//! `migrations_rollback_last`.
//!
//! The queries run on a dedicated runtime (`rws-dev-mcp`), not one made
//! per call: a PostgreSQL or MySQL connection the pool opens belongs to the
//! runtime it was opened on, and one that has ended can't serve it again.

use std::future::Future;
use std::sync::OnceLock;

use crate::mcp::{McpContent, McpServer};
use crate::model::{Backend, DbError, DbPool, ModelRow, Value};

use super::{quoted, read_only_hint};

/// Register the database tools on `mcp`.
pub(super) fn install(mcp: McpServer, pool: DbPool, migrations: Option<String>, writes: bool) -> McpServer {
    let schema_pool = pool.clone();
    let mut mcp = mcp
        .tool(
            "db_schema",
            "The database's tables and their columns (name, type, nullable, default, primary key)",
            r#"{"type":"object"}"#,
            move |_| {
                let pool = schema_pool.clone();
                run(async move { schema_json(&pool).await }).map(McpContent::json)
            },
        )
        .tool_annotations("db_schema", read_only_hint());
    let Some(dir) = migrations else { return mcp };
    let (status_pool, status_dir) = (pool.clone(), dir.clone());
    mcp = mcp
        .tool(
            "migrations",
            "The migrations in the app's migrations directory, in order, with whether each is applied and can be rolled back",
            r#"{"type":"object"}"#,
            move |_| {
                let (pool, dir) = (status_pool.clone(), status_dir.clone());
                run(async move { status_json(&pool, &dir).await }).map(McpContent::json)
            },
        )
        .tool_annotations("migrations", read_only_hint());
    if !writes {
        return mcp;
    }
    let (run_pool, run_dir) = (pool.clone(), dir.clone());
    mcp.tool(
        "migrations_run",
        "Apply every pending migration, each in its own transaction; answers the status afterwards",
        r#"{"type":"object"}"#,
        move |_| {
            let (pool, dir) = (run_pool.clone(), run_dir.clone());
            run(async move {
                pool.migrate(&dir).await?;
                status_json(&pool, &dir).await
            })
            .map(McpContent::json)
        },
    )
    .tool(
        "migrations_rollback_last",
        "Undo the most recently applied migration with its .down.sql file; answers which one (null when none was applied)",
        r#"{"type":"object"}"#,
        move |_| {
            let (pool, dir) = (pool.clone(), dir.clone());
            run(async move {
                let version = pool.rollback_last(&dir).await?;
                Ok(format!(r#"{{"rolled_back":{}}}"#, version.as_deref().map(quoted).unwrap_or_else(|| "null".to_string())))
            })
            .map(McpContent::json)
        },
    )
    .tool_annotations("migrations_run", super::ops::changes(false))
    .tool_annotations("migrations_rollback_last", super::ops::changes(true))
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("rws-dev-mcp")
            .enable_all()
            .build()
            .expect("rws-dev-mcp runtime")
    })
}

/// Run `work` on the dedicated runtime and wait for it — from any thread,
/// inside a runtime or not.
fn run<F>(work: F) -> Result<String, String>
where
    F: Future<Output = Result<String, DbError>> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    runtime().spawn(async move {
        let _ = tx.send(work.await);
    });
    match rx.recv() {
        Ok(result) => result.map_err(|e| e.0),
        Err(_) => Err("the database task ended without an answer".to_string()),
    }
}

async fn status_json(pool: &DbPool, dir: &str) -> Result<String, DbError> {
    let entries: Vec<String> = pool
        .migration_status(dir)
        .await?
        .iter()
        .map(|m| format!(r#"{{"version":{},"applied":{},"has_down":{}}}"#, quoted(&m.version), m.applied, m.has_down))
        .collect();
    Ok(format!(r#"{{"directory":{},"migrations":[{}]}}"#, quoted(dir), entries.join(",")))
}

/// One column as `db_schema` lists it.
struct Column {
    table: String,
    name: String,
    data_type: String,
    nullable: bool,
    default: Option<String>,
    primary_key: bool,
}

async fn schema_json(pool: &DbPool) -> Result<String, DbError> {
    let columns = columns(pool).await?;
    let mut tables: Vec<(String, Vec<String>)> = Vec::new();
    for c in columns {
        let column = format!(
            r#"{{"name":{},"type":{},"nullable":{},"default":{},"primary_key":{}}}"#,
            quoted(&c.name),
            quoted(&c.data_type),
            c.nullable,
            c.default.as_deref().map(quoted).unwrap_or_else(|| "null".to_string()),
            c.primary_key
        );
        match tables.last_mut() {
            Some((table, cols)) if *table == c.table => cols.push(column),
            _ => tables.push((c.table, vec![column])),
        }
    }
    let tables: Vec<String> =
        tables.iter().map(|(name, cols)| format!(r#"{{"name":{},"columns":[{}]}}"#, quoted(name), cols.join(","))).collect();
    Ok(format!(r#"{{"backend":{},"tables":[{}]}}"#, quoted(pool.backend().name()), tables.join(",")))
}

/// A column's value as text (`None` for NULL), whatever type the driver
/// decoded it as.
fn text(row: &ModelRow, column: &str) -> Option<String> {
    let value = row.columns.iter().find(|(name, _)| name.eq_ignore_ascii_case(column)).map(|(_, v)| v.clone())?;
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(b.to_string()),
        Value::Int(n) => Some(n.to_string()),
        Value::Float(f) => Some(f.to_string()),
        Value::Text(s) => Some(s),
        Value::Bytes(b) => Some(String::from_utf8_lossy(&b).into_owned()),
    }
}

/// Every column of every table the app owns, by table name then position;
/// the migrations bookkeeping table included (it is part of the schema).
async fn columns(pool: &DbPool) -> Result<Vec<Column>, DbError> {
    match pool.backend() {
        #[cfg(feature = "model-sqlite")]
        Backend::Sqlite => {
            let tables = pool
                .query_rows("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name", &[])
                .await?;
            let mut columns = Vec::new();
            for table in tables {
                let Some(table) = text(&table, "name") else { continue };
                let rows = pool
                    .query_rows(
                        r#"SELECT name, type, "notnull" AS not_null, dflt_value, pk FROM pragma_table_info(?) ORDER BY cid"#,
                        &[Value::Text(table.clone())],
                    )
                    .await?;
                for row in rows {
                    columns.push(Column {
                        table: table.clone(),
                        name: text(&row, "name").unwrap_or_default(),
                        data_type: text(&row, "type").unwrap_or_default(),
                        nullable: text(&row, "not_null").as_deref() != Some("1"),
                        default: text(&row, "dflt_value"),
                        primary_key: text(&row, "pk").is_some_and(|pk| pk != "0"),
                    });
                }
            }
            Ok(columns)
        }
        #[cfg(feature = "model-postgres")]
        Backend::Postgres => {
            let rows = pool
                .query_rows(
                    "SELECT c.table_name::text AS table_name, c.column_name::text AS column_name, c.data_type::text AS data_type, \
                     c.is_nullable::text AS is_nullable, c.column_default::text AS column_default, \
                     EXISTS (SELECT 1 FROM information_schema.table_constraints tc \
                             JOIN information_schema.key_column_usage k \
                               ON k.constraint_name = tc.constraint_name AND k.table_schema = tc.table_schema \
                             WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = c.table_schema \
                               AND tc.table_name = c.table_name AND k.column_name = c.column_name) AS primary_key \
                     FROM information_schema.columns c \
                     WHERE c.table_schema = current_schema() \
                     ORDER BY c.table_name, c.ordinal_position",
                    &[],
                )
                .await?;
            Ok(rows.iter().map(information_schema_column).collect())
        }
        #[cfg(feature = "model-mysql")]
        Backend::MySql => {
            let rows = pool
                .query_rows(
                    "SELECT CAST(TABLE_NAME AS CHAR) AS table_name, CAST(COLUMN_NAME AS CHAR) AS column_name, \
                     CAST(COLUMN_TYPE AS CHAR) AS data_type, CAST(IS_NULLABLE AS CHAR) AS is_nullable, \
                     CAST(COLUMN_DEFAULT AS CHAR) AS column_default, (COLUMN_KEY = 'PRI') AS primary_key \
                     FROM information_schema.columns \
                     WHERE TABLE_SCHEMA = DATABASE() \
                     ORDER BY TABLE_NAME, ORDINAL_POSITION",
                    &[],
                )
                .await?;
            Ok(rows.iter().map(information_schema_column).collect())
        }
    }
}

#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
fn information_schema_column(row: &ModelRow) -> Column {
    Column {
        table: text(row, "table_name").unwrap_or_default(),
        name: text(row, "column_name").unwrap_or_default(),
        data_type: text(row, "data_type").unwrap_or_default(),
        nullable: text(row, "is_nullable").is_some_and(|v| v.eq_ignore_ascii_case("YES")),
        default: text(row, "column_default"),
        primary_key: text(row, "primary_key").is_some_and(|v| v == "true" || v == "1"),
    }
}
