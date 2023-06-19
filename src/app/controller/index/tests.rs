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


    let path_array = vec!["index.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text.to_vec());




    // default index.html

    copy_file(vec!["index.html"], vec!["index_copy.html"]).unwrap();

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


    let path_array = vec!["src", "app", "controller", "index", "index.html"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text.to_vec());

    copy_file(vec!["index_copy.html"], vec!["index.html"]).unwrap();
    FileExt::delete_file("index_copy.html").unwrap();
}

fn copy_file(from: Vec<&str>, to: Vec<&str>) -> Result<(), String> {
    let from_path = FileExt::build_path(&from);
    let boxed_content_to_copy = FileExt::read_file(from_path.as_str());
    if boxed_content_to_copy.is_err() {
        let message = boxed_content_to_copy.err().unwrap();
        return Err(message);
    }
    let content_to_copy = boxed_content_to_copy.unwrap();
    let to_path = FileExt::build_path(&to);
    if FileExt::does_file_exist(to_path.as_str()) {
        let boxed_delete = FileExt::delete_file(to_path.as_str());
        if boxed_delete.is_err() {
            let message = boxed_delete.err().unwrap();
            return Err(message);
        }
    }
    let boxed_create = FileExt::create_file(to_path.as_str());
    if boxed_create.is_err() {
        let message = boxed_create.err().unwrap();
        return Err(message);
    }

    let boxed_write =
        FileExt::write_file(to_path.as_str(), content_to_copy.as_slice());
    if boxed_write.is_err() {
        let message = boxed_write.err().unwrap();
        return Err(message);
    }
    Ok(boxed_write.unwrap())
}