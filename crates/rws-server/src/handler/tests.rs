use std::collections::HashMap;
use std::sync::Arc;

use super::path::{parse_param, path_fits};
use super::*;
use crate::router::PathParams;
use crate::server::Address;

fn params(pairs: &[(&str, &str)]) -> PathParams {
    PathParams::from_map(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_, _>>())
}

fn conn() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: Address { ip: "127.0.0.1".to_string(), port: 2 },
        request_size: 16000,
        sni_hostname: None,
        tls: false,
    }
}

fn request(method: &str, uri: &str, headers: Vec<Header>, body: &[u8]) -> Request {
    Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers,
        body: body.to_vec().into(),
    }
}

/// Run `T`'s extractor on a request with these route parameters.
fn extract<T: FromRequestParts<u32>>(req: &Request, path: &PathParams, names: &'static [&'static str]) -> Result<T, Response> {
    let state = Arc::new(7u32);
    let connection = conn();
    T::from_request_parts(&RequestParts { request: req, params: path, connection: &connection, state: &state, path_names: names })
}

// ── Path ─────────────────────────────────────────────────────────────────────

#[test]
fn path_parses_a_scalar() {
    let req = request("GET", "/posts/42", vec![], b"");
    let Path(id) = extract::<Path<u64>>(&req, &params(&[("id", "42")]), &["id"]).unwrap();
    assert_eq!(42, id);
}

#[test]
fn path_rejects_a_value_that_does_not_parse_with_400() {
    let req = request("GET", "/posts/abc", vec![], b"");
    let r = extract::<Path<u64>>(&req, &params(&[("id", "abc")]), &["id"]).unwrap_err();
    assert_eq!(400, r.status_code);
    let body = String::from_utf8(r.content_range_list[0].body.clone()).unwrap();
    assert!(body.contains("path parameter `id`"), "{body}");
}

#[test]
fn path_tuples_take_the_names_in_order() {
    let req = request("GET", "/", vec![], b"");
    let p = params(&[("id", "7"), ("slug", "hello")]);
    let Path((slug, id)) = extract::<Path<(String, u32)>>(&req, &p, &["slug", "id"]).unwrap();
    assert_eq!(("hello".to_string(), 7), (slug, id));
    // The wrong number of names is an error, not a panic.
    assert!(<(u32, u32)>::from_path(&p, &["id"]).is_err());
}

#[test]
fn path_values_are_percent_decoded() {
    let p = params(&[("tag", "a%20b%2Fc+d"), ("bad", "%zz")]);
    assert_eq!("a b/c+d", parse_param::<String>(&p, "tag").unwrap());
    assert_eq!("%zz", parse_param::<String>(&p, "bad").unwrap());
    assert!(parse_param::<String>(&p, "missing").unwrap_err().contains("missing"));
}

struct Key;

impl FromPath for Key {
    const ARITY: usize = 2;
    const NAMES: &'static [&'static str] = &["id", "slug"];
    fn from_path(_: &PathParams, _: &[&str]) -> Result<Self, String> {
        Ok(Key)
    }
}

#[test]
fn path_fits_compares_arity_or_names() {
    assert!(path_fits::<u64>(&["id"]));
    assert!(!path_fits::<u64>(&["id", "slug"]));
    assert!(path_fits::<(u64, String)>(&["a", "b"]));
    assert!(path_fits::<Key>(&["slug", "id"]));
    assert!(!path_fits::<Key>(&["id", "name"]));
    assert!(!path_fits::<Key>(&["id"]));
    // Evaluated at compile time by the macros:
    const OK: bool = path_fits::<Key>(&["id", "slug"]);
    assert!(OK);
}

// ── State and the FromRequest blanket ────────────────────────────────────────

#[test]
fn state_is_a_clone_of_the_arc() {
    let req = request("GET", "/", vec![], b"");
    let State(n) = extract::<State<u32>>(&req, &params(&[]), &[]).unwrap();
    assert_eq!(7, *n);
}

#[test]
fn from_request_types_are_arguments() {
    let req = request("POST", "/", vec![], b"\xff");
    let Body(bytes) = extract::<Body>(&req, &params(&[]), &[]).unwrap();
    assert_eq!(vec![0xff], bytes);
    assert_eq!(400, extract::<BodyText>(&req, &params(&[]), &[]).unwrap_err().status_code);
}

// ── Query and Form ───────────────────────────────────────────────────────────

