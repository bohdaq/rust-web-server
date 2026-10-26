use file_ext::FileExt;
use crate::app::controller::static_resource::StaticResourceController;
use crate::controller::Controller;
use crate::core::New;
use crate::header::Header;
use crate::http::VERSION;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::{Address, ConnectionInfo};

#[test]
fn directory_index_html() {
    let path = "/static/";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);


    let path_array = vec!["static", "index.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text);
}

#[test]
fn directory_index_html_with_query() {
    let path = "/static/?param=1234";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);


    let path_array = vec!["static", "index.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text);
}

#[test]
fn directory_index_html_no_slash() {
    let path = "/static";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);


    let path_array = vec!["static", "index.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text);
}

#[test]
fn directory_index_html_no_slash_and_query() {
    let path = "/static?param=1234";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);


    let path_array = vec!["static", "index.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text);
}

#[test]
fn file_retrieval() {
    let path = "/static/test.txt";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);


    let path_array = vec!["static", "test.txt"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text);
}

#[test]
fn not_found() {
    // is_matching's result now depends on RWS_CONFIG_SPA_FALLBACK (unset here,
    // but the SPA-fallback tests below toggle it) — take the lock so this
    // never observes a state left mid-flight by one of those.
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
    let path = "/not_found";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(!is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);


    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n501_not_implemented.status_code);
    assert_eq!(response.reason_phrase, STATUS_CODE_REASON_PHRASE.n501_not_implemented.reason_phrase);
}

#[test]
fn malformed() {
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
    let path = "//randomtext";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(!is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);


    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n501_not_implemented.status_code);
    assert_eq!(response.reason_phrase, STATUS_CODE_REASON_PHRASE.n501_not_implemented.reason_phrase);
}

#[test]
fn file_retrieval_with_query_params() {
    let request_uri = "/static/test.txt?param=as%20df&another_param=1234";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: request_uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);


    let path_array = vec!["static", "test.txt"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text);
}

#[test]
fn file_retrieval_no_html_suffix() {
    let request_uri = "/configure";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: request_uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);


    let path_array = vec!["configure.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text);
}

#[test]
fn file_retrieval_no_html_suffix_with_query_params() {
    let request_uri = "/configure?param1=some_param&another";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: request_uri.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = StaticResourceController::process(&request, response, &connection_info);


    let path_array = vec!["configure.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text);
}

#[test]
fn file_response_includes_etag_header() {
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/static/test.txt".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let response = StaticResourceController::process(&request, Response::new(), &connection_info);

    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n200_ok.status_code);

    let etag = response._get_header(Header::_ETAG.to_string());
    assert!(etag.is_some(), "ETag header should be present");

    let etag_value = &etag.unwrap().value;
    assert!(etag_value.starts_with('"'), "ETag should be quoted: {}", etag_value);
    assert!(etag_value.ends_with('"'), "ETag should be quoted: {}", etag_value);
    assert!(etag_value.contains('-'), "ETag should contain mtime-size separator: {}", etag_value);
}

#[test]
fn if_none_match_returns_304_not_modified() {
    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    // First request — get the ETag
    let request1 = Request {
        method: METHOD.get.to_string(),
        request_uri: "/static/test.txt".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let response1 = StaticResourceController::process(&request1, Response::new(), &connection_info);
    assert_eq!(response1.status_code, *STATUS_CODE_REASON_PHRASE.n200_ok.status_code);
    assert!(!response1.content_range_list.is_empty());

    let etag = response1._get_header(Header::_ETAG.to_string()).unwrap().value.clone();

    // Second request — send matching If-None-Match
    let request2 = Request {
        method: METHOD.get.to_string(),
        request_uri: "/static/test.txt".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![Header { name: Header::_IF_NONE_MATCH.to_string(), value: etag.clone() }],
        body: vec![],
    };

    let response2 = StaticResourceController::process(&request2, Response::new(), &connection_info);

    assert_eq!(response2.status_code, *STATUS_CODE_REASON_PHRASE.n304_not_modified.status_code);
    assert_eq!(response2.reason_phrase, STATUS_CODE_REASON_PHRASE.n304_not_modified.reason_phrase);
    assert!(response2.content_range_list.is_empty(), "304 response must have no body");

    // ETag should still be present in the 304
    let etag304 = response2._get_header(Header::_ETAG.to_string());
    assert!(etag304.is_some(), "ETag should be present in 304 response");
    assert_eq!(etag304.unwrap().value, etag);
}

#[test]
fn if_none_match_star_returns_304() {
    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/static/test.txt".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![Header { name: Header::_IF_NONE_MATCH.to_string(), value: "*".to_string() }],
        body: vec![],
    };

    let response = StaticResourceController::process(&request, Response::new(), &connection_info);

    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n304_not_modified.status_code);
    assert!(response.content_range_list.is_empty(), "304 response must have no body");
}

