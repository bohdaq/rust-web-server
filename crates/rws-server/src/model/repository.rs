//! Async `Repository` trait and `ModelRepository` implementation.
//!
//! # Primary keys (18.30.0)
//!
//! `ModelRepository<T, ID>` implements [`Repository<T, ID>`] for `ID` =
//! `i64`, `i32`, `String` and [`Uuid`] (the [`PrimaryKey`] types). What
//! [`save`](Repository::save) does depends on the key:
//!
//! - `#[primary_key(auto_increment)]`: a key of `0` (or `NULL`) inserts and
//!   the database picks the key; any other key updates that row.
//! - Any other key: `save` looks the key up (soft-deleted rows included)
//!   and updates the row when it is there, inserts it otherwise. A
//!   [`Uuid::NIL`] key (`Uuid::default()`) is replaced by a fresh
//!   [`Uuid::now_v7`] and inserted; an empty `String` key is an error. The
//!   look-up and the write are two statements: two processes saving the
//!   same new key at once can both decide to insert, and the second gets
//!   the database's unique-key error. [`ModelRepository::insert`] always
//!   inserts.
//!
//! # Conventions
//!
//! The columns a model names through [`Model::created_at_column`],
//! [`Model::updated_at_column`], [`Model::soft_delete_column`] and
//! [`Model::version_column`] (`#[timestamps]`, `#[soft_delete]`,
//! `#[version]` on `#[derive(Model)]`) are written by the repository, never
//! from the entity's fields: on insert `created_at` = `updated_at` = now,
//! `deleted_at` = `NULL`, `version` = 1; on update `updated_at` = now and
//! `version` = `version + 1`, only while the row still has the entity's
//! version — otherwise `save` fails with
//! [`DbErrorKind::StaleVersion`](super::DbErrorKind::StaleVersion) and
//! changes nothing. With a soft-delete column `delete_by_id` sets it,
//! `find_by_id` / `find_all` / `count` / `exists_by_id` skip such rows, and
//! [`ModelRepository::restore`] / [`ModelRepository::force_delete_by_id`]
//! undo or finish it. Times are UTC ([`Timestamp`]); see
//! [`timestamp`](super::timestamp) for the column types per backend.

use std::marker::PhantomData;

use super::backend::Backend;
use super::pool::DbPool;
use super::query::QueryBuilder;
use super::timestamp::Timestamp;
use super::uuid::Uuid;
use super::{DbError, Model, ToColumn, Value};

// ── Repository trait ──────────────────────────────────────────────────────────

/// Async CRUD operations for a model type.
pub trait Repository<T: Model, ID> {
    async fn find_by_id(&self, id: ID) -> Result<Option<T>, DbError>;
    async fn find_all(&self) -> Result<Vec<T>, DbError>;
    async fn save(&self, entity: &T) -> Result<T, DbError>;
    async fn save_all(&self, entities: &[T]) -> Result<Vec<T>, DbError>;
    async fn delete_by_id(&self, id: ID) -> Result<(), DbError>;
    async fn delete_all_by_id(&self, ids: &[ID]) -> Result<(), DbError>;
    async fn count(&self) -> Result<i64, DbError>;
    async fn exists_by_id(&self, id: ID) -> Result<bool, DbError>;
}

// ── PrimaryKey ────────────────────────────────────────────────────────────────

mod sealed {
    pub trait Sealed {}
    impl Sealed for i64 {}
    impl Sealed for i32 {}
    impl Sealed for String {}
    impl Sealed for super::Uuid {}
}

/// A type [`ModelRepository`] can key rows by: `i64`, `i32`, `String` or
/// [`Uuid`] (18.30.0). Sealed — these are the types `Repository` is
/// implemented for.
pub trait PrimaryKey: ToColumn + Send + Sync + sealed::Sealed {}
impl PrimaryKey for i64 {}
impl PrimaryKey for i32 {}
impl PrimaryKey for String {}
impl PrimaryKey for Uuid {}

