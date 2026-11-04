//! Eager loading (18.30.0): a [`Relation`] names how one model reaches
//! another; [`QueryBuilder::with`](super::QueryBuilder::with) loads it for a
//! whole result set in one more query per relation (`WHERE fk IN (…)`), not
//! one per row.
//!
//! ```rust,ignore
//! use rust_web_server::model::{Order, Relation};
//!
//! impl Post {
//!     pub fn comments() -> Relation<Post, Comment> { Relation::has_many("post_id") }
//!     pub fn author() -> Relation<Post, User> { Relation::belongs_to("user_id") }
//!     pub fn tags() -> Relation<Post, Tag> { Relation::belongs_to_many("post_tags", "post_id", "tag_id") }
//! }
//!
//! // Three queries: the posts, their comments, their authors.
//! let posts = Post::query(&pool)
//!     .order_by("id", Order::Desc)
//!     .with(Post::comments())
//!     .with(Post::author())
//!     .fetch_all()
//!     .await?;
//! for post in &posts {
//!     let comments: &[Comment] = post.related(&Post::comments());
//!     let author: Option<&User> = post.one(&Post::author());
//!     println!("{} by {:?}: {} comments", post.title, author.map(|a| &a.name), comments.len());
//! }
//! ```
//!
//! Related rows that are soft-deleted (`#[soft_delete]`) are left out, as
//! everywhere else. Keys are sent in batches of [`IN_BATCH`], so a result
//! set larger than that costs one query per batch.

use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use super::pool::DbPool;
use super::query::QueryBuilder;
use super::repository::placeholder;
use super::{DbError, Model, ModelRow, Value};
use crate::pagination::Page;

/// How many keys one `IN (…)` list carries; more are sent in more queries.
pub const IN_BATCH: usize = 500;

/// The column eager loading of a many-to-many relation adds to each related
/// row: the owner's key, from the join table.
const OWNER_KEY: &str = "rws_owner_key";

// ── RelationKind ──────────────────────────────────────────────────────────────

/// What a [`Relation`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RelationKind {
    /// The related table's foreign key holds the owner's primary key; any
    /// number of rows.
    HasMany,
    /// As `HasMany`, at most one row (the one with the lowest key).
    HasOne,
    /// The owner's foreign key holds the related row's primary key.
    BelongsTo,
    /// Through a join table holding both keys.
    BelongsToMany,
}

// ── Relation ──────────────────────────────────────────────────────────────────

/// How a model `O` reaches related rows of `R`: for
/// [`QueryBuilder::with`](super::QueryBuilder::with), and to load the rows
/// of one owner ([`load`](Relation::load)) or of many
/// ([`load_for`](Relation::load_for)). Usually returned by an associated
/// function of the owner, so call sites name it:
///
/// ```rust,ignore
/// impl User {
///     pub fn posts() -> Relation<User, Post> { Relation::has_many("user_id") }
/// }
/// let posts: Vec<Post> = User::posts().load(&pool, &user).await?;
/// ```
pub struct Relation<O, R> {
    kind: RelationKind,
    /// `HasMany`/`HasOne`: the related table's column; `BelongsTo`: the
    /// owner's; `BelongsToMany`: the join table's column holding the
    /// owner's key.
    foreign_key: &'static str,
    /// `BelongsToMany`: the join table and its column holding the related
    /// row's key.
    join: Option<(&'static str, &'static str)>,
    _models: PhantomData<fn() -> (O, R)>,
}

impl<O, R> Clone for Relation<O, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<O, R> Copy for Relation<O, R> {}

impl<O, R> std::fmt::Debug for Relation<O, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Relation")
            .field("kind", &self.kind)
            .field("foreign_key", &self.foreign_key)
            .field("join", &self.join)
            .finish()
    }
}

impl<O: Model, R: Model> Relation<O, R> {
    /// One owner, many `R`: `R`'s column `foreign_key` holds the owner's
    /// primary key (`Relation::<User, Post>::has_many("user_id")`).
    pub fn has_many(foreign_key: &'static str) -> Self {
        Relation { kind: RelationKind::HasMany, foreign_key, join: None, _models: PhantomData }
    }