#[test]
fn stale_etag_returns_200_with_body() {
    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    };

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/static/test.txt".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![Header {
            name: Header::_IF_NONE_MATCH.to_string(),
            value: "\"outdated-etag-value\"".to_string(),
        }],
        body: vec![],
    };

    let response = StaticResourceController::process(&request, Response::new(), &connection_info);

    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n200_ok.status_code);
    assert!(!response.content_range_list.is_empty(), "200 response must include file body");
}

#[test]
fn directory_without_index_html_matches_and_renders_listing() {
    let path = "/static/no_index/";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
        sni_hostname: None,
    };

    assert!(StaticResourceController::is_matching(&request, &connection_info));

    let response = StaticResourceController::process(&request, Response::new(), &connection_info);

    assert_eq!(response.status_code, *STATUS_CODE_REASON_PHRASE.n200_ok.status_code);
    let body = String::from_utf8(response.content_range_list.get(0).unwrap().body.to_vec()).unwrap();
    assert!(body.starts_with("<!DOCTYPE html>"));
    assert!(body.contains("alpha.txt"));
    assert!(body.contains("beta.json"));
    assert!(body.contains("nested/"));
}

#[test]
fn directory_without_index_html_matches_head_and_options() {
    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
        sni_hostname: None,
    };

    let head_request = Request {
        method: METHOD.head.to_string(),
        request_uri: "/static/no_index/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };
    assert!(StaticResourceController::is_matching(&head_request, &connection_info));

    let options_request = Request {
        method: METHOD.options.to_string(),
        request_uri: "/static/no_index/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };
    assert!(StaticResourceController::is_matching(&options_request, &connection_info));
}

#[test]
fn directory_listing_escapes_display_name_and_percent_encodes_href() {
    let path_array = vec!["static", "no_index"];
    let fs_path = FileExt::build_path(&path_array);

    let html = StaticResourceController::render_directory_listing(&fs_path, "/static/no_index");

    // display text is HTML-escaped ("a & b.txt" -> "a &amp; b.txt")...
    assert!(html.contains("a &amp; b.txt"));
    // ...but the href is percent-encoded, not HTML-escaped, so the raw name never
    // appears unescaped inside an href attribute
    assert!(html.contains("href=\"/static/no_index/a%20%26%20b.txt\""));
    assert!(!html.contains("href=\"/static/no_index/a & b.txt\""));
}

#[test]
fn directory_listing_on_empty_or_missing_directory_still_renders_a_valid_page() {
    let html = StaticResourceController::render_directory_listing(
        "/nonexistent-directory-for-html-escaping-test",
        "/no_index",
    );

    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("Index of /no_index/"));
    assert!(html.contains("0 items"));
}

#[test]
fn directory_listing_has_parent_link_except_at_static_root() {
    let nested_html = StaticResourceController::render_directory_listing(
        "/nonexistent-directory-for-parent-link-test",
        "/no_index/nested",
    );
    assert!(nested_html.contains(".. (parent directory)"));
    assert!(nested_html.contains("href=\"/no_index/\""));
}

// ── SPA fallback (RWS_CONFIG_SPA_FALLBACK) ────────────────────────────────────

#[test]
fn path_has_extension_checks_only_the_last_segment() {
    assert!(!super::path_has_extension("/dashboard/settings"));
    assert!(!super::path_has_extension("/user.name/settings"), "dot in a non-last segment shouldn't count");
    assert!(super::path_has_extension("/assets/logo.png"));
    assert!(super::path_has_extension("/app.js"));
    assert!(!super::path_has_extension("/"));
}

