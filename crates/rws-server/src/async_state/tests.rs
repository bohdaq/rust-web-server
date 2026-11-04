use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::application::Application;
use crate::async_state::AsyncAppWithState;
use crate::core::New;
use crate::di::Container;
use crate::header::Header;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};
use crate::server_config::ServerConfig;

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
    sni_hostname: None,
    tls: false,
    }
}

fn get(uri: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    }
}

fn post(uri: &str, body: &[u8]) -> Request {
    Request {
        method: METHOD.post.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: body.to_vec().into(),
    }
}

fn ok_text(s: &str) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(
        s.as_bytes().to_vec(),
        MimeType::TEXT_PLAIN.to_string(),
    )];
    r
}

// ── Basic async dispatch ───────────────────────────────────────────────────────

#[tokio::test]
async fn async_get_handler_is_called() {
    let app = AsyncAppWithState::new(())
        .get("/hello", |_req, _params, _conn, _state| async { ok_text("hi") });

    let resp = app.execute(&get("/hello"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hi", body);
}

#[tokio::test]
async fn async_handler_receives_path_params() {
    let app = AsyncAppWithState::new(())
        .get("/users/:id", |_req, params, _conn, _state| async move {
            let id = params.get("id").unwrap_or("?").to_string();
            ok_text(&format!("user={}", id))
        });

    let resp = app.execute(&get("/users/42"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("user=42", body);
}

#[tokio::test]
async fn async_handler_receives_state() {
    struct State { msg: String }

    let app = AsyncAppWithState::new(State { msg: "from async state".to_string() })
        .get("/msg", |_req, _params, _conn, state| async move {
            ok_text(&state.msg)
        });

    let resp = app.execute(&get("/msg"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("from async state", body);
}

#[tokio::test]
async fn async_handler_can_await() {
    async fn fetch_value() -> String {
        // simulate an async operation
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        "awaited".to_string()
    }

    let app = AsyncAppWithState::new(())
        .get("/async", |_req, _params, _conn, _state| async {
            let value = fetch_value().await;
            ok_text(&value)
        });

    let resp = app.execute(&get("/async"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("awaited", body);
}

// ── Request body and method ───────────────────────────────────────────────────

#[tokio::test]
async fn async_post_handler_reads_body() {
    let app = AsyncAppWithState::new(())
        .post("/echo", |req, _params, _conn, _state| async move {
            let body = String::from_utf8_lossy(&req.body).to_string();
            ok_text(&body)
        });

    let resp = app.execute(&post("/echo", b"hello body"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hello body", body);
}

// ── State sharing ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn state_is_shared_across_handlers() {
    let counter = Arc::new(AtomicU32::new(0));
    let app = AsyncAppWithState::new(Arc::clone(&counter))
        .get("/inc", |_req, _params, _conn, state| async move {
            state.fetch_add(1, Ordering::Relaxed);
            ok_text("ok")
        });

    app.execute(&get("/inc"), &conn()).unwrap();
    app.execute(&get("/inc"), &conn()).unwrap();
    assert_eq!(2, counter.load(Ordering::Relaxed));
}

// ── Routing behaviour ─────────────────────────────────────────────────────────

#[tokio::test]
async fn unmatched_route_falls_through_to_app() {
    let app = AsyncAppWithState::new(())
        .get("/custom", |_, _, _, _| async { ok_text("custom") });

    let resp = app.execute(&get("/healthz"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}

#[tokio::test]
async fn unmatched_route_returns_404() {
    // Falls through to App's built-in StaticResourceController, whose
    // is_matching now depends on RWS_CONFIG_SPA_FALLBACK.
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");

    let app = AsyncAppWithState::new(())
        .get("/custom", |_, _, _, _| async { ok_text("custom") });

    let resp = app.execute(&get("/does-not-exist-xyz"), &conn()).unwrap();
    assert_eq!(404, resp.status_code);
}

#[tokio::test]
async fn first_matching_route_wins() {
    let app = AsyncAppWithState::new(())
        .get("/a", |_, _, _, _| async { ok_text("first") })
        .get("/a", |_, _, _, _| async { ok_text("second") });

    let resp = app.execute(&get("/a"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("first", body);
}

#[tokio::test]
async fn wildcard_param_captures_tail() {
    let app = AsyncAppWithState::new(())
        .get("/files/*path", |_, params, _, _| async move {
            ok_text(params.get("path").unwrap_or(""))
        });

    let resp = app.execute(&get("/files/a/b/c.txt"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("a/b/c.txt", body);
}

#[tokio::test]
async fn put_patch_delete_registered() {
    let app = AsyncAppWithState::new(())
        .put("/r", |_, _, _, _| async { ok_text("put") })
        .patch("/r", |_, _, _, _| async { ok_text("patch") })
        .delete("/r", |_, _, _, _| async { ok_text("delete") });

    for (method, expected) in [("PUT", "put"), ("PATCH", "patch"), ("DELETE", "delete")] {
        let req = Request {
            method: method.to_string(),
            request_uri: "/r".to_string(),
            http_version: VERSION.http_1_1.to_string(),
            headers: vec![],
            body: Default::default(),
        };
        let resp = app.execute(&req, &conn()).unwrap();
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert_eq!(expected, body, "method={}", method);
    }
}

// ── State accessor ────────────────────────────────────────────────────────────

#[tokio::test]
async fn state_accessor_returns_inner_value() {
    let app = AsyncAppWithState::new("sentinel".to_string());
    assert_eq!("sentinel", app.state());
}

// ── di::Container as AsyncAppWithState's state ──────────────────────────────────
//
// Same as `AppWithState`: `Container` is `Send + Sync + 'static` like any other
// state type, so `AsyncAppWithState::new(container)` works directly — no
// `.into_arc()` double-wrap needed.

#[tokio::test]
async fn container_resolves_concrete_service_through_an_async_handler() {
    struct Greeting(String);

    let mut container = Container::new();
    container.register(Greeting("hello from async state".to_string()));

    let app = AsyncAppWithState::new(container).get("/greet", |_req, _params, _conn, state| async move {
        let greeting = state.get::<Greeting>().unwrap();
        ok_text(&greeting.0)
    });

    let resp = app.execute(&get("/greet"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hello from async state", body);
}

#[tokio::test]
async fn container_resolves_trait_object_service_through_an_async_handler() {
    trait Greeter: Send + Sync {
        fn greet(&self) -> String;
    }
    struct EnglishGreeter;
    impl Greeter for EnglishGreeter {
        fn greet(&self) -> String {
            "hi from async".to_string()
        }
    }

    let mut container = Container::new();
    container.provide::<dyn Greeter>(Arc::new(EnglishGreeter));

    let app = AsyncAppWithState::new(container).get("/greet", |_req, _params, _conn, state| async move {
        ok_text(&state.get::<dyn Greeter>().unwrap().greet())
    });

    let resp = app.execute(&get("/greet"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hi from async", body);
}

// ── with_config: fallback App pinned to an explicit ServerConfig ───────────────

#[tokio::test]
async fn with_config_pins_cors_denial_on_fallback_request() {
    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: String::new(),
        ..ServerConfig::default()
    };
    let app = AsyncAppWithState::new(()).with_config(config);

    let mut req = get("/does-not-exist");
    req.headers.push(Header {
        name: Header::_ORIGIN.into(),
        value: "https://evil.example.com".into(),
    });

    let resp = app.execute(&req, &conn()).unwrap();
    assert!(resp._get_header(Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).is_none());
}

#[tokio::test]
async fn with_config_allows_cors_for_configured_origin_on_fallback_request() {
    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: "https://trusted.example.com".to_string(),
        ..ServerConfig::default()
    };
    let app = AsyncAppWithState::new(()).with_config(config);

    let mut req = get("/does-not-exist");
    req.headers.push(Header {
        name: Header::_ORIGIN.into(),
        value: "https://trusted.example.com".into(),
    });

    let resp = app.execute(&req, &conn()).unwrap();
    let acao = resp._get_header(Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).unwrap();
    assert_eq!("https://trusted.example.com", acao.value);
}

// ── openapi() ────────────────────────────────────────────────────────────────

#[cfg(feature = "openapi")]
mod openapi_tests {
    use super::*;
    use crate::openapi::OpenApiConfig;

    #[tokio::test]
    async fn openapi_json_serves_a_spec_covering_registered_routes() {
        let app = AsyncAppWithState::new(())
            .get("/users", |_req, _params, _conn, _state| async { ok_text("users") })
            .get("/users/:id", |_req, _params, _conn, _state| async { ok_text("user") })
            .openapi(OpenApiConfig::new("My API", "1.0.0"));

        let resp = app.execute(&get("/openapi.json"), &conn()).unwrap();
        assert_eq!(200, resp.status_code);
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert!(body.contains(r#""title":"My API""#));
        assert!(body.contains(r#""/users":"#));
        assert!(body.contains(r#""/users/{id}":"#));
    }

    #[tokio::test]
    async fn docs_serves_swagger_ui_html() {
        let app = AsyncAppWithState::new(())
            .get("/users", |_req, _params, _conn, _state| async { ok_text("users") })
            .openapi(OpenApiConfig::new("My API", "1.0.0"));

        let resp = app.execute(&get("/docs"), &conn()).unwrap();
        assert_eq!(200, resp.status_code);
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert!(body.contains("swagger-ui"));
    }

    #[tokio::test]
    async fn own_routes_are_unaffected_after_calling_openapi() {
        let app = AsyncAppWithState::new(())
            .get("/users", |_req, _params, _conn, _state| async { ok_text("users") })
            .openapi(OpenApiConfig::new("My API", "1.0.0"));

        let resp = app.execute(&get("/users"), &conn()).unwrap();
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert_eq!("users", body);
    }
}

#[test]
fn async_route_conflicts_are_reported() {
    use crate::router::RouteConflictKind;
    let app = AsyncAppWithState::new(())
        .get("/items/{id}", |_, _, _, _| async { Response::new() })
        .get("/items/:item", |_, _, _, _| async { Response::new() })
        .delete("/items/:id", |_, _, _, _| async { Response::new() });
    let conflicts = app.route_conflicts();
    assert_eq!(1, conflicts.len());
    assert_eq!(RouteConflictKind::Duplicate, conflicts[0].kind);
    assert_eq!("GET", conflicts[0].method);
}

// ── Where handlers run ────────────────────────────────────────────────────────

fn body_text(response: &Response) -> String {
    String::from_utf8(response.content_range_list[0].body.clone()).unwrap()
}

fn thread_app() -> AsyncAppWithState<()> {
    AsyncAppWithState::new(()).get("/thread", |_, _, _, _| async {
        tokio::task::yield_now().await;
        ok_text(std::thread::current().name().unwrap_or(""))
    })
}

#[test]
fn execute_from_a_plain_thread() {
    let response = thread_app().execute(&get("/thread"), &conn()).unwrap();
    assert_eq!("rws-async", body_text(&response));
}

#[test]
fn execute_inside_a_current_thread_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let response = runtime.block_on(async { thread_app().execute(&get("/thread"), &conn()) }).unwrap();
    assert_eq!("rws-async", body_text(&response));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execute_on_a_multi_thread_worker_and_its_blocking_pool() {
    let on_worker = tokio::spawn(async { thread_app().execute(&get("/thread"), &conn()) }).await.unwrap().unwrap();
    assert_eq!("rws-async", body_text(&on_worker));
    // Where the async servers call `execute`.
    let blocking = tokio::task::spawn_blocking(|| thread_app().execute(&get("/thread"), &conn())).await.unwrap().unwrap();
    assert_eq!("rws-async", body_text(&blocking));
}

#[test]
fn a_handler_calling_another_async_app_synchronously() {
    let inner = thread_app();
    let outer = AsyncAppWithState::new(inner).get("/outer", |_, _, _, inner| async move {
        let response = inner.execute(&get("/thread"), &conn()).unwrap();
        ok_text(&format!("outer>{}", body_text(&response)))
    });
    let response = outer.execute(&get("/outer"), &conn()).unwrap();
    assert_eq!("outer>rws-async", body_text(&response));
}

#[test]
fn execute_async_is_offered_only_for_matching_routes() {
    let app = thread_app();
    assert!(app.execute_async(&get("/thread"), &conn()).is_some());
    assert!(app.execute_async(&get("/missing"), &conn()).is_none(), "the fallback App is synchronous");
    // …and `execute` answers it without a runtime.
    assert_eq!(404, app.execute(&get("/missing"), &conn()).unwrap().status_code);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execute_async_on_a_multi_thread_runtime_stays_on_it() {
    // The shared server runtime: no hand-off to `rws-async`.
    let response = tokio::spawn(async {
        let app = thread_app();
        let request = get("/thread");
        let connection = conn();
        app.execute_async(&request, &connection).unwrap().await
    })
    .await
    .unwrap()
    .unwrap();
    assert_ne!("rws-async", body_text(&response));
}

#[tokio::test]
async fn execute_async_on_a_current_thread_runtime_uses_the_shared_one() {
    // Per-core runtimes are `current_thread`.
    let app = thread_app();
    let request = get("/thread");
    let connection = conn();
    let response = app.execute_async(&request, &connection).unwrap().await.unwrap();
    assert_eq!("rws-async", body_text(&response));
}

#[test]
fn tasks_a_handler_spawns_outlive_the_request() {
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let app = AsyncAppWithState::new(Arc::clone(&done)).post("/later", |_, _, _, done| async move {
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            done.store(true, Ordering::SeqCst);
        });
        ok_text("accepted")
    });
    assert_eq!(200, app.execute(&post("/later", b""), &conn()).unwrap().status_code);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !done.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(done.load(Ordering::SeqCst), "the task was dropped with a per-request runtime");
}

#[test]
fn a_panicking_handler_panics_the_caller() {
    let app = AsyncAppWithState::new(()).get("/boom", |_, _, _, _| async { panic!("boom") });
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.execute(&get("/boom"), &conn())));
    assert!(caught.is_err());
}

// ── A DbPool across requests on the sync path ─────────────────────────────────

#[cfg(feature = "model-sqlite")]
mod db_pool {
    use super::*;
    use crate::model::{DbPool, Value};
    use std::sync::atomic::AtomicUsize;

    fn notes_app(pool: DbPool) -> AsyncAppWithState<DbPool> {
        AsyncAppWithState::new(pool)
            .post("/setup", |_, _, _, pool| async move {
                match pool.execute("CREATE TABLE notes (text TEXT NOT NULL)", &[]).await {
                    Ok(_) => ok_text("created"),
                    Err(e) => ok_text(&format!("error: {e}")),
                }
            })
            .post("/notes", |req, _, _, pool| async move {
                let text = String::from_utf8_lossy(&req.body).to_string();
                match pool.execute("INSERT INTO notes (text) VALUES (?)", &[Value::Text(text)]).await {
                    Ok(n) => ok_text(&n.to_string()),
                    Err(e) => ok_text(&format!("error: {e}")),
                }
            })
            .get("/notes/count", |_, _, _, pool| async move {
                match pool.query_rows("SELECT COUNT(*) AS n FROM notes", &[]).await {
                    Ok(rows) => ok_text(&rows[0].get::<i64>("n").unwrap().to_string()),
                    Err(e) => ok_text(&format!("error: {e}")),
                }
            })
    }

    /// Regression: each sync `execute` ran the handler on a runtime made for
    /// the call, so sqlx's task giving the connection back never ran, the
    /// connection was closed — and `DbPool::memory()`'s database with it.
    #[test]
    fn an_in_memory_sqlite_pool_keeps_its_table_across_sync_requests() {
        // Made on the app's startup runtime, which outlives the requests.
        let startup = tokio::runtime::Runtime::new().unwrap();
        let pool = startup.block_on(DbPool::memory()).unwrap();
        let app = notes_app(pool);
        // From a plain thread, as the HTTP/1.1-only build and `TestClient::send` call it.
        assert_eq!("created", body_text(&app.execute(&post("/setup", b""), &conn()).unwrap()));
        assert_eq!("1", body_text(&app.execute(&post("/notes", b"one"), &conn()).unwrap()));
        // From tokio's blocking pool, as the async servers call it behind a middleware layer.
        let blocking = {
            let app = app.clone();
            startup.block_on(startup.spawn_blocking(move || app.execute(&post("/notes", b"two"), &conn()))).unwrap().unwrap()
        };
        assert_eq!("1", body_text(&blocking));
        assert_eq!("2", body_text(&app.execute(&get("/notes/count"), &conn()).unwrap()));
    }

    #[test]
    fn a_pooled_connection_is_reused_across_sync_requests() {
        let opened = Arc::new(AtomicUsize::new(0));
        let startup = tokio::runtime::Runtime::new().unwrap();
        let counter = Arc::clone(&opened);
        let sqlite = startup
            .block_on(
                sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
                    .max_connections(4)
                    .after_connect(move |_, _| {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Box::pin(async { Ok(()) })
                    })
                    .connect("sqlite::memory:"),
            )
            .unwrap();
        // sqlx gives a connection back from a task, after the handler has
        // returned: wait for it, or the next request opens a second one.
        let back_in_the_pool = || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while sqlite.num_idle() == 0 && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            (sqlite.size(), sqlite.num_idle())
        };
        assert_eq!((1, 1), back_in_the_pool(), "connect() checked one connection");
        let app = notes_app(DbPool::Sqlite(sqlite.clone()));
        assert_eq!("created", body_text(&app.execute(&post("/setup", b""), &conn()).unwrap()));
        for _ in 0..5 {
            assert_eq!((1, 1), back_in_the_pool());
            assert_eq!("1", body_text(&app.execute(&post("/notes", b"n"), &conn()).unwrap()));
        }
        assert_eq!((1, 1), back_in_the_pool());
        assert_eq!("5", body_text(&app.execute(&get("/notes/count"), &conn()).unwrap()));
        assert_eq!((1, 1), back_in_the_pool());
        assert_eq!(1, opened.load(Ordering::SeqCst), "one connection for every request");
    }

    /// Passes everything on, but isn't `is_passthrough`: the servers then
    /// call `execute` on the blocking pool, as behind most real layers.
    struct Synchronous;

    impl crate::middleware::Middleware for Synchronous {
        fn handle(&self, request: &Request, connection: &ConnectionInfo, next: &dyn Application) -> Result<Response, String> {
            next.execute(request, connection)
        }
    }

    async fn through_the_server(app: impl Application + Send + Clone + 'static) -> Vec<String> {
        let mut bodies = Vec::new();
        for request in [post("/setup", b""), post("/notes", b"a"), post("/notes", b"b"), get("/notes/count")] {
            let (_, result) = crate::server::http1_async::execute_request(app.clone(), request, &conn(), None).await.unwrap();
            bodies.push(body_text(&result.unwrap()));
        }
        bodies
    }

    /// What the async servers run per request: `execute_async` awaited on
    /// the connection's runtime, or `execute` on the blocking pool behind a
    /// synchronous layer — under the shared (`multi_thread`) runtime and a
    /// per-core (`current_thread`) one.
    #[test]
    fn the_servers_dispatch_keeps_the_pools_connection() {
        let expected = ["created", "1", "1", "2"];
        for multi_thread in [true, false] {
            let runtime = if multi_thread {
                tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap()
            } else {
                tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
            };
            let pool = runtime.block_on(DbPool::memory()).unwrap();
            assert_eq!(expected, *runtime.block_on(through_the_server(notes_app(pool))), "multi_thread: {multi_thread}");
            let pool = runtime.block_on(DbPool::memory()).unwrap();
            let wrapped = crate::middleware::WithMiddleware::new(notes_app(pool)).wrap(Synchronous);
            assert_eq!(expected, *runtime.block_on(through_the_server(wrapped)), "multi_thread: {multi_thread}, behind a layer");
        }
    }

    /// A `#[tokio::test]`'s pool, the async servers' `execute_async` path.
    #[tokio::test]
    async fn a_pool_made_on_a_current_thread_runtime_through_execute_async() {
        let pool = DbPool::memory().await.unwrap();
        let app = notes_app(pool.clone());
        for (request, expected) in [(post("/setup", b""), "created"), (post("/notes", b"a"), "1"), (get("/notes/count"), "1")] {
            let response = app.execute_async(&request, &conn()).unwrap().await.unwrap();
            assert_eq!(expected, body_text(&response));
        }
        assert_eq!(1, pool.query_rows("SELECT * FROM notes", &[]).await.unwrap().len());
    }
}
