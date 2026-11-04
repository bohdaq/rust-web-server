//! Tests of the helpers beside `TestClient`: WebSocket, SSE, snapshots,
//! `TestDb` and factories.

use std::time::Duration;

use super::sse::{SseFrame, SseParser, TestSse, MAX_EVENT_BYTES};
use super::snapshot::{check_snapshot, line_diff, normalize};
use super::TestClient;
use crate::app::App;
use crate::core::New;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::service_discovery::json_lite;
use crate::sse::{Sse, SseEvent};
use crate::test_env::TempPath;

fn frame(event: &str, data: &str, id: Option<&str>) -> SseFrame {
    SseFrame { event: event.to_string(), data: data.to_string(), id: id.map(str::to_string), retry: None }
}

// ── WebSocket ─────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
mod websocket {
    use super::*;
    use crate::websocket::{encode_frame, Frame, Message, WebSocket};
    use std::io::Write;

    fn echo_app() -> crate::state::AppWithState<()> {
        App::with_state(())
            .get("/echo", |request, _params, _conn, _state| {
                WebSocket::upgrade(request, |mut ws| {
                    while let Ok(Some(message)) = ws.recv() {
                        let _ = match message {
                            Message::Text(text) if text == "bye" => ws.close(4000, "asked to"),
                            Message::Text(text) => ws.send_text(&format!("echo: {text}")),
                            Message::Binary(data) => ws.send_binary(&data),
                        };
                    }
                })
            })
            .get("/plain", |_request, _params, _conn, _state| {
                Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, None)
            })
            .get("/silent", |request, _params, _conn, _state| {
                WebSocket::upgrade(request, |mut ws| while let Ok(Some(_)) = ws.recv() {})
            })
            .get("/pinger", |request, _params, _conn, _state| {
                WebSocket::upgrade(request, |mut ws| {
                    let _ = ws.sender().ping(b"are you there");
                    let _ = ws.send_text("after the ping");
                    while let Ok(Some(_)) = ws.recv() {}
                })
            })
            .get("/fragments", |request, _params, _conn, _state| {
                // Raw frames: a text message in two fragments, a ping between.
                let handshake = WebSocket::handshake_response(request).expect("handshake");
                WebSocket::upgrade_raw(handshake, |_reader, mut writer| {
                    let mut first = encode_frame(Frame::Text("hel".to_string()));
                    first[0] &= 0x7F; // FIN off
                    let _ = writer.write_all(&first);
                    let _ = writer.write_all(&encode_frame(Frame::Ping(b"p".to_vec())));
                    let _ = writer.write_all(&encode_frame(Frame::Continuation { fin: true, data: b"lo".to_vec() }));
                })
            })
    }

    #[test]
    fn echoes_text_and_binary_then_closes() {
        let client = TestClient::new(echo_app());
        let mut ws = client.websocket("/echo").connect().expect("a 101");
        assert_eq!(101, ws.handshake().status());
        assert_eq!(Some(WebSocket::accept_key("dGhlIHNhbXBsZSBub25jZQ==").as_str()), ws.handshake().header("Sec-WebSocket-Accept"));
        assert!(ws.handshake().header("X-Rws-WebSocket-Upgrade").is_none(), "the internal header is taken off");

        ws.send_text("hi");
        assert_eq!("echo: hi", ws.recv_text());
        ws.send_binary(&[0, 1, 2, 255]);
        assert_eq!(vec![0, 1, 2, 255], ws.recv_binary());
        // A long message (16-bit length) survives the masking.
        let long = "x".repeat(70_000);
        ws.send_text(&long);
        assert_eq!(format!("echo: {long}"), ws.recv_text());
        assert_eq!(Some(1000), ws.close(1000, "done"));
    }

    #[test]
    fn the_handler_closing_ends_recv() {
        let client = TestClient::new(echo_app());
        let mut ws = client.websocket("/echo").connect().unwrap();
        ws.send_text("bye");
        assert_eq!(Ok(None), ws.recv());
        assert_eq!(Some(Some(4000)), ws.close_code());
        assert!(ws.handler_finished(Duration::from_secs(5)), "the handler returns after the closing handshake");
        // Already closed: close() doesn't wait for another close frame.
        assert_eq!(Some(4000), ws.close(1000, ""));
    }

    #[test]
    fn a_route_that_doesnt_upgrade_is_an_err_with_its_response() {
        let client = TestClient::new(echo_app());
        let refused = client.websocket("/plain").connect().err().expect("no upgrade");
        assert_eq!(200, refused.status());

        // A wrong version: WebSocket::upgrade refuses with 426 and never runs the handler.
        let refused = client.websocket("/echo").header("Sec-WebSocket-Version", "8").connect().err().expect("refused");
        assert_eq!(426, refused.status());
        assert_eq!(Some("13"), refused.header("Sec-WebSocket-Version"));

        assert_eq!(404, client.websocket("/missing").connect().err().expect("no route").status());
    }

    #[test]
    fn recv_times_out_when_the_handler_sends_nothing() {
        let client = TestClient::new(echo_app());
        let mut ws = client.websocket("/silent").timeout(Duration::from_millis(100)).connect().unwrap();
        let error = ws.recv().unwrap_err();
        assert!(error.contains("within"), "{error}");
    }

    #[test]
    fn pings_are_answered_and_skipped_by_recv() {
        let client = TestClient::new(echo_app());
        let mut ws = client.websocket("/pinger").connect().unwrap();
        assert_eq!(Frame::Ping(b"are you there".to_vec()), ws.recv_frame().unwrap());
        assert_eq!("after the ping", ws.recv_text());

        let mut ws = client.websocket("/pinger").connect().unwrap();
        assert_eq!("after the ping", ws.recv_text(), "recv skips the ping");
        ws.close(1000, "");
    }

    #[test]
    fn fragments_are_joined() {
        let client = TestClient::new(echo_app());
        let mut ws = client.websocket("/fragments").connect().unwrap();
        assert_eq!(Ok(Some(Message::Text("hello".to_string()))), ws.recv());
        // The handler returned without a close frame: the stream just ends.
        assert!(ws.recv().unwrap_err().contains("read error"));
    }

    #[test]
    fn an_unmasked_frame_fails_the_connection_with_1002() {
        let client = TestClient::new(echo_app());
        let mut ws = client.websocket("/echo").connect().unwrap();
        ws.send_raw(&encode_frame(Frame::Text("not masked".to_string()))).unwrap();
        assert_eq!(Ok(None), ws.recv());
        assert_eq!(Some(Some(1002)), ws.close_code());
    }

    #[test]
    fn dropping_the_test_side_ends_the_handler() {
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        let app = App::with_state(()).get("/ws", move |request, _params, _conn, _state| {
            let tx = tx.lock().unwrap().clone();
            WebSocket::upgrade(request, move |mut ws| {
                let result = ws.recv();
                let _ = tx.send(result.is_err());
            })
        });
        let client = TestClient::new(app);
        drop(client.websocket("/ws").connect().unwrap());
        assert_eq!(Ok(true), rx.recv_timeout(Duration::from_secs(5)), "recv errs once the client side is gone");
    }

    #[test]
    #[should_panic(expected = "expected a WebSocket text message")]
    fn recv_text_panics_on_binary() {
        let client = TestClient::new(echo_app());
        let mut ws = client.websocket("/echo").connect().unwrap();
        ws.send_binary(b"raw");
        ws.recv_text();
    }
}

