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
    // user provided html file
    let pwd = FileExt::working_directory().unwrap();

    let path = "/";

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
        body: vec![],
    };

    let connection_info = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 0 },
        server: Address { ip: "127.0.0.1".to_string(), port: 0 },
        request_size: 0,
    };

    let is_matching = IndexController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = IndexController::process(&request, response, &connection_info);


    let path_array = vec![pwd.as_str(), "src", "app", "controller", "index", "index.html"];
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
