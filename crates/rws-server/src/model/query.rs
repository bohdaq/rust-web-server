//! Async fluent `QueryBuilder<T>` for constructing SQL queries.

use std::marker::PhantomData;

use super::backend::Backend;
use super::pool::DbPool;
use super::repository::{extract_count, live_condition, placeholder, timestamp_placeholder};
use super::timestamp::Timestamp;
use super::{DbError, Model, ToColumn, Value};
use crate::pagination::{CursorPage, Page};

// ── Order ─────────────────────────────────────────────────────────────────────

/// Sort direction for `order_by`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Asc,
    Desc,
}

impl Order {
    fn as_sql(self) -> &'static str {
        match self {
            Order::Asc => "ASC",
            Order::Desc => "DESC",
        }
    }
}

// ── Deleted ───────────────────────────────────────────────────────────────────

/// Which rows of a soft-delete model a query sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Deleted {
    /// Not soft-deleted (the default).
    Exclude,
    /// All.
    Include,
    /// Soft-deleted only.
    Only,
}

// ── QueryBuilder ──────────────────────────────────────────────────────────────

/// Fluent async query builder for `SELECT`, `COUNT`, `DELETE`, and `UPDATE`.
///
/// For a model with a soft-delete column (`#[soft_delete]`) every operation
/// sees only rows not deleted, unless the chain has
/// [`with_deleted`](Self::with_deleted) or
/// [`only_deleted`](Self::only_deleted); [`delete`](Self::delete) sets the
/// column and [`force_delete`](Self::force_delete) removes rows. With
/// `#[timestamps]` / `#[version]`, [`update`](Self::update) also sets
/// `updated_at` and increments the version (18.30.0).
///
/// ```rust,no_run
/// # extern crate rws_server as rust_web_server;
/// # #[cfg(feature = "model-sqlite")]
/// # async fn example() -> Result<(), rust_web_server::model::DbError> {
/// use rust_web_server::model::{DbPool, Order, QueryBuilder};
///
/// let pool = DbPool::memory().await?;
/// // pool.execute("CREATE TABLE users …", &[]).await?;
///
/// // let users: Vec<User> = QueryBuilder::<User>::new(&pool)
/// //     .where_eq("active", true)
/// //     .order_by("name", Order::Asc)
/// //     .limit(10)
/// //     .fetch_all()
/// //     .await?;
/// # Ok(())
/// # }
/// ```
pub struct QueryBuilder<'a, T: Model> {
    pool: &'a DbPool,
    /// Each filter is a (sql_fragment, params) pair.
    /// sql_fragment may contain `__placeholder__` (from `where_eq`) or raw `?`.
    filters: Vec<(String, Vec<Value>)>,
    order: Option<(String, Order)>,
    limit: Option<u64>,
    offset: Option<u64>,
    deleted: Deleted,
    _phantom: PhantomData<T>,
}

