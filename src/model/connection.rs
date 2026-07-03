//! Database connection configuration.

use super::DbError;

/// Database connection configuration.
///
/// For SQLite, only `database` matters (the file path or `":memory:"`).
/// For PostgreSQL and MySQL, `host`, `port`, `user`, `password`, and `database` are all used.
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Hostname (ignored for SQLite).
    pub host: String,
    /// Port (ignored for SQLite). PostgreSQL default: 5432. MySQL default: 3306.
    pub port: u16,
    /// Username (ignored for SQLite).
    pub user: String,
    /// Password (ignored for SQLite).
    pub password: String,
    /// Database name. For SQLite, this is the file path.
    pub database: String,
    /// Maximum number of connections in the pool.
    pub pool_size: u32,
}

impl DbConfig {
    /// Read configuration from environment variables.
    ///
    /// | Variable | Default |
    /// |---|---|
    /// | `RWS_DB_HOST` | `localhost` |
    /// | `RWS_DB_PORT` | `5432` |
    /// | `RWS_DB_USER` | _(empty)_ |
    /// | `RWS_DB_PASSWORD` | _(empty)_ |
    /// | `RWS_DB_NAME` | **(required)** |
    /// | `RWS_DB_POOL_SIZE` | `10` |
    pub fn from_env() -> Result<Self, DbError> {
        let host = std::env::var("RWS_DB_HOST").unwrap_or_else(|_| "localhost".into());
        let port = std::env::var("RWS_DB_PORT")
            .unwrap_or_else(|_| "5432".into())
            .parse::<u16>()
            .map_err(|e| DbError::new(format!("RWS_DB_PORT: {}", e)))?;
        let user = std::env::var("RWS_DB_USER").unwrap_or_default();
        let password = std::env::var("RWS_DB_PASSWORD").unwrap_or_default();
        let database = std::env::var("RWS_DB_NAME")
            .map_err(|_| DbError::new("RWS_DB_NAME environment variable is required"))?;
        let pool_size = std::env::var("RWS_DB_POOL_SIZE")
            .unwrap_or_else(|_| "10".into())
            .parse::<u32>()
            .map_err(|e| DbError::new(format!("RWS_DB_POOL_SIZE: {}", e)))?;

        Ok(DbConfig { host, port, user, password, database, pool_size })
    }

    /// Connection URL suitable for passing to sqlx.
    #[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
    pub(crate) fn to_url(&self) -> String {
        #[cfg(feature = "model-sqlite")]
        return format!("sqlite:{}", self.database);

        #[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
        return format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.database
        );

        #[cfg(all(feature = "model-mysql", not(feature = "model-sqlite"), not(feature = "model-postgres")))]
        return format!(
            "mysql://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.database
        );
    }
}
