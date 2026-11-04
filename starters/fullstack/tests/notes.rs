//! The API, through `TestClient` (no socket), and the store against a
//! real PostgreSQL when `PGHOST` is set (CI runs one).

use {{crate_name}}::{app, Note, Store};
use rust_web_server::test_client::TestClient;
use serde_json::Value;

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
fn bad_input_is_a_problem_details_error() {
    let client = TestClient::new(app(Store::memory()));
    let blank = client.post("/api/notes").body_json(r#"{"text":"  "}"#).send();
    assert_eq!(400, blank.status());
    let problem: Value = serde_json::from_slice(blank.body_bytes()).unwrap();
    assert_eq!(problem["detail"], "the note's text is empty", "{problem}");
    assert_eq!(400, client.post("/api/notes").body_json("not json").send().status());
    assert_eq!(400, client.delete("/api/notes/x").send().status());
}

#[test]
fn health_answers() {
    let client = TestClient::new(app(Store::memory()));
    assert_eq!(200, client.get("/healthz").send().status());
}

#[test]
fn no_route_shadows_another() {
    assert!(app(Store::memory()).route_conflicts().is_empty());
}

#[tokio::test]
async fn postgres_round_trip() {
    if std::env::var("PGHOST").is_err() {
        return;
    }
    let store = Store::connect("migrations").await.expect("connect");
    let note = store.add("from postgres".into()).await.unwrap();
    assert!(note.id > 0);
    assert!(store.list().await.unwrap().iter().any(|n| n.id == note.id && n.text == "from postgres"));
    assert!(store.delete(note.id).await.unwrap());
    assert!(!store.delete(note.id).await.unwrap());
    assert!(store.ready().await);
}
