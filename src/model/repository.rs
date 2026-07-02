//! Repository trait and `ModelRepository` implementation.

use std::marker::PhantomData;

use super::connection::DbConnection;
use super::{DbError, Model, Value};

// ── Repository trait ──────────────────────────────────────────────────────────

/// CRUD operations for a model type.
pub trait Repository<T: Model, ID> {
    fn find_by_id(&mut self, id: ID) -> Result<Option<T>, DbError>;
    fn find_all(&mut self) -> Result<Vec<T>, DbError>;
    fn save(&mut self, entity: &T) -> Result<T, DbError>;
    fn save_all(&mut self, entities: &[T]) -> Result<Vec<T>, DbError>;
    fn delete_by_id(&mut self, id: ID) -> Result<(), DbError>;
    fn delete_all_by_id(&mut self, ids: &[ID]) -> Result<(), DbError>;
    fn count(&mut self) -> Result<i64, DbError>;
    fn exists_by_id(&mut self, id: ID) -> Result<bool, DbError>;
}

// ── ModelRepository ───────────────────────────────────────────────────────────

/// Repository tied to a specific model type and a connection.
pub struct ModelRepository<'a, T: Model, ID> {
    pub(crate) conn: &'a mut DbConnection,
    _phantom: PhantomData<(T, ID)>,
}

impl<'a, T: Model, ID> ModelRepository<'a, T, ID> {
    pub fn new(conn: &'a mut DbConnection) -> Self {
        ModelRepository {
            conn,
            _phantom: PhantomData,
        }
    }
}

// ── impl Repository<T, i64> ───────────────────────────────────────────────────

