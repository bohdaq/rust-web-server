use crate::app::controller::static_resource::StaticResourceController;
use crate::controller::Controller;
use crate::http::VERSION;
use crate::request::{METHOD, Request};
use crate::server::{Address, ConnectionInfo};

#[test]
fn file_retrieval() {
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
    };

    let is_matching = StaticResourceController::is_matching(&request, &connection_info);
    assert!(is_matching);
}