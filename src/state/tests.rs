use std::sync::Arc;

use crate::application::Application;
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
use crate::state::AppWithState;

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 16000,
    sni_hostname: None,
    }
}

fn get(uri: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn post(uri: &str, body: &[u8]) -> Request {
    Request {
        method: METHOD.post.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: body.to_vec(),
    }
}

fn ok_text(body: &str) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![Range::get_content_range(body.as_bytes().to_vec(), MimeType::TEXT_PLAIN.to_string())];
    r
}

struct State {
    value: String,
    counter: std::sync::atomic::AtomicU32,
}

#[test]
fn state_accessible_in_get_handler() {
    let app = AppWithState::new(State {
        value: "hello".to_string(),
        counter: std::sync::atomic::AtomicU32::new(0),
    })
    .get("/greet", |_req, _params, _conn, state| ok_text(&state.value));

    let resp = app.execute(&get("/greet"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hello", body);
}

#[test]
fn path_params_and_state_together() {
    let app = AppWithState::new("world".to_string())
        .get("/hello/:name", |_req, params, _conn, state| {
            let name = params.get("name").unwrap_or("?");
            ok_text(&format!("{}, {}!", state, name))
        });

    let resp = app.execute(&get("/hello/alice"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("world, alice!", body);
}

#[test]
fn post_handler_receives_state() {
    let app = AppWithState::new(42u32)
        .post("/echo", |req, _params, _conn, state| {
            let body = format!("state={} body={}", state, String::from_utf8_lossy(&req.body));
            ok_text(&body)
        });

    let resp = app.execute(&post("/echo", b"test"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("state=42 body=test", body);
}

#[test]
fn unmatched_request_falls_through_to_app() {
    let app = AppWithState::new(()).get("/custom", |_, _, _, _| ok_text("custom"));
    // /healthz is served by the built-in HealthController
    let resp = app.execute(&get("/healthz"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
}

#[test]
fn unmatched_request_returns_404() {
    let app = AppWithState::new(()).get("/custom", |_, _, _, _| ok_text("custom"));
    let resp = app.execute(&get("/does-not-exist-xyz"), &conn()).unwrap();
    assert_eq!(404, resp.status_code);
}

#[test]
fn first_matching_route_wins() {
    let app = AppWithState::new(())
        .get("/a", |_, _, _, _| ok_text("first"))
        .get("/a", |_, _, _, _| ok_text("second"));

    let resp = app.execute(&get("/a"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("first", body);
}

#[test]
fn wildcard_captures_remaining_path() {
    let app = AppWithState::new(())
        .get("/files/*path", |_, params, _, _| {
            ok_text(params.get("path").unwrap_or(""))
        });

    let resp = app.execute(&get("/files/a/b/c.txt"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("a/b/c.txt", body);
}

#[test]
fn state_accessor_returns_inner_value() {
    let app = AppWithState::new("sentinel".to_string());
    assert_eq!("sentinel", app.state());
}

#[test]
fn state_is_shared_across_concurrent_handlers() {
    use std::sync::atomic::Ordering;
    let app = std::sync::Arc::new(
        AppWithState::new(std::sync::atomic::AtomicU32::new(0))
            .get("/inc", |_, _, _, state| {
                state.fetch_add(1, Ordering::Relaxed);
                ok_text("ok")
            }),
    );

    let mut handles = vec![];
    for _ in 0..8 {
        let app = std::sync::Arc::clone(&app);
        let conn = conn();
        let req = get("/inc");
        handles.push(std::thread::spawn(move || {
            app.execute(&req, &conn).unwrap();
        }));
    }
    for h in handles { h.join().unwrap(); }

    assert_eq!(8, app.state().load(Ordering::Relaxed));
}

#[test]
fn put_patch_delete_are_registered() {
    let app = AppWithState::new(())
        .put("/r", |_, _, _, _| ok_text("put"))
        .patch("/r", |_, _, _, _| ok_text("patch"))
        .delete("/r", |_, _, _, _| ok_text("delete"));

    for (method, expected) in [("PUT", "put"), ("PATCH", "patch"), ("DELETE", "delete")] {
        let req = Request {
            method: method.to_string(),
            request_uri: "/r".to_string(),
            http_version: VERSION.http_1_1.to_string(),
            headers: vec![],
            body: vec![],
        };
        let resp = app.execute(&req, &conn()).unwrap();
        let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
        assert_eq!(expected, body, "method={}", method);
    }
}

// ── di::Container as AppWithState's state ───────────────────────────────────────
//
// `Container` needs no special-cased integration: it's `Send + Sync + 'static`
// like any other state type, so `AppWithState::new(container)` (not
// `container.into_arc()`, which would double-wrap in `Arc`) works directly.
// These tests exist so that claim is verified, not just documented.

#[test]
fn container_resolves_concrete_service_through_a_request() {
    struct Greeting(String);

    let mut container = Container::new();
    container.register(Greeting("hello from the container".to_string()));

    let app = AppWithState::new(container).get("/greet", |_req, _params, _conn, state| {
        let greeting = state.get::<Greeting>().unwrap();
        ok_text(&greeting.0)
    });

    let resp = app.execute(&get("/greet"), &conn()).unwrap();
    assert_eq!(200, resp.status_code);
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hello from the container", body);
}

#[test]
fn container_resolves_trait_object_service_through_a_request() {
    trait Greeter: Send + Sync {
        fn greet(&self) -> String;
    }
    struct EnglishGreeter;
    impl Greeter for EnglishGreeter {
        fn greet(&self) -> String {
            "hi".to_string()
        }
    }

    let mut container = Container::new();
    container.provide::<dyn Greeter>(Arc::new(EnglishGreeter));

    let app = AppWithState::new(container).get("/greet", |_req, _params, _conn, state| {
        ok_text(&state.get::<dyn Greeter>().unwrap().greet())
    });

    let resp = app.execute(&get("/greet"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hi", body);
}

#[test]
fn container_resolution_miss_does_not_panic_the_handler() {
    let app = AppWithState::new(Container::new()).get("/x", |_req, _params, _conn, state| {
        match state.get::<u32>() {
            Some(_) => ok_text("found"),
            None => ok_text("missing"),
        }
    });

    let resp = app.execute(&get("/x"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("missing", body);
}

// ── with_config: fallback App pinned to an explicit ServerConfig ───────────────
//
// No env writes here — App::with_config's whole point is to avoid them — so
// these run safely without test_env::lock(), unlike a test that touched
// RWS_CONFIG_* directly.

#[test]
fn with_config_pins_cors_denial_on_fallback_request() {
    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: String::new(), // no allowed origins -> CORS denied
        ..ServerConfig::default()
    };
    // No routes registered, so every request falls through to the built-in App.
    let app = AppWithState::new(()).with_config(config);

    let mut req = get("/does-not-exist");
    req.headers.push(Header {
        name: Header::_ORIGIN.to_string(),
        value: "https://evil.example.com".to_string(),
    });

    let resp = app.execute(&req, &conn()).unwrap();
    assert!(resp._get_header(Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).is_none());
}

#[test]
fn with_config_allows_cors_for_configured_origin_on_fallback_request() {
    let config = ServerConfig {
        cors_allow_all: false,
        cors_allow_origins: "https://trusted.example.com".to_string(),
        ..ServerConfig::default()
    };
    let app = AppWithState::new(()).with_config(config);

    let mut req = get("/does-not-exist");
    req.headers.push(Header {
        name: Header::_ORIGIN.to_string(),
        value: "https://trusted.example.com".to_string(),
    });

    let resp = app.execute(&req, &conn()).unwrap();
    let acao = resp._get_header(Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string()).unwrap();
    assert_eq!("https://trusted.example.com", acao.value);
}

#[test]
fn own_routes_still_take_priority_over_fallback_when_config_is_pinned() {
    let config = ServerConfig { cors_allow_all: false, ..ServerConfig::default() };
    let app = AppWithState::new(())
        .with_config(config)
        .get("/greet", |_req, _params, _conn, _state| ok_text("hi"));

    let resp = app.execute(&get("/greet"), &conn()).unwrap();
    let body = String::from_utf8(resp.content_range_list[0].body.clone()).unwrap();
    assert_eq!("hi", body);
}
