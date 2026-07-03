//! Database connection — `DbConfig` and `DbConnection`.
//!
//! The concrete backend is selected by a feature flag:
//! - `model-sqlite`   → rusqlite
//! - `model-postgres` → postgres crate
//! - `model-mysql`    → mysql crate

use super::{DbError, Model, ModelRow, MigrationStatus, Value};

// ── DbConfig ──────────────────────────────────────────────────────────────────

/// Database connection configuration.
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Hostname (ignored for SQLite).
    pub host: String,
    /// Port (ignored for SQLite).
    pub port: u16,
    /// Username (ignored for SQLite).
    pub user: String,
    /// Password (ignored for SQLite).
    pub password: String,
    /// Database name. For SQLite, this is the file path (use `":memory:"` for in-memory).
    pub database: String,
    /// Number of connections to create in the pool.
    pub pool_size: usize,
}

impl DbConfig {
    /// Create a configuration for a SQLite in-memory database.
    ///
    /// The pool size is fixed at 1. Every connection to `":memory:"` opens a
    /// separate, empty database, so using more than one connection would silently
    /// discard each other's writes. A single pooled connection serialises all
    /// access through the `DbPool` mutex, which is correct for tests and
    /// single-threaded development use.
    ///
    /// For multi-threaded use, see [`DbPool::memory`].
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "model-sqlite")]
    /// # {
    /// use rust_web_server::model::{DbConfig, DbConnection};
    /// let mut conn = DbConnection::open(&DbConfig::memory()).unwrap();
    /// conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY)", &[]).unwrap();
    /// # }
    /// ```
    #[cfg(feature = "model-sqlite")]
    pub fn memory() -> Self {
        DbConfig {
            host: String::new(),
            port: 0,
            user: String::new(),
            password: String::new(),
            database: ":memory:".into(),
            pool_size: 1,
        }
    }

    /// Read configuration from environment variables.
    ///
    /// | Variable | Default |
    /// |---|---|
    /// | `RWS_DB_HOST` | `localhost` |
    /// | `RWS_DB_PORT` | `5432` |
    /// | `RWS_DB_USER` | (required) |
    /// | `RWS_DB_PASSWORD` | (required) |
    /// | `RWS_DB_NAME` | (required) |
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
            .parse::<usize>()
            .map_err(|e| DbError::new(format!("RWS_DB_POOL_SIZE: {}", e)))?;

        Ok(DbConfig {
            host,
            port,
            user,
            password,
            database,
            pool_size,
        })
    }
}

// ── DbConnection ──────────────────────────────────────────────────────────────

/// A database connection. The concrete backend is chosen by feature flag.
pub struct DbConnection {
    #[cfg(feature = "model-sqlite")]
    pub(crate) inner: rusqlite::Connection,

    #[cfg(feature = "model-postgres")]
    pub(crate) inner: postgres::Client,

    #[cfg(feature = "model-mysql")]
    pub(crate) inner: mysql::Conn,

    #[cfg(not(any(
        feature = "model-sqlite",
        feature = "model-postgres",
        feature = "model-mysql"
    )))]
    _phantom: (),
}