fn spa_request(path: &str) -> (Request, ConnectionInfo) {
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };
    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
        sni_hostname: None,
    };
    (request, connection_info)
}

#[test]
fn spa_fallback_disabled_by_default() {
    let _g = crate::test_env::lock();
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");

    let (request, connection_info) = spa_request("/dashboard/settings");
    assert!(!StaticResourceController::is_matching(&request, &connection_info));
}

#[test]
fn spa_fallback_serves_configured_file_for_unmatched_route() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_SPA_FALLBACK", "static/index.html");
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES");

    let (request, connection_info) = spa_request("/dashboard/settings");
    assert!(StaticResourceController::is_matching(&request, &connection_info));

    let response = StaticResourceController::process(&request, Response::new(), &connection_info);
    assert_eq!(*STATUS_CODE_REASON_PHRASE.n200_ok.status_code, response.status_code);

    let expected = FileExt::read_file(FileExt::build_path(&["static", "index.html"]).as_str()).unwrap();
    let actual = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(expected, actual);

    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
}

#[test]
fn spa_fallback_still_serves_a_real_file_directly_when_one_exists() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_SPA_FALLBACK", "static/index.html");
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES");

    // /static/test.txt is a real file — must be served as itself, not the fallback.
    let (request, connection_info) = spa_request("/static/test.txt");
    let response = StaticResourceController::process(&request, Response::new(), &connection_info);
    let expected = FileExt::read_file(FileExt::build_path(&["static", "test.txt"]).as_str()).unwrap();
    let actual = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(expected, actual);

    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
}

#[test]
fn spa_fallback_skips_paths_that_look_like_a_missed_asset() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_SPA_FALLBACK", "static/index.html");
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES");

    let (request, connection_info) = spa_request("/assets/missing-logo.png");
    assert!(
        !StaticResourceController::is_matching(&request, &connection_info),
        "a path with a file extension should 404, not fall back to index.html"
    );

    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
}

#[test]
fn spa_fallback_respects_exclude_prefixes() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_SPA_FALLBACK", "static/index.html");
    std::env::set_var("RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES", "/api,/healthz");

    let (request, connection_info) = spa_request("/api/users/999");
    assert!(
        !StaticResourceController::is_matching(&request, &connection_info),
        "an excluded prefix should still 404 rather than fall back"
    );

    // A path outside the excluded prefixes still gets the fallback.
    let (request2, connection_info2) = spa_request("/dashboard/settings");
    assert!(StaticResourceController::is_matching(&request2, &connection_info2));

    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES");
}

#[test]
fn spa_fallback_is_a_noop_when_configured_file_does_not_exist() {
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_SPA_FALLBACK", "static/this-file-does-not-exist.html");
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES");

    let (request, connection_info) = spa_request("/dashboard/settings");
    assert!(!StaticResourceController::is_matching(&request, &connection_info));

    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
}

#[test]
fn spa_fallback_does_not_apply_to_the_root_path() {
    // request_uri == "/" is excluded from StaticResourceController entirely
    // (IndexController's territory) regardless of SPA fallback config.
    let _g = crate::test_env::lock();
    std::env::set_var("RWS_CONFIG_SPA_FALLBACK", "static/index.html");
    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK_EXCLUDE_PREFIXES");

    let (request, connection_info) = spa_request("/");
    assert!(!StaticResourceController::is_matching(&request, &connection_info));

    std::env::remove_var("RWS_CONFIG_SPA_FALLBACK");
}
// ── large files: decide to stream before reading ──────────────────────────────

/// A file under `target/` (inside the working directory, so the static
/// controller serves it), deleted on drop.
struct ServedFile(String);
impl ServedFile {
    fn new(name: &str, len: usize) -> Self {
        let url = format!("/target/rws_static_test_{}_{}", std::process::id(), name);
        let fs = format!("{}{}", crate::app::controller::static_resource::working_directory(), url);
        std::fs::write(&fs, vec![b'x'; len]).unwrap();
        ServedFile(url)
    }
}
impl Drop for ServedFile {
    fn drop(&mut self) {
        let fs = format!("{}{}", crate::app::controller::static_resource::working_directory(), self.0);
        let _ = std::fs::remove_file(fs);
    }
}