// Implemented by hand rather than `#[derive(Clone)]`: the derive macro adds a
// `T: Clone` bound even though `T` only appears behind `PhantomData`, which
// would force every `QueryBuilder<T>` user's model type to be `Clone` just to
// call `.paginate()`.
impl<'a, T: Model> Clone for QueryBuilder<'a, T> {
    fn clone(&self) -> Self {
        QueryBuilder {
            pool: self.pool,
            filters: self.filters.clone(),
            order: self.order.clone(),
            limit: self.limit,
            offset: self.offset,
            deleted: self.deleted,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: Model> QueryBuilder<'a, T> {
    pub fn new(pool: &'a DbPool) -> Self {
        QueryBuilder {
            pool,
            filters: Vec::new(),
            order: None,
            limit: None,
            offset: None,
            deleted: Deleted::Exclude,
            _phantom: PhantomData,
        }
    }

    /// Soft-deleted rows too (no effect on a model without `#[soft_delete]`).
    pub fn with_deleted(mut self) -> Self {
        self.deleted = Deleted::Include;
        self
    }

    /// Soft-deleted rows only (none for a model without `#[soft_delete]`).
    pub fn only_deleted(mut self) -> Self {
        self.deleted = Deleted::Only;
        self
    }

    /// The filters plus the soft-delete scope.
    fn scoped_filters(&self) -> Vec<(String, Vec<Value>)> {
        let mut filters = self.filters.clone();
        if let Some(col) = T::soft_delete_column() {
            match self.deleted {
                Deleted::Exclude => filters.extend(live_condition::<T>().map(|c| (c, Vec::new()))),
                Deleted::Include => {}
                Deleted::Only => filters.push((format!("{col} IS NOT NULL"), Vec::new())),
            }
        } else if self.deleted == Deleted::Only {
            filters.push(("1 = 0".to_string(), Vec::new()));
        }
        filters
    }

    /// Add a `col = ?` equality filter.
    pub fn where_eq(mut self, col: &str, val: impl ToColumn) -> Self {
        self.filters.push((
            format!("{} = __placeholder__", col),
            vec![val.to_column()],
        ));
        self
    }

    /// Add a `col IN (?, ?, …)` filter (18.30.0); no values match no rows.
    pub fn where_in<I>(mut self, col: &str, values: I) -> Self
    where
        I: IntoIterator,
        I::Item: ToColumn,
    {
        let values: Vec<Value> = values.into_iter().map(|v| v.to_column()).collect();
        if values.is_empty() {
            self.filters.push(("1 = 0".to_string(), Vec::new()));
        } else {
            let marks = vec!["__placeholder__"; values.len()].join(", ");
            self.filters.push((format!("{col} IN ({marks})"), values));
        }
        self
    }

    /// Add a `col IS NULL` filter (18.30.0).
    pub fn where_null(mut self, col: &str) -> Self {
        self.filters.push((format!("{col} IS NULL"), Vec::new()));
        self
    }

    /// Add a `col IS NOT NULL` filter (18.30.0).
    pub fn where_not_null(mut self, col: &str) -> Self {
        self.filters.push((format!("{col} IS NOT NULL"), Vec::new()));
        self
    }

    /// Apply a scope (18.30.0): a reusable function from a query to a
    /// narrower one. Scopes are plain functions (or closures), so they
    /// take arguments and compose:
    ///
    /// ```rust,ignore
    /// fn published(q: QueryBuilder<'_, Post>) -> QueryBuilder<'_, Post> {
    ///     q.where_eq("published", true).where_null("hidden_at")
    /// }
    /// fn by(author: i64) -> impl FnOnce(QueryBuilder<'_, Post>) -> QueryBuilder<'_, Post> {
    ///     move |q| q.where_eq("author_id", author)
    /// }
    /// let mine = Post::query(&pool).scope(published).scope(by(7)).fetch_all().await?;
    /// ```
    ///
    /// For `Post::query(&pool).published()`, put them in an extension
    /// trait:
    ///
    /// ```rust,ignore
    /// pub trait PostScopes {
    ///     fn published(self) -> Self;
    /// }
    /// impl PostScopes for QueryBuilder<'_, Post> {
    ///     fn published(self) -> Self {
    ///         self.where_eq("published", true)
    ///     }
    /// }
    /// ```
    pub fn scope(self, scope: impl FnOnce(Self) -> Self) -> Self {
        scope(self)
    }

    /// Load `relation` for every row the query answers (18.30.0): one more
    /// query per relation, however many rows. The `fetch_*` of the
    /// [`EagerQuery`](super::EagerQuery) it returns answer
    /// [`Loaded`](super::Loaded) models.
    ///
    /// ```rust,ignore
    /// let posts = Post::query(&pool).with(Post::comments()).fetch_all().await?;
    /// let first_comments: &[Comment] = posts[0].related(&Post::comments());
    /// ```
    pub fn with<R: Model + Clone + 'static>(self, relation: super::Relation<T, R>) -> super::EagerQuery<'a, T>
    where
        T: 'static,
    {
        super::EagerQuery::new(self).with(relation)
    }

    /// The pool the query runs on.
    pub(crate) fn pool(&self) -> &'a DbPool {
        self.pool
    }

    /// Add a raw SQL filter fragment with bind parameters.
    ///
    /// ```ignore
    /// .filter("age >= ?", vec![Value::Int(18)])
    /// ```
    pub fn filter(mut self, expr: &str, params: Vec<Value>) -> Self {
        self.filters.push((expr.to_owned(), params));
        self
    }

    /// Set the ORDER BY clause.
    pub fn order_by(mut self, col: &str, order: Order) -> Self {
        self.order = Some((col.to_owned(), order));
        self
    }

    /// Set the LIMIT clause.
    pub fn limit(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Set the OFFSET clause.
    pub fn offset(mut self, n: u64) -> Self {
        self.offset = Some(n);
        self
    }

    // ── Terminal operations ───────────────────────────────────────────────────

    /// Execute `SELECT * FROM table WHERE … ORDER BY … LIMIT … OFFSET …`.
    pub async fn fetch_all(self) -> Result<Vec<T>, DbError> {
        let backend = self.pool.backend();
        let filters = self.scoped_filters();
        let (sql, params) = build_select::<T>(
            filters, self.order, self.limit, self.offset, "*", backend,
        );
        let rows = self.pool.query_rows(&sql, &params).await?;
        rows.iter().map(|r| T::from_row(r)).collect()
    }

    /// Execute `SELECT * … LIMIT 1` and return the first result.
    pub async fn fetch_one(self) -> Result<Option<T>, DbError> {
        let backend = self.pool.backend();
        let filters = self.scoped_filters();
        let (sql, params) = build_select::<T>(
            filters, self.order, Some(1), self.offset, "*", backend,
        );
        let rows = self.pool.query_rows(&sql, &params).await?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(T::from_row(&row)?)),
            None => Ok(None),
        }
    }