impl<'a, T: Model> Repository<T, i64> for ModelRepository<'a, T, i64> {
    fn find_by_id(&mut self, id: i64) -> Result<Option<T>, DbError> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = {}",
            T::table_name(),
            T::primary_key_name(),
            placeholder(1)
        );
        let rows = self.conn.query_rows(&sql, &[Value::Int(id)])?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(T::from_row(&row)?)),
            None => Ok(None),
        }
    }

    fn find_all(&mut self) -> Result<Vec<T>, DbError> {
        let sql = format!("SELECT * FROM {}", T::table_name());
        let rows = self.conn.query_rows(&sql, &[])?;
        rows.iter().map(|r| T::from_row(r)).collect()
    }

    fn save(&mut self, entity: &T) -> Result<T, DbError> {
        let pk_val = entity.primary_key_value();
        let is_new = match &pk_val {
            Value::Int(n) => *n == 0,
            Value::Null => true,
            _ => false,
        };

        if is_new || T::primary_key_auto_increment() && is_new {
            insert_entity(self.conn, entity)
        } else {
            update_entity(self.conn, entity)
        }
    }

    fn save_all(&mut self, entities: &[T]) -> Result<Vec<T>, DbError> {
        let mut result = Vec::with_capacity(entities.len());
        for e in entities {
            result.push(self.save(e)?);
        }
        Ok(result)
    }

    fn delete_by_id(&mut self, id: i64) -> Result<(), DbError> {
        let sql = format!(
            "DELETE FROM {} WHERE {} = {}",
            T::table_name(),
            T::primary_key_name(),
            placeholder(1)
        );
        self.conn.execute(&sql, &[Value::Int(id)])?;
        Ok(())
    }

    fn delete_all_by_id(&mut self, ids: &[i64]) -> Result<(), DbError> {
        for &id in ids {
            self.delete_by_id(id)?;
        }
        Ok(())
    }

    fn count(&mut self) -> Result<i64, DbError> {
        let sql = format!("SELECT COUNT(*) FROM {}", T::table_name());
        let rows = self.conn.query_rows(&sql, &[])?;
        match rows.into_iter().next() {
            Some(row) => {
                // The column might be named "COUNT(*)" or "count(*)"
                let cols = &row.columns;
                if let Some((_, val)) = cols.first() {
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

    fn exists_by_id(&mut self, id: i64) -> Result<bool, DbError> {
        Ok(self.find_by_id(id)?.is_some())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return the DB-appropriate placeholder for position `pos` (1-indexed).
/// SQLite and MySQL use `?`, PostgreSQL uses `$1`, `$2`, …
pub(crate) fn placeholder(pos: usize) -> String {
    #[cfg(feature = "model-postgres")]
    {
        return format!("${}", pos);
    }
    #[allow(unreachable_code)]
    "?".to_string()
}

/// Build a comma-separated list of placeholders: `?, ?, ?` or `$1, $2, $3`.
pub(crate) fn placeholders(count: usize) -> String {
    (1..=count)
        .map(|i| placeholder(i))
        .collect::<Vec<_>>()
        .join(", ")
}

/// INSERT a new entity and return the persisted entity with PK filled in.
pub(crate) fn insert_entity<T: Model>(conn: &mut DbConnection, entity: &T) -> Result<T, DbError> {
    let values = entity.to_values();

    // For auto-increment PKs, exclude the PK column from the INSERT list.
    let insert_vals: Vec<(&'static str, Value)> = if T::primary_key_auto_increment() {
        values
            .into_iter()
            .filter(|(col, _)| *col != T::primary_key_name())
            .collect()
    } else {
        values
    };

    let cols: Vec<&str> = insert_vals.iter().map(|(c, _)| *c).collect();
    let params: Vec<Value> = insert_vals.into_iter().map(|(_, v)| v).collect();
    let ph = (1..=cols.len())
        .map(|i| placeholder(i))
        .collect::<Vec<_>>()
        .join(", ");
    let col_list = cols.join(", ");

    // PostgreSQL supports RETURNING; SQLite and MySQL do not (we query after).
    #[cfg(all(feature = "model-postgres", not(feature = "model-sqlite")))]
    {
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({}) RETURNING {}",
            T::table_name(),
            col_list,
            ph,
            T::primary_key_name()
        );
        let rows = conn.query_rows(&sql, &params)?;
        let pk_row = rows.into_iter().next().ok_or_else(|| DbError::new("INSERT RETURNING returned no row"))?;
        let new_id: i64 = pk_row.get(T::primary_key_name())?;
        let select_sql = format!(
            "SELECT * FROM {} WHERE {} = {}",
            T::table_name(),
            T::primary_key_name(),
            placeholder(1)
        );
        let sel_rows = conn.query_rows(&select_sql, &[Value::Int(new_id)])?;
        let row = sel_rows.into_iter().next().ok_or_else(|| DbError::new("inserted row not found"))?;
        return T::from_row(&row);
    }

    #[allow(unreachable_code)]
    {
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            T::table_name(),
            col_list,
            ph
        );
        conn.execute(&sql, &params)?;

        // Retrieve the generated PK.
        let new_id = get_last_insert_id(conn)?;

        let select_sql = format!(
            "SELECT * FROM {} WHERE {} = {}",
            T::table_name(),
            T::primary_key_name(),
            placeholder(1)
        );
        let rows = conn.query_rows(&select_sql, &[Value::Int(new_id)])?;
        let row = rows.into_iter().next().ok_or_else(|| DbError::new("inserted row not found"))?;
        T::from_row(&row)
    }
}

/// Get the last inserted row ID from the appropriate backend.
pub(crate) fn get_last_insert_id(conn: &mut DbConnection) -> Result<i64, DbError> {
    #[cfg(feature = "model-sqlite")]
    {
        return Ok(conn.last_insert_rowid());
    }

    #[cfg(feature = "model-mysql")]
    {
        return Ok(conn.last_insert_id() as i64);
    }

    #[allow(unreachable_code)]
    Err(DbError::new("last insert id not supported for this backend"))
}

/// UPDATE an existing entity.
pub(crate) fn update_entity<T: Model>(conn: &mut DbConnection, entity: &T) -> Result<T, DbError> {
    let values = entity.to_values();

    // Build SET clause excluding the PK column.
    let set_pairs: Vec<(&str, Value)> = values
        .into_iter()
        .filter(|(col, _)| *col != T::primary_key_name())
        .collect();

    let set_clause: String = set_pairs
        .iter()
        .enumerate()
        .map(|(i, (col, _))| format!("{} = {}", col, placeholder(i + 1)))
        .collect::<Vec<_>>()
        .join(", ");

    let pk_placeholder = placeholder(set_pairs.len() + 1);
    let sql = format!(
        "UPDATE {} SET {} WHERE {} = {}",
        T::table_name(),
        set_clause,
        T::primary_key_name(),
        pk_placeholder
    );

    let mut params: Vec<Value> = set_pairs.into_iter().map(|(_, v)| v).collect();
    params.push(entity.primary_key_value());

    conn.execute(&sql, &params)?;

    // Re-fetch to return the updated entity.
    let pk_val = entity.primary_key_value();
    let pk_id = match &pk_val {
        Value::Int(n) => *n,
        _ => return Err(DbError::new("primary key must be an integer for UPDATE")),
    };

    let select_sql = format!(
        "SELECT * FROM {} WHERE {} = {}",
        T::table_name(),
        T::primary_key_name(),
        placeholder(1)
    );
    let rows = conn.query_rows(&select_sql, &[Value::Int(pk_id)])?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| DbError::new("updated row not found"))?;
    T::from_row(&row)
}
