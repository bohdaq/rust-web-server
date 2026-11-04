//! `/todos`: list, add, look up and finish to-dos.

use rust_web_server::error::AppError;
use rust_web_server::handler::{Json, Path, RouteSpec};
use rust_web_server::openapi::{OpenApiConfig, Operation};
use rust_web_server::prelude::*;
use rust_web_server::response::StatusCodeReasonPhrase;
use rust_web_server::state::AppWithState;
use rust_web_server::{get, post, JsonSchema};
use serde::{Deserialize, Serialize};

use crate::Store;

/// A to-do item.
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct Todo {
    pub id: u64,
    pub title: String,
    pub done: bool,
}

/// The body of `POST /todos`.
#[derive(Deserialize, JsonSchema)]
pub struct NewTodo {
    pub title: String,
}

/// What a failed call answers: problem details (RFC 9457,
/// `application/problem+json`), as every rws error does.
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: u16,
    pub detail: Option<String>,
}

fn not_found(id: u64) -> AppError {
    AppError::NotFound(format!("no to-do {id}"))
}

#[get("/todos")]
pub fn list(store: &Store) -> Json<Vec<Todo>> {
    Json(store.todos.lock().unwrap().values().cloned().collect())
}

/// A body that isn't a `NewTodo` is answered `400` by the `Json` extractor
/// before this runs.
#[post("/todos")]
pub fn create(Json(new): Json<NewTodo>, store: &Store) -> (&'static StatusCodeReasonPhrase, Json<Todo>) {
    let mut todos = store.todos.lock().unwrap();
    let id = todos.keys().next_back().map_or(1, |last| last + 1);
    let todo = Todo { id, title: new.title, done: false };
    todos.insert(id, todo.clone());
    (STATUS_CODE_REASON_PHRASE.n201_created, Json(todo))
}

/// `{id}` that isn't a number is answered `400` by `Path`.
#[get("/todos/{id}")]
pub fn show(Path(id): Path<u64>, store: &Store) -> Result<Json<Todo>, AppError> {
    store.todos.lock().unwrap().get(&id).cloned().map(Json).ok_or_else(|| not_found(id))
}

#[post("/todos/{id}/done")]
pub fn finish(Path(id): Path<u64>, store: &Store) -> Result<Json<Todo>, AppError> {
    let mut todos = store.todos.lock().unwrap();
    let todo = todos.get_mut(&id).ok_or_else(|| not_found(id))?;
    todo.done = true;
    Ok(Json(todo.clone()))
}

/// Add this resource's routes to the app.
pub fn register(app: AppWithState<Store>) -> AppWithState<Store> {
    app.route(list).route(create).route(show).route(finish)
}

/// Describe this resource's routes: the same types the handlers use, the
/// patterns from the handlers' attributes.
pub fn describe(config: OpenApiConfig) -> OpenApiConfig {
    config
        .operation(list::METHOD, list::PATTERN, Operation::new("List to-dos").response::<Vec<Todo>>(200, "Every to-do"))
        .operation(
            create::METHOD,
            create::PATTERN,
            Operation::new("Add a to-do").body::<NewTodo>().response::<Todo>(201, "Created").response::<Problem>(400, "Bad body"),
        )
        .operation(
            show::METHOD,
            show::PATTERN,
            Operation::new("Look up a to-do").path_param::<u64>("id").response::<Todo>(200, "The to-do").response::<Problem>(404, "Not found"),
        )
        .operation(
            finish::METHOD,
            finish::PATTERN,
            Operation::new("Mark a to-do done").path_param::<u64>("id").response::<Todo>(200, "The to-do").response::<Problem>(404, "Not found"),
        )
}
