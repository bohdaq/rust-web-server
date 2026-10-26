//! A JSON REST API over shared state: list, create, read and delete todos,
//! with typed errors.
//!
//! ```sh
//! cargo run --example rest_api --features serde
//! curl -X POST localhost:7878/todos -d '{"title":"write docs"}'
//! curl localhost:7878/todos
//! ```

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rust_web_server::error::{AppError, IntoResponse};
use rust_web_server::prelude::*;
use rust_web_server::state::AppWithState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
pub struct Todo {
    id: u64,
    title: String,
    done: bool,
}

#[derive(Deserialize)]
struct NewTodo {
    title: String,
}

/// Cloned per server worker; the clones share one map.
#[derive(Clone, Default)]
pub struct Todos {
    items: Arc<Mutex<BTreeMap<u64, Todo>>>,
}

fn json<T: Serialize>(status: &'static rust_web_server::response::StatusCodeReasonPhrase, value: &T) -> Response {
    Response::json(status, serde_json::to_vec(value).unwrap_or_default())
}

fn id_param(params: &PathParams) -> Result<u64, AppError> {
    params.get("id").and_then(|id| id.parse().ok()).ok_or_else(|| AppError::BadRequest("id must be a number".into()))
}

pub fn app() -> AppWithState<Todos> {
    App::with_state(Todos::default())
        .get("/todos", |_, _, _, todos| {
            let items: Vec<Todo> = todos.items.lock().unwrap().values().cloned().collect();
            json(STATUS_CODE_REASON_PHRASE.n200_ok, &items)
        })
        .post("/todos", |request, _, _, todos| {
            let Ok(new) = serde_json::from_slice::<NewTodo>(&request.body) else {
                return AppError::BadRequest("expected {\"title\": \"...\"}".into()).into_response();
            };
            if new.title.trim().is_empty() {
                return AppError::UnprocessableEntity("title is empty".into()).into_response();
            }
            let mut items = todos.items.lock().unwrap();
            let id = items.keys().next_back().map_or(1, |last| last + 1);
            let todo = Todo { id, title: new.title, done: false };
            items.insert(id, todo.clone());
            json(STATUS_CODE_REASON_PHRASE.n201_created, &todo)
        })
        .get("/todos/:id", |_, params, _, todos| {
            match id_param(params).and_then(|id| todos.items.lock().unwrap().get(&id).cloned().ok_or(AppError::NotFound(format!("no todo {id}")))) {
                Ok(todo) => json(STATUS_CODE_REASON_PHRASE.n200_ok, &todo),
                Err(e) => e.into_response(),
            }
        })
        .delete("/todos/:id", |_, params, _, todos| {
            match id_param(params).and_then(|id| todos.items.lock().unwrap().remove(&id).ok_or(AppError::NotFound(format!("no todo {id}")))) {
                Ok(_) => Response::get_response(STATUS_CODE_REASON_PHRASE.n204_no_content, None, None),
                Err(e) => e.into_response(),
            }
        })
}

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, app()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::test_client::TestClient;

    #[test]
    fn crud() {
        let client = TestClient::new(app());
        let created = client.post("/todos").body_text(r#"{"title":"write docs"}"#).send();
        assert_eq!(201, created.status());
        assert!(created.body_text().contains(r#""id":1"#), "{}", created.body_text());
        assert!(client.get("/todos").send().body_text().contains("write docs"));
        assert_eq!(200, client.get("/todos/1").send().status());
        assert_eq!(204, client.delete("/todos/1").send().status());
        assert_eq!(404, client.get("/todos/1").send().status());
    }

    #[test]
    fn bad_input_is_a_typed_error() {
        let client = TestClient::new(app());
        assert_eq!(400, client.post("/todos").body_text("not json").send().status());
        assert_eq!(422, client.post("/todos").body_text(r#"{"title":" "}"#).send().status());
        assert_eq!(400, client.get("/todos/abc").send().status());
    }
}
