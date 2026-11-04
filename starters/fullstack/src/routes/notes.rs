//! `/api/notes`: list, add and delete notes.

use rust_web_server::async_state::AsyncAppWithState;
use rust_web_server::error::AppError;
use rust_web_server::handler::{Json, Path, State};
use rust_web_server::response::{StatusCodeReasonPhrase, STATUS_CODE_REASON_PHRASE};
use rust_web_server::{delete, get, post};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::Note;
use crate::store::Store;

/// The body of `POST /api/notes`.
#[derive(Deserialize)]
pub struct NewNote {
    pub text: String,
}

#[get("/api/notes")]
pub async fn list(State(store): State<Store>) -> Result<Json<Vec<Note>>, AppError> {
    store.list().await.map(Json).map_err(AppError::Internal)
}

/// A body that isn't a `NewNote` is answered `400` by `Json` before this
/// runs; blank text here.
#[post("/api/notes")]
pub async fn create(State(store): State<Store>, Json(new): Json<NewNote>) -> Result<(&'static StatusCodeReasonPhrase, Json<Note>), AppError> {
    if new.text.trim().is_empty() {
        return Err(AppError::BadRequest("the note's text is empty".into()));
    }
    let note = store.add(new.text).await.map_err(AppError::Internal)?;
    Ok((STATUS_CODE_REASON_PHRASE.n201_created, Json(note)))
}

/// `{id}` that isn't a number is answered `400` by `Path`.
#[delete("/api/notes/{id}")]
pub async fn remove(Path(id): Path<i64>, State(store): State<Store>) -> Result<Json<Value>, AppError> {
    match store.delete(id).await.map_err(AppError::Internal)? {
        true => Ok(Json(json!({ "deleted": id }))),
        false => Err(AppError::NotFound(format!("no note {id}"))),
    }
}

pub fn register(app: AsyncAppWithState<Store>) -> AsyncAppWithState<Store> {
    app.route(list).route(create).route(remove)
}
