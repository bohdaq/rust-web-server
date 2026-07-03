//! Async connection pool — `DbPool` and `DbTransaction`.
//!
//! Backed by `sqlx`. The concrete database type is selected by feature flag:
//! - `model-sqlite`   → `sqlx::Sqlite`
//! - `model-postgres` → `sqlx::Postgres`
//! - `model-mysql`    → `sqlx::MySql`

use std::future::Future;

use super::connection::DbConfig;
use super::migration;
use super::{DbError, Model, ModelRow, MigrationStatus, Value};

// ── Backend type alias ────────────────────────────────────────────────────────

#[cfg(feature = "model-sqlite")]
type Db = sqlx::Sqlite;

#[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
type Db = sqlx::Postgres;

#[cfg(all(
    feature = "model-mysql",
    not(feature = "model-sqlite"),
    not(feature = "model-postgres")
))]
type Db = sqlx::MySql;

// ── DbPool ────────────────────────────────────────────────────────────────────

/// An async connection pool backed by sqlx.
///
/// Cheap to clone — the inner pool is reference-counted.
#[derive(Clone, Debug)]
pub struct DbPool(pub(crate) sqlx::Pool<Db>);

impl DbPool {
    /// Create a new pool with the given configuration.
    pub async fn new(config: DbConfig) -> Result<Self, DbError> {
        let pool = sqlx::pool::PoolOptions::<Db>::new()
            .max_connections(config.pool_size)
            .connect(&config.to_url())
            .await
            .map_err(|e| DbError::new(e.to_string()))?;
        Ok(DbPool(pool))
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
        let pool = sqlx::pool::PoolOptions::<Db>::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| DbError::new(e.to_string()))?;
        Ok(DbPool(pool))
    }

    // ── Core async SQL operations ─────────────────────────────────────────────

    /// Execute a SQL statement (INSERT / UPDATE / DELETE / DDL).
    ///
    /// Returns the number of rows affected.
    pub async fn execute(&self, sql: &str, params: &[Value]) -> Result<u64, DbError> {
        let result = pool_execute_impl(&self.0, sql, params).await?;
        Ok(result)
    }

    /// Execute a SQL query and return untyped rows.
    pub async fn query_rows(&self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        pool_query_rows_impl(&self.0, sql, params).await
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

    // ── Transactions ──────────────────────────────────────────────────────────

    /// Begin a transaction. Call [`DbTransaction::commit`] to commit or let it
    /// drop to automatically roll back.
    pub async fn begin(&self) -> Result<DbTransaction, DbError> {
        let tx = self.0.begin().await.map_err(|e| DbError::new(e.to_string()))?;
        Ok(DbTransaction(tx))
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
}

// ── DbTransaction ─────────────────────────────────────────────────────────────

/// An in-progress database transaction.
///
/// Created with [`DbPool::begin`].  Calling [`commit`][DbTransaction::commit]
/// commits the transaction; dropping without committing rolls it back.
pub struct DbTransaction(pub(crate) sqlx::Transaction<'static, Db>);

impl DbTransaction {
    /// Execute a SQL statement inside this transaction.
    pub async fn execute(&mut self, sql: &str, params: &[Value]) -> Result<u64, DbError> {
        tx_execute_impl(&mut self.0, sql, params).await
    }

    /// Execute a SQL query inside this transaction, returning untyped rows.
    pub async fn query_rows(&mut self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        tx_query_rows_impl(&mut self.0, sql, params).await
    }

    /// Execute a SQL query inside this transaction and deserialise into `T: Model`.
    pub async fn query<T: Model>(&mut self, sql: &str, params: &[Value]) -> Result<Vec<T>, DbError> {
        let rows = self.query_rows(sql, params).await?;
        rows.iter().map(|r| T::from_row(r)).collect()
    }

    /// Commit this transaction.
    pub async fn commit(self) -> Result<(), DbError> {
        self.0.commit().await.map_err(|e| DbError::new(e.to_string()))
    }

    /// Roll back this transaction explicitly (also happens on drop).
    pub async fn rollback(self) -> Result<(), DbError> {
        self.0.rollback().await.map_err(|e| DbError::new(e.to_string()))
    }
}

// ── Internal helpers (SQLite) ─────────────────────────────────────────────────

#[cfg(feature = "model-sqlite")]
async fn pool_execute_impl(
    pool: &sqlx::Pool<Db>,
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
async fn pool_query_rows_impl(
    pool: &sqlx::Pool<Db>,
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
async fn tx_execute_impl(
    tx: &mut sqlx::Transaction<'static, Db>,
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
async fn tx_query_rows_impl(
    tx: &mut sqlx::Transaction<'static, Db>,
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
pub(crate) fn sqlite_row_to_model_row(
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

/// Return the last inserted row ID from a SQLite query result.
/// Used by the repository's INSERT logic.
#[cfg(feature = "model-sqlite")]
pub(crate) async fn sqlite_last_insert_id(
    pool: &sqlx::Pool<Db>,
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

#[cfg(feature = "model-sqlite")]
pub(crate) async fn sqlite_tx_last_insert_id(
    tx: &mut sqlx::Transaction<'static, Db>,
    sql: &str,
    params: &[Value],
) -> Result<i64, DbError> {
    let mut args = sqlx::sqlite::SqliteArguments::default();
    bind_sqlite_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.last_insert_rowid())
}

// ── Internal helpers (PostgreSQL) ─────────────────────────────────────────────

#[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
async fn pool_execute_impl(
    pool: &sqlx::Pool<Db>,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::postgres::PgArguments::default();
    bind_pg_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.rows_affected())
}

#[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
async fn pool_query_rows_impl(
    pool: &sqlx::Pool<Db>,
    sql: &str,
    params: &[Value],
) -> Result<Vec<ModelRow>, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::postgres::PgArguments::default();
    bind_pg_args(&mut args, params)?;
    let rows = sqlx::query_with(sql, args)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    rows.into_iter().map(pg_row_to_model_row).collect()
}

#[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
async fn tx_execute_impl(
    tx: &mut sqlx::Transaction<'static, Db>,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::postgres::PgArguments::default();
    bind_pg_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.rows_affected())
}

#[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
async fn tx_query_rows_impl(
    tx: &mut sqlx::Transaction<'static, Db>,
    sql: &str,
    params: &[Value],
) -> Result<Vec<ModelRow>, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::postgres::PgArguments::default();
    bind_pg_args(&mut args, params)?;
    let rows = sqlx::query_with(sql, args)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    rows.into_iter().map(pg_row_to_model_row).collect()
}

#[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
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

#[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
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
#[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
pub(crate) async fn pg_insert_returning(
    pool: &sqlx::Pool<Db>,
    sql: &str,
    params: &[Value],
    pk_col: &str,
) -> Result<i64, DbError> {
    use sqlx::Arguments;
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

#[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
pub(crate) async fn pg_tx_insert_returning(
    tx: &mut sqlx::Transaction<'static, Db>,
    sql: &str,
    params: &[Value],
    pk_col: &str,
) -> Result<i64, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::postgres::PgArguments::default();
    bind_pg_args(&mut args, params)?;
    let returning_sql = format!("{} RETURNING {}", sql, pk_col);
    let row = sqlx::query_with(&returning_sql, args)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    use sqlx::Row;
    let id: i64 = row.try_get(pk_col).map_err(|e| DbError::new(e.to_string()))?;
    Ok(id)
}

// ── Internal helpers (MySQL) ──────────────────────────────────────────────────

#[cfg(all(
    feature = "model-mysql",
    not(feature = "model-sqlite"),
    not(feature = "model-postgres")
))]
async fn pool_execute_impl(
    pool: &sqlx::Pool<Db>,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.rows_affected())
}