fn static_request(method: &str, uri: &str, headers: Vec<Header>) -> Request {
    Request { method: method.to_string(), request_uri: uri.to_string(), http_version: VERSION.http_1_1.to_string(), headers, body: vec![] }
}

#[test]
fn large_file_is_streamed_without_being_read() {
    let file = ServedFile::new("large.bin", (StaticResourceController::STREAM_THRESHOLD + 1) as usize);
    let response = StaticResourceController::process_request(&static_request(METHOD.get, &file.0, vec![]), Response::new());
    assert_eq!(200, response.status_code);
    assert!(response.stream_file.is_some(), "large file must stream");
    assert!(response.content_range_list.is_empty(), "and must not be loaded into memory");
    let length = response.headers.iter().find(|h| h.name == Header::_CONTENT_LENGTH).map(|h| h.value.clone());
    assert_eq!(Some((StaticResourceController::STREAM_THRESHOLD + 1).to_string()), length);
}

#[test]
fn large_file_honours_if_none_match() {
    let file = ServedFile::new("etag.bin", (StaticResourceController::STREAM_THRESHOLD + 1) as usize);
    let first = StaticResourceController::process_request(&static_request(METHOD.get, &file.0, vec![]), Response::new());
    let etag = first.headers.iter().find(|h| h.name == Header::_ETAG).unwrap().value.clone();
    let second = StaticResourceController::process_request(
        &static_request(METHOD.get, &file.0, vec![Header { name: Header::_IF_NONE_MATCH.to_string(), value: etag }]),
        Response::new(),
    );
    assert_eq!(304, second.status_code);
    assert!(second.stream_file.is_none());
}

#[test]
fn small_files_and_ranges_take_the_regular_path() {
    let small = ServedFile::new("small.bin", 1024);
    let response = StaticResourceController::process_request(&static_request(METHOD.get, &small.0, vec![]), Response::new());
    assert!(response.stream_file.is_none());
    assert_eq!(1024, response.content_range_list.iter().map(|c| c.body.len()).sum::<usize>());

    let large = ServedFile::new("ranged.bin", (StaticResourceController::STREAM_THRESHOLD + 1) as usize);
    let ranged = StaticResourceController::process_request(
        &static_request(METHOD.get, &large.0, vec![Header { name: Header::_RANGE.to_string(), value: "bytes=0-9".to_string() }]),
        Response::new(),
    );
    assert_eq!(206, ranged.status_code);
    assert_eq!(10, ranged.content_range_list[0].body.len());
}

// ── regular files: one open, metadata from the handle ─────────────────────────

#[test]
fn small_file_fast_path_serves_body_type_and_etag() {
    let file = ServedFile::new("fast.txt", 300);
    let response = StaticResourceController::process_request(&static_request(METHOD.get, &file.0, vec![]), Response::new());
    assert_eq!(200, response.status_code);
    assert_eq!(1, response.content_range_list.len());
    assert_eq!(vec![b'x'; 300], response.content_range_list[0].body);
    assert_eq!(crate::mime_type::MimeType::TEXT_PLAIN, response.content_range_list[0].content_type);
    let etag = response.headers.iter().find(|h| h.name == Header::_ETAG).expect("ETag").value.clone();
    assert!(etag.ends_with("-300\""), "{etag}");

    let cached = StaticResourceController::process_request(
        &static_request(METHOD.get, &file.0, vec![Header { name: Header::_IF_NONE_MATCH.to_string(), value: etag }]),
        Response::new(),
    );
    assert_eq!(304, cached.status_code);
    assert!(cached.content_range_list.is_empty());
}

