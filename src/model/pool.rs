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
    /// If the pool is empty, a new connection is opened.
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
            None => DbConnection::open(&self.config)?,
        };

        Ok(PooledConnection {
            conn: Some(conn),
            pool: self,
        })
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
