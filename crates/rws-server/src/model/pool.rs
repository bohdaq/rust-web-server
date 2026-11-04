//! Async connection pool — `DbPool` and `DbTransaction`.
//!
//! Backed by `sqlx`. `model-sqlite`, `model-postgres`, and `model-mysql` are
//! not mutually exclusive: each compiled-in feature adds a variant to the
//! `DbPool`/`DbTransaction` enums below, so a single binary can hold pools to
//! more than one backend at once. Which variant a given `DbPool` is depends
//! on the [`Backend`] passed via [`DbConfig::backend`].

use std::future::Future;

use super::backend::Backend;
use super::connection::{DbConfig, DbTls};
use super::migration;
use super::{DbError, Model, ModelRow, MigrationStatus, Value};

// ── Connect options ───────────────────────────────────────────────────────────

#[cfg(feature = "model-postgres")]
#[doc(hidden)]
pub fn pg_options(config: &DbConfig, tls: &DbTls) -> sqlx::postgres::PgConnectOptions {
    use super::connection::DbSslMode;
    use sqlx::postgres::{PgConnectOptions, PgSslMode};
    let mut options = PgConnectOptions::new_without_pgpass()
        .host(&config.host)
        .port(config.port)
        .username(&config.user)
        .password(&config.password)
        .database(&config.database)
        .ssl_mode(match tls.mode {
            DbSslMode::Disable => PgSslMode::Disable,
            DbSslMode::Prefer => PgSslMode::Prefer,
            DbSslMode::Require => PgSslMode::Require,
            DbSslMode::VerifyCa => PgSslMode::VerifyCa,
            DbSslMode::VerifyFull => PgSslMode::VerifyFull,
        });
    if let Some(ca) = &tls.root_cert {
        options = options.ssl_root_cert(ca);
    }
    if let (Some(cert), Some(key)) = (&tls.client_cert, &tls.client_key) {
        options = options.ssl_client_cert(cert).ssl_client_key(key);
    }
    options
}

