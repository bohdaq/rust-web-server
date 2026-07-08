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