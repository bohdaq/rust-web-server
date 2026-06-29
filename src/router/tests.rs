use crate::core::New;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::router::Router;
use crate::server::{Address, ConnectionInfo};

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 9000 },
        server: Address { ip: "127.0.0.1".to_string(), port: 7878 },
        request_size: 4096,
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

fn post(uri: &str) -> Request {
    Request {
        method: METHOD.post.to_string(),
        request_uri: uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

fn ok_response(body: &str) -> Response {
    let mut r = Response::new();
    r.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
    r.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();
    r.content_range_list = vec![
        Range::get_content_range(body.as_bytes().to_vec(), MimeType::TEXT_PLAIN.to_string())
    ];
    r
}

#[test]
fn literal_route_matches() {
    let router = Router::new().get("/hello", |_req, _params, _conn| ok_response("hi"));
    let res = router.handle(&get("/hello"), &conn());
    assert!(res.is_some());
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n200_ok.status_code, res.unwrap().status_code);
}

#[test]
fn literal_route_no_match_different_path() {
    let router = Router::new().get("/hello", |_req, _params, _conn| ok_response("hi"));
    assert!(router.handle(&get("/world"), &conn()).is_none());
}

#[test]
fn method_mismatch_returns_none() {
    let router = Router::new().get("/hello", |_req, _params, _conn| ok_response("hi"));
    assert!(router.handle(&post("/hello"), &conn()).is_none());
}

#[test]
fn path_param_is_extracted() {
    let router = Router::new().get("/users/:id", |_req, params, _conn| {
        let id = params.get("id").unwrap_or("missing").to_string();
        ok_response(&id)
    });
    let res = router.handle(&get("/users/42"), &conn()).unwrap();
    let body = String::from_utf8(res.content_range_list[0].body.clone()).unwrap();
    assert_eq!("42", body);
}

#[test]
fn multiple_path_params_are_extracted() {
    let router = Router::new().get("/users/:user_id/posts/:post_id", |_req, params, _conn| {
        let uid = params.get("user_id").unwrap_or("").to_string();
        let pid = params.get("post_id").unwrap_or("").to_string();
        ok_response(&format!("{}-{}", uid, pid))
    });
    let res = router.handle(&get("/users/7/posts/99"), &conn()).unwrap();
    let body = String::from_utf8(res.content_range_list[0].body.clone()).unwrap();
    assert_eq!("7-99", body);
}

#[test]
fn wildcard_captures_remainder() {
    let router = Router::new().get("/files/*path", |_req, params, _conn| {
        let p = params.get("path").unwrap_or("").to_string();
        ok_response(&p)
    });
    let res = router.handle(&get("/files/a/b/c.txt"), &conn()).unwrap();
    let body = String::from_utf8(res.content_range_list[0].body.clone()).unwrap();
    assert_eq!("a/b/c.txt", body);
}

#[test]
fn root_route_matches_slash() {
    let router = Router::new().get("/", |_req, _params, _conn| ok_response("root"));
    let res = router.handle(&get("/"), &conn());
    assert!(res.is_some());
}

#[test]
fn query_string_is_ignored_during_matching() {
    let router = Router::new().get("/search", |_req, _params, _conn| ok_response("found"));
    let res = router.handle(&get("/search?q=rust&page=2"), &conn());
    assert!(res.is_some());
}

#[test]
fn first_matching_route_wins() {
    let router = Router::new()
        .get("/users/:id", |_req, _params, _conn| ok_response("first"))
        .get("/users/:id", |_req, _params, _conn| ok_response("second"));
    let res = router.handle(&get("/users/1"), &conn()).unwrap();
    let body = String::from_utf8(res.content_range_list[0].body.clone()).unwrap();
    assert_eq!("first", body);
}

#[test]
fn post_route_registered_with_post_method() {
    let router = Router::new().post("/items", |_req, _params, _conn| ok_response("created"));
    assert!(router.handle(&post("/items"), &conn()).is_some());
    assert!(router.handle(&get("/items"), &conn()).is_none());
}

#[test]
fn no_routes_returns_none() {
    let router = Router::new();
    assert!(router.handle(&get("/anything"), &conn()).is_none());
}

#[test]
fn partial_path_does_not_match() {
    let router = Router::new().get("/users/:id/profile", |_req, _params, _conn| ok_response("ok"));
    assert!(router.handle(&get("/users/42"), &conn()).is_none());
}