impl DbConnection {
    /// Open a new connection using the given configuration.
    pub fn open(config: &DbConfig) -> Result<Self, DbError> {
        #[cfg(feature = "model-sqlite")]
        {
            let conn = rusqlite::Connection::open(&config.database)
                .map_err(|e| DbError::new(e.to_string()))?;
            return Ok(DbConnection { inner: conn });
        }

        #[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
        {
            let url = format!(
                "host={} port={} user={} password={} dbname={}",
                config.host, config.port, config.user, config.password, config.database
            );
            let client = postgres::Client::connect(&url, postgres::NoTls)
                .map_err(|e| DbError::new(e.to_string()))?;
            return Ok(DbConnection { inner: client });
        }

        #[cfg(all(
            feature = "model-mysql",
            not(feature = "model-sqlite"),
            not(feature = "model-postgres")
        ))]
        {
            let url = format!(
                "mysql://{}:{}@{}:{}/{}",
                config.user, config.password, config.host, config.port, config.database
            );
            let conn = mysql::Conn::new(mysql::Opts::from_url(&url).map_err(|e| DbError::new(e.to_string()))?)
                .map_err(|e| DbError::new(e.to_string()))?;
            return Ok(DbConnection { inner: conn });
        }

        #[cfg(not(any(
            feature = "model-sqlite",
            feature = "model-postgres",
            feature = "model-mysql"
        )))]
        Err(DbError::new(
            "No database feature enabled. Enable one of: model-sqlite, model-postgres, model-mysql",
        ))
    }

    /// Execute a SQL statement (INSERT/UPDATE/DELETE/DDL), returning rows affected.
    pub fn execute(&mut self, sql: &str, params: &[Value]) -> Result<u64, DbError> {
        #[cfg(feature = "model-sqlite")]
        {
            return self.sqlite_execute(sql, params);
        }

        #[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
        {
            return self.pg_execute(sql, params);
        }

        #[cfg(all(
            feature = "model-mysql",
            not(feature = "model-sqlite"),
            not(feature = "model-postgres")
        ))]
        {
            return self.mysql_execute(sql, params);
        }

        #[cfg(not(any(
            feature = "model-sqlite",
            feature = "model-postgres",
            feature = "model-mysql"
        )))]
        Err(DbError::new("no database feature enabled"))
    }

    /// Execute a SQL query returning rows.
    pub fn query_rows(&mut self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        #[cfg(feature = "model-sqlite")]
        {
            return self.sqlite_query_rows(sql, params);
        }

        #[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
        {
            return self.pg_query_rows(sql, params);
        }

        #[cfg(all(
            feature = "model-mysql",
            not(feature = "model-sqlite"),
            not(feature = "model-postgres")
        ))]
        {
            return self.mysql_query_rows(sql, params);
        }

        #[cfg(not(any(
            feature = "model-sqlite",
            feature = "model-postgres",
            feature = "model-mysql"
        )))]
        Err(DbError::new("no database feature enabled"))
    }

    /// Begin a transaction.
    pub fn begin(&mut self) -> Result<(), DbError> {
        self.execute("BEGIN", &[])?;
        Ok(())
    }

    /// Commit the current transaction.
    pub fn commit(&mut self) -> Result<(), DbError> {
        self.execute("COMMIT", &[])?;
        Ok(())
    }

    /// Roll back the current transaction.
    pub fn rollback(&mut self) -> Result<(), DbError> {
        self.execute("ROLLBACK", &[])?;
        Ok(())
    }

    /// Run a closure in a transaction, rolling back on `Err`.
    pub fn transaction<F, T>(&mut self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&mut DbConnection) -> Result<T, DbError>,
    {
        self.begin()?;
        match f(self) {
            Ok(val) => {
                self.commit()?;
                Ok(val)
            }
            Err(e) => {
                let _ = self.rollback();
                Err(e)
            }
        }
    }

    /// Execute a SQL query and deserialise results into `T: Model`.
    pub fn query<T: Model>(&mut self, sql: &str, params: &[Value]) -> Result<Vec<T>, DbError> {
        let rows = self.query_rows(sql, params)?;
        rows.iter().map(|r| T::from_row(r)).collect()
    }

    /// Execute a SQL query returning untyped rows.
    pub fn query_raw(&mut self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        self.query_rows(sql, params)
    }

    /// Run pending migrations from SQL files in `dir`.
    pub fn migrate(&mut self, dir: &str) -> Result<(), DbError> {
        crate::model::migration::run_migrations(self, dir)
    }

    /// Return migration status (applied / pending) for files in `dir`.
    pub fn migration_status(&mut self, dir: &str) -> Result<Vec<MigrationStatus>, DbError> {
        crate::model::migration::migration_status(self, dir)
    }

    /// Open a fresh SQLite in-memory database with no pre-existing schema.
    ///
    /// Every call returns an independent, isolated connection — ideal for unit
    /// tests where each test needs a clean slate.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "model-sqlite")]
    /// # {
    /// use rust_web_server::model::{DbConnection, Value};
    ///
    /// let mut conn = DbConnection::memory().unwrap();
    /// conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)", &[]).unwrap();
    /// conn.execute("INSERT INTO users (name) VALUES (?1)", &[Value::Text("Alice".into())]).unwrap();
    /// # }
    /// ```
    #[cfg(feature = "model-sqlite")]
    pub fn memory() -> Result<Self, DbError> {
        rusqlite::Connection::open(":memory:")
            .map(|inner| DbConnection { inner })
            .map_err(|e| DbError::new(e.to_string()))
    }

    // ── SQLite backend ────────────────────────────────────────────────────────

    #[cfg(feature = "model-sqlite")]
    fn sqlite_execute(&mut self, sql: &str, params: &[Value]) -> Result<u64, DbError> {
        use rusqlite::types::ToSql;
        let p: Vec<Box<dyn ToSql>> = params.iter().map(value_to_rusqlite_box).collect();
        let p_refs: Vec<&dyn ToSql> = p.iter().map(|b| b.as_ref()).collect();
        let affected = self
            .inner
            .execute(sql, p_refs.as_slice())
            .map_err(|e| DbError::new(e.to_string()))?;
        Ok(affected as u64)
    }

    #[cfg(feature = "model-sqlite")]
    fn sqlite_query_rows(&mut self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        use rusqlite::types::ToSql;
        let p: Vec<Box<dyn ToSql>> = params.iter().map(value_to_rusqlite_box).collect();
        let p_refs: Vec<&dyn ToSql> = p.iter().map(|b| b.as_ref()).collect();

        let mut stmt = self
            .inner
            .prepare(sql)
            .map_err(|e| DbError::new(e.to_string()))?;

        let col_names: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_owned())
            .collect();

        let rows = stmt
            .query_map(p_refs.as_slice(), |row| {
                let mut cols: Vec<(String, Value)> = Vec::new();
                for (i, name) in col_names.iter().enumerate() {
                    let val: rusqlite::types::Value = row.get(i)?;
                    cols.push((name.clone(), rusqlite_value_to_value(val)));
                }
                Ok(ModelRow::new(cols))
            })
            .map_err(|e| DbError::new(e.to_string()))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| DbError::new(e.to_string()))?);
        }
        Ok(result)
    }

    /// For SQLite, get the last auto-inserted row id.
    #[cfg(feature = "model-sqlite")]
    pub(crate) fn last_insert_rowid(&self) -> i64 {
        self.inner.last_insert_rowid()
    }

    // ── PostgreSQL backend ────────────────────────────────────────────────────

    #[cfg(feature = "model-postgres")]
    fn pg_execute(&mut self, sql: &str, params: &[Value]) -> Result<u64, DbError> {
        let p: Vec<&(dyn postgres::types::ToSql + Sync)> =
            params.iter().map(value_to_pg_tosql_ref).collect();
        let affected = self
            .inner
            .execute(sql, &p)
            .map_err(|e| DbError::new(e.to_string()))?;
        Ok(affected)
    }

    #[cfg(feature = "model-postgres")]
    fn pg_query_rows(&mut self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        let p: Vec<&(dyn postgres::types::ToSql + Sync)> =
            params.iter().map(value_to_pg_tosql_ref).collect();
        let rows = self
            .inner
            .query(sql, &p)
            .map_err(|e| DbError::new(e.to_string()))?;

        let mut result = Vec::new();
        for row in &rows {
            let cols: Vec<(String, Value)> = row
                .columns()
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    let val = pg_row_value(row, i, col);
                    (col.name().to_owned(), val)
                })
                .collect();
            result.push(ModelRow::new(cols));
        }
        Ok(result)
    }

    // ── MySQL backend ─────────────────────────────────────────────────────────

    #[cfg(feature = "model-mysql")]
    fn mysql_execute(&mut self, sql: &str, params: &[Value]) -> Result<u64, DbError> {
        use mysql::prelude::Queryable;
        let p: Vec<mysql::Value> = params.iter().map(value_to_mysql).collect();
        let result = self
            .inner
            .exec_iter(sql, p)
            .map_err(|e| DbError::new(e.to_string()))?;
        Ok(result.affected_rows())
    }

    #[cfg(feature = "model-mysql")]
    fn mysql_query_rows(&mut self, sql: &str, params: &[Value]) -> Result<Vec<ModelRow>, DbError> {
        use mysql::prelude::Queryable;
        let p: Vec<mysql::Value> = params.iter().map(value_to_mysql).collect();
        let result: Vec<mysql::Row> = self
            .inner
            .exec(sql, p)
            .map_err(|e| DbError::new(e.to_string()))?;

        let mut out = Vec::new();
        for row in result {
            let cols: Vec<(String, Value)> = row
                .columns_ref()
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    let name = col.name_str().to_string();
                    let val = mysql_value_to_value(row.get(i).unwrap_or(mysql::Value::NULL));
                    (name, val)
                })
                .collect();
            out.push(ModelRow::new(cols));
        }
        Ok(out)
    }

    /// For MySQL: get last insert id after an INSERT.
    #[cfg(feature = "model-mysql")]
    pub(crate) fn last_insert_id(&mut self) -> u64 {
        self.inner.last_insert_id()
    }
}

