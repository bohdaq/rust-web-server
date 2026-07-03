//! Async fluent `QueryBuilder<T>` for constructing SQL queries.

use std::marker::PhantomData;

use super::pool::DbPool;
use super::repository::{extract_count, placeholder};
use super::{DbError, Model, ToColumn, Value};

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