    /// As [`has_many`](Self::has_many), at most one `R` per owner (the one
    /// with the lowest primary key when there are more).
    pub fn has_one(foreign_key: &'static str) -> Self {
        Relation { kind: RelationKind::HasOne, foreign_key, join: None, _models: PhantomData }
    }

    /// The owner's column `foreign_key` holds `R`'s primary key
    /// (`Relation::<Post, User>::belongs_to("user_id")`); `NULL` means
    /// none.
    pub fn belongs_to(foreign_key: &'static str) -> Self {
        Relation { kind: RelationKind::BelongsTo, foreign_key, join: None, _models: PhantomData }
    }

    /// Many-to-many through `join_table`, whose column `owner_key` holds
    /// the owner's primary key and `related_key` `R`'s
    /// (`Relation::<Post, Tag>::belongs_to_many("post_tags", "post_id",
    /// "tag_id")`). [`BelongsToMany`](super::BelongsToMany) adds and
    /// removes the links.
    pub fn belongs_to_many(join_table: &'static str, owner_key: &'static str, related_key: &'static str) -> Self {
        Relation { kind: RelationKind::BelongsToMany, foreign_key: owner_key, join: Some((join_table, related_key)), _models: PhantomData }
    }

    /// What the relation is.
    pub fn kind(&self) -> RelationKind {
        self.kind
    }

    /// The column holding the key that links the two: `R`'s for
    /// `HasMany`/`HasOne`, `O`'s for `BelongsTo`, the join table's owner
    /// column for `BelongsToMany`.
    pub fn foreign_key(&self) -> &'static str {
        self.foreign_key
    }

    /// `BelongsToMany`: the join table and its column holding `R`'s key.
    pub fn join_table(&self) -> Option<(&'static str, &'static str)> {
        self.join
    }

    /// Identifies the relation among an owner's loaded ones.
    fn key(&self) -> String {
        format!("{:?}:{}:{}:{:?}", self.kind, R::table_name(), self.foreign_key, self.join)
    }

    /// The related rows of one owner (one query).
    pub async fn load(&self, pool: &DbPool, owner: &O) -> Result<Vec<R>, DbError>
    where
        R: Clone,
    {
        Ok(self.load_for(pool, std::slice::from_ref(owner)).await?.pop().unwrap_or_default())
    }

    /// The related rows of every owner, in the owners' order — one query
    /// per [`IN_BATCH`] distinct keys, the related rows by primary key. A
    /// row related to several owners (a shared author, a tag) is cloned
    /// into each.
    pub async fn load_for(&self, pool: &DbPool, owners: &[O]) -> Result<Vec<Vec<R>>, DbError>
    where
        R: Clone,
    {
        // The key each owner is matched by.
        let owner_keys: Vec<Value> = match self.kind {
            RelationKind::BelongsTo => {
                let mut keys = Vec::with_capacity(owners.len());
                for owner in owners {
                    let value = owner.to_values().into_iter().find(|(c, _)| c.eq_ignore_ascii_case(self.foreign_key)).map(|(_, v)| v);
                    keys.push(value.ok_or_else(|| {
                        DbError::new(format!("belongs_to: {} has no column {}", O::table_name(), self.foreign_key))
                    })?);
                }
                keys
            }
            _ => owners.iter().map(|o| o.primary_key_value()).collect(),
        };
        let distinct = distinct_keys(&owner_keys);
        let pk = R::primary_key_name();
        let (rows, match_column) = match self.kind {
            RelationKind::HasMany | RelationKind::HasOne => {
                let live = live_and::<R>("");
                let rows = fetch_in(pool, &distinct, |list| {
                    format!("SELECT * FROM {} WHERE {} IN ({list}){live} ORDER BY {pk}", R::table_name(), self.foreign_key)
                })
                .await?;
                (rows, self.foreign_key)
            }
            RelationKind::BelongsTo => {
                let live = live_and::<R>("");
                let rows = fetch_in(pool, &distinct, |list| {
                    format!("SELECT * FROM {} WHERE {pk} IN ({list}){live} ORDER BY {pk}", R::table_name())
                })
                .await?;
                (rows, pk)
            }
            RelationKind::BelongsToMany => {
                let (join, related_key) = self.join.unwrap_or(("", ""));
                let live = live_and::<R>("r.");
                let rows = fetch_in(pool, &distinct, |list| {
                    format!(
                        "SELECT r.*, j.{owner} AS {OWNER_KEY} FROM {} r INNER JOIN {join} j ON j.{related_key} = r.{pk} \
                         WHERE j.{owner} IN ({list}){live} ORDER BY r.{pk}",
                        R::table_name(),
                        owner = self.foreign_key,
                    )
                })
                .await?;
                (rows, OWNER_KEY)
            }
        };
        let mut by_key: HashMap<String, Vec<R>> = HashMap::new();
        for row in &rows {
            let Some(key) = column(row, match_column).and_then(key_of) else { continue };
            by_key.entry(key).or_default().push(R::from_row(row)?);
        }
        let one = matches!(self.kind, RelationKind::HasOne | RelationKind::BelongsTo);
        Ok(owner_keys
            .iter()
            .map(|k| {
                let mut related = key_of(k.clone()).and_then(|k| by_key.get(&k)).cloned().unwrap_or_default();
                if one {
                    related.truncate(1);
                }
                related
            })
            .collect())
    }
}