// ── SSE ───────────────────────────────────────────────────────────────────────

#[test]
fn streamed_sse_events_arrive_as_sent() {
    let app = App::with_state(()).get("/events", |_req, _params, _conn, _state| {
        let (events, response) = Sse::stream();
        std::thread::spawn(move || {
            for n in 1..=3 {
                let _ = events.send(SseEvent::data(format!("line {n}\nsecond")).event_type("tick").id(n.to_string()));
            }
        });
        response
    });
    let client = TestClient::new(app);
    let response = client.get("/events").send();
    assert_eq!(Some("text/event-stream"), response.header("Content-Type"));
    assert!(response.is_streamed());
    let events = response.sse_events(3, Duration::from_secs(5));
    assert_eq!(
        vec![
            frame("tick", "line 1\nsecond", Some("1")),
            frame("tick", "line 2\nsecond", Some("2")),
            frame("tick", "line 3\nsecond", Some("3")),
        ],
        events
    );
}

#[test]
fn a_stream_ends_when_its_senders_are_dropped() {
    let app = App::with_state(()).get("/once", |_req, _params, _conn, _state| {
        let (events, response) = Sse::stream();
        let _ = events.data("only");
        response
    });
    let mut sse = TestClient::new(app).get("/once").send().sse();
    assert_eq!(vec![frame("message", "only", None)], sse.collect_until_end(Duration::from_secs(5)));
    assert!(sse.is_ended());
    assert_eq!(None, sse.error());
    assert_eq!(None, sse.next_event(Duration::from_millis(10)));
}

