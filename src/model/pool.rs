//! Async connection pool — `DbPool` and `DbTransaction`.
//!
//! Backed by `sqlx`. `model-sqlite`, `model-postgres`, and `model-mysql` are
//! not mutually exclusive: each compiled-in feature adds a variant to the
//! `DbPool`/`DbTransaction` enums below, so a single binary can hold pools to
//! more than one backend at once. Which variant a given `DbPool` is depends
//! on the [`Backend`] passed via [`DbConfig::backend`].

use std::future::Future;

use super::backend::Backend;
use super::connection::DbConfig;
use super::migration;
use super::{DbError, Model, ModelRow, MigrationStatus, Value};

// ── DbPool ────────────────────────────────────────────────────────────────────

/// An async connection pool backed by sqlx.
///
/// Cheap to clone — the inner pool is reference-counted. Which variant a
/// `DbPool` is depends on the [`Backend`] it was created with; see the module
/// docs for running more than one backend in the same binary.
#[derive(Clone, Debug)]
pub enum DbPool {
    #[cfg(feature = "model-sqlite")]
    Sqlite(sqlx::Pool<sqlx::Sqlite>),
    #[cfg(feature = "model-postgres")]
    Postgres(sqlx::Pool<sqlx::Postgres>),
    #[cfg(feature = "model-mysql")]
    MySql(sqlx::Pool<sqlx::MySql>),
}