#[test]
fn fast_path_matches_the_general_path() {
    // Same file through the fast path (GET) and the general path (a Range
    // covering the whole file): same bytes and same type.
    let file = ServedFile::new("same.css", 700);
    let fast = StaticResourceController::process_request(&static_request(METHOD.get, &file.0, vec![]), Response::new());
    let general = StaticResourceController::process_static_resources(&static_request(METHOD.get, &file.0, vec![])).unwrap();
    assert_eq!(general[0].body, fast.content_range_list[0].body);
    assert_eq!(general[0].content_type, fast.content_range_list[0].content_type);
}

#[test]
fn fast_path_declines_directories_and_options() {
    let dir = static_request(METHOD.get, "/src/", vec![]);
    let mut response = Response::new();
    assert!(StaticResourceController::serve_regular_file(&dir, &mut response).is_none());
    let file = ServedFile::new("opts.txt", 10);
    assert!(StaticResourceController::serve_regular_file(&static_request(METHOD.options, &file.0, vec![]), &mut Response::new()).is_none());
}


// ── #43: static files stay inside the web root ───────────────────────────────

fn static_status(uri: &str) -> i16 {
    use crate::application::Application;
    let request = Request { method: "GET".into(), request_uri: uri.to_string(), http_version: "HTTP/1.1".into(), headers: vec![], body: vec![] };
    let conn = ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None };
    crate::app::App::new().execute(&request, &conn).unwrap().status_code
}

#[test]
fn dot_dot_segments_cannot_leave_the_web_root() {
    // `/../<root dir name>/Cargo.toml` leaves the root and comes back in:
    // it was served (200); any `..` is refused now.
    let name = std::env::current_dir().unwrap().file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(404, static_status(&format!("/../{}/Cargo.toml", name)));
    assert_eq!(404, static_status("/src/../Cargo.toml"));
    assert_eq!(404, static_status("/../../../../../../etc/passwd"));
    assert_eq!(200, static_status("/Cargo.toml"));
}

#[test]
fn hidden_files_are_not_served_but_well_known_is() {
    assert_eq!(404, static_status("/.gitignore"));
    assert!(super::check_request_path("/.git/config").is_err());
    assert!(super::check_request_path("/a/.env").is_err());
    assert!(super::check_request_path("/.well-known/security.txt").is_ok());
    assert!(super::check_request_path("/./index.html").is_ok());
    assert!(super::check_request_path("/a\\..\\b").is_err());
}

#[test]
fn key_and_config_files_are_not_served() {
    // rws.config.toml sits in the working directory (the web root) here.
    assert_eq!(404, static_status("/rws.config.toml"));
    for path in ["/key.pem", "/cert.PEM", "/a/acme_account.key", "/rws.config.toml"] {
        assert!(super::check_request_path(path).is_err(), "{path}");
    }
    assert!(super::check_request_path("/keys.html").is_ok());
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_root_is_not_followed() {
    let link = crate::test_env::TempPath::new_in_cwd("rws_test_escape_link");
    std::os::unix::fs::symlink("/etc", &*link).unwrap();
    let name = link.file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(404, static_status(&format!("/{}/hostname", name)));
    assert_eq!(404, static_status(&format!("/{}/passwd", name)));
    assert!(!super::inside_root(&format!("{}/{}/passwd", super::working_directory(), name)));
    // A link that stays inside the root is still served.
    let inside = crate::test_env::TempPath::new_in_cwd("rws_test_inside_link.toml");
    std::os::unix::fs::symlink("Cargo.toml", &*inside).unwrap();
    let inside_name = inside.file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(200, static_status(&format!("/{}", inside_name)));
}


// ── #55: the request path is percent-decoded once ───────────────────────────

#[test]
fn percent_encoded_names_are_found() {
    let file = crate::test_env::TempPath::new_in_cwd("spaced name.txt");
    std::fs::write(&*file, "spaced").unwrap();
    let name = file.file_name().unwrap().to_string_lossy().to_string();
    assert!(name.contains(' '));
    assert_eq!(200, static_status(&format!("/{}", name.replace(' ', "%20"))));
    // The general path (a Range request) finds it too, with its ETag.
    {
        use crate::application::Application;
        let request = Request { method: "GET".into(), request_uri: format!("/{}", name.replace(' ', "%20")), http_version: "HTTP/1.1".into(),
            headers: vec![Header { name: "Range".into(), value: "bytes=0-2".into() }], body: vec![] };
        let conn = ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None };
        let response = crate::app::App::new().execute(&request, &conn).unwrap();
        assert_eq!(206, response.status_code, "{}", String::from_utf8_lossy(&response.content_range_list.first().map(|c| c.body.clone()).unwrap_or_default()));
        assert_eq!(b"spa".to_vec(), response.content_range_list[0].body);
        assert!(response.headers.iter().any(|h| h.name == "ETag"), "no ETag for an escaped name");
    }
    // Decoded once: `%2520` is a literal `%20`, which isn't this file.
    assert_eq!(404, static_status(&format!("/{}", name.replace(' ', "%2520"))));
    assert_eq!(200, static_status("/Cargo%2Etoml"));
}

