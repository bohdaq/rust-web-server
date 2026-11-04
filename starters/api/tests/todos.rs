//! The API, through `TestClient`: no socket, no server.

use {{crate_name}}::{app, Problem, Todo};
use rust_web_server::test_client::TestClient;

#[test]
fn a_todo_is_added_found_and_finished() {
    let client = TestClient::new(app());
    let created = client.post("/todos").body_json(r#"{"title":"write tests"}"#).send();
    assert_eq!(201, created.status());
    let todo: Todo = serde_json::from_slice(created.body_bytes()).unwrap();
    assert_eq!(("write tests", false), (todo.title.as_str(), todo.done));

    let found: Todo = serde_json::from_slice(client.get(&format!("/todos/{}", todo.id)).send().body_bytes()).unwrap();
    assert_eq!(todo.id, found.id);

    let done = client.post(&format!("/todos/{}/done", todo.id)).send();
    assert_eq!(200, done.status());
    let listed: Vec<Todo> = serde_json::from_slice(client.get("/todos").send().body_bytes()).unwrap();
    assert!(listed[0].done);
}

#[test]
fn errors_are_problem_details() {
    let client = TestClient::new(app());
    let missing = client.get("/todos/99").send();
    assert_eq!(404, missing.status());
    let problem: Problem = serde_json::from_slice(missing.body_bytes()).unwrap();
    assert_eq!((404, Some("no to-do 99")), (problem.status, problem.detail.as_deref()));

    assert_eq!(400, client.post("/todos").body_json("not json").send().status());
    assert_eq!(400, client.get("/todos/abc").send().status(), "the id must be a number");
    assert_eq!(404, client.post("/todos/7/done").send().status());
}

#[test]
fn the_api_is_described_in_openapi() {
    let spec = TestClient::new(app()).get("/openapi.json").send();
    assert_eq!(200, spec.status());
    let spec: serde_json::Value = serde_json::from_slice(spec.body_bytes()).unwrap();
    assert_eq!("3.1.0", spec["openapi"]);
    assert!(spec["paths"]["/todos/{id}"]["get"]["responses"]["200"].is_object());
    assert!(spec["paths"]["/todos"]["post"]["requestBody"].is_object());
}

#[test]
fn no_route_shadows_another() {
    assert!(app().route_conflicts().is_empty());
}