#[test]
fn waiting_for_events_that_dont_come_times_out() {
    let (sender, response) = Sse::stream();
    let mut sse = super::TestResponse::from_response(response).sse();
    assert!(sse.events(1, Duration::from_millis(100)).is_empty());
    assert!(!sse.is_ended(), "the sender is still alive");
    let _ = sender.data("late");
    assert_eq!(Some(frame("message", "late", None)), sse.next_event(Duration::from_secs(5)));
}

#[test]
fn a_buffered_sse_body_is_parsed_too() {
    let app = App::with_state(()).get("/buffered", |_req, _params, _conn, _state| {
        Sse::new()
            .event("connected", "ready")
            .push(SseEvent::data("{\"count\":1}").id("1").event_type("update").retry(500))
            .comment("keep-alive")
            .into_response()
    });
    let response = TestClient::new(app).get("/buffered").send();
    assert!(!response.is_streamed());
    let mut sse = response.sse();
    let events = sse.events(10, Duration::from_secs(1));
    assert_eq!(2, events.len());
    assert_eq!(frame("connected", "ready", None), events[0]);
    assert_eq!(SseFrame { retry: Some(500), ..frame("update", "{\"count\":1}", Some("1")) }, events[1]);
    assert_eq!(1, sse.comments());
    assert!(sse.is_ended());
}

#[test]
fn a_failing_reader_is_reported() {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("connection reset"))
        }
    }
    let mut sse = TestSse::from_reader(Box::new(Broken));
    assert_eq!(None, sse.next_event(Duration::from_secs(5)));
    assert!(sse.is_ended());
    assert_eq!(Some("connection reset"), sse.error());
}

#[test]
fn sse_parser_follows_the_event_stream_rules() {
    let mut parser = SseParser::new();
    // A BOM, all three line endings, a comment, a field without a colon,
    // `data:` with and without the space, an unknown field.
    let input = b"\xEF\xBB\xBF: hello\r\nevent: a\rdata: one\ndata:two\r\ndata\nfoo: bar\n\n";
    assert_eq!(vec![frame("a", "one\ntwo\n", None)], parser.feed(input).unwrap());
    assert_eq!(1, parser.comments());

    // A block without data isn't dispatched; its fields don't leak into the next.
    assert!(parser.feed(b"event: lonely\nid: 9\n\n").unwrap().is_empty());
    assert_eq!(vec![frame("message", "x", None)], parser.feed(b"data: x\n\n").unwrap());

    // An id with NUL is ignored; retry must be all digits.
    let events = parser.feed(b"id: a\0b\nretry: 12x\ndata: y\n\nretry: 3000\ndata: z\n\n").unwrap();
    assert_eq!(frame("message", "y", None), events[0]);
    assert_eq!(Some(3000), events[1].retry);
}

#[test]
fn sse_parser_takes_any_chunking() {
    let input = b"event: e\r\ndata: 1\r\n\r\ndata: 2\r\n\r\n";
    let whole = SseParser::new().feed(input).unwrap();
    assert_eq!(2, whole.len());
    // Byte by byte — the CRLF split across feeds still ends one line.
    let mut parser = SseParser::new();
    let mut split = Vec::new();
    for byte in input {
        split.extend(parser.feed(std::slice::from_ref(byte)).unwrap());
    }
    assert_eq!(whole, split);
}

#[test]
fn sse_parser_refuses_an_oversized_event() {
    let mut parser = SseParser::new();
    let line = vec![b'a'; MAX_EVENT_BYTES + 1];
    let error = parser.feed(&line).unwrap_err();
    assert!(error.contains("bytes"), "{error}");
    // Many short data lines are bounded too.
    let mut parser = SseParser::new();
    let flood = b"data:\n".repeat(MAX_EVENT_BYTES / 6 + 2);
    assert!(parser.feed(&flood).is_err());
    // …and after an error the parser starts over.
    assert_eq!(vec![frame("message", "ok", None)], parser.feed(b"data: ok\n\n").unwrap());
}