#[cfg(feature = "model-mysql")]
#[doc(hidden)]
pub fn mysql_options(config: &DbConfig, tls: &DbTls) -> sqlx::mysql::MySqlConnectOptions {
    use super::connection::DbSslMode;
    use sqlx::mysql::{MySqlConnectOptions, MySqlSslMode};
    let mut options = MySqlConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .username(&config.user)
        .password(&config.password)
        .database(&config.database)
        .ssl_mode(match tls.mode {
            DbSslMode::Disable => MySqlSslMode::Disabled,
            DbSslMode::Prefer => MySqlSslMode::Preferred,
            DbSslMode::Require => MySqlSslMode::Required,
            DbSslMode::VerifyCa => MySqlSslMode::VerifyCa,
            DbSslMode::VerifyFull => MySqlSslMode::VerifyIdentity,
        });
    if let Some(ca) = &tls.root_cert {
        options = options.ssl_ca(ca);
    }
    if let (Some(cert), Some(key)) = (&tls.client_cert, &tls.client_key) {
        options = options.ssl_client_cert(cert).ssl_client_key(key);
    }
    options
}

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
    /// Create a new pool for `config.backend`, with the given configuration
    /// and the default TLS ([`DbTls::default`]: TLS when the server offers
    /// it, certificate not checked).
    pub async fn new(config: DbConfig) -> Result<Self, DbError> {
        DbPool::new_with_tls(config, DbTls::default()).await
    }

    /// As [`DbPool::new`], with `tls` for PostgreSQL and MySQL (SQLite
    /// ignores it). User and password are passed as they are, not through a
    /// URL, so any characters work.
    pub async fn new_with_tls(config: DbConfig, tls: DbTls) -> Result<Self, DbError> {
        let _ = &tls;
        // sqlx builds its rustls config with `ClientConfig::builder()`, which
        // panics when no process-wide crypto provider is installed — so far
        // only rws's TLS listener installed one, and an app serving plain
        // HTTP behind a load balancer panicked at its first TLS connection to
        // the database. Install rws's (aws-lc-rs) unless one is set already.
        #[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
        let _ = crate::crypto_provider().install_default();
        match config.backend {
            #[cfg(feature = "model-sqlite")]
            Backend::Sqlite => {
                let pool = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
                    .max_connections(config.pool_size)
                    .connect(&config.to_url())
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbPool::Sqlite(pool))
            }
            #[cfg(feature = "model-postgres")]
            Backend::Postgres => {
                let pool = sqlx::pool::PoolOptions::<sqlx::Postgres>::new()
                    .max_connections(config.pool_size)
                    .connect_with(pg_options(&config, &tls))
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbPool::Postgres(pool))
            }
            #[cfg(feature = "model-mysql")]
            Backend::MySql => {
                let pool = sqlx::pool::PoolOptions::<sqlx::MySql>::new()
                    .max_connections(config.pool_size)
                    .connect_with(mysql_options(&config, &tls))
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbPool::MySql(pool))
            }
        }
    }

    /// Create a pool using [`DbConfig::from_env`] and [`DbTls::from_env`].
    pub async fn from_env() -> Result<Self, DbError> {
        DbPool::new_with_tls(DbConfig::from_env()?, DbTls::from_env()?).await
    }

    /// Create a pool backed by a SQLite in-memory database.
    ///
    /// All connections in the pool share the same in-memory database
    /// (max_connections = 1).  Each call returns an independent, isolated
    /// database — ideal for tests.
    ///
    /// ```rust,no_run
    /// # extern crate rws_server as rust_web_server;
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
        let span = db_span(self.backend(), sql);
        let result = match self {
            #[cfg(feature = "model-sqlite")]
            DbPool::Sqlite(pool) => sqlite_pool_execute(pool, sql, params).await,
            #[cfg(feature = "model-postgres")]
            DbPool::Postgres(pool) => pg_pool_execute(pool, sql, params).await,
            #[cfg(feature = "model-mysql")]
            DbPool::MySql(pool) => mysql_pool_execute(pool, sql, params).await,
        };
        finish_db_span(span, &result);
        result
    }

    /// Execute a SQL query and return untyped rows.
    pub async fn query_rows(&self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        let span = db_span(self.backend(), sql);
        let result = match self {
            #[cfg(feature = "model-sqlite")]
            DbPool::Sqlite(pool) => sqlite_pool_query_rows(pool, sql, params).await,
            #[cfg(feature = "model-postgres")]
            DbPool::Postgres(pool) => pg_pool_query_rows(pool, sql, params).await,
            #[cfg(feature = "model-mysql")]
            DbPool::MySql(pool) => mysql_pool_query_rows(pool, sql, params).await,
        };
        finish_db_span(span, &result);
        result
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
    #[doc(hidden)]
    pub async fn insert_returning_id(
        &self,
        insert_sql: &str,
        params: &[Value],
        _pk_col: &str,
    ) -> Result<i64, DbError> {
        let span = db_span(self.backend(), insert_sql);
        let result = match self {
            #[cfg(feature = "model-sqlite")]
            DbPool::Sqlite(pool) => sqlite_last_insert_id(pool, insert_sql, params).await,
            #[cfg(feature = "model-postgres")]
            DbPool::Postgres(pool) => pg_insert_returning(pool, insert_sql, params, _pk_col).await,
            #[cfg(feature = "model-mysql")]
            DbPool::MySql(pool) => mysql_last_insert_id(pool, insert_sql, params).await,
        };
        finish_db_span(span, &result);
        result
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
    /// The closure owns the transaction: call `tx.commit()` before returning
    /// `Ok`. A transaction that isn't committed — the closure returns `Err`,
    /// or `Ok` without committing — is dropped, which rolls it back.
    ///
    /// ```rust,no_run
    /// # extern crate rws_server as rust_web_server;
    /// # #[cfg(feature = "model-sqlite")]
    /// # async fn example() -> Result<(), rust_web_server::model::DbError> {
    /// use rust_web_server::model::{DbPool, Value};
    ///
    /// let pool = DbPool::memory().await?;
    /// let result = pool.transaction(|mut tx| async move {
    ///     tx.execute("INSERT INTO t (v) VALUES (?)", &[Value::Text("hi".into())]).await?;
    ///     tx.commit().await?;
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

    /// Run the seed files in `dir` (18.30.0): every `*.sql` file, in
    /// lexicographic order, each in its own transaction (statements split
    /// at `;`, as migrations). Unlike migrations nothing records that a
    /// seed ran — it runs again every time, so write seeds that can (`INSERT
    /// … ON CONFLICT DO NOTHING` on PostgreSQL and SQLite, `INSERT IGNORE`
    /// on MySQL). Answers the files run; stops at the first that fails,
    /// whose statements are rolled back.
    pub async fn seed(&self, dir: &str) -> Result<Vec<String>, DbError> {
        migration::run_seeds(self, dir).await
    }

    /// A pool for a database URL (18.30.0): `sqlite:<path>` (or
    /// `sqlite::memory:`), `postgres://user:password@host:port/db` (also
    /// `postgresql://`), `mysql://user:password@host:port/db` (also
    /// `mariadb://`) — sqlx's syntax, query parameters included (`sslmode`,
    /// `ssl-mode`, …). The backend's `model-*` feature must be compiled in.
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        let url = url.trim();
        let scheme = url.split(':').next().unwrap_or("").to_ascii_lowercase();
        let backend = match scheme.as_str() {
            "sqlite" => "sqlite",
            "postgres" | "postgresql" => "postgres",
            "mysql" | "mariadb" => "mysql",
            _ => {
                return Err(DbError::new(format!(
                    "database URL {:?}: expected sqlite:<path>, postgres://… or mysql://…",
                    redact_url(url)
                )))
            }
        };
        let backend = Backend::parse(backend)
            .map_err(|e| DbError::new(format!("database URL {:?}: {}", redact_url(url), e.0)))?;
        #[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
        let _ = crate::crypto_provider().install_default();
        let bad_url = |e: sqlx::Error| DbError::new(format!("database URL {:?}: {e}", redact_url(url)));
        match backend {
            #[cfg(feature = "model-sqlite")]
            Backend::Sqlite => {
                let options: sqlx::sqlite::SqliteConnectOptions = url.parse().map_err(bad_url)?;
                let pool = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
                    .max_connections(1)
                    .connect_with(options)
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbPool::Sqlite(pool))
            }
            #[cfg(feature = "model-postgres")]
            Backend::Postgres => {
                let options: sqlx::postgres::PgConnectOptions = url.parse().map_err(bad_url)?;
                let pool = sqlx::pool::PoolOptions::<sqlx::Postgres>::new()
                    .max_connections(4)
                    .connect_with(options)
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbPool::Postgres(pool))
            }
            #[cfg(feature = "model-mysql")]
            Backend::MySql => {
                let options: sqlx::mysql::MySqlConnectOptions = url.parse().map_err(bad_url)?;
                let pool = sqlx::pool::PoolOptions::<sqlx::MySql>::new()
                    .max_connections(4)
                    .connect_with(options)
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(DbPool::MySql(pool))
            }
        }
    }
}