// ── ModelRepository ───────────────────────────────────────────────────────────

/// Repository tied to a model type and a pool.
pub struct ModelRepository<'a, T: Model, ID> {
    #[doc(hidden)]
    pub pool: &'a DbPool,
    _phantom: PhantomData<(T, ID)>,
}

impl<'a, T: Model, ID> ModelRepository<'a, T, ID> {
    pub fn new(pool: &'a DbPool) -> Self {
        ModelRepository { pool, _phantom: PhantomData }
    }
}

impl<'a, T: Model, ID: PrimaryKey> ModelRepository<'a, T, ID> {
    /// Insert `entity` (never an update, unlike [`Repository::save`]) and
    /// answer the row as stored — with the generated key, timestamps and
    /// version. A key that exists already is the database's unique-key
    /// error.
    pub async fn insert(&self, entity: &T) -> Result<T, DbError> {
        insert_entity(self.pool, entity).await
    }

    /// The row with key `id`, soft-deleted or not.
    pub async fn find_by_id_with_deleted(&self, id: ID) -> Result<Option<T>, DbError> {
        select_by_pk(self.pool, id.to_column(), false).await
    }

    /// Clear the soft-delete column of row `id`: `true` when it was
    /// deleted, `false` when it wasn't (or doesn't exist). An error for a
    /// model without one.
    pub async fn restore(&self, id: ID) -> Result<bool, DbError> {
        let column = soft_delete_column_or_err::<T>("restore")?;
        let backend = self.pool.backend();
        let sql = format!(
            "UPDATE {} SET {column} = NULL WHERE {} = {} AND {column} IS NOT NULL",
            T::table_name(),
            T::primary_key_name(),
            placeholder(backend, 1),
        );
        Ok(self.pool.execute(&sql, &[id.to_column()]).await? > 0)
    }

    /// Delete row `id` for real (`DELETE`), soft-delete column or not.
    pub async fn force_delete_by_id(&self, id: ID) -> Result<(), DbError> {
        let sql = format!("DELETE FROM {} WHERE {} = {}", T::table_name(), T::primary_key_name(), placeholder(self.pool.backend(), 1));
        self.pool.execute(&sql, &[id.to_column()]).await?;
        Ok(())
    }

    /// A [`QueryBuilder`] over the same table and pool.
    pub fn query(&self) -> QueryBuilder<'a, T> {
        QueryBuilder::new(self.pool)
    }
}

// ── impl Repository<T, ID> ────────────────────────────────────────────────────

macro_rules! impl_repository {
    ($($id:ty),*) => {$(
        impl<'a, T: Model> Repository<T, $id> for ModelRepository<'a, T, $id> {
            async fn find_by_id(&self, id: $id) -> Result<Option<T>, DbError> {
                select_by_pk(self.pool, id.to_column(), true).await
            }

            async fn find_all(&self) -> Result<Vec<T>, DbError> {
                let sql = format!("SELECT * FROM {}{}", T::table_name(), live_where::<T>());
                let rows = self.pool.query_rows(&sql, &[]).await?;
                rows.iter().map(|r| T::from_row(r)).collect()
            }

            async fn save(&self, entity: &T) -> Result<T, DbError> {
                save_entity(self.pool, entity).await
            }

            async fn save_all(&self, entities: &[T]) -> Result<Vec<T>, DbError> {
                let mut result = Vec::with_capacity(entities.len());
                for e in entities {
                    result.push(save_entity(self.pool, e).await?);
                }
                Ok(result)
            }

            async fn delete_by_id(&self, id: $id) -> Result<(), DbError> {
                delete_entity::<T>(self.pool, id.to_column()).await
            }

            async fn delete_all_by_id(&self, ids: &[$id]) -> Result<(), DbError> {
                for id in ids {
                    delete_entity::<T>(self.pool, id.to_column()).await?;
                }
                Ok(())
            }

            async fn count(&self) -> Result<i64, DbError> {
                let sql = format!("SELECT COUNT(*) FROM {}{}", T::table_name(), live_where::<T>());
                let rows = self.pool.query_rows(&sql, &[]).await?;
                extract_count(rows)
            }

            async fn exists_by_id(&self, id: $id) -> Result<bool, DbError> {
                Ok(select_by_pk::<T>(self.pool, id.to_column(), true).await?.is_some())
            }
        }
    )*};
}