// ── Snapshots ─────────────────────────────────────────────────────────────────

const DOC: &str = r#"{"openapi":"3.1.0","info":{"title":"T","version":"1"},"paths":{"/b":{"get":{}},"/a":{"get":{"responses":{"200":{"description":"OK"}}}}}}"#;

#[test]
fn normalize_sorts_keys_and_indents() {
    let value = json_lite::parse(r#"{"b":[1,2.5,{"y":true,"x":null}],"a":"q\"","e":{},"f":[]}"#).unwrap();
    assert_eq!(
        "{\n  \"a\": \"q\\\"\",\n  \"b\": [\n    1,\n    2.5,\n    {\n      \"x\": null,\n      \"y\": true\n    }\n  ],\n  \"e\": {},\n  \"f\": []\n}\n",
        normalize(&value)
    );
}

#[test]
fn a_snapshot_is_written_then_matched_whatever_the_key_order() {
    let dir = TempPath::new("snapshot_write");
    let path = dir.join("nested/openapi.json");
    let value = json_lite::parse(DOC).unwrap();

    let missing = check_snapshot(&value, &path, false).unwrap_err();
    assert!(missing.contains("RWS_UPDATE_SNAPSHOTS=1"), "{missing}");

    check_snapshot(&value, &path, true).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(normalize(&value), written);
    assert!(written.find("\"/a\"").unwrap() < written.find("\"/b\"").unwrap(), "keys sorted");

    let reordered = json_lite::parse(r#"{"paths":{"/a":{"get":{"responses":{"200":{"description":"OK"}}}},"/b":{"get":{}}},"info":{"version":"1","title":"T"},"openapi":"3.1.0"}"#).unwrap();
    check_snapshot(&reordered, &path, false).unwrap();
    // A hand-formatted (compact) snapshot file matches as well.
    std::fs::write(&path, DOC).unwrap();
    check_snapshot(&value, &path, false).unwrap();
}

#[test]
fn a_changed_document_fails_with_a_diff_until_updated() {
    let dir = TempPath::new("snapshot_diff");
    let path = dir.join("openapi.json");
    check_snapshot(&json_lite::parse(DOC).unwrap(), &path, true).unwrap();

    let changed = json_lite::parse(&DOC.replace("\"OK\"", "\"Found\"")).unwrap();
    let error = check_snapshot(&changed, &path, false).unwrap_err();
    assert!(error.lines().any(|l| l.starts_with("- ") && l.trim_end().ends_with("\"description\": \"OK\"")), "{error}");
    assert!(error.lines().any(|l| l.starts_with("+ ") && l.trim_end().ends_with("\"description\": \"Found\"")), "{error}");
    assert!(error.contains("RWS_UPDATE_SNAPSHOTS=1"), "{error}");

    check_snapshot(&changed, &path, true).unwrap();
    check_snapshot(&changed, &path, false).unwrap();
}

#[test]
fn a_snapshot_that_isnt_json_fails() {
    let dir = TempPath::new("snapshot_garbage");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("openapi.json");
    std::fs::write(&path, "<html>").unwrap();
    let error = check_snapshot(&json_lite::parse(DOC).unwrap(), &path, false).unwrap_err();
    assert!(error.contains("isn't JSON"), "{error}");
}

#[test]
fn line_diff_shows_changes_with_context() {
    let a = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
    let b = "1\n2\n3\n4\n5\nsix\n7\n8\n9\n10\n11\n";
    let diff = line_diff(a, b);
    assert_eq!("  …\n  3\n  4\n  5\n- 6\n+ six\n  7\n  8\n  9\n  10\n+ 11\n", diff);
    assert_eq!("", line_diff(a, a));
}

#[test]
#[should_panic(expected = "not an OpenAPI document")]
fn a_document_without_an_openapi_field_is_refused() {
    super::assert_openapi_snapshot("{\"error\":\"not found\"}", "tests/snapshots/never-written.json");
}

#[test]
#[should_panic(expected = "isn't JSON")]
fn a_document_that_isnt_json_is_refused() {
    super::assert_openapi_snapshot("Not Found", "tests/snapshots/never-written.json");
}

#[cfg(feature = "openapi")]
#[test]
fn an_apps_document_is_read_through_the_client() {
    use crate::openapi::OpenApiConfig;
    let app = App::with_state(())
        .get("/users/:id", |_req, _params, _conn, _state| Response::new())
        .openapi(OpenApiConfig::new("Users", "1.0.0"));
    let dir = TempPath::new("snapshot_app");
    let path = dir.join("openapi.json");
    let client = TestClient::new(app);
    let document = super::OpenApiSource::openapi_json(&client);
    check_snapshot(&json_lite::parse(&document).unwrap(), &path, true).unwrap();
    super::assert_openapi_snapshot(&client, &path);
    assert!(std::fs::read_to_string(&path).unwrap().contains("\"/users/{id}\""));

    let without = TestClient::new(App::with_state(()));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| super::assert_openapi_snapshot(&without, &path)));
    assert!(result.is_err(), "no /openapi.json is a failure, not a pass");
}