impl DbPool {
    /// Create a new pool for `config.backend`, with the given configuration.
    pub async fn new(config: DbConfig) -> Result<Self, DbError> {
        let url = config.to_url();
        match config.backend {
            #[cfg(feature = "model-sqlite")]
            Backend::Sqlite => {
                let pool = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
                    .max_connections(config.pool_size)
                    .connect(&url)
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbPool::Sqlite(pool))
            }
            #[cfg(feature = "model-postgres")]
            Backend::Postgres => {
                let pool = sqlx::pool::PoolOptions::<sqlx::Postgres>::new()
                    .max_connections(config.pool_size)
                    .connect(&url)
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbPool::Postgres(pool))
            }
            #[cfg(feature = "model-mysql")]
            Backend::MySql => {
                let pool = sqlx::pool::PoolOptions::<sqlx::MySql>::new()
                    .max_connections(config.pool_size)
                    .connect(&url)
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbPool::MySql(pool))
            }
        }
    }

    /// Create a pool using [`DbConfig::from_env`].
    pub async fn from_env() -> Result<Self, DbError> {
        DbPool::new(DbConfig::from_env()?).await
    }

    /// Create a pool backed by a SQLite in-memory database.
    ///
    /// All connections in the pool share the same in-memory database
    /// (max_connections = 1).  Each call returns an independent, isolated
    /// database — ideal for tests.
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "model-sqlite")]
    /// # async fn example() -> Result<(), rust_web_server::model::DbError> {
    /// use rust_web_server::model::{DbPool, Value};
    ///
    /// let pool = DbPool::memory().await?;
    /// pool.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)", &[]).await?;
    /// pool.execute("INSERT INTO t (v) VALUES (?)", &[Value::Text("hello".into())]).await?;
    /// let rows = pool.query_rows("SELECT * FROM t", &[]).await?;
    /// assert_eq!(1, rows.len());
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "model-sqlite")]
    pub async fn memory() -> Result<Self, DbError> {
        let pool = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| DbError::new(e.to_string()))?;
        Ok(DbPool::Sqlite(pool))
    }

    /// Which backend this pool talks to.
    pub fn backend(&self) -> Backend {
        match self {
            #[cfg(feature = "model-sqlite")]
            DbPool::Sqlite(_) => Backend::Sqlite,
            #[cfg(feature = "model-postgres")]
            DbPool::Postgres(_) => Backend::Postgres,
            #[cfg(feature = "model-mysql")]
            DbPool::MySql(_) => Backend::MySql,
        }
    }

    // ── Core async SQL operations ─────────────────────────────────────────────

    /// Execute a SQL statement (INSERT / UPDATE / DELETE / DDL).
    ///
    /// Returns the number of rows affected.
    pub async fn execute(&self, sql: &str, params: &[Value]) -> Result<u64, DbError> {
        match self {
            #[cfg(feature = "model-sqlite")]
            DbPool::Sqlite(pool) => sqlite_pool_execute(pool, sql, params).await,
            #[cfg(feature = "model-postgres")]
            DbPool::Postgres(pool) => pg_pool_execute(pool, sql, params).await,
            #[cfg(feature = "model-mysql")]
            DbPool::MySql(pool) => mysql_pool_execute(pool, sql, params).await,
        }
    }

    /// Execute a SQL query and return untyped rows.
    pub async fn query_rows(&self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        match self {
            #[cfg(feature = "model-sqlite")]
            DbPool::Sqlite(pool) => sqlite_pool_query_rows(pool, sql, params).await,
            #[cfg(feature = "model-postgres")]
            DbPool::Postgres(pool) => pg_pool_query_rows(pool, sql, params).await,
            #[cfg(feature = "model-mysql")]
            DbPool::MySql(pool) => mysql_pool_query_rows(pool, sql, params).await,
        }
    }

    /// Execute a SQL query and deserialise results into `T: Model`.
    pub async fn query<T: Model>(&self, sql: &str, params: &[Value]) -> Result<Vec<T>, DbError> {
        let rows = self.query_rows(sql, params).await?;
        rows.iter().map(|r| T::from_row(r)).collect()
    }

    /// Execute a SQL query returning untyped rows (alias for `query_rows`).
    pub async fn query_raw(&self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        self.query_rows(sql, params).await
    }

    /// Insert-then-fetch-generated-id, dispatched per backend: SQLite uses
    /// `last_insert_rowid()`, PostgreSQL appends `RETURNING`, MySQL uses
    /// `last_insert_id()`. Used by the repository's INSERT logic.
    pub(crate) async fn insert_returning_id(
        &self,
        insert_sql: &str,
        params: &[Value],
        _pk_col: &str,
    ) -> Result<i64, DbError> {
        match self {
            #[cfg(feature = "model-sqlite")]
            DbPool::Sqlite(pool) => sqlite_last_insert_id(pool, insert_sql, params).await,
            #[cfg(feature = "model-postgres")]
            DbPool::Postgres(pool) => pg_insert_returning(pool, insert_sql, params, _pk_col).await,
            #[cfg(feature = "model-mysql")]
            DbPool::MySql(pool) => mysql_last_insert_id(pool, insert_sql, params).await,
        }
    }

    // ── Transactions ──────────────────────────────────────────────────────────

    /// Begin a transaction. Call [`DbTransaction::commit`] to commit or let it
    /// drop to automatically roll back.
    pub async fn begin(&self) -> Result<DbTransaction, DbError> {
        match self {
            #[cfg(feature = "model-sqlite")]
            DbPool::Sqlite(pool) => {
                let tx = pool.begin().await.map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbTransaction::Sqlite(tx))
            }
            #[cfg(feature = "model-postgres")]
            DbPool::Postgres(pool) => {
                let tx = pool.begin().await.map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbTransaction::Postgres(tx))
            }
            #[cfg(feature = "model-mysql")]
            DbPool::MySql(pool) => {
                let tx = pool.begin().await.map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbTransaction::MySql(tx))
            }
        }
    }

    /// Run a closure in a transaction.
    ///
    /// The transaction is committed automatically on `Ok` and rolled back on `Err`.
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "model-sqlite")]
    /// # async fn example() -> Result<(), rust_web_server::model::DbError> {
    /// use rust_web_server::model::{DbPool, Value};
    ///
    /// let pool = DbPool::memory().await?;
    /// let result = pool.transaction(|mut tx| async move {
    ///     tx.execute("INSERT INTO t (v) VALUES (?)", &[Value::Text("hi".into())]).await?;
    ///     Ok(42i32)
    /// }).await?;
    /// assert_eq!(42, result);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn transaction<F, T, Fut>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(DbTransaction) -> Fut + Send,
        Fut: Future<Output = Result<T, DbError>> + Send,
        T: Send,
    {
        let tx = self.begin().await?;
        match f(tx).await {
            Ok(v) => Ok(v),
            Err(e) => Err(e),
        }
    }

    // ── Migration ─────────────────────────────────────────────────────────────

    /// Run pending migrations from SQL files in `dir`.
    ///
    /// Files are applied in lexicographic order. Already-applied versions are
    /// tracked in a `_schema_migrations` table (created if absent). Idempotent.
    pub async fn migrate(&self, dir: &str) -> Result<(), DbError> {
        migration::run_migrations(self, dir).await
    }

    /// Return the status (applied / pending) for each SQL file in `dir`.
    pub async fn migration_status(&self, dir: &str) -> Result<Vec<MigrationStatus>, DbError> {
        migration::migration_status(self, dir).await
    }

    /// Roll back the most recently applied migration in `dir`.
    ///
    /// Requires a companion `<version-stem>.down.sql` file next to the up
    /// migration it undoes (e.g. `0001_x.sql` → `0001_x.down.sql`); returns
    /// `Err` if that file is missing. Returns `Ok(None)` if nothing is
    /// currently applied.
    pub async fn rollback_last(&self, dir: &str) -> Result<Option<String>, DbError> {
        migration::rollback_last(self, dir).await
    }

    /// Roll back the last `n` applied migrations, most recently applied
    /// first. Returns the versions actually rolled back — fewer than `n` if
    /// fewer than `n` migrations were applied. Stops at the first missing
    /// down file or SQL error; migrations already rolled back in earlier
    /// iterations stay rolled back.
    pub async fn rollback(&self, dir: &str, n: usize) -> Result<Vec<String>, DbError> {
        migration::rollback(self, dir, n).await
    }
}

