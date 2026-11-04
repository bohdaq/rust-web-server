//! The databases the model tests run on (18.30.0): SQLite in memory always
//! (with `model-sqlite`), and PostgreSQL / MySQL when `RWS_TEST_POSTGRES_URL`
//! / `RWS_TEST_MYSQL_URL` name a server (sqlx URLs, as `TestDb` reads them).
//! A test gets a schema (PostgreSQL) or database (MySQL) of its own, made
//! for it and dropped by [`TestBackend::finish`], so tests run in parallel,
//! commit for real and may run DDL. Without a URL the backend is skipped
//! with a message on stderr; `RWS_TEST_REQUIRE_BACKENDS=postgres,mysql`
//! makes a missing one fail the test instead (CI sets it, so a typo in the
//! job can't turn into a silent pass).

use super::{Backend, DbPool};

/// One database for one test.
pub(crate) struct TestBackend {
    pub(crate) pool: DbPool,
    pub(crate) name: &'static str,
    #[allow(dead_code)]
    cleanup: Cleanup,
}

enum Cleanup {
    #[allow(dead_code)]
    None,
    #[cfg(feature = "model-postgres")]
    PgSchema(sqlx::Pool<sqlx::Postgres>, String),
    #[cfg(feature = "model-mysql")]
    MySqlDatabase(sqlx::Pool<sqlx::MySql>, String),
}

impl TestBackend {
    pub(crate) fn backend(&self) -> Backend {
        self.pool.backend()
    }

    /// `sql` with this backend's spelling of the type tokens (see [`ddl`]).
    pub(crate) fn ddl(&self, sql: &str) -> String {
        ddl(self.backend(), sql)
    }

    /// Run each `;`-separated DDL statement of `sql` (tokens replaced).
    pub(crate) async fn create(&self, sql: &str) {
        for stmt in self.ddl(sql).split(';').map(str::trim).filter(|s| !s.is_empty()) {
            if let Err(e) = self.pool.execute(stmt, &[]).await {
                panic!("[{}] {stmt}: {e}", self.name);
            }
        }
    }

