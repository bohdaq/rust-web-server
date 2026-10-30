//! A full-stack app on rust-web-server: a JSON API under `/api`, the lit
//! frontend (built into `frontend/dist`) served as static files, and
//! PostgreSQL when `PGHOST` is set — an in-memory store otherwise, so
//! `cargo run` and `cargo test` need no database. See AGENTS.md.

use std::sync::{Arc, Mutex};

use rust_web_server::async_state::AsyncAppWithState;
use rust_web_server::model::{DbPool, Value};
use rust_web_server::prelude::*;
use rust_web_server::response::StatusCodeReasonPhrase;
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

/// A note.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub text: String,
}

/// The body of `POST /api/notes`.
#[derive(Deserialize)]
pub struct NewNote {
    pub text: String,
}

/// Where notes live. Cloned per server worker, so the memory store sits
/// behind an `Arc`; `DbPool` is already a cheap handle.
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

    pub async fn list(&self) -> Result<Vec<Note>, String> {
        match self {
            Store::Memory(notes) => Ok(notes.lock().unwrap().clone()),
            Store::Postgres(pool) => {
                let rows = pool.query_rows("SELECT id, text FROM notes ORDER BY id", &[]).await.map_err(|e| e.to_string())?;
                rows.iter()
                    .map(|row| Ok(Note { id: row.get("id").map_err(|e| e.to_string())?, text: row.get("text").map_err(|e| e.to_string())? }))
                    .collect()
            }
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
            Store::Postgres(pool) => {
                let rows = pool.query_rows("INSERT INTO notes (text) VALUES ($1) RETURNING id", &[Value::Text(text.clone())]).await.map_err(|e| e.to_string())?;
                let id = rows.first().ok_or("no id returned")?.get("id").map_err(|e| e.to_string())?;
                Ok(Note { id, text })
            }
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

fn json<T: Serialize>(status: &'static StatusCodeReasonPhrase, value: &T) -> Response {
    Response::json(status, serde_json::to_vec(value).unwrap_or_default())
}

fn error(status: &'static StatusCodeReasonPhrase, message: &str) -> Response {
    json(status, &serde_json::json!({ "error": message }))
}

pub fn app(store: Store) -> AsyncAppWithState<Store> {
    App::with_async_state(store)
        .get("/healthz", |_, _, _, store| async move {
            if store.ready().await {
                json(STATUS_CODE_REASON_PHRASE.n200_ok, &serde_json::json!({ "status": "ok" }))
            } else {
                error(STATUS_CODE_REASON_PHRASE.n503_service_unavailable, "database unavailable")
            }
        })
        .get("/api/notes", |_, _, _, store| async move {
            match store.list().await {
                Ok(notes) => json(STATUS_CODE_REASON_PHRASE.n200_ok, &notes),
                Err(e) => error(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, &e),
            }
        })
        .post("/api/notes", |request, _, _, store| async move {
            let text = match serde_json::from_slice::<NewNote>(&request.body) {
                Ok(new) if !new.text.trim().is_empty() => new.text,
                _ => return error(STATUS_CODE_REASON_PHRASE.n400_bad_request, r#"expected {"text": "..."}"#),
            };
            match store.add(text).await {
                Ok(note) => json(STATUS_CODE_REASON_PHRASE.n201_created, &note),
                Err(e) => error(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, &e),
            }
        })
        .delete("/api/notes/{id}", |_, params, _, store| async move {
            let Some(id) = params.get("id").and_then(|id| id.parse::<i64>().ok()) else {
                return error(STATUS_CODE_REASON_PHRASE.n400_bad_request, "id must be a number");
            };
            match store.delete(id).await {
                Ok(true) => json(STATUS_CODE_REASON_PHRASE.n200_ok, &serde_json::json!({ "deleted": id })),
                Ok(false) => error(STATUS_CODE_REASON_PHRASE.n404_not_found, "no such note"),
                Err(e) => error(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, &e),
            }
        })
}

#[tokio::main]
async fn main() {
    // Resolve the migrations before moving into the static directory: rws
    // serves files from its working directory, so it must be the built
    // frontend, never the source tree.
    let migrations = std::fs::canonicalize("migrations").expect("migrations directory");
    let store = Store::connect(&migrations.to_string_lossy()).await.expect("store");
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "frontend/dist".into());
    std::env::set_current_dir(&static_dir).unwrap_or_else(|e| panic!("STATIC_DIR {static_dir}: {e} (build the frontend first: cd frontend && npm ci && npm run build)"));

    let (listener, pool) = Server::setup().expect("server setup");
    Server::run_tls(listener, pool, app(store)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    #[test]
    fn notes_are_added_listed_and_deleted() {
        let client = TestClient::new(app(Store::memory()));
        let created = client.post("/api/notes").body_json(r#"{"text":"ship it"}"#).send();
        assert_eq!(201, created.status());
        let note: Note = serde_json::from_slice(created.body_bytes()).unwrap();
        assert_eq!("ship it", note.text);

        let listed: Vec<Note> = serde_json::from_slice(client.get("/api/notes").send().body_bytes()).unwrap();
        assert_eq!(1, listed.len());

        assert_eq!(200, client.delete(&format!("/api/notes/{}", note.id)).send().status());
        assert_eq!(404, client.delete(&format!("/api/notes/{}", note.id)).send().status());
    }

    #[test]
    fn bad_input_is_a_json_error() {
        let client = TestClient::new(app(Store::memory()));
        let bad = client.post("/api/notes").body_json(r#"{"text":"  "}"#).send();
        assert_eq!(400, bad.status());
        assert!(bad.body_text().contains("error"));
        assert_eq!(400, client.delete("/api/notes/x").send().status());
    }

    #[test]
    fn health_answers() {
        let client = TestClient::new(app(Store::memory()));
        assert_eq!(200, client.get("/healthz").send().status());
    }

    /// Against a real PostgreSQL when `PGHOST` is set (CI runs one).
    #[tokio::test]
    async fn postgres_round_trip() {
        if std::env::var("PGHOST").is_err() {
            return;
        }
        let store = Store::connect("migrations").await.expect("connect");
        let note = store.add("from postgres".into()).await.unwrap();
        assert!(store.list().await.unwrap().iter().any(|n| n.id == note.id));
        assert!(store.delete(note.id).await.unwrap());
        assert!(store.ready().await);
    }
}
