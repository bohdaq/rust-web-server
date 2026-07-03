//! Async fluent `QueryBuilder<T>` for constructing SQL queries.

use std::marker::PhantomData;

use super::pool::DbPool;
use super::repository::{extract_count, placeholder};
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

// ── QueryBuilder ──────────────────────────────────────────────────────────────

/// Fluent async query builder for `SELECT`, `COUNT`, `DELETE`, and `UPDATE`.
///
/// ```rust,no_run
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
            _phantom: PhantomData,
        }
    }

    /// Add a `col = ?` equality filter.
    pub fn where_eq(mut self, col: &str, val: impl ToColumn) -> Self {
        self.filters.push((
            format!("{} = __placeholder__", col),
            vec![val.to_column()],
        ));
        self
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
        let (sql, params) = build_select::<T>(
            self.filters, self.order, self.limit, self.offset, "*",
        );
        let rows = self.pool.query_rows(&sql, &params).await?;
        rows.iter().map(|r| T::from_row(r)).collect()
    }

    /// Execute `SELECT * … LIMIT 1` and return the first result.
    pub async fn fetch_one(self) -> Result<Option<T>, DbError> {
        let (sql, params) = build_select::<T>(
            self.filters, self.order, Some(1), self.offset, "*",
        );
        let rows = self.pool.query_rows(&sql, &params).await?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(T::from_row(&row)?)),
            None => Ok(None),
        }
    }

    /// Execute `SELECT COUNT(*) FROM table WHERE …`.
    pub async fn count(self) -> Result<i64, DbError> {
        let (sql, params) = build_select::<T>(
            self.filters, self.order, self.limit, self.offset, "COUNT(*)",
        );
        let rows = self.pool.query_rows(&sql, &params).await?;
        extract_count(rows)
    }

    /// Execute `DELETE FROM table WHERE …`.
    pub async fn delete(self) -> Result<(), DbError> {
        let (where_clause, params, _) = build_where(self.filters, 0);
        let sql = format!("DELETE FROM {}{}", T::table_name(), where_clause);
        self.pool.execute(&sql, &params).await?;
        Ok(())
    }

    /// Execute `UPDATE table SET col = ? WHERE …`.
    pub async fn update(self, col: &str, val: impl ToColumn) -> Result<(), DbError> {
        let set_ph = placeholder(1);
        let set_val = val.to_column();
        let (where_clause, mut where_params, _) = build_where(self.filters, 1);
        let mut params = vec![set_val];
        params.append(&mut where_params);
        let sql = format!(
            "UPDATE {} SET {} = {}{}",
            T::table_name(), col, set_ph, where_clause,
        );
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
) -> (String, Vec<Value>) {
    let (where_clause, params, _) = build_where(filters, 0);
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
pub(crate) fn build_where(
    filters: Vec<(String, Vec<Value>)>,
    start_idx: usize,
) -> (String, Vec<Value>, usize) {
    let mut all_params: Vec<Value> = Vec::new();
    let mut conditions: Vec<String> = Vec::new();
    let mut idx = start_idx;

    for (mut fragment, params) in filters {
        for param in params {
            idx += 1;
            if fragment.contains("__placeholder__") {
                fragment = fragment.replacen("__placeholder__", &placeholder(idx), 1);
            } else {
                #[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
                {
                    fragment = fragment.replacen("?", &placeholder(idx), 1);
                }
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
