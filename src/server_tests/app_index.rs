//! Tests of rws-server's `app::controller::index` that read repository fixtures (`static/`,
//! `index.html`): a crate's tests run in the crate's directory.
#![allow(unused_imports, dead_code)]

use file_ext::FileExt;
use crate::app::controller::index::IndexController;
use crate::controller::Controller;
use crate::core::New;
use crate::http::VERSION;
use crate::request::{METHOD, Request};
use crate::response::{Response};
use crate::server::{Address, ConnectionInfo};



#[test]
fn file_retrieval() {
    // Deletes and restores ./index.html: tests reading it hold the same lock.
    let _g = crate::test_env::lock();
    // user provided html file
    let pwd = FileExt::working_directory().unwrap();

    let path = "/";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    tls: false,
    };

    let is_matching = IndexController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = IndexController::process(&request, response, &connection_info);


    let path_array = vec![pwd.as_str(), "index.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text.to_vec());

    FileExt::copy_file(
        vec![pwd.as_str(), "index.html"],
        vec![pwd.as_str(), "index_copy.html"]
    ).unwrap();

    FileExt::delete_file("index.html").unwrap();

    let path = "/";

    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: path.to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    sni_hostname: None,
    tls: false,
    };

    let is_matching = IndexController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = IndexController::process(&request, response, &connection_info);


    let path_array = vec![pwd.as_str(), "crates", "rws-server", "src", "app", "controller", "index", "index.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text.to_vec());

    FileExt::copy_file(
        vec![pwd.as_str(), "index_copy.html"],
        vec![pwd.as_str(), "index.html"]
    ).unwrap();
    FileExt::delete_file("index_copy.html").unwrap();
}

#[test]
fn the_index_page_is_revalidated_like_other_static_files() {
    // Reads ./index.html: same lock as file_retrieval, which moves it.
    let _g = crate::test_env::lock();
    use crate::application::Application;
    use crate::header::Header;
    let request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body: Default::default(),
    };
    let conn = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
        sni_hostname: None,
        tls: false,
    };
    let response = crate::app::App::with_config(crate::server_config::ServerConfig::default()).execute(&request, &conn).unwrap();
    assert_eq!(200, response.status_code);
    assert_eq!("no-cache", response.header(Header::_CACHE_CONTROL).unwrap().value, "the home page, not no-store");
    let custom = crate::server_config::ServerConfig { static_cache_control: "public, max-age=60".to_string(), ..Default::default() };
    let response = crate::app::App::with_config(custom).execute(&request, &conn).unwrap();
    assert_eq!("public, max-age=60", response.header(Header::_CACHE_CONTROL).unwrap().value);
}
