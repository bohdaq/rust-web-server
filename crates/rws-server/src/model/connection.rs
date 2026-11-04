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
    #[doc(hidden)]
    pub fn to_url(&self) -> String {
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

/// How a PostgreSQL or MySQL connection uses TLS (ignored for SQLite).
///
/// Separate from [`DbConfig`] so code that builds a `DbConfig` literal keeps
/// compiling; [`DbPool::from_env`](super::DbPool::from_env) reads both, and
/// [`DbPool::new_with_tls`](super::DbPool::new_with_tls) takes both.
///
/// | Variable | Default |
/// |---|---|
/// | `RWS_DB_SSL_MODE` | `prefer` |
/// | `RWS_DB_SSL_ROOT_CERT` | _(none: the system's roots)_ |
/// | `RWS_DB_SSL_CLIENT_CERT` | _(none)_ |
/// | `RWS_DB_SSL_CLIENT_KEY` | _(none)_ |
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DbTls {
    /// When to use TLS and what to check.
    pub mode: DbSslMode,
    /// PEM file with the CA certificates that sign the server's certificate
    /// (a managed database's private CA).
    pub root_cert: Option<String>,
    /// PEM client certificate, for servers that require one.
    pub client_cert: Option<String>,
    /// PEM private key of `client_cert`.
    pub client_key: Option<String>,
}

/// TLS modes, named as libpq's `sslmode`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DbSslMode {
    /// Never TLS.
    Disable,
    /// TLS when the server offers it, without checking its certificate
    /// (the default, as before these settings existed).
    #[default]
    Prefer,
    /// TLS always, certificate not checked.
    Require,
    /// TLS always, certificate signed by a trusted CA.
    VerifyCa,
    /// As `VerifyCa`, and the certificate names the host connected to.
    VerifyFull,
}

impl DbSslMode {
    /// Parse `disable`, `prefer`, `require`, `verify-ca` or `verify-full`
    /// (MySQL's spellings `disabled`, `preferred`, `required`,
    /// `verify_identity` too).
    pub fn parse(value: &str) -> Result<DbSslMode, DbError> {
        Ok(match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "disable" | "disabled" => DbSslMode::Disable,
            "" | "prefer" | "preferred" => DbSslMode::Prefer,
            "require" | "required" => DbSslMode::Require,
            "verify-ca" => DbSslMode::VerifyCa,
            "verify-full" | "verify-identity" => DbSslMode::VerifyFull,
            other => {
                return Err(DbError::new(format!(
                    "RWS_DB_SSL_MODE: {other:?} is not disable, prefer, require, verify-ca or verify-full"
                )))
            }
        })
    }
}

impl DbTls {
    /// Read `RWS_DB_SSL_*` (see the table above). A client certificate
    /// without its key, or the other way round, is an error.
    pub fn from_env() -> Result<DbTls, DbError> {
        let get = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let tls = DbTls {
            mode: DbSslMode::parse(&get("RWS_DB_SSL_MODE").unwrap_or_default())?,
            root_cert: get("RWS_DB_SSL_ROOT_CERT"),
            client_cert: get("RWS_DB_SSL_CLIENT_CERT"),
            client_key: get("RWS_DB_SSL_CLIENT_KEY"),
        };
        if tls.client_cert.is_some() != tls.client_key.is_some() {
            return Err(DbError::new("RWS_DB_SSL_CLIENT_CERT and RWS_DB_SSL_CLIENT_KEY go together"));
        }
        Ok(tls)
    }
}
