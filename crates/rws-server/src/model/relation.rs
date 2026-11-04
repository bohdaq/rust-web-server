//! Async relationship helpers: `HasMany`, `HasOne`, `BelongsTo`, and
//! `BelongsToMany` (18.30.0) through a join table.
//!
//! These are explicit-load helpers — no hidden N+1 queries, no lazy loading.
//! Call `.load(&pool)` when you want the related records. Soft-deleted
//! related rows (`#[soft_delete]`) are left out. To load a relation for a
//! whole result set, see [`Relation`](super::Relation) and
//! [`QueryBuilder::with`](super::QueryBuilder::with).

use std::collections::HashSet;
use std::marker::PhantomData;

use super::eager::{distinct_keys, key_of, IN_BATCH};
use super::pool::{DbPool, DbTransaction};
use super::repository::{live_condition, placeholder};
use super::{DbError, Model, ToColumn, Value};

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
            "SELECT * FROM {} WHERE {} = {}{}",
            T::table_name(), self.fk_col, placeholder(pool.backend(), 1), and_live::<T>(),
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
            "SELECT * FROM {} WHERE {} = {}{} LIMIT 1",
            O::table_name(), self.fk_col, placeholder(pool.backend(), 1), and_live::<O>(),
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
            "SELECT * FROM {} WHERE {} = {}{} LIMIT 1",
            O::table_name(), O::primary_key_name(), placeholder(pool.backend(), 1), and_live::<O>(),
        );
        let rows = pool.query_rows(&sql, &[self.fk_value.clone()]).await?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(O::from_row(&row)?)),
            None => Ok(None),
        }
    }
}

fn and_live<T: Model>() -> String {
    live_condition::<T>().map(|c| format!(" AND {c}")).unwrap_or_default()
}

// ── BelongsToMany ─────────────────────────────────────────────────────────────

/// Many-to-many through a join table (18.30.0): the rows of `R` one owner
/// is linked to, and the links themselves — [`attach`](Self::attach),
/// [`detach`](Self::detach), [`sync`](Self::sync).
///
/// The join table holds the owner's key in `owner_key` and `R`'s in
/// `related_key`; give it a primary key or unique index over both (the
/// methods never insert a link twice, but two processes attaching at once
/// could). Other columns need defaults: the methods insert the two keys
/// only. [`Relation::belongs_to_many`](super::Relation::belongs_to_many)
/// loads the same links for a whole result set.
///
/// ```ignore
/// impl Post {
///     pub fn tags(&self) -> BelongsToMany<Tag> {
///         BelongsToMany::new(Value::Int(self.id), "post_tags", "post_id", "tag_id")
///     }
/// }
/// post.tags().attach(&pool, [1, 2]).await?;
/// post.tags().sync(&pool, [1, 3]).await?; // detaches 2, attaches 3
/// let tags: Vec<Tag> = post.tags().load(&pool).await?;
/// ```
pub struct BelongsToMany<R: Model> {
    owner_pk: Value,
    join_table: &'static str,
    owner_key: &'static str,
    related_key: &'static str,
    _phantom: PhantomData<fn() -> R>,
}

impl<R: Model> Clone for BelongsToMany<R> {
    fn clone(&self) -> Self {
        BelongsToMany { owner_pk: self.owner_pk.clone(), ..*self }
    }
}

impl<R: Model> std::fmt::Debug for BelongsToMany<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BelongsToMany")
            .field("owner_pk", &self.owner_pk)
            .field("join_table", &self.join_table)
            .field("owner_key", &self.owner_key)
            .field("related_key", &self.related_key)
            .finish()
    }
}

/// What [`BelongsToMany::sync`] changed: the related keys it linked and
/// unlinked.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct SyncChanges {
    pub attached: Vec<Value>,
    pub detached: Vec<Value>,
}

/// `values` as their to-column values, `NULL`s and repeats left out.
fn keys<I>(ids: I) -> Vec<Value>
where
    I: IntoIterator,
    I::Item: ToColumn,
{
    distinct_keys(&ids.into_iter().map(|id| id.to_column()).collect::<Vec<_>>())
}

fn first_column(row: super::ModelRow) -> Option<Value> {
    row.columns.into_iter().next().map(|(_, v)| v)
}

impl<R: Model> BelongsToMany<R> {
    /// The links of the owner with key `owner_pk` in `join_table`.
    pub fn new(owner_pk: Value, join_table: &'static str, owner_key: &'static str, related_key: &'static str) -> Self {
        BelongsToMany { owner_pk, join_table, owner_key, related_key, _phantom: PhantomData }
    }

    /// The linked rows of `R` (soft-deleted ones left out), by primary key.
    pub async fn load(&self, pool: &DbPool) -> Result<Vec<R>, DbError> {
        let pk = R::primary_key_name();
        let live = R::soft_delete_column().map(|c| format!(" AND r.{c} IS NULL")).unwrap_or_default();
        let sql = format!(
            "SELECT r.* FROM {} r INNER JOIN {} j ON j.{} = r.{pk} WHERE j.{} = {}{live} ORDER BY r.{pk}",
            R::table_name(),
            self.join_table,
            self.related_key,
            self.owner_key,
            placeholder(pool.backend(), 1),
        );
        let rows = pool.query_rows(&sql, &[self.owner_pk.clone()]).await?;
        rows.iter().map(|r| R::from_row(r)).collect()
    }

