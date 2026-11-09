use super::*;
use crate::app::App;
use crate::test_client::TestClient;

fn boom(_: &Request, _: &crate::router::PathParams, _: &ConnectionInfo, _: &()) -> Response {
    panic!("the cache is <empty>")
}

struct Failing;

impl Application for Failing {
    fn execute(&self, _: &Request, _: &ConnectionInfo) -> Result<Response, String> {
        Err("the upstream said no".into())
    }
}

#[test]
fn details_show_the_panic_in_html_and_json() {
    let app = App::with_state(())
        .get("/boom", boom)
        .get("/ok", |_, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, "fine"))
        .wrap(ErrorPages::detailed(true));
    let client = TestClient::new(app);
    assert_eq!("fine", client.get("/ok").send().body_text());

    let html = client.get("/boom?x=1").header("Accept", "text/html").header("Cookie", "session=s3cret").send();
    assert_eq!(500, html.status());
    let body = html.body_text();
    assert!(body.contains("<h1>the cache is &lt;empty&gt;</h1>"), "{body}");
    assert!(body.contains("error_pages/tests.rs:"), "the location: {body}");
    assert!(body.contains("GET /boom?x=1"));
    assert!(body.contains("(hidden)") && !body.contains("s3cret"), "secrets stay hidden");
    assert_eq!(Some("no-store"), html.header("cache-control"));

    let json = client.get("/boom").header("Accept", "application/json").send();
    assert_eq!(Some("application/problem+json"), json.header("content-type"));
    let problem: serde_json::Value = serde_json::from_slice(json.body_bytes()).unwrap();
    assert_eq!("the cache is <empty>", problem["detail"]);
    assert!(problem["location"].as_str().unwrap().contains("error_pages/tests.rs"));
    assert!(problem["backtrace"].is_array());
}

#[test]
fn without_details_the_client_gets_a_bare_500() {
    let app = App::with_state(()).get("/boom", boom).wrap(ErrorPages::detailed(false));
    let response = TestClient::new(app).get("/boom").header("Accept", "text/html").send();
    assert_eq!(500, response.status());
    assert!(!response.body_text().contains("cache"), "{}", response.body_text());
}

#[test]
fn app_errors_become_pages_too() {
    let detailed = crate::middleware::WithMiddleware::new(Failing).wrap(ErrorPages::detailed(true));
    let response = TestClient::new(detailed).get("/").send();
    let problem: serde_json::Value = serde_json::from_slice(response.body_bytes()).unwrap();
    assert_eq!((500, "the upstream said no"), (response.status(), problem["detail"].as_str().unwrap()));
    let bare = crate::middleware::WithMiddleware::new(Failing).wrap(ErrorPages::detailed(false));
    assert!(!TestClient::new(bare).get("/").send().body_text().contains("upstream"));
}

#[test]
fn passthrough_only_without_details() {
    let conn = ConnectionInfo { client: crate::server::Address { ip: "127.0.0.1".into(), port: 1 }, server: crate::server::Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 1, sni_hostname: None, tls: false };
    let request = Request { method: "GET".into(), request_uri: "/".into(), http_version: "HTTP/1.1".into(), headers: vec![], body: Vec::new().into() };
    assert!(ErrorPages::detailed(false).is_passthrough(&request, &conn));
    assert!(!ErrorPages::detailed(true).is_passthrough(&request, &conn));
    assert!(ErrorPages::detailed(true).shows_details());
}

#[test]
fn json_strings_are_escaped() {
    let json = serde_free_json("a \"q\"\n\\ \u{1}", "f.rs:1:2", &["frame 0", "x\ty"]);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!("a \"q\"\n\\ \u{1}", v["detail"]);
    assert_eq!("x\ty", v["backtrace"][1]);
}
