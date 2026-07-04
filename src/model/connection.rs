//! Database connection configuration.

use super::{Backend, DbError};

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
    /// Which compiled-in backend this config connects to.
    ///
    /// Only needs to be set explicitly when more than one `model-*` feature
    /// is compiled into the binary — see [`Backend`]. When only one is
    /// compiled in, [`DbConfig::from_env`] infers it automatically.
    pub backend: Backend,
}

impl DbConfig {
    /// Read configuration from environment variables.
    ///
    /// | Variable | Default |
    /// |---|---|
    /// | `RWS_DB_BACKEND` | Inferred if exactly one `model-*` feature is compiled in; **required** otherwise |
    /// | `RWS_DB_HOST` | `localhost` |
    /// | `RWS_DB_PORT` | `5432` |
    /// | `RWS_DB_USER` | _(empty)_ |
    /// | `RWS_DB_PASSWORD` | _(empty)_ |
    /// | `RWS_DB_NAME` | **(required)** |
    /// | `RWS_DB_POOL_SIZE` | `10` |
    pub fn from_env() -> Result<Self, DbError> {
        let backend = match std::env::var("RWS_DB_BACKEND") {
            Ok(s) => Backend::parse(&s)?,
            Err(_) => Backend::unambiguous_default().ok_or_else(|| {
                DbError::new(
                    "RWS_DB_BACKEND must be set to \"sqlite\", \"postgres\", or \"mysql\" \
                     when more than one model-* feature is compiled in",
                )
            })?,
        };
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

        Ok(DbConfig { host, port, user, password, database, pool_size, backend })
    }

    /// Connection URL suitable for passing to sqlx.
    pub(crate) fn to_url(&self) -> String {
        match self.backend {
            #[cfg(feature = "model-sqlite")]
            Backend::Sqlite => format!("sqlite:{}", self.database),
            #[cfg(feature = "model-postgres")]
            Backend::Postgres => format!(
                "postgres://{}:{}@{}:{}/{}",
                self.user, self.password, self.host, self.port, self.database
            ),
            #[cfg(feature = "model-mysql")]
            Backend::MySql => format!(
                "mysql://{}:{}@{}:{}/{}",
                self.user, self.password, self.host, self.port, self.database
            ),
        }
    }
}
