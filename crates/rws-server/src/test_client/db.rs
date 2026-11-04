//! A database per test that leaves nothing behind.
//!
//! [`TestDb`] hands out a [`DbPool`] (deref, or [`TestDb::pool`] for a clone
//! the app's state can own), so a test builds its app on it and drives it
//! with [`TestClient`](super::TestClient) as usual:
//!
//! ```rust,no_run
//! # extern crate rws_server as rust_web_server;
//! # #[cfg(feature = "model-sqlite")]
//! # async fn example() -> Result<(), rust_web_server::model::DbError> {
//! use rust_web_server::test_client::TestDb;
//!
//! let db = TestDb::sqlite().migrations("migrations").await?;
//! db.execute("INSERT INTO notes (text) VALUES ('hi')", &[]).await?;
//! let app_state_pool = db.pool(); // cheap clone, same database
//! # let _ = app_state_pool;
//! # Ok(())
//! # }
//! ```
//!
//! How each backend isolates a test:
//!
//! - **SQLite** ([`TestDb::sqlite`]): a fresh in-memory database, its own
//!   per `TestDb`, with the migrations applied. It disappears when the last
//!   clone of the pool is dropped. Nothing is shared between tests, so they
//!   run in parallel.
//! - **PostgreSQL / MySQL** ([`TestDb::postgres`], [`TestDb::mysql`], or the
//!   `_from_env` forms reading `RWS_TEST_POSTGRES_URL` / `RWS_TEST_MYSQL_URL`):
//!   the migrations are applied to the database for real (once per process
//!   and directory, serialized), then the pool is pinned to **one
//!   connection** that is inside a transaction from the moment it connects.
//!   The test's writes stay in that transaction, which is never committed:
//!   [`TestDb::rollback`] rolls it back, and dropping the `TestDb` closes
//!   the pool, so the server rolls it back when the connection goes.
//!
//! Limits of the transaction (the usual ones for transactional tests):
//!
//! - Use a database of its own for tests: the schema (migrations) persists.
//! - One connection: code that holds two connections at once (two
//!   `DbTransaction`s alive together, a query while iterating another) waits
//!   for itself. `DbPool::begin`/`transaction` work — sqlx turns them into
//!   savepoints inside the test's transaction, so their `commit` commits
//!   nothing.
//! - Raw `COMMIT` / `BEGIN` statements, and on MySQL DDL (`CREATE`, `ALTER`,
//!   `DROP`, `TRUNCATE` commit implicitly), do end the transaction:
//!   whatever ran before them stays. Put such statements in migrations.
//! - On PostgreSQL a failed statement aborts the test's transaction; every
//!   later statement fails until the test ends. Run statements expected to
//!   fail inside `DbPool::transaction` (a savepoint) instead.
//! - Concurrent tests on the same rows can block each other on row locks
//!   until one ends.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use crate::model::{Backend, DbError, DbPool};

/// A database for one test; see the module docs.
pub struct TestDb {
    pool: DbPool,
    /// The pool's one connection is inside the test's transaction.
    transactional: bool,
}

enum Target {
    #[cfg(feature = "model-sqlite")]
    Sqlite,
    #[cfg(feature = "model-postgres")]
    Postgres(String),
    #[cfg(feature = "model-mysql")]
    MySql(String),
}

/// A [`TestDb`] being set up: add migration directories, then `.await` it
/// (or call [`build`](Self::build)).
#[must_use = "a TestDbBuilder does nothing until it is awaited"]
pub struct TestDbBuilder {
    target: Target,
    migrations: Vec<String>,
}

impl TestDb {
    /// A fresh SQLite in-memory database.
    #[cfg(feature = "model-sqlite")]
    pub fn sqlite() -> TestDbBuilder {
        TestDbBuilder { target: Target::Sqlite, migrations: Vec::new() }
    }

    /// The PostgreSQL database at `url` (`postgres://user:password@host/db`,
    /// sqlx's URL syntax), each test inside a transaction that's rolled back.
    #[cfg(feature = "model-postgres")]
    pub fn postgres(url: &str) -> TestDbBuilder {
        TestDbBuilder { target: Target::Postgres(url.to_string()), migrations: Vec::new() }
    }

    /// The MySQL database at `url` (`mysql://user:password@host/db`), each
    /// test inside a transaction that's rolled back.
    #[cfg(feature = "model-mysql")]
    pub fn mysql(url: &str) -> TestDbBuilder {
        TestDbBuilder { target: Target::MySql(url.to_string()), migrations: Vec::new() }
    }

