use super::*;
use crate::app::App;
use crate::test_client::TestClient;

fn req(headers: Vec<Header>) -> Request {
    Request { method: "GET".into(), request_uri: "/".into(), http_version: "HTTP/1.1".into(), headers, body: Vec::new().into() }
}

fn flash_cookie(response: &crate::test_client::TestResponse) -> Option<String> {
    response
        .headers()
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("set-cookie") && h.value.starts_with("rws_flash="))
        .map(|h| h.value.to_string())
}

#[test]
fn messages_round_trip_through_the_cookie() {
    let messages = vec![
        FlashMessage { kind: "notice".into(), text: "Saved; 100% done & ✓".into() },
        FlashMessage { kind: "alert".into(), text: "a=b".into() },
    ];
    assert_eq!(messages, decode_messages(&encode_messages(&messages)));
}

#[test]
fn bad_cookie_values_are_skipped() {
    assert!(decode_messages("notice=%ZZ&broken&alert=%E2%9C").is_empty());
    assert_eq!(vec![FlashMessage { kind: "notice".into(), text: "ok".into() }], decode_messages("notice=ok&x=%G1"));
}

#[test]
fn oversized_flash_is_cut() {
    let long = "x".repeat(MAX_COOKIE_BYTES);
    let messages = vec![FlashMessage { kind: "notice".into(), text: "short".into() }, FlashMessage { kind: "alert".into(), text: long }];
    assert_eq!("notice=short", encode_messages(&messages));
}

#[test]
fn redirect_sets_location_status_and_flash() {
    let response = Redirect::to("/posts\r\nX-Evil: 1").notice("Saved").alert("Careful").into_response();
    assert_eq!(303, response.status_code);
    assert_eq!("/postsX-Evil: 1", &*response.header("location").unwrap().value);
    let cookies: Vec<_> = response.headers.iter().filter(|h| h.name.eq_ignore_ascii_case("set-cookie")).collect();
    assert_eq!(1, cookies.len());
    assert!(cookies[0].value.starts_with("rws_flash=notice=Saved&alert=Careful;"), "{}", cookies[0].value);
    assert!(cookies[0].value.contains("HttpOnly"));
    assert_eq!(302, Redirect::found("/").into_response().status_code);
    assert_eq!(301, Redirect::permanent("/").into_response().status_code);
    crate::router::name_route("flash_test_post", "/fposts/{id}");
    assert_eq!("/fposts/2", Redirect::to_route("flash_test_post", &[("id", "2")]).location());
    assert_eq!("/", Redirect::to_route("flash_no_route", &[]).location());
}

#[test]
fn flash_is_shown_once_then_cleared() {
    let app = App::with_state(())
        .post("/save", |_, _, _, _| Redirect::to("/list").notice("Saved").into_response())
        .get("/list", |request, _, _, _| Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, Flash::read(request).notice().unwrap_or("-")))
        .post("/again", |_, _, _, _| Redirect::to("/list").alert("Again").into_response())
        .wrap(FlashLayer::new());
    let client = TestClient::new(app);

    let saved = client.post("/save").send();
    let cookie = flash_cookie(&saved).unwrap();
    let value = cookie.split(';').next().unwrap().to_string();

    let shown = client.get("/list").header("Cookie", &value).send();
    assert_eq!("Saved", shown.body_text());
    let cleared = flash_cookie(&shown).unwrap();
    assert!(cleared.contains("Max-Age=0"), "{cleared}");

    // A response that sets a new flash keeps it.
    let again = client.post("/again").header("Cookie", &value).send();
    assert!(flash_cookie(&again).unwrap().starts_with("rws_flash=alert=Again"));

    // No flash, no cookie.
    let plain = client.get("/list").send();
    assert_eq!("-", plain.body_text());
    assert!(flash_cookie(&plain).is_none());
}

#[test]
fn flash_is_a_handler_argument() {
    let request = req(vec![Header::new("Cookie", "a=1; rws_flash=notice=Hi")]);
    let flash = Flash::from_request(&request).unwrap();
    assert_eq!(Some("Hi"), flash.notice());
    assert_eq!(None, flash.alert());
    assert_eq!(1, flash.messages().len());
    assert!(Flash::from_request(&req(vec![])).unwrap().is_empty());
}

#[test]
fn set_appends_to_an_existing_flash() {
    let mut response = Response::get_response(STATUS_CODE_REASON_PHRASE.n200_ok, None, None);
    set(&mut response, "notice", "one");
    set(&mut response, "notice", "two");
    let cookies: Vec<_> = response.headers.iter().filter(|h| h.name.eq_ignore_ascii_case("set-cookie")).collect();
    assert_eq!(1, cookies.len());
    assert!(cookies[0].value.starts_with("rws_flash=notice=one&notice=two;"));
}
