//! Async relationship helpers: `HasMany`, `HasOne`, `BelongsTo`.
//!
//! These are explicit-load helpers — no hidden N+1 queries, no lazy loading.
//! Call `.load(&pool)` when you want the related records.

use std::marker::PhantomData;

use super::pool::DbPool;
use super::repository::placeholder;
use super::{DbError, Model, Value};

// ── HasMany ───────────────────────────────────────────────────────────────────

/// Represents a one-to-many relationship from an owner to a collection of `T`.
///
/// The `fk_col` column on the child table holds the owner's PK.
///
/// ```ignore
/// let posts: Vec<Post> = HasMany::new(Value::Int(user.id), "user_id")
///     .load(&pool).await?;
/// ```
#[derive(Debug, Clone)]
pub struct HasMany<T: Model> {
    owner_pk: Value,
    fk_col: &'static str,
    _phantom: PhantomData<T>,
}

impl<T: Model> HasMany<T> {
    pub fn new(owner_pk: Value, fk_col: &'static str) -> Self {
        HasMany { owner_pk, fk_col, _phantom: PhantomData }
    }

    /// Load all related records from the database.
    pub async fn load(&self, pool: &DbPool) -> Result<Vec<T>, DbError> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = {}",
            T::table_name(), self.fk_col, placeholder(1),
        );
        let rows = pool.query_rows(&sql, &[self.owner_pk.clone()]).await?;
        rows.iter().map(|r| T::from_row(r)).collect()
    }
}

// ── HasOne ────────────────────────────────────────────────────────────────────

/// Represents a one-to-one relationship (owner → child).
///
/// The `fk_col` column on the child table holds the owner's PK.
#[derive(Debug, Clone)]
pub struct HasOne<O: Model> {
    owner_pk: Value,
    fk_col: &'static str,
    _phantom: PhantomData<O>,
}

impl<O: Model> HasOne<O> {
    pub fn new(owner_pk: Value, fk_col: &'static str) -> Self {
        HasOne { owner_pk, fk_col, _phantom: PhantomData }
    }

    /// Load the related record from the database.
    pub async fn load(&self, pool: &DbPool) -> Result<Option<O>, DbError> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = {} LIMIT 1",
            O::table_name(), self.fk_col, placeholder(1),
        );
        let rows = pool.query_rows(&sql, &[self.owner_pk.clone()]).await?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(O::from_row(&row)?)),
            None => Ok(None),
        }
    }
}

// ── BelongsTo ─────────────────────────────────────────────────────────────────

/// Represents the inverse side of a relationship (child → owner).
///
/// `fk_value` is the foreign key stored on this record pointing to the owner's PK.
#[derive(Debug, Clone)]
pub struct BelongsTo<O: Model> {
    fk_value: Value,
    _phantom: PhantomData<O>,
}

impl<O: Model> BelongsTo<O> {
    pub fn new(fk_value: Value) -> Self {
        BelongsTo { fk_value, _phantom: PhantomData }
    }

    /// Load the owner record from the database.
    pub async fn load(&self, pool: &DbPool) -> Result<Option<O>, DbError> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = {} LIMIT 1",
            O::table_name(), O::primary_key_name(), placeholder(1),
        );
        let rows = pool.query_rows(&sql, &[self.fk_value.clone()]).await?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(O::from_row(&row)?)),
            None => Ok(None),
        }
    }
}