/// `url` with the password replaced by `***`, for error messages.
#[doc(hidden)]
pub fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    match authority.rsplit_once('@') {
        Some((userinfo, host)) => match userinfo.split_once(':') {
            Some((user, _)) => format!("{scheme}://{user}:***@{host}{tail}"),
            None => url.to_string(),
        },
        None => url.to_string(),
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
    fn backend(&self) -> Backend {
        match self {
            #[cfg(feature = "model-sqlite")]
            DbTransaction::Sqlite(_) => Backend::Sqlite,
            #[cfg(feature = "model-postgres")]
            DbTransaction::Postgres(_) => Backend::Postgres,
            #[cfg(feature = "model-mysql")]
            DbTransaction::MySql(_) => Backend::MySql,
        }
    }

    /// Execute a SQL statement inside this transaction.
    pub async fn execute(&mut self, sql: &str, params: &[Value]) -> Result<u64, DbError> {
        let span = db_span(self.backend(), sql);
        let result = match self {
            #[cfg(feature = "model-sqlite")]
            DbTransaction::Sqlite(tx) => sqlite_tx_execute(tx, sql, params).await,
            #[cfg(feature = "model-postgres")]
            DbTransaction::Postgres(tx) => pg_tx_execute(tx, sql, params).await,
            #[cfg(feature = "model-mysql")]
            DbTransaction::MySql(tx) => mysql_tx_execute(tx, sql, params).await,
        };
        finish_db_span(span, &result);
        result
    }

    /// Execute a SQL query inside this transaction, returning untyped rows.
    pub async fn query_rows(&mut self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        let span = db_span(self.backend(), sql);
        let result = match self {
            #[cfg(feature = "model-sqlite")]
            DbTransaction::Sqlite(tx) => sqlite_tx_query_rows(tx, sql, params).await,
            #[cfg(feature = "model-postgres")]
            DbTransaction::Postgres(tx) => pg_tx_query_rows(tx, sql, params).await,
            #[cfg(feature = "model-mysql")]
            DbTransaction::MySql(tx) => mysql_tx_query_rows(tx, sql, params).await,
        };
        finish_db_span(span, &result);
        result
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
            // TEXT, VARCHAR, CHAR → string; a declared type sqlx won't read
            // as one (DATETIME, DATE, …) → whatever the value is stored as.
            match row.try_get::<Option<String>, _>(col.ordinal()) {
                Ok(v) => v.map(Value::Text).unwrap_or(Value::Null),
                Err(_) => sqlite_stored_value(&row, col.ordinal()),
            }
        };
        cols.push((name, value));
    }
    Ok(ModelRow::new(cols))
}