    /// Execute `SELECT COUNT(*) FROM table WHERE …` (without the chain's
    /// `ORDER BY`, which PostgreSQL refuses next to an aggregate).
    pub async fn count(self) -> Result<i64, DbError> {
        let backend = self.pool.backend();
        let filters = self.scoped_filters();
        let (sql, params) = build_select::<T>(
            filters, None, self.limit, self.offset, "COUNT(*)", backend,
        );
        let rows = self.pool.query_rows(&sql, &params).await?;
        extract_count(rows)
    }

    /// Execute `DELETE FROM table WHERE …` — for a soft-delete model,
    /// `UPDATE table SET deleted_at = now WHERE … AND deleted_at IS NULL`
    /// instead ([`force_delete`](Self::force_delete) removes the rows).
    pub async fn delete(self) -> Result<(), DbError> {
        let Some(col) = T::soft_delete_column() else {
            return self.force_delete().await.map(|_| ());
        };
        let backend = self.pool.backend();
        let mut filters = self.filters.clone();
        filters.extend(live_condition::<T>().map(|c| (c, Vec::new())));
        let (where_clause, where_params, _) = build_where(filters, 1, backend);
        let mut params = vec![Timestamp::now().to_value(backend)];
        params.extend(where_params);
        let sql = format!("UPDATE {} SET {} = {}{}", T::table_name(), col, timestamp_placeholder(backend, 1), where_clause);
        self.pool.execute(&sql, &params).await?;
        Ok(())
    }

    /// Execute `DELETE FROM table WHERE …`, removing the rows whatever the
    /// soft-delete column says (within the query's scope: add
    /// [`with_deleted`](Self::with_deleted) to include deleted rows).
    /// Answers the number of rows removed.
    pub async fn force_delete(self) -> Result<u64, DbError> {
        let backend = self.pool.backend();
        let (where_clause, params, _) = build_where(self.scoped_filters(), 0, backend);
        let sql = format!("DELETE FROM {}{}", T::table_name(), where_clause);
        self.pool.execute(&sql, &params).await
    }

    /// Clear the soft-delete column of the matching deleted rows; answers
    /// how many. An error for a model without `#[soft_delete]`.
    pub async fn restore(self) -> Result<u64, DbError> {
        let Some(col) = T::soft_delete_column() else {
            return Err(DbError::new(format!(
                "restore: {} has no soft-delete column (add #[soft_delete] and a deleted_at field)",
                T::table_name()
            )));
        };
        let backend = self.pool.backend();
        let mut filters = self.filters.clone();
        filters.push((format!("{col} IS NOT NULL"), Vec::new()));
        let (where_clause, params, _) = build_where(filters, 0, backend);
        let sql = format!("UPDATE {} SET {} = NULL{}", T::table_name(), col, where_clause);
        self.pool.execute(&sql, &params).await
    }

    /// Execute `UPDATE table SET col = ? WHERE …` (also `updated_at = now`
    /// and `version = version + 1` when the model has them).
    pub async fn update(self, col: &str, val: impl ToColumn) -> Result<(), DbError> {
        let backend = self.pool.backend();
        let (mut sets, mut params) = match val.to_column() {
            // A literal NULL: PostgreSQL won't assign one bound as text to
            // a column of another type.
            Value::Null => (vec![format!("{col} = NULL")], Vec::new()),
            value => (vec![format!("{} = {}", col, placeholder(backend, 1))], vec![value]),
        };
        if let Some(updated) = T::updated_at_column().filter(|c| *c != col) {
            params.push(Timestamp::now().to_value(backend));
            sets.push(format!("{} = {}", updated, timestamp_placeholder(backend, params.len())));
        }
        if let Some(version) = T::version_column().filter(|c| *c != col) {
            sets.push(format!("{version} = {version} + 1"));
        }
        let (where_clause, where_params, _) = build_where(self.scoped_filters(), params.len(), backend);
        params.extend(where_params);
        let sql = format!("UPDATE {} SET {}{}", T::table_name(), sets.join(", "), where_clause);
        self.pool.execute(&sql, &params).await?;
        Ok(())
    }

