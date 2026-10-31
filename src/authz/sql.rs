//! [`SqlStore`]: relationship tuples in a database (SQLite, Postgres or
//! MySQL through the model layer), with a short read cache.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::{AuthzError, ObjectRef, Subject, Tuple, TupleFilter, TupleStore};
use crate::model::backend::Backend;
use crate::model::{DbConfig, DbPool, Value};

/// How long a read is reused (this process's own writes clear it at once).
pub const READ_CACHE: Duration = Duration::from_secs(1);

const TABLE: &str = "rws_authz_tuples";

/// The runtime the store's queries run on: the pool's connections belong to
/// it, whatever runtime (or none) the caller is on.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("rws-authz-sql")
            .enable_all()
            .build()
            .expect("the authorization store runtime")
    })
}

/// Run `f` on the store's runtime and wait for it (from any thread that
/// isn't one of its workers).
fn run<T: Send + 'static>(f: impl std::future::Future<Output = T> + Send + 'static) -> Result<T, AuthzError> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    runtime().spawn(async move {
        let _ = tx.send(f.await);
    });
    rx.recv().map_err(|_| AuthzError::Store("the query was dropped".to_string()))
}

fn store_error(e: impl std::fmt::Display) -> AuthzError {
    AuthzError::Store(e.to_string())
}

/// Tuples in the `rws_authz_tuples` table (created if missing): one row per
/// tuple, keyed by the SHA-256 of its text so the key fits every database's
/// index limit, indexed by object and relation for checks.
pub struct SqlStore {
    pool: DbPool,
    cache: Mutex<HashMap<(ObjectRef, String), (Instant, Vec<Subject>)>>,
}

impl SqlStore {
    /// Connect with `config` (its pool lives on the store's own runtime) and
    /// create the table if it's missing.
    pub fn open(config: DbConfig) -> Result<SqlStore, AuthzError> {
        let pool = run(async move { DbPool::new(config).await })?.map_err(store_error)?;
        SqlStore::new(pool)
    }

    /// [`open`](Self::open) with `RWS_DB_*` from the environment.
    pub fn from_env() -> Result<SqlStore, AuthzError> {
        SqlStore::open(DbConfig::from_env().map_err(store_error)?)
    }

    /// Use `pool` and create the table if it's missing. Queries run on the
    /// store's runtime: a pool made on another runtime must outlive it.
    pub fn new(pool: DbPool) -> Result<SqlStore, AuthzError> {
        let store = SqlStore { pool, cache: Mutex::default() };
        let text = if matches!(store.pool.backend(), b if is_mysql(b)) { "VARCHAR(256) CHARACTER SET utf8mb4" } else { "VARCHAR(256)" };
        let create = format!(
            "CREATE TABLE IF NOT EXISTS {TABLE} (tuple_key CHAR(64) NOT NULL PRIMARY KEY, object_type {text} NOT NULL, \
             object_id {text} NOT NULL, relation {text} NOT NULL, subject_type {text} NOT NULL, subject_id {text} NOT NULL, \
             subject_relation {text} NOT NULL)"
        );
        store.execute(create, Vec::new())?;
        let index = if is_mysql(store.pool.backend()) {
            // MySQL has no IF NOT EXISTS for indexes: a duplicate is fine.
            format!("CREATE INDEX {TABLE}_object ON {TABLE} (object_type(64), object_id(128), relation(64))")
        } else {
            format!("CREATE INDEX IF NOT EXISTS {TABLE}_object ON {TABLE} (object_type, object_id, relation)")
        };
        if let Err(e) = store.execute(index, Vec::new()) {
            if !is_mysql(store.pool.backend()) {
                return Err(e);
            }
        }
        Ok(store)
    }

    fn execute(&self, sql: String, params: Vec<Value>) -> Result<u64, AuthzError> {
        let pool = self.pool.clone();
        run(async move { pool.execute(&sql, &params).await })?.map_err(store_error)
    }

    fn query(&self, sql: String, params: Vec<Value>) -> Result<Vec<crate::model::ModelRow>, AuthzError> {
        let pool = self.pool.clone();
        run(async move { pool.query_rows(&sql, &params).await })?.map_err(store_error)
    }

    /// The `n`th placeholder (from 1) in this database's syntax.
    fn placeholder(&self, n: usize) -> String {
        if is_postgres(self.pool.backend()) { format!("${n}") } else { "?".to_string() }
    }

