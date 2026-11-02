use crate::app::App;
use crate::core::New;
use crate::response::STATUS_CODE_REASON_PHRASE;
use crate::test_client::TestClient;

fn client() -> TestClient<App> {
    TestClient::new(App::new())
}

#[test]
fn get_healthz_returns_200() {
    let res = client().get("/healthz").send();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n200_ok.status_code, res.status());
    assert!(res.is_success());
}

#[test]
fn get_metrics_returns_200() {
    let res = client().get("/metrics").send();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n200_ok.status_code, res.status());
}

#[test]
fn get_unknown_path_returns_404() {
    // Goes through App's built-in StaticResourceController, whose is_matching
    // now depends on RWS_CONFIG_SPA_FALLBACK.
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");

    let res = client().get("/does-not-exist-xyzzy").send();
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n404_not_found.status_code, res.status());
}

#[test]
fn body_text_is_accessible() {
    let res = client().get("/healthz").send();
    assert_eq!("OK", res.body_text());
}

#[test]
fn header_is_accessible_case_insensitive() {
    let res = client().get("/healthz").send();
    assert!(res.header("content-type").is_some());
    assert!(res.header("Content-Type").is_some());
}

#[test]
fn request_header_is_forwarded() {
    let res = client()
        .get("/healthz")
        .header("Accept", "text/plain")
        .send();
    assert!(res.is_success());
}

#[test]
fn post_with_body_text() {
    let res = client()
        .post("/healthz")
        .body_text("hello")
        .send();
    // /healthz only handles GET; POST falls through to 404
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n404_not_found.status_code, res.status());
}

#[test]
fn is_success_false_for_4xx() {
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");

    let res = client().get("/does-not-exist-xyzzy").send();
    assert!(!res.is_success());
}

#[test]
fn body_bytes_matches_body_text() {
    let res = client().get("/healthz").send();
    assert_eq!(res.body_text().as_bytes(), res.body_bytes());
}