    /// [`TestDb::postgres`] with the URL in `RWS_TEST_POSTGRES_URL`; `None`
    /// (after printing that the test is skipped) when it isn't set, so a
    /// test can return early on a machine without the server:
    ///
    /// ```rust,no_run
    /// # extern crate rws_server as rust_web_server;
    /// # #[cfg(feature = "model-postgres")]
    /// # async fn example() -> Result<(), rust_web_server::model::DbError> {
    /// use rust_web_server::test_client::TestDb;
    ///
    /// let Some(db) = TestDb::postgres_from_env() else { return Ok(()) };
    /// let db = db.migrations("migrations").await?;
    /// # let _ = db;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "model-postgres")]
    pub fn postgres_from_env() -> Option<TestDbBuilder> {
        url_from_env("RWS_TEST_POSTGRES_URL", "postgres://user:password@localhost/app_test").map(|url| TestDb::postgres(&url))
    }

    /// [`TestDb::mysql`] with the URL in `RWS_TEST_MYSQL_URL`; `None` (after
    /// printing that the test is skipped) when it isn't set.
    #[cfg(feature = "model-mysql")]
    pub fn mysql_from_env() -> Option<TestDbBuilder> {
        url_from_env("RWS_TEST_MYSQL_URL", "mysql://user:password@localhost/app_test").map(|url| TestDb::mysql(&url))
    }

    /// A clone of the pool (cheap: same connections, same database), for
    /// the app's state.
    pub fn pool(&self) -> DbPool {
        self.pool.clone()
    }

    /// Which backend this database is.
    pub fn backend(&self) -> Backend {
        self.pool.backend()
    }

    /// `true` for PostgreSQL / MySQL, whose writes are rolled back; `false`
    /// for SQLite, whose database is dropped instead.
    pub fn is_transactional(&self) -> bool {
        self.transactional
    }

    /// End the test now: roll the transaction back (PostgreSQL, MySQL) and
    /// close the pool. Dropping the `TestDb` does the same without waiting
    /// for it; this reports errors.
    pub async fn rollback(mut self) -> Result<(), DbError> {
        let transactional = std::mem::replace(&mut self.transactional, false);
        if !transactional {
            return Ok(());
        }
        match &self.pool {
            #[cfg(feature = "model-sqlite")]
            DbPool::Sqlite(_) => Ok(()),
            #[cfg(feature = "model-postgres")]
            DbPool::Postgres(pool) => rollback_and_close(pool).await,
            #[cfg(feature = "model-mysql")]
            DbPool::MySql(pool) => rollback_and_close(pool).await,
        }
    }
}

impl std::ops::Deref for TestDb {
    type Target = DbPool;
    fn deref(&self) -> &DbPool {
        &self.pool
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        if !self.transactional {
            return;
        }
        match &self.pool {
            #[cfg(feature = "model-sqlite")]
            DbPool::Sqlite(_) => {}
            #[cfg(feature = "model-postgres")]
            DbPool::Postgres(pool) => close_soon(pool.clone()),
            #[cfg(feature = "model-mysql")]
            DbPool::MySql(pool) => close_soon(pool.clone()),
        }
    }
}

impl TestDbBuilder {
    /// Apply the `*.sql` migrations in `dir` (see `DbPool::migrate`); call
    /// again for more directories, applied in order. Relative to the working
    /// directory — the package root under `cargo test`.
    pub fn migrations(mut self, dir: &str) -> Self {
        self.migrations.push(dir.to_string());
        self
    }

    /// Connect and migrate; the same as `.await`ing the builder.
    pub async fn build(self) -> Result<TestDb, DbError> {
        match self.target {
            #[cfg(feature = "model-sqlite")]
            Target::Sqlite => {
                // One connection that never expires: an in-memory database
                // lives as long as its connection.
                let pool = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
                    .max_connections(1)
                    .idle_timeout(None)
                    .max_lifetime(None)
                    .connect("sqlite::memory:")
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                let pool = DbPool::Sqlite(pool);
                for dir in &self.migrations {
                    pool.migrate(dir).await?;
                }
                Ok(TestDb { pool, transactional: false })
            }
            #[cfg(feature = "model-postgres")]
            Target::Postgres(url) => {
                let _ = crate::crypto_provider().install_default();
                let options: sqlx::postgres::PgConnectOptions =
                    url.parse().map_err(|e: sqlx::Error| DbError::new(format!("RWS test database URL: {e}")))?;
                migrate_once(&url, &self.migrations, || async {
                    let pool = sqlx::pool::PoolOptions::<sqlx::Postgres>::new()
                        .max_connections(1)
                        .connect_with(options.clone())
                        .await
                        .map_err(|e| DbError::new(e.to_string()))?;
                    Ok(DbPool::Postgres(pool))
                })
                .await?;
                let pool = transactional_pool::<sqlx::Postgres>()
                    .connect_with(options)
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(TestDb { pool: DbPool::Postgres(pool), transactional: true })
            }
            #[cfg(feature = "model-mysql")]
            Target::MySql(url) => {
                let _ = crate::crypto_provider().install_default();
                let options: sqlx::mysql::MySqlConnectOptions =
                    url.parse().map_err(|e: sqlx::Error| DbError::new(format!("RWS test database URL: {e}")))?;
                migrate_once(&url, &self.migrations, || async {
                    let pool = sqlx::pool::PoolOptions::<sqlx::MySql>::new()
                        .max_connections(1)
                        .connect_with(options.clone())
                        .await
                        .map_err(|e| DbError::new(e.to_string()))?;
                    Ok(DbPool::MySql(pool))
                })
                .await?;
                let pool = transactional_pool::<sqlx::MySql>()
                    .connect_with(options)
                    .await
                    .map_err(|e| DbError::new(e.to_string()))?;
                Ok(TestDb { pool: DbPool::MySql(pool), transactional: true })
            }
        }
    }
}

