//! Relationship helpers: `HasMany`, `HasOne`, `BelongsTo`.
//!
//! These are explicit-load helpers — no hidden N+1 queries, no lazy loading
//! proxies.  Call `.load(&mut db)` when you want the related records.

use std::marker::PhantomData;

use super::connection::DbConnection;
use super::repository::placeholder;
use super::{DbError, Model, Value};

// ── HasMany ───────────────────────────────────────────────────────────────────

/// Represents a one-to-many relationship from an owner to a collection of
/// `T` records.
///
/// The `fk_col` column on the child table holds the owner's PK.
///
/// # Example
///
/// ```ignore
/// let posts: Vec<Post> = user.posts.load(&mut db)?;
/// ```
#[derive(Debug, Clone)]
pub struct HasMany<T: Model> {
    owner_pk: Value,
    fk_col: &'static str,
    _phantom: PhantomData<T>,
}

impl<T: Model> HasMany<T> {
    pub fn new(owner_pk: Value, fk_col: &'static str) -> Self {
        HasMany {
            owner_pk,
            fk_col,
            _phantom: PhantomData,
        }
    }

    /// Load all related records from the database.
    pub fn load(&self, conn: &mut DbConnection) -> Result<Vec<T>, DbError> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = {}",
            T::table_name(),
            self.fk_col,
            placeholder(1)
        );
        let rows = conn.query_rows(&sql, &[self.owner_pk.clone()])?;
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
        HasOne {
            owner_pk,
            fk_col,
            _phantom: PhantomData,
        }
    }

    /// Load the related record from the database.
    pub fn load(&self, conn: &mut DbConnection) -> Result<Option<O>, DbError> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = {} LIMIT 1",
            O::table_name(),
            self.fk_col,
            placeholder(1)
        );
        let rows = conn.query_rows(&sql, &[self.owner_pk.clone()])?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(O::from_row(&row)?)),
            None => Ok(None),
        }
    }
}

// ── BelongsTo ─────────────────────────────────────────────────────────────────

/// Represents the inverse side of a relationship (child → owner).
///
/// `fk_value` is the foreign key value stored on this record that points to the
/// owner's primary key.
#[derive(Debug, Clone)]
pub struct BelongsTo<O: Model> {
    fk_value: Value,
    _phantom: PhantomData<O>,
}

impl<O: Model> BelongsTo<O> {
    pub fn new(fk_value: Value) -> Self {
        BelongsTo {
            fk_value,
            _phantom: PhantomData,
        }
    }

    /// Load the owner record from the database.
    pub fn load(&self, conn: &mut DbConnection) -> Result<Option<O>, DbError> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = {} LIMIT 1",
            O::table_name(),
            O::primary_key_name(),
            placeholder(1)
        );
        let rows = conn.query_rows(&sql, &[self.fk_value.clone()])?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(O::from_row(&row)?)),
            None => Ok(None),
        }
    }
}