/// `" AND <prefix>deleted_at IS NULL"` for a soft-delete model.
fn live_and<T: Model>(prefix: &str) -> String {
    T::soft_delete_column().map(|c| format!(" AND {prefix}{c} IS NULL")).unwrap_or_default()
}

/// A row's value of `name`.
fn column(row: &ModelRow, name: &str) -> Option<Value> {
    row.columns.iter().find(|(c, _)| c.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
}

/// A value as a map key: `None` for `NULL` (it matches nothing). Integers
/// and their text spelling are different keys — the drivers decode a column
/// as one or the other, consistently.
pub(crate) fn key_of(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(format!("i{}", b as i64)),
        Value::Int(n) => Some(format!("i{n}")),
        Value::Float(f) => Some(format!("f{f}")),
        Value::Text(s) => Some(format!("t{s}")),
        Value::Bytes(b) => Some(format!("b{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>())),
    }
}

/// `values` without `NULL`s and repeats, in first-seen order.
pub(crate) fn distinct_keys(values: &[Value]) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    values.iter().filter(|v| key_of((*v).clone()).is_some_and(|k| seen.insert(k))).cloned().collect()
}

/// Rows of `sql(list)` for every batch of `keys`, `list` being the batch's
/// placeholders.
async fn fetch_in(pool: &DbPool, keys: &[Value], sql: impl Fn(String) -> String) -> Result<Vec<ModelRow>, DbError> {
    let backend = pool.backend();
    let mut rows = Vec::new();
    for batch in keys.chunks(IN_BATCH) {
        let list = (1..=batch.len()).map(|i| placeholder(backend, i)).collect::<Vec<_>>().join(", ");
        rows.extend(pool.query_rows(&sql(list), batch).await?);
    }
    Ok(rows)
}

// ── Loaded ────────────────────────────────────────────────────────────────────

/// A model with the relations [`QueryBuilder::with`](super::QueryBuilder::with)
/// loaded for it. Derefs to the model.
pub struct Loaded<T> {
    model: T,
    relations: Vec<(String, Box<dyn Any + Send + Sync>)>,
}

impl<T: Model> Loaded<T> {
    /// The model.
    pub fn model(&self) -> &T {
        &self.model
    }

    /// The model, relations dropped.
    pub fn into_inner(self) -> T {
        self.model
    }

    /// The rows of `relation` (empty when there are none, or when the query
    /// didn't load it — see [`is_loaded`](Self::is_loaded)).
    pub fn related<R: Model + 'static>(&self, relation: &Relation<T, R>) -> &[R] {
        self.get(relation).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The first row of `relation`: the one of a `belongs_to` / `has_one`.
    pub fn one<R: Model + 'static>(&self, relation: &Relation<T, R>) -> Option<&R> {
        self.related(relation).first()
    }

    /// Whether the query loaded `relation`.
    pub fn is_loaded<R: Model + 'static>(&self, relation: &Relation<T, R>) -> bool {
        self.get(relation).is_some()
    }

    /// The rows of `relation`, taken out (empty when not loaded).
    pub fn take<R: Model + 'static>(&mut self, relation: &Relation<T, R>) -> Vec<R> {
        let key = relation.key();
        match self.relations.iter().position(|(k, _)| *k == key) {
            Some(i) => self.relations.remove(i).1.downcast::<Vec<R>>().map(|b| *b).unwrap_or_default(),
            None => Vec::new(),
        }
    }

    fn get<R: Model + 'static>(&self, relation: &Relation<T, R>) -> Option<&Vec<R>> {
        let key = relation.key();
        self.relations.iter().find(|(k, _)| *k == key).and_then(|(_, v)| v.downcast_ref::<Vec<R>>())
    }
}