impl IntoFuture for TestDbBuilder {
    type Output = Result<TestDb, DbError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Result<TestDb, DbError>> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.build())
    }
}

#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
fn url_from_env(name: &str, example: &str) -> Option<String> {
    match std::env::var(name).map(|v| v.trim().to_string()) {
        Ok(url) if !url.is_empty() => Some(url),
        _ => {
            eprintln!("skipped: {name} is not set (the URL of a database for tests, e.g. {example})");
            None
        }
    }
}

/// Options for a pool of one connection that starts a transaction as it
/// connects (through sqlx's transaction manager, so `DbPool::begin` inside
/// it makes a savepoint) and is never closed for age or idleness — a new
/// connection would be outside the test's transaction.
#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
fn transactional_pool<DB: sqlx::Database>() -> sqlx::pool::PoolOptions<DB> {
    sqlx::pool::PoolOptions::<DB>::new()
        .max_connections(1)
        .min_connections(0)
        .idle_timeout(None)
        .max_lifetime(None)
        .after_connect(|conn, _meta| <DB::TransactionManager as sqlx::TransactionManager>::begin(conn, None))
}

#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
async fn rollback_and_close<DB: sqlx::Database>(pool: &sqlx::Pool<DB>) -> Result<(), DbError> {
    let result = async {
        let mut conn = pool.acquire().await.map_err(|e| DbError::new(e.to_string()))?;
        <DB::TransactionManager as sqlx::TransactionManager>::rollback(&mut conn).await.map_err(|e| DbError::new(e.to_string()))
    }
    .await;
    pool.close().await;
    result
}

/// Close `pool` from a destructor: closing it marks it closed at once (so
/// nothing more runs in the test's transaction); its connection closes in a
/// task on the current runtime — or, if the runtime ends first, when the
/// last clone is dropped. Either way the server rolls the transaction back
/// with the connection.
#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
fn close_soon<DB: sqlx::Database>(pool: sqlx::Pool<DB>) {
    drop(pool.close());
    if let Ok(runtime) = tokio::runtime::Handle::try_current() {
        runtime.spawn(async move { pool.close().await });
    }
}

/// Run `dirs`' migrations on a fresh pool from `connect`, unless this
/// process already did for `url` + dir. Serialized, so parallel tests don't
/// race to create the same tables.
#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
async fn migrate_once<F, Fut>(url: &str, dirs: &[String], connect: F) -> Result<(), DbError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<DbPool, DbError>>,
{
    use std::collections::HashSet;
    use std::sync::OnceLock;
    static DONE: OnceLock<tokio::sync::Mutex<HashSet<String>>> = OnceLock::new();
    if dirs.is_empty() {
        return Ok(());
    }
    let mut done = DONE.get_or_init(Default::default).lock().await;
    let keys: Vec<String> = dirs.iter().map(|dir| format!("{url}\n{dir}")).collect();
    if keys.iter().all(|key| done.contains(key)) {
        return Ok(());
    }
    let pool = connect().await?;
    let mut result = Ok(());
    for (dir, key) in dirs.iter().zip(keys) {
        if done.contains(&key) {
            continue;
        }
        if let Err(e) = pool.migrate(dir).await {
            result = Err(e);
            break;
        }
        done.insert(key);
    }
    match &pool {
        #[cfg(feature = "model-sqlite")]
        DbPool::Sqlite(p) => p.close().await,
        #[cfg(feature = "model-postgres")]
        DbPool::Postgres(p) => p.close().await,
        #[cfg(feature = "model-mysql")]
        DbPool::MySql(p) => p.close().await,
    }
    result
}