// ── DbTransaction ─────────────────────────────────────────────────────────────

/// An in-progress database transaction.
///
/// Created with [`DbPool::begin`].  Calling [`commit`][DbTransaction::commit]
/// commits the transaction; dropping without committing rolls it back.
pub enum DbTransaction {
    #[cfg(feature = "model-sqlite")]
    Sqlite(sqlx::Transaction<'static, sqlx::Sqlite>),
    #[cfg(feature = "model-postgres")]
    Postgres(sqlx::Transaction<'static, sqlx::Postgres>),
    #[cfg(feature = "model-mysql")]
    MySql(sqlx::Transaction<'static, sqlx::MySql>),
}

impl DbTransaction {
    /// Execute a SQL statement inside this transaction.
    pub async fn execute(&mut self, sql: &str, params: &[Value]) -> Result<u64, DbError> {
        match self {
            #[cfg(feature = "model-sqlite")]
            DbTransaction::Sqlite(tx) => sqlite_tx_execute(tx, sql, params).await,
            #[cfg(feature = "model-postgres")]
            DbTransaction::Postgres(tx) => pg_tx_execute(tx, sql, params).await,
            #[cfg(feature = "model-mysql")]
            DbTransaction::MySql(tx) => mysql_tx_execute(tx, sql, params).await,
        }
    }

    /// Execute a SQL query inside this transaction, returning untyped rows.
    pub async fn query_rows(&mut self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        match self {
            #[cfg(feature = "model-sqlite")]
            DbTransaction::Sqlite(tx) => sqlite_tx_query_rows(tx, sql, params).await,
            #[cfg(feature = "model-postgres")]
            DbTransaction::Postgres(tx) => pg_tx_query_rows(tx, sql, params).await,
            #[cfg(feature = "model-mysql")]
            DbTransaction::MySql(tx) => mysql_tx_query_rows(tx, sql, params).await,
        }
    }

    /// Execute a SQL query inside this transaction and deserialise into `T: Model`.
    pub async fn query<T: Model>(&mut self, sql: &str, params: &[Value]) -> Result<Vec<T>, DbError> {
        let rows = self.query_rows(sql, params).await?;
        rows.iter().map(|r| T::from_row(r)).collect()
    }

    /// Commit this transaction.
    pub async fn commit(self) -> Result<(), DbError> {
        match self {
            #[cfg(feature = "model-sqlite")]
            DbTransaction::Sqlite(tx) => tx.commit().await.map_err(|e| DbError::new(e.to_string())),
            #[cfg(feature = "model-postgres")]
            DbTransaction::Postgres(tx) => tx.commit().await.map_err(|e| DbError::new(e.to_string())),
            #[cfg(feature = "model-mysql")]
            DbTransaction::MySql(tx) => tx.commit().await.map_err(|e| DbError::new(e.to_string())),
        }
    }

    /// Roll back this transaction explicitly (also happens on drop).
    pub async fn rollback(self) -> Result<(), DbError> {
        match self {
            #[cfg(feature = "model-sqlite")]
            DbTransaction::Sqlite(tx) => tx.rollback().await.map_err(|e| DbError::new(e.to_string())),
            #[cfg(feature = "model-postgres")]
            DbTransaction::Postgres(tx) => tx.rollback().await.map_err(|e| DbError::new(e.to_string())),
            #[cfg(feature = "model-mysql")]
            DbTransaction::MySql(tx) => tx.rollback().await.map_err(|e| DbError::new(e.to_string())),
        }
    }
}

// ── Internal helpers (SQLite) ─────────────────────────────────────────────────

#[cfg(feature = "model-sqlite")]
async fn sqlite_pool_execute(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    let mut args = sqlx::sqlite::SqliteArguments::default();
    bind_sqlite_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.rows_affected())
}

#[cfg(feature = "model-sqlite")]
async fn sqlite_pool_query_rows(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    sql: &str,
    params: &[Value],
) -> Result<Vec<ModelRow>, DbError> {
    let mut args = sqlx::sqlite::SqliteArguments::default();
    bind_sqlite_args(&mut args, params)?;
    let rows = sqlx::query_with(sql, args)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    rows.into_iter().map(sqlite_row_to_model_row).collect()
}

#[cfg(feature = "model-sqlite")]
async fn sqlite_tx_execute(
    tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    let mut args = sqlx::sqlite::SqliteArguments::default();
    bind_sqlite_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.rows_affected())
}

#[cfg(feature = "model-sqlite")]
async fn sqlite_tx_query_rows(
    tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
    sql: &str,
    params: &[Value],
) -> Result<Vec<ModelRow>, DbError> {
    let mut args = sqlx::sqlite::SqliteArguments::default();
    bind_sqlite_args(&mut args, params)?;
    let rows = sqlx::query_with(sql, args)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    rows.into_iter().map(sqlite_row_to_model_row).collect()
}

#[cfg(feature = "model-sqlite")]
fn bind_sqlite_args(
    args: &mut sqlx::sqlite::SqliteArguments<'_>,
    params: &[Value],
) -> Result<(), DbError> {
    use sqlx::Arguments;
    for v in params {
        match v {
            Value::Null  => args.add(None::<String>).map_err(|e| DbError::new(e.to_string()))?,
            Value::Bool(b)  => args.add(*b).map_err(|e| DbError::new(e.to_string()))?,
            Value::Int(i)   => args.add(*i).map_err(|e| DbError::new(e.to_string()))?,
            Value::Float(f) => args.add(*f).map_err(|e| DbError::new(e.to_string()))?,
            Value::Text(s)  => args.add(s.clone()).map_err(|e| DbError::new(e.to_string()))?,
            Value::Bytes(b) => args.add(b.clone()).map_err(|e| DbError::new(e.to_string()))?,
        }
    }
    Ok(())
}

#[cfg(feature = "model-sqlite")]
fn sqlite_row_to_model_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<ModelRow, DbError> {
    use sqlx::{Column, Row, TypeInfo};
    let mut cols: Vec<(String, Value)> = Vec::with_capacity(row.columns().len());
    for col in row.columns() {
        let name = col.name().to_string();
        let type_name = col.type_info().name();
        let value = if type_name.contains("INT") {
            row.try_get::<Option<i64>, _>(col.ordinal())
                .map(|v| v.map(Value::Int).unwrap_or(Value::Null))
                .unwrap_or(Value::Null)
        } else if type_name.contains("REAL")
            || type_name.contains("FLOAT")
            || type_name.contains("DOUBLE")
            || type_name.contains("NUMERIC")
            || type_name.contains("DECIMAL")
        {
            row.try_get::<Option<f64>, _>(col.ordinal())
                .map(|v| v.map(Value::Float).unwrap_or(Value::Null))
                .unwrap_or(Value::Null)
        } else if type_name.contains("BOOL") {
            row.try_get::<Option<bool>, _>(col.ordinal())
                .map(|v| v.map(Value::Bool).unwrap_or(Value::Null))
                .unwrap_or(Value::Null)
        } else if type_name.contains("BLOB") {
            row.try_get::<Option<Vec<u8>>, _>(col.ordinal())
                .map(|v| v.map(Value::Bytes).unwrap_or(Value::Null))
                .unwrap_or(Value::Null)
        } else if type_name.is_empty() || type_name == "NULL" {
            // No declared type (expression, aggregate, function result) — probe by value type.
            if let Ok(Some(v)) = row.try_get::<Option<i64>, _>(col.ordinal()) {
                Value::Int(v)
            } else if let Ok(Some(v)) = row.try_get::<Option<f64>, _>(col.ordinal()) {
                Value::Float(v)
            } else if let Ok(Some(v)) = row.try_get::<Option<String>, _>(col.ordinal()) {
                Value::Text(v)
            } else if let Ok(Some(v)) = row.try_get::<Option<Vec<u8>>, _>(col.ordinal()) {
                Value::Bytes(v)
            } else {
                Value::Null
            }
        } else {
            // TEXT, VARCHAR, CHAR, and anything else → try string
            row.try_get::<Option<String>, _>(col.ordinal())
                .map(|v| v.map(Value::Text).unwrap_or(Value::Null))
                .unwrap_or(Value::Null)
        };
        cols.push((name, value));
    }
    Ok(ModelRow::new(cols))
}

/// Insert then return the last inserted row ID from SQLite's
/// `last_insert_rowid()`. Used by the repository's INSERT logic.
#[cfg(feature = "model-sqlite")]
async fn sqlite_last_insert_id(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    sql: &str,
    params: &[Value],
) -> Result<i64, DbError> {
    let mut args = sqlx::sqlite::SqliteArguments::default();
    bind_sqlite_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.last_insert_rowid())
}