impl_repository!(i64, i32, String, Uuid);

// ── Placeholder helpers ───────────────────────────────────────────────────────

/// Return the DB-appropriate placeholder for position `pos` (1-indexed) on
/// `backend`. SQLite and MySQL use `?`, PostgreSQL uses `$1`, `$2`, …
#[doc(hidden)]
pub fn placeholder(backend: Backend, _pos: usize) -> String {
    match backend {
        #[cfg(feature = "model-sqlite")]
        Backend::Sqlite => "?".to_string(),
        #[cfg(feature = "model-postgres")]
        Backend::Postgres => format!("${}", _pos),
        #[cfg(feature = "model-mysql")]
        Backend::MySql => "?".to_string(),
    }
}

/// Build a comma-separated list of placeholders.
#[doc(hidden)]
pub fn placeholders(backend: Backend, count: usize) -> String {
    (1..=count).map(|i| placeholder(backend, i)).collect::<Vec<_>>().join(", ")
}

/// The placeholder for a timestamp parameter: rws binds text, which
/// PostgreSQL only assigns to a `TIMESTAMPTZ` column through a cast (and
/// assigns the cast's result to a text column too).
#[doc(hidden)]
pub fn timestamp_placeholder(backend: Backend, pos: usize) -> String {
    match backend {
        #[cfg(feature = "model-postgres")]
        Backend::Postgres => format!("CAST(${} AS TIMESTAMPTZ)", pos),
        #[allow(unreachable_patterns)]
        _ => placeholder(backend, pos),
    }
}

/// `col IS NULL` for a soft-delete model (the rows not deleted).
#[doc(hidden)]
pub fn live_condition<T: Model>() -> Option<String> {
    T::soft_delete_column().map(|c| format!("{c} IS NULL"))
}

fn live_where<T: Model>() -> String {
    live_condition::<T>().map(|c| format!(" WHERE {c}")).unwrap_or_default()
}

fn soft_delete_column_or_err<T: Model>(what: &str) -> Result<&'static str, DbError> {
    T::soft_delete_column().ok_or_else(|| {
        DbError::new(format!("{what}: {} has no soft-delete column (add #[soft_delete] and a deleted_at field)", T::table_name()))
    })
}

fn is_timestamp_column<T: Model>(col: &str) -> bool {
    [T::created_at_column(), T::updated_at_column(), T::soft_delete_column()].contains(&Some(col))
}

fn is_nil_uuid(v: &Value) -> bool {
    matches!(v, Value::Text(s) if s == "00000000-0000-0000-0000-000000000000")
}

// ── Insert / Update helpers ───────────────────────────────────────────────────

/// The row with primary key `pk` (`live_only`: not soft-deleted).
#[doc(hidden)]
pub async fn select_by_pk<T: Model>(pool: &DbPool, pk: Value, live_only: bool) -> Result<Option<T>, DbError> {
    let mut sql = format!("SELECT * FROM {} WHERE {} = {}", T::table_name(), T::primary_key_name(), placeholder(pool.backend(), 1));
    if live_only {
        if let Some(c) = live_condition::<T>() {
            sql.push_str(" AND ");
            sql.push_str(&c);
        }
    }
    let rows = pool.query_rows(&sql, &[pk]).await?;
    match rows.into_iter().next() {
        Some(row) => Ok(Some(T::from_row(&row)?)),
        None => Ok(None),
    }
}

