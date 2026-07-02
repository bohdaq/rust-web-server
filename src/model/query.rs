//! Fluent `QueryBuilder<T>` for constructing SQL queries.

use std::marker::PhantomData;

use super::connection::DbConnection;
use super::repository::placeholder;
use super::{DbError, Model, ToColumn, Value};

// ── Order ─────────────────────────────────────────────────────────────────────

/// Sort direction for `order_by`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Asc,
    Desc,
}

impl Order {
    fn as_sql(&self) -> &'static str {
        match self {
            Order::Asc => "ASC",
            Order::Desc => "DESC",
        }
    }
}

// ── QueryBuilder ──────────────────────────────────────────────────────────────

/// Fluent query builder for `SELECT`, `COUNT`, `DELETE`, and `UPDATE`.
pub struct QueryBuilder<'a, T: Model> {
    conn: &'a mut DbConnection,
    /// Each filter is a (sql_fragment, params) pair. The sql_fragment uses
    /// `__placeholder__` (for where_eq) or raw `?` / positional markers.
    filters: Vec<(String, Vec<Value>)>,
    order: Option<(String, Order)>,
    limit: Option<u64>,
    offset: Option<u64>,
    _phantom: PhantomData<T>,
}

impl<'a, T: Model> QueryBuilder<'a, T> {
    pub fn new(conn: &'a mut DbConnection) -> Self {
        QueryBuilder {
            conn,
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
    /// # Example
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

    /// Execute `SELECT * FROM table WHERE … ORDER BY … LIMIT … OFFSET …`.
    pub fn fetch_all(self) -> Result<Vec<T>, DbError> {
        let QueryBuilder { conn, filters, order, limit, offset, .. } = self;
        let (sql, params) = build_select::<T>(filters, order, limit, offset, "*", None);
        let rows = conn.query_rows(&sql, &params)?;
        rows.iter().map(|r| T::from_row(r)).collect()
    }

    /// Execute `SELECT * … LIMIT 1` and return the first result.
    pub fn fetch_one(self) -> Result<Option<T>, DbError> {
        let QueryBuilder { conn, filters, order, limit: _, offset, .. } = self;
        let (sql, params) = build_select::<T>(filters, order, Some(1), offset, "*", None);
        let rows = conn.query_rows(&sql, &params)?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(T::from_row(&row)?)),
            None => Ok(None),
        }
    }

    /// Execute `SELECT COUNT(*) FROM table WHERE …`.
    pub fn count(self) -> Result<i64, DbError> {
        let QueryBuilder { conn, filters, order, limit, offset, .. } = self;
        let (sql, params) = build_select::<T>(filters, order, limit, offset, "COUNT(*)", None);
        let rows = conn.query_rows(&sql, &params)?;
        match rows.into_iter().next() {
            Some(row) => {
                let cols = &row.columns;
                if let Some((_, val)) = cols.first() {
                    match val.clone() {
                        Value::Int(n) => return Ok(n),
                        other => return Err(DbError::new(format!("unexpected count value: {:?}", other))),
                    }
                }
                Err(DbError::new("count returned empty row"))
            }
            None => Ok(0),
        }
    }

    /// Execute `DELETE FROM table WHERE …`.
    pub fn delete(self) -> Result<(), DbError> {
        let QueryBuilder { conn, filters, .. } = self;
        let (where_clause, params, _) = build_where(filters, 0);
        let sql = format!("DELETE FROM {}{}", T::table_name(), where_clause);
        conn.execute(&sql, &params)?;
        Ok(())
    }

    /// Execute `UPDATE table SET col = ? WHERE …`.
    pub fn update(self, col: &str, val: impl ToColumn) -> Result<(), DbError> {
        let QueryBuilder { conn, filters, .. } = self;
        let set_ph = placeholder(1);
        let set_val = val.to_column();
        let (where_clause, mut where_params, _) = build_where(filters, 1);
        let mut params = vec![set_val];
        params.append(&mut where_params);
        let sql = format!(
            "UPDATE {} SET {} = {}{}",
            T::table_name(),
            col,
            set_ph,
            where_clause
        );
        conn.execute(&sql, &params)?;
        Ok(())
    }
}

// ── Internal builders (free functions to avoid borrow issues) ─────────────────

/// Build a SELECT query string and its params.
fn build_select<T: Model>(
    filters: Vec<(String, Vec<Value>)>,
    order: Option<(String, Order)>,
    limit: Option<u64>,
    offset: Option<u64>,
    projection: &str,
    _extra_limit: Option<u64>,
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

/// Build the WHERE clause and collect all params. `start_idx` offsets
/// PostgreSQL `$N` placeholders so they don't collide with SET params.
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
            // Replace `__placeholder__` (from where_eq) with the real placeholder.
            if fragment.contains("__placeholder__") {
                fragment = fragment.replacen("__placeholder__", &placeholder(idx), 1);
            } else {
                // For raw filters using `?` syntax with Postgres, replace `?` with `$N`.
                #[cfg(feature = "model-postgres")]
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
