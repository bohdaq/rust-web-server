use std::{env, fs};
use crate::constant::{HTTP_VERSIONS, RESPONSE_STATUS_CODE_REASON_PHRASES};

use crate::request::Request;
use crate::response::Response;


pub struct App {}

impl App {
    pub(crate) fn handle_request(request: Request) -> Response {
        let mut contents = fs::read_to_string("404.html").unwrap();
        let mut response = Response {
            http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
            status_code: "404".to_string(),
            reason_phrase: "NOT FOUND".to_string(),
            headers: vec![],
            message_body: contents
        };

        if request.request_uri == "/" {
            contents = fs::read_to_string("index.html").unwrap();
            response = Response {
                http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
                status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE.to_string(),
                reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE.to_string(),
                headers: vec![],
                message_body: contents
            };
        }

        response
    }
}