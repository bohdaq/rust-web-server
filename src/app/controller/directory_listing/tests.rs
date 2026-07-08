use file_ext::FileExt;
use crate::app::controller::directory_listing::DirectoryListingAssetsController;
use crate::controller::Controller;
use crate::core::New;
use crate::http::VERSION;
use crate::mime_type::MimeType;
use crate::request::{METHOD, Request};
use crate::response::Response;
use crate::server::{Address, ConnectionInfo};

fn connection_info() -> ConnectionInfo {
    ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
        sni_hostname: None,
    }
}

fn get(path: &str) -> Request {
    Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    }
}

#[test]
fn matches_css_and_js_paths_only() {
    let connection_info = connection_info();

    assert!(DirectoryListingAssetsController::is_matching(&get("/rws-directory-listing.css"), &connection_info));
    assert!(DirectoryListingAssetsController::is_matching(&get("/rws-directory-listing.js"), &connection_info));
    assert!(!DirectoryListingAssetsController::is_matching(&get("/rws-directory-listing.txt"), &connection_info));
    assert!(!DirectoryListingAssetsController::is_matching(&get("/static/style.css"), &connection_info));

    let post = Request {
        method: METHOD.post.to_string(),
        request_uri: "/rws-directory-listing.css".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: vec![],
    };
    assert!(!DirectoryListingAssetsController::is_matching(&post, &connection_info));
}

// Mirrors `ScriptController`/`StyleController`'s own `file_retrieval` tests: all
// steps run sequentially in one test function against the same shared-cwd file
// path, so this doesn't race other tests touching the same override file.
#[test]
fn css_file_retrieval_with_disk_override() {
    if FileExt::does_file_exist(DirectoryListingAssetsController::CSS_FILEPATH) {
        FileExt::delete_file(DirectoryListingAssetsController::CSS_FILEPATH).unwrap();
    }

    let connection_info = connection_info();
    let request = get("/rws-directory-listing.css");

    let response = DirectoryListingAssetsController::process(&request, Response::new(), &connection_info);
    assert_eq!(response.content_range_list.get(0).unwrap().content_type, MimeType::TEXT_CSS);
    let embedded_body = String::from_utf8(response.content_range_list.get(0).unwrap().body.to_vec()).unwrap();
    assert!(embedded_body.contains(".breadcrumb"));

    let override_css = "body { color: red; }";
    FileExt::create_file(DirectoryListingAssetsController::CSS_FILEPATH).unwrap();
    FileExt::write_file(DirectoryListingAssetsController::CSS_FILEPATH, override_css.as_bytes()).unwrap();

    let response = DirectoryListingAssetsController::process(&request, Response::new(), &connection_info);
    let body = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(override_css.as_bytes().to_vec(), body);

    FileExt::delete_file(DirectoryListingAssetsController::CSS_FILEPATH).unwrap();

    let response = DirectoryListingAssetsController::process(&request, Response::new(), &connection_info);
    let body = String::from_utf8(response.content_range_list.get(0).unwrap().body.to_vec()).unwrap();
    assert!(body.contains(".breadcrumb"));
}

#[test]
fn js_file_retrieval_with_disk_override() {
    if FileExt::does_file_exist(DirectoryListingAssetsController::JS_FILEPATH) {
        FileExt::delete_file(DirectoryListingAssetsController::JS_FILEPATH).unwrap();
    }

    let connection_info = connection_info();
    let request = get("/rws-directory-listing.js");

    let response = DirectoryListingAssetsController::process(&request, Response::new(), &connection_info);
    assert_eq!(response.content_range_list.get(0).unwrap().content_type, MimeType::TEXT_JAVASCRIPT);
    let embedded_body = String::from_utf8(response.content_range_list.get(0).unwrap().body.to_vec()).unwrap();
    assert!(embedded_body.contains("getElementById('filter')"));

    let override_js = "console.log('override');";
    FileExt::create_file(DirectoryListingAssetsController::JS_FILEPATH).unwrap();
    FileExt::write_file(DirectoryListingAssetsController::JS_FILEPATH, override_js.as_bytes()).unwrap();

    let response = DirectoryListingAssetsController::process(&request, Response::new(), &connection_info);
    let body = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(override_js.as_bytes().to_vec(), body);

    FileExt::delete_file(DirectoryListingAssetsController::JS_FILEPATH).unwrap();

    let response = DirectoryListingAssetsController::process(&request, Response::new(), &connection_info);
    let body = String::from_utf8(response.content_range_list.get(0).unwrap().body.to_vec()).unwrap();
    assert!(body.contains("getElementById('filter')"));
}