    /// Drop the test's schema or database.
    pub(crate) async fn finish(self) {
        let TestBackend { pool, cleanup, .. } = self;
        close(&pool).await;
        match cleanup {
            Cleanup::None => {}
            #[cfg(feature = "model-postgres")]
            Cleanup::PgSchema(admin, schema) => {
                let _ = sqlx::query(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE")).execute(&admin).await;
                admin.close().await;
            }
            #[cfg(feature = "model-mysql")]
            Cleanup::MySqlDatabase(admin, database) => {
                let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {database}")).execute(&admin).await;
                admin.close().await;
            }
        }
    }
}

async fn close(pool: &DbPool) {
    pool.close().await;
}

/// Replace the type tokens in `sql`:
///
/// | Token | SQLite | PostgreSQL | MySQL |
/// |---|---|---|---|
/// | `{pk}` | `INTEGER PRIMARY KEY AUTOINCREMENT` | `BIGSERIAL PRIMARY KEY` | `BIGINT AUTO_INCREMENT PRIMARY KEY` |
/// | `{pk32}` | `INTEGER PRIMARY KEY AUTOINCREMENT` | `SERIAL PRIMARY KEY` | `INT AUTO_INCREMENT PRIMARY KEY` |
/// | `{text}` | `TEXT` | `TEXT` | `VARCHAR(255)` |
/// | `{ts}` | `TEXT` | `TIMESTAMPTZ` | `DATETIME(6)` |
/// | `{uuid}` | `TEXT` | `TEXT` | `CHAR(36)` |
/// | `{engine}` | | | ` ENGINE=InnoDB` |
pub(crate) fn ddl(backend: Backend, sql: &str) -> String {
    let tokens: [(&str, &str); 6] = match backend {
        #[cfg(feature = "model-sqlite")]
        Backend::Sqlite => [
            ("{pk}", "INTEGER PRIMARY KEY AUTOINCREMENT"),
            ("{pk32}", "INTEGER PRIMARY KEY AUTOINCREMENT"),
            ("{text}", "TEXT"),
            ("{ts}", "TEXT"),
            ("{uuid}", "TEXT"),
            ("{engine}", ""),
        ],
        #[cfg(feature = "model-postgres")]
        Backend::Postgres => [
            ("{pk}", "BIGSERIAL PRIMARY KEY"),
            ("{pk32}", "SERIAL PRIMARY KEY"),
            ("{text}", "TEXT"),
            ("{ts}", "TIMESTAMPTZ"),
            ("{uuid}", "TEXT"),
            ("{engine}", ""),
        ],
        #[cfg(feature = "model-mysql")]
        Backend::MySql => [
            ("{pk}", "BIGINT AUTO_INCREMENT PRIMARY KEY"),
            ("{pk32}", "INT AUTO_INCREMENT PRIMARY KEY"),
            ("{text}", "VARCHAR(255)"),
            ("{ts}", "DATETIME(6)"),
            ("{uuid}", "CHAR(36)"),
            ("{engine}", " ENGINE=InnoDB"),
        ],
    };
    let mut out = sql.to_string();
    for (token, spelling) in tokens {
        out = out.replace(token, spelling);
    }
    out
}

/// Every backend this build and environment can test, each with a fresh
/// database.
pub(crate) async fn all() -> Vec<TestBackend> {
    #[allow(unused_mut)]
    let mut out = Vec::new();
    #[cfg(feature = "model-sqlite")]
    out.push(TestBackend { pool: DbPool::memory().await.expect("SQLite in memory"), name: "sqlite", cleanup: Cleanup::None });
    #[cfg(feature = "model-postgres")]
    if let Some(url) = url("RWS_TEST_POSTGRES_URL", "postgres") {
        out.push(postgres(&url).await);
    }
    #[cfg(feature = "model-mysql")]
    if let Some(url) = url("RWS_TEST_MYSQL_URL", "mysql") {
        out.push(mysql(&url).await);
    }
    out
}

/// The URL in `var`; `None` (after saying so) when unset — a failure when
/// `RWS_TEST_REQUIRE_BACKENDS` lists `name`.
#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
fn url(var: &str, name: &str) -> Option<String> {
    match std::env::var(var).map(|v| v.trim().to_string()) {
        Ok(url) if !url.is_empty() => Some(url),
        _ => {
            let required = std::env::var("RWS_TEST_REQUIRE_BACKENDS").unwrap_or_default();
            if required.split(',').any(|r| r.trim().eq_ignore_ascii_case(name)) {
                panic!("RWS_TEST_REQUIRE_BACKENDS lists {name}, but {var} is not set");
            }
            eprintln!("skipped {name}: {var} is not set");
            None
        }
    }
}

#[cfg(any(feature = "model-postgres", feature = "model-mysql"))]
fn unique_name() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    format!("rws_t_{}_{}_{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed), nanos % 100_000)
}

#[cfg(feature = "model-postgres")]
async fn postgres(url: &str) -> TestBackend {
    let _ = crate::crypto_provider().install_default();
    let options: sqlx::postgres::PgConnectOptions = url.parse().expect("RWS_TEST_POSTGRES_URL");
    let admin = sqlx::pool::PoolOptions::<sqlx::Postgres>::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap_or_else(|e| panic!("RWS_TEST_POSTGRES_URL: cannot connect: {e}"));
    let schema = unique_name();
    sqlx::query(&format!("CREATE SCHEMA {schema}")).execute(&admin).await.expect("CREATE SCHEMA");
    let pool = sqlx::pool::PoolOptions::<sqlx::Postgres>::new()
        .max_connections(4)
        .connect_with(options.options([("search_path", schema.as_str())]))
        .await
        .expect("connect to the test schema");
    TestBackend { pool: DbPool::Postgres(pool), name: "postgres", cleanup: Cleanup::PgSchema(admin, schema) }
}

#[cfg(feature = "model-mysql")]
async fn mysql(url: &str) -> TestBackend {
    let _ = crate::crypto_provider().install_default();
    let options: sqlx::mysql::MySqlConnectOptions = url.parse().expect("RWS_TEST_MYSQL_URL");
    let admin = sqlx::pool::PoolOptions::<sqlx::MySql>::new()
        .max_connections(1)
        .connect_with(options.clone())
        .await
        .unwrap_or_else(|e| panic!("RWS_TEST_MYSQL_URL: cannot connect: {e}"));
    let database = unique_name();
    sqlx::query(&format!("CREATE DATABASE {database}")).execute(&admin).await.expect("CREATE DATABASE (the test user needs the privilege)");
    let pool = sqlx::pool::PoolOptions::<sqlx::MySql>::new()
        .max_connections(4)
        .connect_with(options.database(&database))
        .await
        .expect("connect to the test database");
    TestBackend { pool: DbPool::MySql(pool), name: "mysql", cleanup: Cleanup::MySqlDatabase(admin, database) }
}