#[test]
fn encoded_traversal_and_hidden_names_are_refused() {
    let name = std::env::current_dir().unwrap().file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(404, static_status(&format!("/%2e%2e/{}/Cargo.toml", name)));
    assert_eq!(404, static_status(&format!("/%2E%2E%2F{}%2FCargo.toml", name)));
    assert_eq!(404, static_status("/%2egitignore"));
    assert_eq!(404, static_status("/rws.config%2Etoml"));
    assert_eq!(404, static_status("/Cargo.toml%00.html"));
}

#[test]
fn percent_decode_path_rules() {
    use super::percent_decode_path;
    assert!(matches!(percent_decode_path("/plain/path"), Ok(std::borrow::Cow::Borrowed("/plain/path"))));
    assert_eq!("/a b", percent_decode_path("/a%20b").unwrap());
    assert_eq!("/a+b", percent_decode_path("/a+b").unwrap());
    assert_eq!("/100%", percent_decode_path("/100%").unwrap());
    assert_eq!("/%zz%4", percent_decode_path("/%zz%4").unwrap());
    assert_eq!("/é", percent_decode_path("/%C3%A9").unwrap());
    assert!(percent_decode_path("/%FF").is_err());
}

#[test]
fn directory_listing_links_resolve() {
    use crate::application::Application;
    let dir = crate::test_env::TempPath::new_in_cwd("listing_dir");
    std::fs::create_dir(&*dir).unwrap();
    std::fs::write(dir.join("a b&c.txt"), "x").unwrap();
    let dir_name = dir.file_name().unwrap().to_string_lossy().to_string();
    let request = Request { method: "GET".into(), request_uri: format!("/{}/", dir_name), http_version: "HTTP/1.1".into(), headers: vec![], body: vec![] };
    let conn = ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None };
    let listing = crate::app::App::new().execute(&request, &conn).unwrap();
    let html = String::from_utf8_lossy(&listing.content_range_list[0].body).to_string();
    let href = html.split("href=\"").skip(1)
        .map(|rest| rest.split('"').next().unwrap())
        .find(|h| h.contains("a%20b"))
        .unwrap_or_else(|| panic!("no link to the file in {html}"))
        .replace("&amp;", "&");
    let target = if href.starts_with('/') { href } else { format!("/{}/{}", dir_name, href) };
    assert_eq!(200, static_status(&target), "{target}");
}

// ── #56: conditional requests ───────────────────────────────────────────────

#[test]
fn if_none_match_is_a_list_compared_weakly() {
    use super::if_none_match_matches as m;
    let etag = "\"1700-42\"";
    assert!(m("\"1700-42\"", etag));
    assert!(m("W/\"1700-42\"", etag));
    assert!(m("\"other\", \"1700-42\"", etag));
    assert!(m("\"a\",W/\"1700-42\" , \"b\"", etag));
    assert!(m("*", etag));
    assert!(!m("\"1700-43\"", etag));
    assert!(!m("\"a\", \"b\"", etag));
    assert!(!m("", etag));
    assert!(!m("1700-42", "\"1700-42-x\""));
}

