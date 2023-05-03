use crate::http::VERSION;
use crate::request::{METHOD, Request};

#[test]
fn body_in_request() {
    // can be any piece of data
    let body : Vec<u8> = Vec::from("request body can be anythings");

    let request : Request = Request {
        method: METHOD.get.to_string(),
        request_uri: "/some/path".to_string(),
        http_version: VERSION.http_1_1.to_string(),
        headers: vec![],
        body, // same as `body: body`
    };

    // replace with your logic
    assert_eq!(Vec::from("request body can be anythings"), request.body);
}