// ── TestDb and factories ──────────────────────────────────────────────────────

#[cfg(feature = "model-sqlite")]
mod db {
    use super::*;
    use crate::model::{Backend, DbPool, Value};
    use crate::test_client::TestDb;

    fn migrations(name: &str) -> TempPath {
        let dir = TempPath::new(name);
        std::fs::create_dir_all(&*dir).unwrap();
        std::fs::write(dir.join("0001_notes.sql"), "CREATE TABLE notes (id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL);").unwrap();
        std::fs::write(dir.join("0002_tags.sql"), "CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE);").unwrap();
        dir
    }

    async fn count(pool: &DbPool, table: &str) -> i64 {
        let rows = pool.query_rows(&format!("SELECT COUNT(*) AS n FROM {table}"), &[]).await.unwrap();
        rows[0].get::<i64>("n").unwrap()
    }

    #[tokio::test]
    async fn sqlite_gets_the_migrations_and_its_own_database() {
        let dir = migrations("testdb_sqlite");
        let a = TestDb::sqlite().migrations(dir.as_str()).await.unwrap();
        let b = TestDb::sqlite().migrations(dir.as_str()).build().await.unwrap();
        assert_eq!(Backend::Sqlite, a.backend());
        assert!(!a.is_transactional());

        a.execute("INSERT INTO notes (text) VALUES (?)", &[Value::Text("in a".into())]).await.unwrap();
        assert_eq!(1, count(&a, "notes").await);
        assert_eq!(0, count(&b, "notes").await, "each TestDb is its own database");
        assert_eq!(0, count(&a.pool(), "tags").await, "both migrations ran");
        let status = a.migration_status(dir.as_str()).await.unwrap();
        assert!(status.iter().all(|s| s.applied));
        a.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_without_migrations_is_empty() {
        let db = TestDb::sqlite().await.unwrap();
        assert!(db.query_rows("SELECT * FROM notes", &[]).await.is_err());
    }

    #[tokio::test]
    async fn a_broken_migration_is_an_error() {
        let dir = TempPath::new("testdb_broken");
        std::fs::create_dir_all(&*dir).unwrap();
        std::fs::write(dir.join("0001_bad.sql"), "CREATE TABLE (oops;").unwrap();
        let error = TestDb::sqlite().migrations(dir.as_str()).await.err().expect("a failed migration");
        assert!(error.to_string().contains("0001_bad.sql"), "{error}");
        assert!(TestDb::sqlite().migrations("/no/such/migrations").await.is_err());
    }

    fn count_app(pool: DbPool) -> crate::async_state::AsyncAppWithState<DbPool> {
        App::with_async_state(pool).get("/notes/count", |_req, _params, _conn, pool| async move {
            let rows = match pool.query_rows("SELECT COUNT(*) AS n FROM notes", &[]).await {
                Ok(rows) => rows,
                Err(e) => {
                    let body = e.to_string().into_bytes();
                    let range = crate::range::Range::get_content_range(body, crate::mime_type::MimeType::TEXT_PLAIN.to_string());
                    return Response::get_response(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, None, Some(vec![range]));
                }
            };
            let n: i64 = rows[0].get("n").unwrap();
            let range = crate::range::Range::get_content_range(n.to_string().into_bytes(), crate::mime_type::MimeType::TEXT_PLAIN.to_string());
            Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, Some(vec![range]))
        })
    }

