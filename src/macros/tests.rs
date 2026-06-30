use crate::app::App;
use crate::core::New;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::router::PathParams;
use crate::request::Request;
use crate::server::ConnectionInfo;
use crate::test_client::TestClient;
use crate::routes;

// ── helpers ───────────────────────────────────────────────────────────────────

struct Counter;

fn ok() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r
}

fn created() -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n201_created.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n201_created.reason_phrase.to_string();
    r
}

// AppWithState<S> passes &S to the handler, not &Arc<S>.

fn list(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &Counter) -> Response {
    ok()
}

fn create(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &Counter) -> Response {
    created()
}

fn update(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &Counter) -> Response {
    ok()
}

fn remove(_: &Request, _: &PathParams, _: &ConnectionInfo, _: &Counter) -> Response {
    ok()
}

fn by_id(_: &Request, params: &PathParams, _: &ConnectionInfo, _: &Counter) -> Response {
    let _id = params.get("id").unwrap_or("?");
    ok()
}

// ── routes! basic expansion ───────────────────────────────────────────────────

#[test]
fn single_get_route() {
    let app = routes! { App::with_state(Counter), GET "/items" => list };
    assert_eq!(200, TestClient::new(app).get("/items").send().status());
}

#[test]
fn single_post_route() {
    let app = routes! { App::with_state(Counter), POST "/items" => create };
    assert_eq!(201, TestClient::new(app).post("/items").send().status());
}

#[test]
fn multiple_methods_on_same_path() {
    let app = routes! {
        App::with_state(Counter),
        GET  "/items" => list,
        POST "/items" => create,
    };
    let client = TestClient::new(app);
    assert_eq!(200, client.get("/items").send().status());
    assert_eq!(201, client.post("/items").send().status());
}

#[test]
fn put_patch_delete_routes() {
    let app = routes! {
        App::with_state(Counter),
        PUT    "/item" => update,
        PATCH  "/item" => update,
        DELETE "/item" => remove,
    };
    let client = TestClient::new(app);
    assert_eq!(200, client.put("/item").send().status());
    assert_eq!(200, client.patch("/item").send().status());
    assert_eq!(200, client.delete("/item").send().status());
}

#[test]
fn path_parameters_work() {
    let app = routes! { App::with_state(Counter), GET "/items/:id" => by_id };
    assert_eq!(200, TestClient::new(app).get("/items/42").send().status());
}

#[test]
fn trailing_comma_accepted() {
    let app = routes! {
        App::with_state(Counter),
        GET  "/items"     => list,
        POST "/items"     => create,
        GET  "/items/:id" => by_id,
    };
    let client = TestClient::new(app);
    assert_eq!(200, client.get("/items").send().status());
    assert_eq!(200, client.get("/items/5").send().status());
}

#[test]
fn no_trailing_comma_accepted() {
    let app = routes! { App::with_state(Counter), GET "/ping" => list };
    assert_eq!(200, TestClient::new(app).get("/ping").send().status());
}

#[test]
fn inline_closure_handler() {
    let app = routes! {
        App::with_state(Counter),
        GET "/hello" => |_req, _params, _conn, _state: &Counter| ok(),
    };
    assert_eq!(200, TestClient::new(app).get("/hello").send().status());
}

#[test]
fn closure_captures_outer_variable() {
    let magic: u32 = 42;
    let app = routes! {
        App::with_state(Counter),
        GET "/magic" => move |_req, _params, _conn, _state: &Counter| { let _ = magic; ok() },
    };
    assert_eq!(200, TestClient::new(app).get("/magic").send().status());
}

#[test]
fn unmatched_route_falls_through_to_non_200() {
    let app = routes! { App::with_state(Counter), GET "/known" => list };
    let status = TestClient::new(app).get("/no-such-path-xyz").send().status();
    assert_ne!(200, status);
}

#[test]
fn many_routes_all_work() {
    let app = routes! {
        App::with_state(Counter),
        GET    "/a" => list,
        POST   "/a" => create,
        PUT    "/a" => update,
        PATCH  "/a" => update,
        DELETE "/a" => remove,
        GET    "/b" => list,
        POST   "/b" => create,
    };
    let client = TestClient::new(app);
    assert_eq!(200, client.get("/a").send().status());
    assert_eq!(201, client.post("/a").send().status());
    assert_eq!(200, client.get("/b").send().status());
}
