use std::{env, fs};
use crate::constant::{HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};
use crate::CONSTANTS;
use crate::header::Header;

use crate::request::Request;
use crate::response::Response;


pub struct App {}

impl App {
    pub(crate) fn handle_request(request: Request) -> Response {

        // by default we assume route or static asset is not found
        let mut contents = fs::read_to_string("404.html").unwrap();
        let mut response = Response {
            http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
            status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.STATUS_CODE.to_string(),
            reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.REASON_PHRASE.to_string(),
            headers: vec![],
            message_body: contents
        };


        if request.request_uri == CONSTANTS.SLASH {
            contents = fs::read_to_string("index.html").unwrap();
            response = Response {
                http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
                status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE.to_string(),
                reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE.to_string(),
                headers: vec![],
                message_body: contents
            };
        }

        if request.method == REQUEST_METHODS.GET {
            let result = App::process_static_resources(request);

            match result {
                Ok(contents) => {
                    response = Response {
                        http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
                        status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE.to_string(),
                        reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE.to_string(),
                        headers: vec![],
                        message_body: contents
                    };

                },
                Err(error) => {
                    // 'No such file or directory' or 'Is a directory' errors etc.
                    println!("{}", error.to_string());
                },
            }
        }

        response
    }

    pub(crate) fn process_static_resources(request: Request) -> std::io::Result<String> {
        let dir = env::current_dir().unwrap();
        let working_directory = dir.as_path().to_str().unwrap();
        let static_filepath = [working_directory, request.request_uri.as_str()].join(CONSTANTS.EMPTY_STRING);

        let unwrapped_contents = fs::read_to_string(static_filepath);
        unwrapped_contents
    }
}