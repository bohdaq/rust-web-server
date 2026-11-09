//! The test helpers (#89) as an application sees them: the derives from
//! this crate's root, `TestDb` and the factory through `test_client`.

use crate::model::Repository;
use crate::test_client::{TestClient, TestDb};
use crate::test_env::TempPath;
use crate::{Factory, Model};

#[derive(Model, Factory, Debug, Clone, PartialEq)]
#[table(name = "notes")]
struct Note {
    #[primary_key(auto_increment)]
    id: i64,
    text: String,
    pinned: bool,
}

/// A migrations directory per test: two tests sharing one raced (the
/// first to finish deleted it under the other).
fn migrations(name: &str) -> TempPath {
    let dir = TempPath::new(&format!("facade_test_helpers_{name}"));
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(
        dir.join("0001_create_notes.sql"),
        "CREATE TABLE notes (id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL, pinned INTEGER NOT NULL);",
    )
    .unwrap();
    dir
}

#[tokio::test]
async fn factories_save_into_a_test_db() {
    let dir = migrations("factories");
    let db = TestDb::sqlite().migrations(dir.as_str()).await.unwrap();
    let pinned = Note::factory().with(|n| n.pinned = true).create(&db).await.unwrap();
    let plain = Note::factory().create(&db).await.unwrap();
    assert!(pinned.pinned && !plain.pinned);
    assert!(plain.text.starts_with("text-"));
    assert_eq!(2, Note::repository(&db).count().await.unwrap());
    assert_eq!(Some(pinned.clone()), Note::repository(&db).find_by_id(pinned.id).await.unwrap());

    let other = TestDb::sqlite().migrations(dir.as_str()).await.unwrap();
    assert_eq!(0, Note::repository(&other).count().await.unwrap(), "each TestDb starts empty");
}

#[tokio::test]
async fn an_async_app_on_a_test_db() {
    use crate::app::App;
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
    let dir = migrations("async_app");
    let db = TestDb::sqlite().migrations(dir.as_str()).await.unwrap();
    Note::factory().create_many(3, &db).await.unwrap();
    let app = App::with_async_state(db.pool()).get("/notes", |_req, _params, _conn, pool| async move {
        let notes = Note::repository(&pool).count().await.unwrap_or(-1);
        let mut response = Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, None);
        response.headers.push(crate::header::Header::new("X-Count", notes.to_string()));
        response
    });
    let response = TestClient::new(app).get("/notes").send_async().await;
    assert_eq!(Some("3"), response.header("X-Count"));
}
