//! # Model — Async Database Access Layer
//!
//! An async JPA/Hibernate-style ORM backed by [sqlx]. Provides:
//! - `#[derive(Model)]` proc-macro for struct-to-table mapping
//! - `Repository<T, i64>` trait for zero-boilerplate async CRUD
//! - `QueryBuilder<T>` for fluent async filtering
//! - `DbPool` for connection management (wraps `sqlx::Pool`)
//! - Async migration runner
//! - Relationship helpers (`HasMany`, `HasOne`, `BelongsTo`)
//! - Primary keys of type `i64`, `i32`, `String` or [`Uuid`] (18.30.0)
//! - Conventions `save` and `delete` maintain (18.30.0): `#[timestamps]`
//!   (`created_at` / `updated_at`, [`Timestamp`]), `#[soft_delete]`
//!   (`deleted_at`; deleted rows are left out of queries unless asked for)
//!   and `#[version]` (optimistic locking: saving a stale copy fails with
//!   [`DbErrorKind::StaleVersion`] instead of overwriting)
//!
//! Enable one or more backend features — `model-sqlite`, `model-postgres`,
//! and `model-mysql` are not mutually exclusive, so a single binary can hold
//! pools to more than one backend at once (see [`Backend`]):
//!
//! ```toml
//! # SQLite
//! rust-web-server = { features = ["model-sqlite"] }
//! # PostgreSQL
//! rust-web-server = { features = ["model-postgres"] }
//! # MySQL
//! rust-web-server = { features = ["model-mysql"] }
//! # Or more than one at once
//! rust-web-server = { features = ["model-sqlite", "model-postgres"] }
//! ```

pub mod backend;
pub mod connection;
pub mod migration;
pub mod pool;
pub mod query;
pub mod relation;
pub mod repository;
pub mod timestamp;
mod decode;
pub mod uuid;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod backends;
#[cfg(test)]
mod convention_tests;
#[cfg(test)]
mod tls_tests;

pub use backend::Backend;
pub use connection::{DbConfig, DbSslMode, DbTls};
pub use migration::MigrationStatus;
pub use pool::{DbPool, DbTransaction};
pub use query::{Order, QueryBuilder};
pub use relation::{BelongsTo, HasMany, HasOne};
pub use repository::{ModelRepository, PrimaryKey, Repository};
pub use timestamp::Timestamp;
pub use uuid::Uuid;
pub use crate::pagination::{CursorPage, Page};

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
///
/// The message says what failed. Errors a caller handles differently —
/// a stale optimistic-locking version, a row that isn't there — have a
/// [`kind`](DbError::kind) too (18.30.0); it is carried by a fixed message
/// prefix (`stale version: `, `not found: `), since `DbError` stays a
/// tuple struct of its message until a major release.
#[derive(Debug)]
pub struct DbError(pub String);

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DbError: {}", self.0)
    }
}

impl std::error::Error for DbError {}

/// What a [`DbError`] is about, for errors a caller handles differently
/// from the rest (18.30.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DbErrorKind {
    /// `save` of a `#[version]`ed model whose version no longer matches the
    /// row's: someone saved it since this copy was read. Reload and retry,
    /// or tell the user.
    StaleVersion,
    /// The row to update or restore doesn't exist.
    NotFound,
    /// Anything else: SQL errors, connection errors, conversions.
    Other,
}

const STALE_VERSION: &str = "stale version: ";
const NOT_FOUND: &str = "not found: ";

impl DbError {
    pub fn new(msg: impl Into<String>) -> Self {
        DbError(msg.into())
    }

    /// A [`DbErrorKind::StaleVersion`] error; `detail` says which row and
    /// versions.
    pub fn stale_version(detail: impl std::fmt::Display) -> Self {
        DbError(format!("{STALE_VERSION}{detail}"))
    }

    /// A [`DbErrorKind::NotFound`] error.
    pub fn not_found(detail: impl std::fmt::Display) -> Self {
        DbError(format!("{NOT_FOUND}{detail}"))
    }

    /// What this error is about.
    ///
    /// ```rust
    /// # extern crate rws_server as rust_web_server;
    /// use rust_web_server::model::{DbError, DbErrorKind};
    ///
    /// let e = DbError::stale_version("users id=1: the row is at version 3, this copy at 2");
    /// assert_eq!(e.kind(), DbErrorKind::StaleVersion);
    /// assert_eq!(DbError::new("syntax error").kind(), DbErrorKind::Other);
    /// ```
    pub fn kind(&self) -> DbErrorKind {
        if self.0.starts_with(STALE_VERSION) {
            DbErrorKind::StaleVersion
        } else if self.0.starts_with(NOT_FOUND) {
            DbErrorKind::NotFound
        } else {
            DbErrorKind::Other
        }
    }