// ── SQLite value conversions ──────────────────────────────────────────────────

#[cfg(feature = "model-sqlite")]
fn value_to_rusqlite_box(v: &Value) -> Box<dyn rusqlite::types::ToSql> {
    match v {
        Value::Null => Box::new(rusqlite::types::Null),
        Value::Bool(b) => Box::new(*b),
        Value::Int(n) => Box::new(*n),
        Value::Float(f) => Box::new(*f),
        Value::Text(s) => Box::new(s.clone()),
        Value::Bytes(b) => Box::new(b.clone()),
    }
}

#[cfg(feature = "model-sqlite")]
fn rusqlite_value_to_value(v: rusqlite::types::Value) -> Value {
    match v {
        rusqlite::types::Value::Null => Value::Null,
        rusqlite::types::Value::Integer(n) => Value::Int(n),
        rusqlite::types::Value::Real(f) => Value::Float(f),
        rusqlite::types::Value::Text(s) => Value::Text(s),
        rusqlite::types::Value::Blob(b) => Value::Bytes(b),
    }
}

// ── PostgreSQL value conversions ──────────────────────────────────────────────

#[cfg(feature = "model-postgres")]
fn value_to_pg_tosql_ref(v: &Value) -> &(dyn postgres::types::ToSql + Sync) {
    // We need owned storage — use a local enum trick via a wrapper.
    // For simplicity, we convert to concrete pg types using a Vec.
    // This is called once per param; the actual conversion happens below.
    match v {
        Value::Null => &Option::<i64>::None,
        Value::Bool(b) => b,
        Value::Int(n) => n,
        Value::Float(f) => f,
        Value::Text(s) => s,
        Value::Bytes(b) => b,
    }
}