fn get_with(uri: &str, headers: Vec<(&str, &str)>) -> Response {
    use crate::application::Application;
    let request = Request { method: "GET".into(), request_uri: uri.to_string(), http_version: "HTTP/1.1".into(),
        headers: headers.into_iter().map(|(n, v)| Header { name: n.to_string(), value: v.to_string() }).collect(), body: vec![] };
    let conn = ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None };
    crate::app::App::new().execute(&request, &conn).unwrap()
}

fn etag_of(response: &Response) -> String {
    response.headers.iter().find(|h| h.name == Header::_ETAG).map(|h| h.value.clone()).expect("no ETag")
}

#[test]
fn a_listed_or_weak_etag_gets_304() {
    let etag = etag_of(&get_with("/Cargo.toml", vec![]));
    assert_eq!(304, get_with("/Cargo.toml", vec![("If-None-Match", &format!("\"x\", {etag}"))]).status_code);
    assert_eq!(304, get_with("/Cargo.toml", vec![("If-None-Match", &format!("W/{etag}"))]).status_code);
    assert_eq!(200, get_with("/Cargo.toml", vec![("If-None-Match", "\"x\", \"y\"")]).status_code);
}

#[test]
fn if_range_with_a_stale_validator_sends_the_whole_file() {
    let whole = get_with("/Cargo.toml", vec![]);
    let etag = etag_of(&whole);
    let len = whole.content_range_list[0].body.len();
    // Current strong tag: the range is honored.
    let partial = get_with("/Cargo.toml", vec![("Range", "bytes=0-9"), ("If-Range", &etag)]);
    assert_eq!(206, partial.status_code);
    assert_eq!(10, partial.content_range_list[0].body.len());
    // Another tag, the weak form, or a date: the whole file, 200.
    for stale in ["\"0-0\"".to_string(), format!("W/{etag}"), "Sat, 01 Jan 2000 00:00:00 GMT".to_string()] {
        let response = get_with("/Cargo.toml", vec![("Range", "bytes=0-9"), ("If-Range", &stale)]);
        assert_eq!(200, response.status_code, "If-Range: {stale}");
        assert_eq!(len, response.content_range_list[0].body.len());
    }
    // If-Range without Range changes nothing.
    assert_eq!(200, get_with("/Cargo.toml", vec![("If-Range", "\"0-0\"")]).status_code);
}

#[test]
fn one_satisfiable_range_is_enough_and_416_names_the_length() {
    let len = std::fs::metadata("Cargo.toml").unwrap().len();
    let partial = get_with("/Cargo.toml", vec![("Range", "bytes=0-9, 99999999-")]);
    assert_eq!(206, partial.status_code);
    assert_eq!(1, partial.content_range_list.len());
    assert_eq!(10, partial.content_range_list[0].body.len());

    let none = get_with("/Cargo.toml", vec![("Range", "bytes=99999999-, 99999999-100000000")]);
    assert_eq!(416, none.status_code);
    let content_range = none.headers.iter().find(|h| h.name == Header::_CONTENT_RANGE).map(|h| h.value.clone());
    assert_eq!(Some(format!("bytes */{len}")), content_range);
}

#[test]
fn an_unsupported_method_on_a_file_is_405_with_allow() {
    use crate::application::Application;
    let conn = ConnectionInfo { client: Address { ip: "127.0.0.1".into(), port: 1 }, server: Address { ip: "127.0.0.1".into(), port: 2 }, request_size: 0, sni_hostname: None };
    let send = |method: &str, uri: &str| {
        let request = Request { method: method.into(), request_uri: uri.into(), http_version: "HTTP/1.1".into(), headers: vec![], body: vec![] };
        crate::app::App::new().execute(&request, &conn).unwrap()
    };
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        let response = send(method, "/Cargo.toml");
        assert_eq!(405, response.status_code, "{method}");
        let allow = response.headers.iter().find(|h| h.name == Header::_ALLOW).map(|h| h.value.clone());
        assert_eq!(Some("GET, HEAD, OPTIONS".to_string()), allow);
    }
    // A path that doesn't exist, or one outside the root, stays 404.
    assert_eq!(404, send("POST", "/no-such-file.txt").status_code);
    assert_eq!(404, send("DELETE", "/../etc/passwd").status_code);
    assert_eq!(404, send("POST", "/rws.config.toml").status_code);
}