/// `save`: insert or update, as the module docs describe.
#[doc(hidden)]
pub async fn save_entity<T: Model>(pool: &DbPool, entity: &T) -> Result<T, DbError> {
    let pk = entity.primary_key_value();
    if T::primary_key_auto_increment() {
        return match pk {
            Value::Null | Value::Int(0) => insert_entity(pool, entity).await,
            _ => update_entity(pool, entity).await,
        };
    }
    match &pk {
        Value::Null => Err(DbError::new(format!(
            "save: {}.{} is NULL — set the key, or mark it #[primary_key(auto_increment)]",
            T::table_name(),
            T::primary_key_name()
        ))),
        Value::Text(s) if s.is_empty() => Err(DbError::new(format!(
            "save: {}.{} is empty — set the key before saving",
            T::table_name(),
            T::primary_key_name()
        ))),
        key if is_nil_uuid(key) => insert_entity(pool, entity).await,
        _ => {
            let sql = format!("SELECT 1 FROM {} WHERE {} = {}", T::table_name(), T::primary_key_name(), placeholder(pool.backend(), 1));
            if pool.query_rows(&sql, &[pk]).await?.is_empty() {
                insert_entity(pool, entity).await
            } else {
                update_entity(pool, entity).await
            }
        }
    }
}

#[doc(hidden)]
pub async fn insert_entity<T: Model>(pool: &DbPool, entity: &T) -> Result<T, DbError> {
    let backend = pool.backend();
    let now = Timestamp::now().to_value(backend);
    let pk_name = T::primary_key_name();
    let mut key = entity.primary_key_value();

    let mut cols: Vec<&str> = Vec::new();
    let mut params: Vec<Value> = Vec::new();
    let mut marks: Vec<String> = Vec::new();
    for (col, value) in entity.to_values() {
        let value = if col == pk_name {
            if T::primary_key_auto_increment() {
                continue;
            }
            if is_nil_uuid(&value) {
                key = Value::Text(Uuid::now_v7().to_string());
            }
            key.clone()
        } else if Some(col) == T::created_at_column() || Some(col) == T::updated_at_column() {
            now.clone()
        } else if Some(col) == T::soft_delete_column() {
            Value::Null
        } else if Some(col) == T::version_column() {
            Value::Int(1)
        } else {
            value
        };
        cols.push(col);
        if value == Value::Null {
            // A literal: PostgreSQL won't assign a NULL bound as text to a
            // column of another type.
            marks.push("NULL".to_string());
            continue;
        }
        params.push(value);
        let pos = params.len();
        marks.push(if is_timestamp_column::<T>(col) { timestamp_placeholder(backend, pos) } else { placeholder(backend, pos) });
    }

    let insert_sql = if cols.is_empty() {
        match backend {
            #[cfg(feature = "model-mysql")]
            Backend::MySql => format!("INSERT INTO {} () VALUES ()", T::table_name()),
            #[allow(unreachable_patterns)]
            _ => format!("INSERT INTO {} DEFAULT VALUES", T::table_name()),
        }
    } else {
        format!("INSERT INTO {} ({}) VALUES ({})", T::table_name(), cols.join(", "), marks.join(", "))
    };

    if T::primary_key_auto_increment() {
        key = Value::Int(pool.insert_returning_id(&insert_sql, &params, pk_name).await?);
    } else {
        pool.execute(&insert_sql, &params).await?;
    }
    select_by_pk(pool, key, false).await?.ok_or_else(|| DbError::new("inserted row not found"))
}