impl<T> std::ops::Deref for Loaded<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.model
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Loaded<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loaded")
            .field("model", &self.model)
            .field("relations", &self.relations.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>())
            .finish()
    }
}

// ── EagerQuery ────────────────────────────────────────────────────────────────

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
type Erased = Box<dyn Any + Send + Sync>;

/// A relation of `O` with its related type erased.
trait Loader<O>: Send + Sync {
    fn key(&self) -> String;
    fn load<'a>(&'a self, pool: &'a DbPool, owners: &'a [O]) -> BoxFuture<'a, Result<Vec<Erased>, DbError>>;
}

impl<O: Model + 'static, R: Model + Clone + 'static> Loader<O> for Relation<O, R> {
    fn key(&self) -> String {
        Relation::key(self)
    }

    fn load<'a>(&'a self, pool: &'a DbPool, owners: &'a [O]) -> BoxFuture<'a, Result<Vec<Erased>, DbError>> {
        Box::pin(async move {
            let related = self.load_for(pool, owners).await?;
            Ok(related.into_iter().map(|r| Box::new(r) as Erased).collect())
        })
    }
}

/// A [`QueryBuilder`] with relations to load: from
/// [`QueryBuilder::with`](super::QueryBuilder::with). Its `fetch_*` answer
/// [`Loaded`] models.
pub struct EagerQuery<'a, T: Model> {
    query: QueryBuilder<'a, T>,
    loaders: Vec<Box<dyn Loader<T>>>,
}

impl<'a, T: Model + 'static> EagerQuery<'a, T> {
    pub(crate) fn new(query: QueryBuilder<'a, T>) -> Self {
        EagerQuery { query, loaders: Vec::new() }
    }

    /// Load `relation` too (once, however often it is named).
    pub fn with<R: Model + Clone + 'static>(mut self, relation: Relation<T, R>) -> Self {
        let key = relation.key();
        if !self.loaders.iter().any(|l| l.key() == key) {
            self.loaders.push(Box::new(relation));
        }
        self
    }

    /// The query's rows, each with its relations: one query for the rows,
    /// then one per relation.
    pub async fn fetch_all(self) -> Result<Vec<Loaded<T>>, DbError> {
        let pool = self.query.pool();
        let models = self.query.fetch_all().await?;
        attach(pool, models, &self.loaders).await
    }

    /// The query's first row, with its relations.
    pub async fn fetch_one(self) -> Result<Option<Loaded<T>>, DbError> {
        let pool = self.query.pool();
        let model = self.query.fetch_one().await?;
        Ok(attach(pool, model.into_iter().collect(), &self.loaders).await?.pop())
    }

    /// [`QueryBuilder::paginate`], each row with its relations.
    pub async fn paginate(self, page: u64, per_page: u64) -> Result<Page<Loaded<T>>, DbError> {
        let pool = self.query.pool();
        let found = self.query.paginate(page, per_page).await?;
        let (page, per_page, total) = (found.page, found.per_page, found.total_items);
        let items = attach(pool, found.items, &self.loaders).await?;
        Ok(Page::new(items, page, per_page, total))
    }
}

async fn attach<T: Model>(pool: &DbPool, models: Vec<T>, loaders: &[Box<dyn Loader<T>>]) -> Result<Vec<Loaded<T>>, DbError> {
    let mut relations: Vec<Vec<(String, Erased)>> = models.iter().map(|_| Vec::with_capacity(loaders.len())).collect();
    if !models.is_empty() {
        for loader in loaders {
            let key = loader.key();
            for (slot, related) in relations.iter_mut().zip(loader.load(pool, &models).await?) {
                slot.push((key.clone(), related));
            }
        }
    }
    Ok(models.into_iter().zip(relations).map(|(model, relations)| Loaded { model, relations }).collect())
}
