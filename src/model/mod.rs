//! # Model — Database Access Layer
//!
//! A JPA/Hibernate-style ORM for `rust-web-server`. Provides:
//! - `#[derive(Model)]` proc-macro for struct-to-table mapping
//! - `Repository<T, ID>` trait for CRUD operations
//! - `QueryBuilder<T>` for fluent filtering
//! - `DbPool` / `DbConnection` for connection management
//! - Migration runner
//! - Relationship helpers (`HasMany`, `HasOne`, `BelongsTo`)

pub mod connection;
pub mod migration;
pub mod pool;
pub mod query;
pub mod relation;
pub mod repository;
#[cfg(all(test, feature = "model-sqlite"))]
mod tests;

pub use connection::{DbConfig, DbConnection};
pub use migration::MigrationStatus;
pub use pool::{DbPool, PooledConnection};
pub use query::{Order, QueryBuilder};
pub use relation::{BelongsTo, HasMany, HasOne};
pub use repository::{ModelRepository, Repository};

// ── Value ─────────────────────────────────────────────────────────────────────

/// Backend-independent SQL value representation.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
}

// ── DbError ───────────────────────────────────────────────────────────────────

/// Error type returned by all model/DB operations.
#[derive(Debug)]
pub struct DbError(pub String);

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DbError: {}", self.0)
    }
}

impl std::error::Error for DbError {}

impl DbError {
    pub fn new(msg: impl Into<String>) -> Self {
        DbError(msg.into())
    }
}

// ── ModelRow ──────────────────────────────────────────────────────────────────

/// Backend-independent database row.
#[derive(Debug, Clone)]
pub struct ModelRow {
    pub(crate) columns: Vec<(String, Value)>,
}

impl ModelRow {
    pub fn new(columns: Vec<(String, Value)>) -> Self {
        ModelRow { columns }
    }

    /// Retrieve and convert a column value by name.
    pub fn get<T: FromColumn>(&self, col: &str) -> Result<T, DbError> {
        for (name, val) in &self.columns {
            if name.eq_ignore_ascii_case(col) {
                return T::from_column(val.clone());
            }
        }
        Err(DbError::new(format!("column '{}' not found in row", col)))
    }
}

// ── FromColumn / ToColumn ─────────────────────────────────────────────────────

/// Convert a `Value` into a Rust type.
pub trait FromColumn: Sized {
    fn from_column(v: Value) -> Result<Self, DbError>;
}

/// Convert a Rust value into a `Value`.
pub trait ToColumn {
    fn to_column(&self) -> Value;
}

// ── FromColumn impls ──────────────────────────────────────────────────────────

impl FromColumn for i16 {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Int(n) => Ok(n as i16),
            Value::Null => Err(DbError::new("unexpected NULL for i16")),
            other => Err(DbError::new(format!("cannot convert {:?} to i16", other))),
        }
    }
}

impl FromColumn for i32 {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Int(n) => Ok(n as i32),
            Value::Null => Err(DbError::new("unexpected NULL for i32")),
            other => Err(DbError::new(format!("cannot convert {:?} to i32", other))),
        }
    }
}

impl FromColumn for i64 {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Int(n) => Ok(n),
            Value::Null => Err(DbError::new("unexpected NULL for i64")),
            other => Err(DbError::new(format!("cannot convert {:?} to i64", other))),
        }
    }
}

impl FromColumn for u32 {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Int(n) => Ok(n as u32),
            Value::Null => Err(DbError::new("unexpected NULL for u32")),
            other => Err(DbError::new(format!("cannot convert {:?} to u32", other))),
        }
    }
}

impl FromColumn for u64 {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Int(n) => Ok(n as u64),
            Value::Null => Err(DbError::new("unexpected NULL for u64")),
            other => Err(DbError::new(format!("cannot convert {:?} to u64", other))),
        }
    }
}

impl FromColumn for f32 {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Float(f) => Ok(f as f32),
            Value::Int(n) => Ok(n as f32),
            Value::Null => Err(DbError::new("unexpected NULL for f32")),
            other => Err(DbError::new(format!("cannot convert {:?} to f32", other))),
        }
    }
}

impl FromColumn for f64 {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Float(f) => Ok(f),
            Value::Int(n) => Ok(n as f64),
            Value::Null => Err(DbError::new("unexpected NULL for f64")),
            other => Err(DbError::new(format!("cannot convert {:?} to f64", other))),
        }
    }
}

impl FromColumn for bool {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Bool(b) => Ok(b),
            Value::Int(n) => Ok(n != 0),
            Value::Null => Err(DbError::new("unexpected NULL for bool")),
            other => Err(DbError::new(format!("cannot convert {:?} to bool", other))),
        }
    }
}

impl FromColumn for String {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Text(s) => Ok(s),
            Value::Null => Err(DbError::new("unexpected NULL for String")),
            other => Err(DbError::new(format!("cannot convert {:?} to String", other))),
        }
    }
}

impl<T: FromColumn> FromColumn for Option<T> {
    fn from_column(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Null => Ok(None),
            other => Ok(Some(T::from_column(other)?)),
        }
    }
}

// ── ToColumn impls ────────────────────────────────────────────────────────────

impl ToColumn for i16 {
    fn to_column(&self) -> Value {
        Value::Int(*self as i64)
    }
}

impl ToColumn for i32 {
    fn to_column(&self) -> Value {
        Value::Int(*self as i64)
    }
}

impl ToColumn for i64 {
    fn to_column(&self) -> Value {
        Value::Int(*self)
    }
}

impl ToColumn for u32 {
    fn to_column(&self) -> Value {
        Value::Int(*self as i64)
    }
}

impl ToColumn for u64 {
    fn to_column(&self) -> Value {
        Value::Int(*self as i64)
    }
}

impl ToColumn for f32 {
    fn to_column(&self) -> Value {
        Value::Float(*self as f64)
    }
}

impl ToColumn for f64 {
    fn to_column(&self) -> Value {
        Value::Float(*self)
    }
}

impl ToColumn for bool {
    fn to_column(&self) -> Value {
        Value::Bool(*self)
    }
}

impl ToColumn for String {
    fn to_column(&self) -> Value {
        Value::Text(self.clone())
    }
}

impl ToColumn for str {
    fn to_column(&self) -> Value {
        Value::Text(self.to_owned())
    }
}

impl ToColumn for &str {
    fn to_column(&self) -> Value {
        Value::Text((*self).to_owned())
    }
}

impl<T: ToColumn> ToColumn for Option<T> {
    fn to_column(&self) -> Value {
        match self {
            Some(v) => v.to_column(),
            None => Value::Null,
        }
    }
}

// ── Model trait ───────────────────────────────────────────────────────────────

/// Trait generated by `#[derive(Model)]`. Maps a Rust struct to a database table.
pub trait Model: Sized {
    /// The database table name.
    fn table_name() -> &'static str;

    /// All column names (excluding `#[ignore]` fields).
    fn column_names() -> &'static [&'static str];

    /// The primary key column name.
    fn primary_key_name() -> &'static str;

    /// The primary key value for this instance.
    fn primary_key_value(&self) -> Value;

    /// Whether the primary key is auto-incremented by the database.
    fn primary_key_auto_increment() -> bool {
        false
    }

    /// Deserialise a database row into this type.
    fn from_row(row: &ModelRow) -> Result<Self, DbError>;

    /// Serialise this instance's fields as (column, value) pairs.
    /// Excludes `#[ignore]` fields.
    fn to_values(&self) -> Vec<(&'static str, Value)>;
}