#[cfg(feature = "serde")]
mod urlencoded {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "lowercase")]
    enum Sort {
        New,
        Top,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Search {
        q: String,
        page: Option<u32>,
        limit: Option<u32>,
        #[serde(default)]
        tag: Vec<String>,
        sort: Sort,
        exact: bool,
    }

    #[test]
    fn query_deserializes_into_a_struct() {
        let req = request("GET", "/s?q=rust+web&page=2&limit=&tag=a&tag=b%26c&sort=top&exact=true#frag", vec![], b"");
        let Query(s) = extract::<Query<Search>>(&req, &params(&[]), &[]).unwrap();
        assert_eq!(
            Search { q: "rust web".into(), page: Some(2), limit: None, tag: vec!["a".into(), "b&c".into()], sort: Sort::Top, exact: true },
            s
        );
    }

    #[test]
    fn query_that_does_not_fit_is_400() {
        for uri in ["/s?q=x&sort=new", "/s?q=x&sort=old&exact=true", "/s?q=x&page=two&sort=new&exact=true", "/s"] {
            let req = request("GET", uri, vec![], b"");
            let r = extract::<Query<Search>>(&req, &params(&[]), &[]).unwrap_err();
            assert_eq!(400, r.status_code, "{uri}");
        }
        // An untyped map still works through the typed extractor.
        let req = request("GET", "/s?a=1", vec![], b"");
        let Query(m) = extract::<Query<HashMap<String, String>>>(&req, &params(&[]), &[]).unwrap();
        assert_eq!(Some(&"1".to_string()), m.get("a"));
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Signup {
        email: String,
        age: u8,
    }

    fn form_request(content_type: &str, body: &[u8]) -> Request {
        request("POST", "/signup", vec![Header::new("Content-Type", content_type.to_string())], body)
    }

    #[test]
    fn form_deserializes_an_urlencoded_body() {
        let req = form_request("application/x-www-form-urlencoded; charset=utf-8", b"email=a%40b.c&age=30");
        let Form(s) = extract::<Form<Signup>>(&req, &params(&[]), &[]).unwrap();
        assert_eq!(Signup { email: "a@b.c".into(), age: 30 }, s);
    }

    #[test]
    fn form_rejections() {
        let wrong_type = form_request("application/json", b"{}");
        assert_eq!(415, extract::<Form<Signup>>(&wrong_type, &params(&[]), &[]).unwrap_err().status_code);
        let no_type = request("POST", "/", vec![], b"email=x&age=1");
        assert_eq!(415, extract::<Form<Signup>>(&no_type, &params(&[]), &[]).unwrap_err().status_code);
        let not_utf8 = form_request("application/x-www-form-urlencoded", b"email=\xff");
        assert_eq!(400, extract::<Form<Signup>>(&not_utf8, &params(&[]), &[]).unwrap_err().status_code);
        let out_of_range = form_request("application/x-www-form-urlencoded", b"email=x&age=300");
        assert_eq!(422, extract::<Form<Signup>>(&out_of_range, &params(&[]), &[]).unwrap_err().status_code);
        let missing = form_request("application/x-www-form-urlencoded", b"age=3");
        assert_eq!(422, extract::<Form<Signup>>(&missing, &params(&[]), &[]).unwrap_err().status_code);
    }
}

// ── Typed handlers generated by the route macros ─────────────────────────────

#[cfg(feature = "macros")]
mod macros {
    use rws_macros::{get, post, route};

    use crate::app::App;
    use crate::error::AppError;
    use crate::handler::{Path, RouteSpec, State};
    use crate::request::Request;
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
    use crate::router::{PathParams, Router};
    use crate::server::ConnectionInfo;
    use crate::server_config::ServerConfig;
    use crate::test_client::TestClient;

    struct Db {
        greeting: String,
    }

    /// Shows a post.
    #[get("/posts/{id}")]
    fn show_post(Path(id): Path<u64>, State(db): State<Db>) -> Result<String, AppError> {
        if id == 0 {
            return Err(AppError::NotFound("no post 0".to_string()));
        }
        Ok(format!("{} post {id}", db.greeting))
    }

    #[get("/posts/:id/comments/{n}")]
    fn comment(Path((n, id)): Path<(u8, u64)>, db: &Db, req: &Request) -> String {
        format!("{} {id}/{n} {}", db.greeting, req.method)
    }

    #[route(PUT, "/raw/:id")]
    fn raw(params: &PathParams, conn: &ConnectionInfo) -> Response {
        let body = format!("{} {}", params.get("id").unwrap_or(""), conn.client.port);
        Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &body)
    }

    #[get("/files/{*rest}")]
    fn file(p: Path<String>) -> String {
        p.0
    }

    #[derive(crate::FromPath)]
    struct PostKey {
        id: u64,
        slug: String,
    }

    #[get("/blog/{slug}/{id}")]
    fn blog(Path(key): Path<PostKey>) -> String {
        format!("{}:{}", key.id, key.slug)
    }

    /// A classic handler: the attribute leaves it a function.
    #[get("/classic")]
    fn classic(_: &Request, _: &PathParams, _: &ConnectionInfo, db: &Db) -> Response {
        Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &db.greeting)
    }

    #[post("/stateless")]
    fn stateless(body: crate::extract::BodyText) -> (&'static crate::response::StatusCodeReasonPhrase, String) {
        (STATUS_CODE_REASON_PHRASE.n201_created, body.0)
    }

    fn app() -> crate::state::AppWithState<Db> {
        App::with_state(Db { greeting: "hi".to_string() })
            .with_config(ServerConfig::default())
            .route(show_post)
            .route(comment)
            .route(raw)
            .route(file)
            .route(blog)
            .route(stateless)
            .get("/classic", classic)
    }

    #[test]
    fn route_spec_comes_from_the_attribute() {
        assert_eq!(("GET", "/posts/{id}"), (show_post::METHOD, show_post::PATTERN));
        assert_eq!(("PUT", "/raw/:id"), (raw::METHOD, raw::PATTERN));
    }

    #[test]
    fn typed_handlers_answer_through_their_extractors() {
        let client = TestClient::new(app());
        let r = client.get("/posts/5").send();
        assert_eq!((200, "hi post 5"), (r.status(), r.body_text()));
        let r = client.get("/posts/0").send();
        assert_eq!(404, r.status());
        let r = client.get("/posts/5/comments/3").send();
        assert_eq!("hi 5/3 GET", r.body_text());
        let r = client.put("/raw/x").send();
        assert_eq!("x 12345", r.body_text());
        let r = client.get("/files/a/b%20c").send();
        assert_eq!("a/b c", r.body_text());
        let r = client.get("/blog/hello/9").send();
        assert_eq!("9:hello", r.body_text());
        let r = client.post("/stateless").body_text("made").send();
        assert_eq!((201, "made"), (r.status(), r.body_text()));
        assert_eq!("hi", client.get("/classic").send().body_text());
    }

    #[test]
    fn extraction_failures_answer_with_the_rejection() {
        let client = TestClient::new(app());
        assert_eq!(400, client.get("/posts/abc").send().status());
        // u8 overflow in the tuple
        assert_eq!(400, client.get("/posts/1/comments/300").send().status());
        assert_eq!(400, client.post("/stateless").body_bytes(vec![0xff]).send().status());
    }

    #[test]
    fn the_function_stays_callable() {
        let db = std::sync::Arc::new(Db { greeting: "yo".to_string() });
        assert_eq!(Ok("yo post 3".to_string()), show_post::call(Path(3), State(db)));
    }

    #[test]
    fn stateless_handlers_work_on_a_router() {
        let router = Router::new().route(file).route(stateless);
        let client = TestClient::new(RouterApp(router));
        assert_eq!("x/y", client.get("/files/x/y").send().body_text());
        assert_eq!(201, client.post("/stateless").body_text("a").send().status());
    }

    struct RouterApp(Router);

    impl crate::application::Application for RouterApp {
        fn execute(&self, request: &Request, connection: &ConnectionInfo) -> Result<Response, String> {
            Ok(self.0.handle(request, connection).unwrap_or_else(Response::empty))
        }
    }

    #[cfg(feature = "serde")]
    mod serde_extractors {
        use serde::{Deserialize, Serialize};

        use super::*;
        use crate::handler::{Form, Json, Query};

        #[derive(Deserialize)]
        struct Page {
            page: u32,
        }

        #[derive(Deserialize, Serialize)]
        struct Post {
            title: String,
        }

        #[get("/list")]
        fn list(Query(p): Query<Page>) -> Json<Vec<u32>> {
            Json(vec![p.page])
        }

        #[post("/posts")]
        fn create(Json(post): Json<Post>, State(db): State<Db>) -> Json<Post> {
            Json(Post { title: format!("{} {}", db.greeting, post.title) })
        }

        #[post("/signup")]
        fn signup(Form(p): Form<Page>) -> String {
            p.page.to_string()
        }

        #[test]
        fn json_query_and_form_arguments() {
            let client = TestClient::new(
                App::with_state(Db { greeting: "re:".to_string() })
                    .with_config(ServerConfig::default())
                    .route(list)
                    .route(create)
                    .route(signup),
            );
            assert_eq!("[4]", client.get("/list?page=4").send().body_text());
            assert_eq!(400, client.get("/list?page=x").send().status());
            let r = client.post("/posts").body_json(r#"{"title":"a"}"#).send();
            assert_eq!(r#"{"title":"re: a"}"#, r.body_text());
            assert_eq!(400, client.post("/posts").body_json("{").send().status());
            let r = client
                .post("/signup")
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body_text("page=8")
                .send();
            assert_eq!("8", r.body_text());
            assert_eq!(415, client.post("/signup").body_text("page=8").send().status());
        }
    }

    #[cfg(feature = "http2")]
    mod async_handlers {
        use super::*;
        use crate::async_state::AsyncAppWithState;

        #[get("/a/{id}")]
        async fn async_show(Path(id): Path<u32>, db: &Db, req: &Request) -> String {
            async { () }.await;
            format!("{} {id} {}", db.greeting, req.request_uri)
        }

        #[test]
        fn async_and_sync_typed_handlers_on_the_async_app() {
            let app = AsyncAppWithState::new(Db { greeting: "hey".to_string() })
                .with_config(ServerConfig::default())
                .route(async_show)
                .route(show_post);
            let client = TestClient::new(app);
            assert_eq!("hey 3 /a/3", client.get("/a/3").send().body_text());
            assert_eq!(400, client.get("/a/x").send().status());
            assert_eq!("hey post 2", client.get("/posts/2").send().body_text());
        }
    }
}
