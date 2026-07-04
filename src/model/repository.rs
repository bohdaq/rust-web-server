//! Async `Repository` trait and `ModelRepository` implementation.

use std::marker::PhantomData;

use super::backend::Backend;
use super::pool::DbPool;
use super::{DbError, Model, Value};

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

// ── ModelRepository ───────────────────────────────────────────────────────────

/// Repository tied to a model type and a pool.
pub struct ModelRepository<'a, T: Model, ID> {
    pub(crate) pool: &'a DbPool,
    _phantom: PhantomData<(T, ID)>,
}

impl<'a, T: Model, ID> ModelRepository<'a, T, ID> {
    pub fn new(pool: &'a DbPool) -> Self {
        ModelRepository { pool, _phantom: PhantomData }
    }
}

// ── impl Repository<T, i64> ───────────────────────────────────────────────────

impl<'a, T: Model> Repository<T, i64> for ModelRepository<'a, T, i64> {
    async fn find_by_id(&self, id: i64) -> Result<Option<T>, DbError> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = {}",
            T::table_name(),
            T::primary_key_name(),
            placeholder(self.pool.backend(), 1),
        );
        let rows = self.pool.query_rows(&sql, &[Value::Int(id)]).await?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(T::from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> Result<Vec<T>, DbError> {
        let sql = format!("SELECT * FROM {}", T::table_name());
        let rows = self.pool.query_rows(&sql, &[]).await?;
        rows.iter().map(|r| T::from_row(r)).collect()
    }

    async fn save(&self, entity: &T) -> Result<T, DbError> {
        let pk_val = entity.primary_key_value();
        let is_new = matches!(&pk_val, Value::Int(n) if *n == 0) || matches!(&pk_val, Value::Null);
        if is_new {
            insert_entity(self.pool, entity).await
        } else {
            update_entity(self.pool, entity).await
        }
    }

    async fn save_all(&self, entities: &[T]) -> Result<Vec<T>, DbError> {
        let mut result = Vec::with_capacity(entities.len());
        for e in entities {
            result.push(self.save(e).await?);
        }
        Ok(result)
    }

    async fn delete_by_id(&self, id: i64) -> Result<(), DbError> {
        let sql = format!(
            "DELETE FROM {} WHERE {} = {}",
            T::table_name(),
            T::primary_key_name(),
            placeholder(self.pool.backend(), 1),
        );
        self.pool.execute(&sql, &[Value::Int(id)]).await?;
        Ok(())
    }

    async fn delete_all_by_id(&self, ids: &[i64]) -> Result<(), DbError> {
        for &id in ids {
            self.delete_by_id(id).await?;
        }
        Ok(())
    }

    async fn count(&self) -> Result<i64, DbError> {
        let sql = format!("SELECT COUNT(*) FROM {}", T::table_name());
        let rows = self.pool.query_rows(&sql, &[]).await?;
        extract_count(rows)
    }

    async fn exists_by_id(&self, id: i64) -> Result<bool, DbError> {
        Ok(self.find_by_id(id).await?.is_some())
    }
}

// ── Placeholder helpers ───────────────────────────────────────────────────────

/// Return the DB-appropriate placeholder for position `pos` (1-indexed) on
/// `backend`. SQLite and MySQL use `?`, PostgreSQL uses `$1`, `$2`, …
pub(crate) fn placeholder(backend: Backend, _pos: usize) -> String {
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
pub(crate) fn placeholders(backend: Backend, count: usize) -> String {
    (1..=count).map(|i| placeholder(backend, i)).collect::<Vec<_>>().join(", ")
}

// ── Insert / Update helpers ───────────────────────────────────────────────────

pub(crate) async fn insert_entity<T: Model>(pool: &DbPool, entity: &T) -> Result<T, DbError> {
    let backend = pool.backend();
    let values = entity.to_values();

    let insert_vals: Vec<(&'static str, Value)> = if T::primary_key_auto_increment() {
        values.into_iter().filter(|(col, _)| *col != T::primary_key_name()).collect()
    } else {
        values
    };

    let cols: Vec<&str> = insert_vals.iter().map(|(c, _)| *c).collect();
    let params: Vec<Value> = insert_vals.into_iter().map(|(_, v)| v).collect();
    let ph = placeholders(backend, cols.len());
    let col_list = cols.join(", ");

    let insert_sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        T::table_name(), col_list, ph
    );

    let new_id = pool.insert_returning_id(&insert_sql, &params, T::primary_key_name()).await?;
    let select_sql = format!(
        "SELECT * FROM {} WHERE {} = {}",
        T::table_name(), T::primary_key_name(), placeholder(backend, 1),
    );
    let rows = pool.query_rows(&select_sql, &[Value::Int(new_id)]).await?;
    rows.into_iter().next()
        .ok_or_else(|| DbError::new("inserted row not found"))
        .and_then(|r| T::from_row(&r))
}

pub(crate) async fn update_entity<T: Model>(pool: &DbPool, entity: &T) -> Result<T, DbError> {
    let backend = pool.backend();
    let values = entity.to_values();
    let set_pairs: Vec<(&str, Value)> = values
        .into_iter()
        .filter(|(col, _)| *col != T::primary_key_name())
        .collect();

    let set_clause: String = set_pairs
        .iter()
        .enumerate()
        .map(|(i, (col, _))| format!("{} = {}", col, placeholder(backend, i + 1)))
        .collect::<Vec<_>>()
        .join(", ");

    let pk_placeholder = placeholder(backend, set_pairs.len() + 1);
    let sql = format!(
        "UPDATE {} SET {} WHERE {} = {}",
        T::table_name(), set_clause, T::primary_key_name(), pk_placeholder,
    );

    let mut params: Vec<Value> = set_pairs.into_iter().map(|(_, v)| v).collect();
    params.push(entity.primary_key_value());
    pool.execute(&sql, &params).await?;

    let pk_id = match entity.primary_key_value() {
        Value::Int(n) => n,
        _ => return Err(DbError::new("primary key must be an integer for UPDATE")),
    };
    let select_sql = format!(
        "SELECT * FROM {} WHERE {} = {}",
        T::table_name(), T::primary_key_name(), placeholder(backend, 1),
    );
    let rows = pool.query_rows(&select_sql, &[Value::Int(pk_id)]).await?;
    rows.into_iter().next()
        .ok_or_else(|| DbError::new("updated row not found"))
        .and_then(|r| T::from_row(&r))
}

pub(crate) fn extract_count(rows: Vec<super::ModelRow>) -> Result<i64, DbError> {
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