// ── Internal helpers (PostgreSQL) ─────────────────────────────────────────────

#[cfg(feature = "model-postgres")]
async fn pg_pool_execute(
    pool: &sqlx::Pool<sqlx::Postgres>,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    let mut args = sqlx::postgres::PgArguments::default();
    bind_pg_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.rows_affected())
}

#[cfg(feature = "model-postgres")]
async fn pg_pool_query_rows(
    pool: &sqlx::Pool<sqlx::Postgres>,
    sql: &str,
    params: &[Value],
) -> Result<Vec<ModelRow>, DbError> {
    let mut args = sqlx::postgres::PgArguments::default();
    bind_pg_args(&mut args, params)?;
    let rows = sqlx::query_with(sql, args)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    rows.into_iter().map(pg_row_to_model_row).collect()
}

#[cfg(feature = "model-postgres")]
async fn pg_tx_execute(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    let mut args = sqlx::postgres::PgArguments::default();
    bind_pg_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.rows_affected())
}

#[cfg(feature = "model-postgres")]
async fn pg_tx_query_rows(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    sql: &str,
    params: &[Value],
) -> Result<Vec<ModelRow>, DbError> {
    let mut args = sqlx::postgres::PgArguments::default();
    bind_pg_args(&mut args, params)?;
    let rows = sqlx::query_with(sql, args)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    rows.into_iter().map(pg_row_to_model_row).collect()
}

