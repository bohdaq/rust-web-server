//! Connection pool — `DbPool` and `PooledConnection`.

use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

use super::connection::DbConnection;
use super::{DbConfig, DbError};

// ── DbPool ────────────────────────────────────────────────────────────────────

/// A simple blocking connection pool.
///
/// Connections are pre-created at construction time and returned to the pool
/// when the `PooledConnection` guard is dropped.
pub struct DbPool {
    config: DbConfig,
    connections: Mutex<Vec<DbConnection>>,
}

impl DbPool {
    /// Create a new pool, opening `config.pool_size` connections.
    pub fn new(config: DbConfig) -> Result<Self, DbError> {
        let mut conns = Vec::with_capacity(config.pool_size);
        for _ in 0..config.pool_size {
            conns.push(DbConnection::open(&config)?);
        }
        Ok(DbPool {
            config,
            connections: Mutex::new(conns),
        })
    }

    /// Create a pool using `DbConfig::from_env()`.
    pub fn from_env() -> Result<Self, DbError> {
        DbPool::new(DbConfig::from_env()?)
    }

    /// Check out a connection from the pool.
    ///
    /// If the pool is empty and the configured database is a file or network
    /// backend, a new connection is opened on demand.
    ///
    /// **SQLite `:memory:` exception** — when the database is `":memory:"`,
    /// opening a new connection on overflow would create a *separate* empty
    /// database, silently discarding all data written by other connections.
    /// Instead, `get()` returns `Err` when the pool is exhausted so that the
    /// caller sees a clear error rather than operating on stale state.
    pub fn get(&self) -> Result<PooledConnection<'_>, DbError> {
        let conn = {
            let mut guard = self
                .connections
                .lock()
                .map_err(|e| DbError::new(format!("pool lock poisoned: {}", e)))?;
            guard.pop()
        };

        let conn = match conn {
            Some(c) => c,
            None => {
                // Guard against the silent empty-database bug for in-memory SQLite.
                #[cfg(feature = "model-sqlite")]
                if self.config.database == ":memory:" {
                    return Err(DbError::new(
                        "in-memory SQLite pool exhausted — all connections are in use. \
                         Use pool_size = 1 or wait for a connection to be returned.",
                    ));
                }
                DbConnection::open(&self.config)?
            }
        };

        Ok(PooledConnection {
            conn: Some(conn),
            pool: self,
        })
    }

    /// Create a pool backed by a SQLite in-memory database.
    ///
    /// Equivalent to `DbPool::new(DbConfig::memory())` — creates a single
    /// pooled connection to `":memory:"`. All callers that check out this
    /// connection via [`DbPool::get`] see the same data because they share
    /// the one pre-created connection.
    ///
    /// If the pool is exhausted (the single connection is already checked out),
    /// [`DbPool::get`] returns an error rather than opening a new empty
    /// in-memory database. See [`DbConfig::memory`] for details on the
    /// single-connection design.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "model-sqlite")]
    /// # {
    /// use rust_web_server::model::{DbPool, Value};
    ///
    /// let pool = DbPool::memory().unwrap();
    /// let mut conn = pool.get().unwrap();
    /// conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)", &[]).unwrap();
    /// conn.execute("INSERT INTO t (v) VALUES (?1)", &[Value::Text("hello".into())]).unwrap();
    /// # }
    /// ```
    #[cfg(feature = "model-sqlite")]
    pub fn memory() -> Result<Self, DbError> {
        DbPool::new(DbConfig::memory())
    }

    /// Return a connection to the pool.
    fn return_connection(&self, conn: DbConnection) {
        if let Ok(mut guard) = self.connections.lock() {
            guard.push(conn);
        }
    }
}

// ── PooledConnection ──────────────────────────────────────────────────────────

/// A connection checked out from `DbPool`. Returned to the pool on drop.
pub struct PooledConnection<'a> {
    conn: Option<DbConnection>,
    pool: &'a DbPool,
}

impl<'a> PooledConnection<'a> {
    /// Run a closure inside a transaction on this connection.
    pub fn transaction<F, T>(&mut self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&mut DbConnection) -> Result<T, DbError>,
    {
        let conn = self
            .conn
            .as_mut()
            .ok_or_else(|| DbError::new("connection already returned to pool"))?;
        conn.transaction(f)
    }
}

impl<'a> Deref for PooledConnection<'a> {
    type Target = DbConnection;

    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().expect("connection already returned to pool")
    }
}

impl<'a> DerefMut for PooledConnection<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().expect("connection already returned to pool")
    }
}

impl<'a> Drop for PooledConnection<'a> {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.pool.return_connection(conn);
        }
    }
}
