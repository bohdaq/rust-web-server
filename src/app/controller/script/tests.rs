use file_ext::FileExt;
use crate::app::controller::script::ScriptController;
use crate::controller::Controller;
use crate::core::New;
use crate::http::VERSION;
use crate::request::{METHOD, Request};
use crate::response::{Response};
use crate::server::{Address, ConnectionInfo};

#[test]
fn file_retrieval() {
    let path = "/script.js";

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

    let is_matching = ScriptController::is_matching(&request, &connection_info);
    assert!(is_matching);

    let mut response = Response::new();
    response = ScriptController::process(&request, response, &connection_info);


    let path_array = vec!["src", "app", "controller", "script", "script.js"];
    let path = FileExt::build_path(&path_array);
    let expected_text = FileExt::read_file(path.as_str()).unwrap();

    let actual_text = response.content_range_list.get(0).unwrap().body.to_vec();
    assert_eq!(actual_text, expected_text);
}