#[cfg(feature = "model-postgres")]
fn bind_pg_args(
    args: &mut sqlx::postgres::PgArguments,
    params: &[Value],
) -> Result<(), DbError> {
    use sqlx::Arguments;
    for v in params {
        match v {
            Value::Null  => args.add(None::<String>).map_err(|e| DbError::new(e.to_string()))?,
            Value::Bool(b)  => args.add(*b).map_err(|e| DbError::new(e.to_string()))?,
            Value::Int(i)   => args.add(*i).map_err(|e| DbError::new(e.to_string()))?,
            Value::Float(f) => args.add(*f).map_err(|e| DbError::new(e.to_string()))?,
            Value::Text(s)  => args.add(s.clone()).map_err(|e| DbError::new(e.to_string()))?,
            Value::Bytes(b) => args.add(b.clone()).map_err(|e| DbError::new(e.to_string()))?,
        }
    }
    Ok(())
}

#[cfg(feature = "model-postgres")]
fn pg_row_to_model_row(row: sqlx::postgres::PgRow) -> Result<ModelRow, DbError> {
    use sqlx::{Column, Row, TypeInfo};
    let mut cols: Vec<(String, Value)> = Vec::with_capacity(row.columns().len());
    for col in row.columns() {
        let name = col.name().to_string();
        let type_name = col.type_info().name();
        let value = match type_name {
            "BOOL" => row.try_get::<Option<bool>, _>(col.ordinal())
                .map(|v| v.map(Value::Bool).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
            "INT2" | "INT4" | "INT8" => row.try_get::<Option<i64>, _>(col.ordinal())
                .map(|v| v.map(Value::Int).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
            "FLOAT4" | "FLOAT8" | "NUMERIC" => row.try_get::<Option<f64>, _>(col.ordinal())
                .map(|v| v.map(Value::Float).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
            "BYTEA" => row.try_get::<Option<Vec<u8>>, _>(col.ordinal())
                .map(|v| v.map(Value::Bytes).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
            _ => row.try_get::<Option<String>, _>(col.ordinal())
                .map(|v| v.map(Value::Text).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
        };
        cols.push((name, value));
    }
    Ok(ModelRow::new(cols))
}

/// Execute an INSERT and return the RETURNING id (PostgreSQL).
#[cfg(feature = "model-postgres")]
async fn pg_insert_returning(
    pool: &sqlx::Pool<sqlx::Postgres>,
    sql: &str,
    params: &[Value],
    pk_col: &str,
) -> Result<i64, DbError> {
    let mut args = sqlx::postgres::PgArguments::default();
    bind_pg_args(&mut args, params)?;
    let returning_sql = format!("{} RETURNING {}", sql, pk_col);
    let row = sqlx::query_with(&returning_sql, args)
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    use sqlx::Row;
    let id: i64 = row.try_get(pk_col).map_err(|e| DbError::new(e.to_string()))?;
    Ok(id)
}

// ── Internal helpers (MySQL) ──────────────────────────────────────────────────

#[cfg(feature = "model-mysql")]
async fn mysql_pool_execute(
    pool: &sqlx::Pool<sqlx::MySql>,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.rows_affected())
}

#[cfg(feature = "model-mysql")]
async fn mysql_pool_query_rows(
    pool: &sqlx::Pool<sqlx::MySql>,
    sql: &str,
    params: &[Value],
) -> Result<Vec<ModelRow>, DbError> {
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let rows = sqlx::query_with(sql, args)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    rows.into_iter().map(mysql_row_to_model_row).collect()
}

#[cfg(feature = "model-mysql")]
async fn mysql_tx_execute(
    tx: &mut sqlx::Transaction<'static, sqlx::MySql>,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.rows_affected())
}

#[cfg(feature = "model-mysql")]
async fn mysql_tx_query_rows(
    tx: &mut sqlx::Transaction<'static, sqlx::MySql>,
    sql: &str,
    params: &[Value],
) -> Result<Vec<ModelRow>, DbError> {
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let rows = sqlx::query_with(sql, args)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    rows.into_iter().map(mysql_row_to_model_row).collect()
}

#[cfg(feature = "model-mysql")]
fn bind_mysql_args(
    args: &mut sqlx::mysql::MySqlArguments,
    params: &[Value],
) -> Result<(), DbError> {
    use sqlx::Arguments;
    for v in params {
        match v {
            Value::Null  => args.add(None::<String>).map_err(|e| DbError::new(e.to_string()))?,
            Value::Bool(b)  => args.add(*b).map_err(|e| DbError::new(e.to_string()))?,
            Value::Int(i)   => args.add(*i).map_err(|e| DbError::new(e.to_string()))?,
            Value::Float(f) => args.add(*f).map_err(|e| DbError::new(e.to_string()))?,
            Value::Text(s)  => args.add(s.clone()).map_err(|e| DbError::new(e.to_string()))?,
            Value::Bytes(b) => args.add(b.clone()).map_err(|e| DbError::new(e.to_string()))?,
        }
    }
    Ok(())
}

#[cfg(feature = "model-mysql")]
fn mysql_row_to_model_row(row: sqlx::mysql::MySqlRow) -> Result<ModelRow, DbError> {
    use sqlx::{Column, Row, TypeInfo};
    let mut cols: Vec<(String, Value)> = Vec::with_capacity(row.columns().len());
    for col in row.columns() {
        let name = col.name().to_string();
        let type_name = col.type_info().name().to_uppercase();
        let value = if type_name.contains("INT") || type_name.contains("SERIAL") {
            row.try_get::<Option<i64>, _>(col.ordinal())
                .map(|v| v.map(Value::Int).unwrap_or(Value::Null))
                .unwrap_or(Value::Null)
        } else if type_name.contains("FLOAT")
            || type_name.contains("DOUBLE")
            || type_name.contains("DECIMAL")
            || type_name.contains("NUMERIC")
        {
            row.try_get::<Option<f64>, _>(col.ordinal())
                .map(|v| v.map(Value::Float).unwrap_or(Value::Null))
                .unwrap_or(Value::Null)
        } else if type_name.contains("BOOL") {
            row.try_get::<Option<bool>, _>(col.ordinal())
                .map(|v| v.map(Value::Bool).unwrap_or(Value::Null))
                .unwrap_or(Value::Null)
        } else if type_name.contains("BLOB") || type_name.contains("BINARY") {
            row.try_get::<Option<Vec<u8>>, _>(col.ordinal())
                .map(|v| v.map(Value::Bytes).unwrap_or(Value::Null))
                .unwrap_or(Value::Null)
        } else {
            row.try_get::<Option<String>, _>(col.ordinal())
                .map(|v| v.map(Value::Text).unwrap_or(Value::Null))
                .unwrap_or(Value::Null)
        };
        cols.push((name, value));
    }
    Ok(ModelRow::new(cols))
}

/// Insert then return the last inserted row ID from MySQL's `last_insert_id()`.
/// Used by the repository's INSERT logic.
#[cfg(feature = "model-mysql")]
async fn mysql_last_insert_id(
    pool: &sqlx::Pool<sqlx::MySql>,
    sql: &str,
    params: &[Value],
) -> Result<i64, DbError> {
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.last_insert_id() as i64)
}