    fn forget(&self) {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

fn is_mysql(backend: Backend) -> bool {
    #[cfg(feature = "model-mysql")]
    if matches!(backend, Backend::MySql) {
        return true;
    }
    let _ = backend;
    false
}

fn is_postgres(backend: Backend) -> bool {
    #[cfg(feature = "model-postgres")]
    if matches!(backend, Backend::Postgres) {
        return true;
    }
    let _ = backend;
    false
}

fn key(tuple: &Tuple) -> String {
    crate::core::sha256::sha256(tuple.to_string().as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn subject_from(row: &crate::model::ModelRow) -> Result<Subject, AuthzError> {
    let kind: String = row.get("subject_type").map_err(store_error)?;
    let id: String = row.get("subject_id").map_err(store_error)?;
    let relation: String = row.get("subject_relation").map_err(store_error)?;
    Ok(Subject { object: ObjectRef { kind, id }, relation: (!relation.is_empty()).then_some(relation) })
}

impl TupleStore for SqlStore {
    fn write(&self, tuple: &Tuple) -> Result<bool, AuthzError> {
        let backend = self.pool.backend();
        let values: Vec<String> = (1..=7).map(|n| self.placeholder(n)).collect();
        let columns = "tuple_key, object_type, object_id, relation, subject_type, subject_id, subject_relation";
        let sql = if is_mysql(backend) {
            format!("INSERT IGNORE INTO {TABLE} ({columns}) VALUES ({})", values.join(", "))
        } else if is_postgres(backend) {
            format!("INSERT INTO {TABLE} ({columns}) VALUES ({}) ON CONFLICT DO NOTHING", values.join(", "))
        } else {
            format!("INSERT OR IGNORE INTO {TABLE} ({columns}) VALUES ({})", values.join(", "))
        };
        let params = vec![
            Value::Text(key(tuple)),
            Value::Text(tuple.object.kind.clone()),
            Value::Text(tuple.object.id.clone()),
            Value::Text(tuple.relation.clone()),
            Value::Text(tuple.subject.object.kind.clone()),
            Value::Text(tuple.subject.object.id.clone()),
            Value::Text(tuple.subject.relation.clone().unwrap_or_default()),
        ];
        let inserted = self.execute(sql, params)? > 0;
        self.forget();
        Ok(inserted)
    }

    fn delete(&self, tuple: &Tuple) -> Result<bool, AuthzError> {
        let sql = format!("DELETE FROM {TABLE} WHERE tuple_key = {}", self.placeholder(1));
        let removed = self.execute(sql, vec![Value::Text(key(tuple))])? > 0;
        self.forget();
        Ok(removed)
    }

    fn read(&self, object: &ObjectRef, relation: &str) -> Result<Vec<Subject>, AuthzError> {
        let cache_key = (object.clone(), relation.to_string());
        if let Some((at, subjects)) = self.cache.lock().unwrap_or_else(|e| e.into_inner()).get(&cache_key) {
            if at.elapsed() < READ_CACHE {
                return Ok(subjects.clone());
            }
        }
        let sql = format!(
            "SELECT subject_type, subject_id, subject_relation FROM {TABLE} WHERE object_type = {} AND object_id = {} AND relation = {}",
            self.placeholder(1),
            self.placeholder(2),
            self.placeholder(3)
        );
        let rows = self.query(sql, vec![Value::Text(object.kind.clone()), Value::Text(object.id.clone()), Value::Text(relation.to_string())])?;
        let subjects = rows.iter().map(subject_from).collect::<Result<Vec<_>, _>>()?;
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.len() > 10_000 {
            cache.clear();
        }
        cache.insert(cache_key, (Instant::now(), subjects.clone()));
        Ok(subjects)
    }

    fn list(&self, filter: &TupleFilter, limit: usize) -> Result<Vec<Tuple>, AuthzError> {
        let mut conditions = Vec::new();
        let mut params = Vec::new();
        let mut add = |column: &str, value: String, conditions: &mut Vec<String>| {
            params.push(Value::Text(value));
            conditions.push(format!("{column} = {}", self.placeholder(params.len())));
        };
        if let Some(object) = &filter.object {
            add("object_type", object.kind.clone(), &mut conditions);
            add("object_id", object.id.clone(), &mut conditions);
        }
        if let Some(kind) = &filter.object_kind {
            add("object_type", kind.clone(), &mut conditions);
        }
        if let Some(relation) = &filter.relation {
            add("relation", relation.clone(), &mut conditions);
        }
        if let Some(subject) = &filter.subject {
            add("subject_type", subject.object.kind.clone(), &mut conditions);
            add("subject_id", subject.object.id.clone(), &mut conditions);
            add("subject_relation", subject.relation.clone().unwrap_or_default(), &mut conditions);
        }
        let filter_sql = if conditions.is_empty() { String::new() } else { format!(" WHERE {}", conditions.join(" AND ")) };
        let sql = format!(
            "SELECT object_type, object_id, relation, subject_type, subject_id, subject_relation FROM {TABLE}{filter_sql} \
             ORDER BY object_type, object_id, relation, subject_type, subject_id, subject_relation LIMIT {}",
            limit.min(100_000)
        );
        let rows = self.query(sql, params)?;
        rows.iter()
            .map(|row| {
                Ok(Tuple {
                    object: ObjectRef { kind: row.get("object_type").map_err(store_error)?, id: row.get("object_id").map_err(store_error)? },
                    relation: row.get("relation").map_err(store_error)?,
                    subject: subject_from(row)?,
                })
            })
            .collect()
    }
}

/// The config file's SQL store (`store = "sql"`, `RWS_DB_*`), opened once
/// per process.
pub(crate) fn config_store() -> Result<Arc<dyn TupleStore>, String> {
    static STORE: OnceLock<Result<Arc<SqlStore>, String>> = OnceLock::new();
    STORE
        .get_or_init(|| SqlStore::from_env().map(Arc::new).map_err(|e| e.to_string()))
        .clone()
        .map(|store| store as Arc<dyn TupleStore>)
}

#[cfg(test)]
mod tests;