    /// Offset-paginated fetch: runs a `COUNT(*)` (with the same filters, no
    /// limit/offset) and a `SELECT … LIMIT … OFFSET …`, and wraps both into a
    /// [`Page`]. `page` is 1-based; both `page` and `per_page` are clamped to
    /// a minimum of `1`. Overrides any `.limit()`/`.offset()` set earlier in
    /// the chain — `page`/`per_page` are authoritative.
    ///
    /// Two queries, not one — if you're paginating a very large, frequently
    /// appended-to table and don't need `total_pages`, [`Self::paginate_after`]
    /// (keyset pagination) needs only one.
    pub async fn paginate(self, page: u64, per_page: u64) -> Result<Page<T>, DbError> {
        let page = page.max(1);
        let per_page = per_page.max(1);
        let offset = (page - 1) * per_page;

        let total_items = self.clone().count().await? as u64;
        let items = self.limit(per_page).offset(offset).fetch_all().await?;

        Ok(Page::new(items, page, per_page, total_items))
    }

    /// Cursor (keyset) paginated fetch: orders by the primary key ascending,
    /// fetches `per_page + 1` rows to cheaply detect whether there's a next
    /// page, and returns a [`CursorPage`] whose `next_cursor` is the last
    /// returned row's primary key (as a string) — pass it back as `cursor` to
    /// get the next page. `cursor: None` fetches the first page.
    ///
    /// A single query, unlike [`Self::paginate`] — no `COUNT(*)`, and no
    /// `OFFSET` to skip over on every subsequent page, which is what makes
    /// keyset pagination scale on large tables where offset pagination gets
    /// slower page by page. The tradeoff: no `total_items`/`total_pages`, and
    /// only forward iteration (no jumping to an arbitrary page).
    ///
    /// Overrides any `.order_by()` set earlier in the chain — keyset
    /// pagination requires ordering by the cursor column. Requires the
    /// primary key to be numeric (parsed as `i64`); returns `Err` if `cursor`
    /// isn't a valid integer.
    pub async fn paginate_after(self, cursor: Option<&str>, per_page: u64) -> Result<CursorPage<T>, DbError> {
        let per_page = per_page.max(1);
        let pk_col = T::primary_key_name();

        let mut builder = self;
        if let Some(cursor_str) = cursor {
            let cursor_val: i64 = cursor_str
                .parse()
                .map_err(|_| DbError::new(format!("invalid cursor '{}': expected an integer primary key", cursor_str)))?;
            builder = builder.filter(&format!("{} > __placeholder__", pk_col), vec![Value::Int(cursor_val)]);
        }

        let mut rows = builder.order_by(pk_col, Order::Asc).limit(per_page + 1).fetch_all().await?;

        let has_more = rows.len() as u64 > per_page;
        if has_more {
            rows.truncate(per_page as usize);
        }

        let next_cursor = if has_more {
            rows.last().map(|item| match item.primary_key_value() {
                Value::Int(n) => n.to_string(),
                Value::Text(s) => s,
                other => format!("{:?}", other),
            })
        } else {
            None
        };

        Ok(CursorPage { items: rows, next_cursor })
    }
}

// ── Internal SQL builders ─────────────────────────────────────────────────────

fn build_select<T: Model>(
    filters: Vec<(String, Vec<Value>)>,
    order: Option<(String, Order)>,
    limit: Option<u64>,
    offset: Option<u64>,
    projection: &str,
    backend: Backend,
) -> (String, Vec<Value>) {
    let (where_clause, params, _) = build_where(filters, 0, backend);
    let mut sql = format!("SELECT {} FROM {}{}", projection, T::table_name(), where_clause);
    if let Some((col, ord)) = order {
        sql.push_str(&format!(" ORDER BY {} {}", col, ord.as_sql()));
    }
    if let Some(n) = limit {
        sql.push_str(&format!(" LIMIT {}", n));
    }
    if let Some(n) = offset {
        sql.push_str(&format!(" OFFSET {}", n));
    }
    (sql, params)
}

/// Build the WHERE clause. `start_idx` offsets PostgreSQL `$N` placeholders.
#[doc(hidden)]
pub fn build_where(
    filters: Vec<(String, Vec<Value>)>,
    start_idx: usize,
    backend: Backend,
) -> (String, Vec<Value>, usize) {
    let mut all_params: Vec<Value> = Vec::new();
    let mut conditions: Vec<String> = Vec::new();
    let mut idx = start_idx;

    for (mut fragment, params) in filters {
        for param in params {
            idx += 1;
            if fragment.contains("__placeholder__") {
                fragment = fragment.replacen("__placeholder__", &placeholder(backend, idx), 1);
            } else {
                // A raw fragment written with a literal `?` (e.g. from
                // `.filter("age >= ?", ...)`) — no-op on backends that
                // already use `?`, rewrites it to `$N` on PostgreSQL.
                fragment = fragment.replacen("?", &placeholder(backend, idx), 1);
            }
            all_params.push(param);
        }
        conditions.push(fragment);
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    (where_clause, all_params, idx)
}
