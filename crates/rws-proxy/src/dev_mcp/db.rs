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
use crate::model::{DbError, DbPool};

use super::{quoted, read_only_hint};

/// Register the database tools on `mcp`.
pub(super) fn install(mcp: McpServer, pool: DbPool, migrations: Option<String>, writes: bool) -> McpServer {
    let schema_pool = pool.clone();
    let mut mcp = mcp
        .tool(
            "db_schema",
            "The database's tables: columns (name, type, nullable, default, primary key), primary key, indexes and foreign keys",
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
pub(crate) fn run<F, T>(work: F) -> Result<T, String>
where
    F: Future<Output = Result<T, DbError>> + Send + 'static,
    T: Send + 'static,
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

pub(crate) async fn status_json(pool: &DbPool, dir: &str) -> Result<String, DbError> {
    let entries: Vec<String> = pool
        .migration_status(dir)
        .await?
        .iter()
        .map(|m| format!(r#"{{"version":{},"applied":{},"has_down":{}}}"#, quoted(&m.version), m.applied, m.has_down))
        .collect();
    Ok(format!(r#"{{"directory":{},"migrations":[{}]}}"#, quoted(dir), entries.join(",")))
}

/// One column as `db_schema` lists it.
pub(super) struct Column {
    pub(super) table: String,
    pub(super) name: String,
    pub(super) data_type: String,
    pub(super) nullable: bool,
    pub(super) default: Option<String>,
    pub(super) primary_key: bool,
}

/// `db_schema`'s answer: `DbPool::schema` as JSON — tables with their
/// columns, primary key, indexes and foreign keys (18.30.0; columns only
/// before).
pub(crate) async fn schema_json(pool: &DbPool) -> Result<String, DbError> {
    Ok(pool.schema().await?.to_json())
}

/// Every column of every table the app owns, by table name then position;
/// the migrations bookkeeping table included (it is part of the schema).
pub(super) async fn columns(pool: &DbPool) -> Result<Vec<Column>, DbError> {
    let schema = pool.schema().await?;
    Ok(schema
        .tables
        .into_iter()
        .flat_map(|t| {
            let table = t.name;
            t.columns.into_iter().map(move |c| Column {
                table: table.clone(),
                name: c.name,
                data_type: c.data_type,
                nullable: c.nullable,
                default: c.default,
                primary_key: c.primary_key,
            })
        })
        .collect())
}