#[cfg(all(
    feature = "model-mysql",
    not(feature = "model-sqlite"),
    not(feature = "model-postgres")
))]
async fn pool_query_rows_impl(
    pool: &sqlx::Pool<Db>,
    sql: &str,
    params: &[Value],
) -> Result<Vec<ModelRow>, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let rows = sqlx::query_with(sql, args)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    rows.into_iter().map(mysql_row_to_model_row).collect()
}

#[cfg(all(
    feature = "model-mysql",
    not(feature = "model-sqlite"),
    not(feature = "model-postgres")
))]
async fn tx_execute_impl(
    tx: &mut sqlx::Transaction<'static, Db>,
    sql: &str,
    params: &[Value],
) -> Result<u64, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.rows_affected())
}

#[cfg(all(
    feature = "model-mysql",
    not(feature = "model-sqlite"),
    not(feature = "model-postgres")
))]
async fn tx_query_rows_impl(
    tx: &mut sqlx::Transaction<'static, Db>,
    sql: &str,
    params: &[Value],
) -> Result<Vec<ModelRow>, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let rows = sqlx::query_with(sql, args)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    rows.into_iter().map(mysql_row_to_model_row).collect()
}

#[cfg(all(
    feature = "model-mysql",
    not(feature = "model-sqlite"),
    not(feature = "model-postgres")
))]
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

#[cfg(all(
    feature = "model-mysql",
    not(feature = "model-sqlite"),
    not(feature = "model-postgres")
))]
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

#[cfg(all(
    feature = "model-mysql",
    not(feature = "model-sqlite"),
    not(feature = "model-postgres")
))]
pub(crate) async fn mysql_last_insert_id(
    pool: &sqlx::Pool<Db>,
    sql: &str,
    params: &[Value],
) -> Result<i64, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(pool)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.last_insert_id() as i64)
}

#[cfg(all(
    feature = "model-mysql",
    not(feature = "model-sqlite"),
    not(feature = "model-postgres")
))]
pub(crate) async fn mysql_tx_last_insert_id(
    tx: &mut sqlx::Transaction<'static, Db>,
    sql: &str,
    params: &[Value],
) -> Result<i64, DbError> {
    use sqlx::Arguments;
    let mut args = sqlx::mysql::MySqlArguments::default();
    bind_mysql_args(&mut args, params)?;
    let r = sqlx::query_with(sql, args)
        .execute(&mut **tx)
        .await
        .map_err(|e| DbError::new(e.to_string()))?;
    Ok(r.last_insert_id() as i64)
}