    #[tokio::test]
    async fn an_app_on_a_testdb_is_driven_by_test_client() {
        let dir = migrations("testdb_app");
        let db = TestDb::sqlite().migrations(dir.as_str()).await.unwrap();
        db.execute("INSERT INTO notes (text) VALUES ('one'), ('two')", &[]).await.unwrap();
        let client = TestClient::new(count_app(db.pool()));
        let response = client.get("/notes/count").send_async().await;
        assert_eq!(200, response.status(), "{}", response.body_text());
        assert_eq!("2", response.body_text());
        assert_eq!("2", client.get("/notes/count").send_async().await.body_text());
        // The pool kept its one connection, so the in-memory database too.
        assert_eq!(2, count(&db, "notes").await);
        // A route the async app doesn't have still answers (through execute).
        assert_eq!(404, client.get("/nothing").send_async().await.status());
    }

    /// Runs only with a reachable server: RWS_TEST_POSTGRES_URL.
    #[cfg(feature = "model-postgres")]
    #[tokio::test(flavor = "multi_thread")]
    async fn postgres_writes_are_rolled_back() {
        let Some(builder) = TestDb::postgres_from_env() else { return };
        let dir = TempPath::new("testdb_pg");
        std::fs::create_dir_all(&*dir).unwrap();
        std::fs::write(dir.join("0001_rws_testdb.sql"), "CREATE TABLE IF NOT EXISTS rws_testdb_notes (id BIGSERIAL PRIMARY KEY, text TEXT NOT NULL);").unwrap();
        let db = builder.migrations(dir.as_str()).await.unwrap();
        assert!(db.is_transactional());
        let before = count(&db, "rws_testdb_notes").await;
        db.execute("INSERT INTO rws_testdb_notes (text) VALUES ('rolled back')", &[]).await.unwrap();
        // A DbPool transaction inside is a savepoint; its commit keeps the row in the test's transaction.
        let mut tx = db.begin().await.unwrap();
        tx.execute("INSERT INTO rws_testdb_notes (text) VALUES ('savepoint')", &[]).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(before + 2, count(&db, "rws_testdb_notes").await);
        db.rollback().await.unwrap();

        let again = TestDb::postgres_from_env().unwrap().await.unwrap();
        assert_eq!(before, count(&again, "rws_testdb_notes").await, "nothing was committed");
        drop(again);
        let third = TestDb::postgres_from_env().unwrap().await.unwrap();
        assert_eq!(before, count(&third, "rws_testdb_notes").await);
    }

    /// Runs only with a reachable server: RWS_TEST_MYSQL_URL.
    #[cfg(feature = "model-mysql")]
    #[tokio::test(flavor = "multi_thread")]
    async fn mysql_writes_are_rolled_back() {
        let Some(builder) = TestDb::mysql_from_env() else { return };
        let dir = TempPath::new("testdb_mysql");
        std::fs::create_dir_all(&*dir).unwrap();
        std::fs::write(dir.join("0001_rws_testdb.sql"), "CREATE TABLE IF NOT EXISTS rws_testdb_notes (id BIGINT AUTO_INCREMENT PRIMARY KEY, text TEXT NOT NULL) ENGINE=InnoDB;").unwrap();
        let db = builder.migrations(dir.as_str()).await.unwrap();
        let before = count(&db, "rws_testdb_notes").await;
        db.execute("INSERT INTO rws_testdb_notes (text) VALUES ('rolled back')", &[]).await.unwrap();
        assert_eq!(before + 1, count(&db, "rws_testdb_notes").await);
        db.rollback().await.unwrap();
        let again = TestDb::mysql_from_env().unwrap().await.unwrap();
        assert_eq!(before, count(&again, "rws_testdb_notes").await, "nothing was committed");
    }

    #[cfg(feature = "model-postgres")]
    #[tokio::test]
    async fn a_bad_postgres_url_is_an_error() {
        let error = TestDb::postgres("not a url").await.err().expect("refused");
        assert!(error.to_string().contains("URL"), "{error}");
    }
}

#[cfg(all(feature = "model-sqlite", feature = "macros"))]
mod factories {
    use crate::model::{DbError, FromColumn, Repository, ToColumn, Value};
    use crate::test_client::TestDb;