/// A SQLite value by its storage class, whatever the column's declared type.
#[cfg(feature = "model-sqlite")]
fn sqlite_stored_value(row: &sqlx::sqlite::SqliteRow, ordinal: usize) -> Value {
    use sqlx::{Row, TypeInfo, ValueRef};
    let Ok(raw) = row.try_get_raw(ordinal) else { return Value::Null };
    if raw.is_null() {
        return Value::Null;
    }
    let class = raw.type_info().name().to_string();
    match class.as_str() {
        "INTEGER" => row.try_get_unchecked::<i64, _>(ordinal).map(Value::Int).unwrap_or(Value::Null),
        "REAL" => row.try_get_unchecked::<f64, _>(ordinal).map(Value::Float).unwrap_or(Value::Null),
        "BLOB" => row.try_get_unchecked::<Vec<u8>, _>(ordinal).map(Value::Bytes).unwrap_or(Value::Null),
        _ => row.try_get_unchecked::<String, _>(ordinal).map(Value::Text).unwrap_or(Value::Null),
    }
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
        // sqlx decodes a Rust type only from the PostgreSQL types it maps
        // to (an `i64` from INT8 alone): each width is read as itself.
        let value = match type_name {
            "BOOL" => row.try_get::<Option<bool>, _>(col.ordinal())
                .map(|v| v.map(Value::Bool).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
            "INT2" => row.try_get::<Option<i16>, _>(col.ordinal())
                .map(|v| v.map(|n| Value::Int(n as i64)).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
            "INT4" => row.try_get::<Option<i32>, _>(col.ordinal())
                .map(|v| v.map(|n| Value::Int(n as i64)).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
            "INT8" => row.try_get::<Option<i64>, _>(col.ordinal())
                .map(|v| v.map(Value::Int).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
            "FLOAT4" => row.try_get::<Option<f32>, _>(col.ordinal())
                .map(|v| v.map(|f| Value::Float(f as f64)).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
            "FLOAT8" | "NUMERIC" => row.try_get::<Option<f64>, _>(col.ordinal())
                .map(|v| v.map(Value::Float).unwrap_or(Value::Null))
                .unwrap_or(Value::Null),
            "TIMESTAMPTZ" | "TIMESTAMP" | "DATE" | "UUID" => {
                use sqlx::ValueRef;
                match row.try_get_raw(col.ordinal()) {
                    Ok(raw) if !raw.is_null() => {
                        let binary = matches!(raw.format(), sqlx::postgres::PgValueFormat::Binary);
                        raw.as_bytes().ok().and_then(|bytes| super::decode::pg_value(type_name, binary, bytes)).unwrap_or(Value::Null)
                    }
                    _ => Value::Null,
                }
            }
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
    // Through the row conversion, so an INT4 (SERIAL) key reads as well as
    // an INT8 (BIGSERIAL) one.
    match pg_row_to_model_row(row)?.columns.into_iter().next() {
        Some((_, Value::Int(id))) => Ok(id),
        other => Err(DbError::new(format!("INSERT … RETURNING {pk_col}: expected an integer key, got {other:?}"))),
    }
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
            match row.try_get::<Option<i64>, _>(col.ordinal()) {
                Ok(v) => v.map(Value::Int).unwrap_or(Value::Null),
                // UNSIGNED columns decode as u64 only.
                Err(_) => row
                    .try_get::<Option<u64>, _>(col.ordinal())
                    .map(|v| v.map(|n| Value::Int(n as i64)).unwrap_or(Value::Null))
                    .unwrap_or(Value::Null),
            }
        } else if matches!(type_name.as_str(), "DATETIME" | "TIMESTAMP" | "DATE") {
            // sqlx decodes these only into chrono/time types; read the wire
            // bytes instead (decoded without its type check).
            row.try_get_unchecked::<Option<Vec<u8>>, _>(col.ordinal())
                .ok()
                .flatten()
                .and_then(|bytes| super::decode::mysql_temporal(&type_name, &bytes))
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

// ── tracing ───────────────────────────────────────────────────────────────────

/// A client span for one statement, inside a traced request: named after the
/// operation (`SELECT`, `INSERT`, …) with `db.system.name` and
/// `db.operation.name` — not the query text, which can carry literals.
fn db_span(backend: Backend, sql: &str) -> Option<crate::otel::DetachedSpan> {
    let operation = sql_operation(sql);
    let mut span = crate::otel::detached_span(operation, crate::otel::SpanKind::Client)?;
    let system = match backend {
        #[cfg(feature = "model-sqlite")]
        Backend::Sqlite => "sqlite",
        #[cfg(feature = "model-postgres")]
        Backend::Postgres => "postgresql",
        #[cfg(feature = "model-mysql")]
        Backend::MySql => "mysql",
    };
    span.set_attribute("db.system.name", system);
    span.set_attribute("db.operation.name", operation);
    Some(span)
}

#[cfg(all(test, feature = "model-sqlite"))]
#[doc(hidden)]
pub fn db_span_for_test(sql: &str) -> Option<crate::otel::DetachedSpan> {
    db_span(Backend::Sqlite, sql)
}

fn finish_db_span<T>(span: Option<crate::otel::DetachedSpan>, result: &Result<T, DbError>) {
    if let (Some(mut span), Err(e)) = (span, result) {
        span.record_error(&e.to_string());
    }
}

/// The statement's first keyword, upper-cased (`"query"` when there's none).
#[doc(hidden)]
pub fn sql_operation(sql: &str) -> &'static str {
    const KNOWN: [&str; 14] = [
        "SELECT", "INSERT", "UPDATE", "DELETE", "WITH", "CREATE", "ALTER", "DROP", "BEGIN", "COMMIT", "ROLLBACK", "PRAGMA", "REPLACE", "TRUNCATE",
    ];
    let word = sql.trim_start().split(|c: char| !c.is_ascii_alphabetic()).next().unwrap_or("");
    KNOWN.iter().find(|k| k.eq_ignore_ascii_case(word)).copied().unwrap_or("query")
}
