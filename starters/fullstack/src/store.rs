//! Where notes live: PostgreSQL when `PGHOST` is set, in memory otherwise,
//! so `cargo run` and `cargo test` need no database.

use std::sync::{Arc, Mutex};

use rust_web_server::model::{DbPool, Order, Repository, Value};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::models::Note;

/// Cloned per server worker, so the memory store sits behind an `Arc`;
/// `DbPool` is already a cheap handle.
#[derive(Clone)]
pub enum Store {
    Memory(Arc<Mutex<Vec<Note>>>),
    Postgres(DbPool),
}

impl Store {
    pub fn memory() -> Store {
        Store::Memory(Arc::new(Mutex::new(Vec::new())))
    }

    /// PostgreSQL from the standard `PG*` variables (`PGHOST`, `PGPORT`,
    /// `PGUSER`, `PGPASSWORD`, `PGDATABASE`, `PGSSLMODE`, `PGSSLROOTCERT`),
    /// with the migrations in `migrations_dir` applied; in memory without
    /// `PGHOST`.
    pub async fn connect(migrations_dir: &str) -> Result<Store, String> {
        if std::env::var("PGHOST").is_err() {
            println!("PGHOST is not set: notes are kept in memory.");
            return Ok(Store::memory());
        }
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(PgConnectOptions::new())
            .await
            .map_err(|e| format!("connect to PostgreSQL: {e}"))?;
        let pool = DbPool::Postgres(pool);
        pool.migrate(migrations_dir).await.map_err(|e| format!("migrations: {e}"))?;
        Ok(Store::Postgres(pool))
    }

    /// The database, when there is one (for the dev MCP tools).
    pub fn pool(&self) -> Option<&DbPool> {
        match self {
            Store::Memory(_) => None,
            Store::Postgres(pool) => Some(pool),
        }
    }

    pub async fn list(&self) -> Result<Vec<Note>, String> {
        match self {
            Store::Memory(notes) => Ok(notes.lock().unwrap().clone()),
            Store::Postgres(pool) => Note::query(pool).order_by("id", Order::Asc).fetch_all().await.map_err(|e| e.to_string()),
        }
    }

    pub async fn add(&self, text: String) -> Result<Note, String> {
        match self {
            Store::Memory(notes) => {
                let mut notes = notes.lock().unwrap();
                let id = notes.last().map_or(1, |last| last.id + 1);
                let note = Note { id, text };
                notes.push(note.clone());
                Ok(note)
            }
            Store::Postgres(pool) => Note::repository(pool).save(&Note { id: 0, text }).await.map_err(|e| e.to_string()),
        }
    }

    pub async fn delete(&self, id: i64) -> Result<bool, String> {
        match self {
            Store::Memory(notes) => {
                let mut notes = notes.lock().unwrap();
                let before = notes.len();
                notes.retain(|note| note.id != id);
                Ok(notes.len() != before)
            }
            Store::Postgres(pool) => {
                let deleted = pool.execute("DELETE FROM notes WHERE id = $1", &[Value::Int(id)]).await.map_err(|e| e.to_string())?;
                Ok(deleted > 0)
            }
        }
    }

    /// Whether the store answers: the readiness probe.
    pub async fn ready(&self) -> bool {
        match self {
            Store::Memory(_) => true,
            Store::Postgres(pool) => pool.query_rows("SELECT 1", &[]).await.is_ok(),
        }
    }
}