    /// `kind() == DbErrorKind::StaleVersion`.
    pub fn is_stale_version(&self) -> bool {
        self.kind() == DbErrorKind::StaleVersion
    }

    /// `kind() == DbErrorKind::NotFound`.
    pub fn is_not_found(&self) -> bool {
        self.kind() == DbErrorKind::NotFound
    }
}

// ── ModelRow ──────────────────────────────────────────────────────────────────

/// Backend-independent database row.
#[derive(Debug, Clone)]
pub struct ModelRow {
    #[doc(hidden)]
    pub columns: Vec<(String, Value)>,
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

/// The value as the driver decoded it (18.30.0).
impl FromColumn for Value {
    fn from_column(v: Value) -> Result<Self, DbError> {
        Ok(v)
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
    fn to_column(&self) -> Value { Value::Int(*self as i64) }
}
impl ToColumn for i32 {
    fn to_column(&self) -> Value { Value::Int(*self as i64) }
}
impl ToColumn for i64 {
    fn to_column(&self) -> Value { Value::Int(*self) }
}
impl ToColumn for u32 {
    fn to_column(&self) -> Value { Value::Int(*self as i64) }
}
impl ToColumn for u64 {
    fn to_column(&self) -> Value { Value::Int(*self as i64) }
}
impl ToColumn for f32 {
    fn to_column(&self) -> Value { Value::Float(*self as f64) }
}
impl ToColumn for f64 {
    fn to_column(&self) -> Value { Value::Float(*self) }
}
impl ToColumn for bool {
    fn to_column(&self) -> Value { Value::Bool(*self) }
}
impl ToColumn for String {
    fn to_column(&self) -> Value { Value::Text(self.clone()) }
}
impl ToColumn for str {
    fn to_column(&self) -> Value { Value::Text(self.to_owned()) }
}
impl ToColumn for &str {
    fn to_column(&self) -> Value { Value::Text((*self).to_owned()) }
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
pub trait Model: Sized + Send + Sync {
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

    /// The column `save` sets to the time of the insert and never writes
    /// again (`#[timestamps]`: `created_at`). `None`: no such column.
    fn created_at_column() -> Option<&'static str> {
        None
    }

    /// The column `save` sets to the time of every insert and update
    /// (`#[timestamps]`: `updated_at`).
    fn updated_at_column() -> Option<&'static str> {
        None
    }

    /// The soft-delete column (`#[soft_delete]`: `deleted_at`): `delete`
    /// sets it to the time instead of removing the row, and queries leave
    /// out rows where it isn't `NULL` unless asked for them.
    fn soft_delete_column() -> Option<&'static str> {
        None
    }

    /// The optimistic-locking column (`#[version]`): `1` on insert, one
    /// more on every update, which only succeeds while the row still has
    /// the version this copy was read with.
    fn version_column() -> Option<&'static str> {
        None
    }
}

/// What a [`Model`] type maps to, read from its trait implementation: the
/// type, its table, its columns and primary key (18.28.0). Rust can't list
/// the `#[derive(Model)]` types of a program at run time, so whatever
/// reports models (the dev MCP server's `models` tool,
/// `DevTools::model::<T>()`) is given each type once.
///
/// ```rust,ignore
/// let info = ModelInfo::of::<User>();
/// assert_eq!(info.table, "users");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelInfo {
    /// The Rust type, as [`std::any::type_name`] spells it.
    pub type_name: &'static str,
    /// [`Model::table_name`].
    pub table: &'static str,
    /// [`Model::column_names`], `#[ignore]` fields left out.
    pub columns: &'static [&'static str],
    /// [`Model::primary_key_name`].
    pub primary_key: &'static str,
    /// [`Model::primary_key_auto_increment`].
    pub auto_increment: bool,
}

impl ModelInfo {
    /// The mapping of `T`.
    pub fn of<T: Model>() -> Self {
        ModelInfo {
            type_name: std::any::type_name::<T>(),
            table: T::table_name(),
            columns: T::column_names(),
            primary_key: T::primary_key_name(),
            auto_increment: T::primary_key_auto_increment(),
        }
    }

    /// The type's name without its module path (`User` for
    /// `app::models::User`, `Page` for `app::Page<u8>`).
    pub fn short_name(&self) -> &'static str {
        let base = self.type_name.split('<').next().unwrap_or(self.type_name);
        base.rsplit("::").next().unwrap_or(base)
    }
}