    /// The related keys linked to the owner, as the join table holds them.
    pub async fn ids(&self, pool: &DbPool) -> Result<Vec<Value>, DbError> {
        let rows = pool.query_rows(&self.ids_sql(pool), &[self.owner_pk.clone()]).await?;
        Ok(rows.into_iter().filter_map(first_column).collect())
    }

    fn ids_sql(&self, pool: &DbPool) -> String {
        format!("SELECT {} FROM {} WHERE {} = {}", self.related_key, self.join_table, self.owner_key, placeholder(pool.backend(), 1))
    }

    /// Link the owner to each of `ids` it isn't linked to yet, in one
    /// transaction; answers how many links were added.
    pub async fn attach<I>(&self, pool: &DbPool, ids: I) -> Result<u64, DbError>
    where
        I: IntoIterator,
        I::Item: ToColumn,
    {
        let wanted = keys(ids);
        if wanted.is_empty() {
            return Ok(0);
        }
        let mut tx = pool.begin().await?;
        let existing = self.linked(&mut tx, pool).await?.1;
        let mut added = 0;
        for id in wanted {
            if key_of(id.clone()).is_some_and(|k| !existing.contains(&k)) {
                self.insert_link(&mut tx, pool, id).await?;
                added += 1;
            }
        }
        tx.commit().await?;
        Ok(added)
    }

    /// Unlink the owner from each of `ids`; answers how many links were
    /// removed.
    pub async fn detach<I>(&self, pool: &DbPool, ids: I) -> Result<u64, DbError>
    where
        I: IntoIterator,
        I::Item: ToColumn,
    {
        let backend = pool.backend();
        let mut removed = 0;
        for batch in keys(ids).chunks(IN_BATCH) {
            let list = (2..=batch.len() + 1).map(|i| placeholder(backend, i)).collect::<Vec<_>>().join(", ");
            let sql = format!(
                "DELETE FROM {} WHERE {} = {} AND {} IN ({list})",
                self.join_table,
                self.owner_key,
                placeholder(backend, 1),
                self.related_key
            );
            let mut params = vec![self.owner_pk.clone()];
            params.extend(batch.iter().cloned());
            removed += pool.execute(&sql, &params).await?;
        }
        Ok(removed)
    }

    /// Unlink the owner from every row; answers how many links were
    /// removed.
    pub async fn detach_all(&self, pool: &DbPool) -> Result<u64, DbError> {
        let sql = format!("DELETE FROM {} WHERE {} = {}", self.join_table, self.owner_key, placeholder(pool.backend(), 1));
        pool.execute(&sql, &[self.owner_pk.clone()]).await
    }

    /// Make `ids` the owner's links, exactly: unlink the others and link
    /// the missing ones, in one transaction.
    pub async fn sync<I>(&self, pool: &DbPool, ids: I) -> Result<SyncChanges, DbError>
    where
        I: IntoIterator,
        I::Item: ToColumn,
    {
        let wanted = keys(ids);
        let wanted_keys: HashSet<String> = wanted.iter().filter_map(|v| key_of(v.clone())).collect();
        let backend = pool.backend();
        let mut tx = pool.begin().await?;
        let (existing, existing_keys) = self.linked(&mut tx, pool).await?;
        let mut changes = SyncChanges::default();
        for id in existing {
            if key_of(id.clone()).is_some_and(|k| !wanted_keys.contains(&k)) {
                let sql = format!(
                    "DELETE FROM {} WHERE {} = {} AND {} = {}",
                    self.join_table,
                    self.owner_key,
                    placeholder(backend, 1),
                    self.related_key,
                    placeholder(backend, 2)
                );
                tx.execute(&sql, &[self.owner_pk.clone(), id.clone()]).await?;
                changes.detached.push(id);
            }
        }
        for id in wanted {
            if key_of(id.clone()).is_some_and(|k| !existing_keys.contains(&k)) {
                self.insert_link(&mut tx, pool, id.clone()).await?;
                changes.attached.push(id);
            }
        }
        tx.commit().await?;
        Ok(changes)
    }

    /// The linked keys, and their map keys, read in `tx`.
    async fn linked(&self, tx: &mut DbTransaction, pool: &DbPool) -> Result<(Vec<Value>, HashSet<String>), DbError> {
        let rows = tx.query_rows(&self.ids_sql(pool), &[self.owner_pk.clone()]).await?;
        let values: Vec<Value> = rows.into_iter().filter_map(first_column).collect();
        let keys = values.iter().filter_map(|v| key_of(v.clone())).collect();
        Ok((values, keys))
    }

    async fn insert_link(&self, tx: &mut DbTransaction, pool: &DbPool, id: Value) -> Result<(), DbError> {
        let backend = pool.backend();
        let sql = format!(
            "INSERT INTO {} ({}, {}) VALUES ({}, {})",
            self.join_table,
            self.owner_key,
            self.related_key,
            placeholder(backend, 1),
            placeholder(backend, 2)
        );
        tx.execute(&sql, &[self.owner_pk.clone(), id]).await?;
        Ok(())
    }
}
