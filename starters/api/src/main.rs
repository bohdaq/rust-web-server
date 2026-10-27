//! A JSON API on rust-web-server: routes with path parameters, request and
//! response bodies as serde types, an OpenAPI 3.1 document from the same
//! types (`/openapi.json`, Swagger UI at `/docs`), and tests that need no
//! socket. Start here; see AGENTS.md.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rust_web_server::openapi::{OpenApiConfig, Operation};
use rust_web_server::prelude::*;
use rust_web_server::response::StatusCodeReasonPhrase;
use rust_web_server::state::AppWithState;
use rust_web_server::JsonSchema;
use serde::{Deserialize, Serialize};

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

/// What a failed call answers.
#[derive(Serialize, JsonSchema)]
pub struct ApiError {
    pub error: String,
}

/// Shared state: cloned per server worker, so the data sits behind an `Arc`.
#[derive(Clone, Default)]
pub struct Store {
    todos: Arc<Mutex<BTreeMap<u64, Todo>>>,
}

fn json<T: Serialize>(status: &'static StatusCodeReasonPhrase, value: &T) -> Response {
    Response::json(status, serde_json::to_vec(value).unwrap_or_default())
}

fn error(status: &'static StatusCodeReasonPhrase, message: &str) -> Response {
    json(status, &ApiError { error: message.to_string() })
}

pub fn app() -> AppWithState<Store> {
    App::with_state(Store::default())
        .get("/todos", |_, _, _, store| {
            let todos: Vec<Todo> = store.todos.lock().unwrap().values().cloned().collect();
            json(STATUS_CODE_REASON_PHRASE.n200_ok, &todos)
        })
        .post("/todos", |request, _, _, store| {
            let Ok(new) = serde_json::from_slice::<NewTodo>(&request.body) else {
                return error(STATUS_CODE_REASON_PHRASE.n400_bad_request, r#"expected {"title": "..."}"#);
            };
            let mut todos = store.todos.lock().unwrap();
            let id = todos.keys().next_back().map_or(1, |last| last + 1);
            let todo = Todo { id, title: new.title, done: false };
            todos.insert(id, todo.clone());
            json(STATUS_CODE_REASON_PHRASE.n201_created, &todo)
        })
        .get("/todos/{id}", |_, params, _, store| {
            let found = params.get("id").and_then(|id| id.parse::<u64>().ok()).and_then(|id| store.todos.lock().unwrap().get(&id).cloned());
            match found {
                Some(todo) => json(STATUS_CODE_REASON_PHRASE.n200_ok, &todo),
                None => error(STATUS_CODE_REASON_PHRASE.n404_not_found, "no such to-do"),
            }
        })
        .post("/todos/{id}/done", |_, params, _, store| {
            let id = params.get("id").and_then(|id| id.parse::<u64>().ok()).unwrap_or(0);
            match store.todos.lock().unwrap().get_mut(&id) {
                Some(todo) => {
                    todo.done = true;
                    json(STATUS_CODE_REASON_PHRASE.n200_ok, todo)
                }
                None => error(STATUS_CODE_REASON_PHRASE.n404_not_found, "no such to-do"),
            }
        })
        .openapi(openapi())
}

/// The API's OpenAPI document: the same types the handlers use.
fn openapi() -> OpenApiConfig {
    OpenApiConfig::new("To-dos", env!("CARGO_PKG_VERSION"))
        .operation("GET", "/todos", Operation::new("List to-dos").response::<Vec<Todo>>(200, "Every to-do"))
        .operation("POST", "/todos", Operation::new("Add a to-do").body::<NewTodo>().response::<Todo>(201, "Created").response::<ApiError>(400, "Bad body"))
        .operation("GET", "/todos/{id}", Operation::new("Look up a to-do").path_param::<u64>("id").response::<Todo>(200, "The to-do").response::<ApiError>(404, "Not found"))
        .operation("POST", "/todos/{id}/done", Operation::new("Mark a to-do done").path_param::<u64>("id").response::<Todo>(200, "The to-do").response::<ApiError>(404, "Not found"))
}

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().expect("server setup");
    Server::run_tls(listener, pool, app()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    #[test]
    fn a_todo_is_added_found_and_finished() {
        let client = TestClient::new(app());
        let created = client.post("/todos").body_json(r#"{"title":"write tests"}"#).send();
        assert_eq!(201, created.status());
        let todo: Todo = serde_json::from_slice(created.body_bytes()).unwrap();
        assert_eq!(("write tests", false), (todo.title.as_str(), todo.done));

        let done = client.post(&format!("/todos/{}/done", todo.id)).send();
        assert_eq!(200, done.status());
        let listed: Vec<Todo> = serde_json::from_slice(client.get("/todos").send().body_bytes()).unwrap();
        assert!(listed[0].done);
    }

    #[test]
    fn errors_are_json() {
        let client = TestClient::new(app());
        assert_eq!(404, client.get("/todos/99").send().status());
        let bad = client.post("/todos").body_json("not json").send();
        assert_eq!(400, bad.status());
        assert!(bad.body_text().contains("error"));
    }

    #[test]
    fn the_api_is_described_in_openapi() {
        let spec = TestClient::new(app()).get("/openapi.json").send();
        assert_eq!(200, spec.status());
        let spec: serde_json::Value = serde_json::from_slice(spec.body_bytes()).unwrap();
        assert_eq!("3.1.0", spec["openapi"]);
        assert!(spec["paths"]["/todos/{id}"]["get"]["responses"]["200"].is_object());
    }
}