    /// A column type with no FactoryValue: the factory falls back to Default.
    #[derive(Debug, Clone, Default, PartialEq)]
    struct Level(i64);
    impl FromColumn for Level {
        fn from_column(v: Value) -> Result<Self, DbError> {
            i64::from_column(v).map(Level)
        }
    }
    impl ToColumn for Level {
        fn to_column(&self) -> Value {
            Value::Int(self.0)
        }
    }

    // `#[ignore]` is the built-in attribute, which rustc warns about on a field.
    #[allow(unused_attributes)]
    #[derive(rws_macros::Model, rws_macros::Factory, Debug, Clone, PartialEq)]
    #[table(name = "people")]
    struct Person {
        #[primary_key(auto_increment)]
        id: i64,
        name: String,
        email: String,
        age: Option<i32>,
        active: bool,
        score: f64,
        level: Level,
        #[ignore]
        label: String,
    }

    #[derive(rws_macros::Model, rws_macros::Factory, Debug, Clone)]
    #[table(name = "codes")]
    struct Code {
        #[primary_key]
        code: i64,
        #[column(name = "code_text")]
        text: String,
    }

    async fn db() -> TestDb {
        let db = TestDb::sqlite().await.unwrap();
        db.execute(
            "CREATE TABLE people (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, email TEXT NOT NULL UNIQUE, \
             age INTEGER, active INTEGER NOT NULL, score REAL NOT NULL, level INTEGER NOT NULL)",
            &[],
        )
        .await
        .unwrap();
        db.execute("CREATE TABLE codes (code INTEGER PRIMARY KEY, code_text TEXT NOT NULL)", &[]).await.unwrap();
        db
    }

    #[test]
    fn build_fills_every_field_with_its_default() {
        let person = Person::factory().build();
        assert_eq!(0, person.id, "auto_increment keys are left to the database");
        assert!(person.name.starts_with("name-"), "{}", person.name);
        let seq = person.name.trim_start_matches("name-");
        assert_eq!(format!("email-{seq}@example.com"), person.email);
        assert_eq!((None, false, 0.0, Level(0), String::new()), (person.age, person.active, person.score, person.level, person.label));

        let next = Person::factory().build();
        assert_ne!(person.name, next.name, "the sequence moves on");
        let many = Person::factory().build_many(3);
        assert_eq!(3, many.len());
    }

    #[test]
    fn with_overrides_apply_in_order() {
        let person = Person::factory()
            .with(|p| p.name = "Ann".into())
            .with(|p| p.age = Some(30))
            .with(|p| p.name.push_str(" Lee"))
            .build();
        assert_eq!(("Ann Lee", Some(30)), (person.name.as_str(), person.age));
    }

    #[test]
    fn a_key_that_isnt_auto_increment_gets_the_sequence_number() {
        let code = Code::factory().build();
        assert!(code.code > 0);
        assert_eq!(format!("text-{}", code.code), code.text);
    }

    #[tokio::test]
    async fn create_saves_and_returns_the_row_with_its_id() {
        let db = db().await;
        let ann = Person::factory().with(|p| p.name = "Ann".into()).create(&db).await.unwrap();
        assert!(ann.id > 0);
        assert_eq!("Ann", ann.name);
        let found = Person::repository(&db).find_by_id(ann.id).await.unwrap().expect("saved");
        assert_eq!(ann, found);

        let created = Person::factory().with(|p| p.active = true).create_many(3, &db).await.unwrap();
        assert!(created.iter().all(|p| p.active && p.id > ann.id));
        assert_eq!(4, Person::repository(&db).count().await.unwrap());

        let code = Code::factory().create(&db).await.unwrap();
        assert!(Code::repository(&db).find_by_id(code.code).await.unwrap().is_some(), "inserted, not updated");
    }

    #[tokio::test]
    async fn create_reports_database_errors() {
        let db = db().await;
        let dupe = Person::factory().with(|p| p.email = "same@example.com".into());
        dupe.create(&db).await.unwrap();
        assert!(dupe.create(&db).await.is_err(), "the UNIQUE column refuses a second row");

        let empty = TestDb::sqlite().await.unwrap();
        assert!(Person::factory().create(&empty).await.is_err(), "no table");
    }
}