#[doc(hidden)]
pub async fn update_entity<T: Model>(pool: &DbPool, entity: &T) -> Result<T, DbError> {
    let backend = pool.backend();
    let pk = entity.primary_key_value();
    let pk_name = T::primary_key_name();
    let mut sets: Vec<String> = Vec::new();
    let mut params: Vec<Value> = Vec::new();
    let mut expected_version: Option<Value> = None;
    for (col, value) in entity.to_values() {
        if col == pk_name || Some(col) == T::created_at_column() || Some(col) == T::updated_at_column() || Some(col) == T::soft_delete_column() {
            continue;
        }
        if Some(col) == T::version_column() {
            expected_version = Some(value);
            continue;
        }
        if value == Value::Null {
            sets.push(format!("{col} = NULL"));
            continue;
        }
        params.push(value);
        sets.push(format!("{} = {}", col, placeholder(backend, params.len())));
    }
    if let Some(col) = T::updated_at_column() {
        params.push(Timestamp::now().to_value(backend));
        sets.push(format!("{} = {}", col, timestamp_placeholder(backend, params.len())));
    }
    if let Some(col) = T::version_column() {
        sets.push(format!("{col} = {col} + 1"));
    }
    if sets.is_empty() {
        // Nothing but the key: nothing to write.
        return select_by_pk(pool, pk.clone(), false)
            .await?
            .ok_or_else(|| DbError::not_found(format!("{} {}={}: no such row to update", T::table_name(), pk_name, show(&pk))));
    }

    params.push(pk.clone());
    let mut sql = format!(
        "UPDATE {} SET {} WHERE {} = {}",
        T::table_name(),
        sets.join(", "),
        pk_name,
        placeholder(backend, params.len()),
    );
    if let (Some(col), Some(version)) = (T::version_column(), &expected_version) {
        params.push(version.clone());
        sql.push_str(&format!(" AND {} = {}", col, placeholder(backend, params.len())));
    }

    if pool.execute(&sql, &params).await? == 0 {
        let current = select_by_pk::<T>(pool, pk.clone(), false).await?;
        let (Some(row), Some(col), Some(expected)) = (current, T::version_column(), expected_version) else {
            return Err(DbError::not_found(format!("{} {}={}: no such row to update", T::table_name(), pk_name, show(&pk))));
        };
        let now_at = row.to_values().into_iter().find(|(c, _)| *c == col).map(|(_, v)| v).unwrap_or(Value::Null);
        return Err(DbError::stale_version(format!(
            "{} {}={}: the row is at {} {}, this copy has {} — someone saved it since it was read; reload it and apply the change again",
            T::table_name(),
            pk_name,
            show(&pk),
            col,
            show(&now_at),
            show(&expected),
        )));
    }

    select_by_pk(pool, pk, false).await?.ok_or_else(|| DbError::new("updated row not found"))
}

/// `delete_by_id`: `UPDATE … SET deleted_at` for a soft-delete model (rows
/// deleted already keep their time), `DELETE` otherwise.
#[doc(hidden)]
pub async fn delete_entity<T: Model>(pool: &DbPool, pk: Value) -> Result<(), DbError> {
    let backend = pool.backend();
    match T::soft_delete_column() {
        Some(col) => {
            let sql = format!(
                "UPDATE {} SET {col} = {} WHERE {} = {} AND {col} IS NULL",
                T::table_name(),
                timestamp_placeholder(backend, 1),
                T::primary_key_name(),
                placeholder(backend, 2),
            );
            pool.execute(&sql, &[Timestamp::now().to_value(backend), pk]).await?;
        }
        None => {
            let sql = format!("DELETE FROM {} WHERE {} = {}", T::table_name(), T::primary_key_name(), placeholder(backend, 1));
            pool.execute(&sql, &[pk]).await?;
        }
    }
    Ok(())
}

fn show(v: &Value) -> String {
    match v {
        Value::Null => "NULL".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Text(s) => format!("{s:?}"),
        Value::Bytes(b) => format!("<{} bytes>", b.len()),
    }
}

#[doc(hidden)]
pub fn extract_count(rows: Vec<super::ModelRow>) -> Result<i64, DbError> {
    match rows.into_iter().next() {
        Some(row) => {
            if let Some((_, val)) = row.columns.first() {
                match val.clone() {
                    Value::Int(n) => return Ok(n),
                    other => return Err(DbError::new(format!("unexpected count value: {:?}", other))),
                }
            }
            Err(DbError::new("count query returned empty row"))
        }
        None => Ok(0),
    }
}