#[cfg(feature = "model-postgres")]
fn pg_row_value(row: &postgres::Row, i: usize, col: &postgres::Column) -> Value {
    use postgres::types::Type;
    match col.type_() {
        &Type::BOOL => row.try_get::<_, bool>(i).map(Value::Bool).unwrap_or(Value::Null),
        &Type::INT2 => row.try_get::<_, i16>(i).map(|n| Value::Int(n as i64)).unwrap_or(Value::Null),
        &Type::INT4 => row.try_get::<_, i32>(i).map(|n| Value::Int(n as i64)).unwrap_or(Value::Null),
        &Type::INT8 => row.try_get::<_, i64>(i).map(Value::Int).unwrap_or(Value::Null),
        &Type::FLOAT4 => row.try_get::<_, f32>(i).map(|f| Value::Float(f as f64)).unwrap_or(Value::Null),
        &Type::FLOAT8 => row.try_get::<_, f64>(i).map(Value::Float).unwrap_or(Value::Null),
        &Type::TEXT | &Type::VARCHAR => row.try_get::<_, String>(i).map(Value::Text).unwrap_or(Value::Null),
        &Type::BYTEA => row.try_get::<_, Vec<u8>>(i).map(Value::Bytes).unwrap_or(Value::Null),
        _ => row.try_get::<_, String>(i).map(Value::Text).unwrap_or(Value::Null),
    }
}

// ── MySQL value conversions ───────────────────────────────────────────────────

#[cfg(feature = "model-mysql")]
fn value_to_mysql(v: &Value) -> mysql::Value {
    match v {
        Value::Null => mysql::Value::NULL,
        Value::Bool(b) => mysql::Value::Int(if *b { 1 } else { 0 }),
        Value::Int(n) => mysql::Value::Int(*n),
        Value::Float(f) => mysql::Value::Float(*f as f32),
        Value::Text(s) => mysql::Value::Bytes(s.as_bytes().to_vec()),
        Value::Bytes(b) => mysql::Value::Bytes(b.clone()),
    }
}

#[cfg(feature = "model-mysql")]
fn mysql_value_to_value(v: mysql::Value) -> Value {
    match v {
        mysql::Value::NULL => Value::Null,
        mysql::Value::Bytes(b) => {
            String::from_utf8(b.clone()).map(Value::Text).unwrap_or(Value::Bytes(b))
        }
        mysql::Value::Int(n) => Value::Int(n),
        mysql::Value::UInt(n) => Value::Int(n as i64),
        mysql::Value::Float(f) => Value::Float(f as f64),
        mysql::Value::Double(f) => Value::Float(f),
        mysql::Value::Date(y, mo, d, h, mi, s, _) => {
            Value::Text(format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, s))
        }
        mysql::Value::Time(neg, d, h, mi, s, _) => {
            let sign = if neg { "-" } else { "" };
            Value::Text(format!("{}{}:{:02}:{:02}", sign, d * 24 + h as u32, mi, s))
        }
    }
}
