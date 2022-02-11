use std::{env, fs};
use crate::constant::{HTTP_HEADERS, HTTP_VERSIONS, REQUEST_METHODS, RESPONSE_STATUS_CODE_REASON_PHRASES};
use crate::CONSTANTS;
use crate::header::Header;
use crate::mime_type::MimeType;

use crate::request::Request;
use crate::response::Response;


pub struct App {}

impl App {
    pub(crate) const NOT_FOUND_PAGE_FILEPATH: &'static str = "404.html";
    pub(crate) const INDEX_FILEPATH: &'static str = "index.html";

    pub(crate) fn handle_request(request: Request) -> Response {

        // by default we assume route or static asset is not found
        let mut contents = fs::read_to_string(App::NOT_FOUND_PAGE_FILEPATH).unwrap();
        let content_type = MimeType::detect_mime_type(App::NOT_FOUND_PAGE_FILEPATH);

        let content_type_header = Header {
            header_name: HTTP_HEADERS.CONTENT_TYPE.to_string(),
            header_value: content_type,
        };

        let mut response = Response {
            http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
            status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.STATUS_CODE.to_string(),
            reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.REASON_PHRASE.to_string(),
            headers: vec![content_type_header, App::get_x_content_type_options_header()],
            message_body: contents
        };

        if request.request_uri == CONSTANTS.SLASH {
            contents = fs::read_to_string(App::INDEX_FILEPATH).unwrap();
            let content_type = MimeType::detect_mime_type(App::INDEX_FILEPATH);

            let content_type_header = Header {
                header_name: HTTP_HEADERS.CONTENT_TYPE.to_string(),
                header_value: content_type,
            };

            response = Response {
                http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
                status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE.to_string(),
                reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE.to_string(),
                headers: vec![content_type_header, App::get_x_content_type_options_header()],
                message_body: contents
            };
        }

        if request.method == REQUEST_METHODS.GET {
            let result = App::process_static_resources(&request);

            match result {
                Ok(contents) => {
                    let content_type = MimeType::detect_mime_type(&request.request_uri);

                    let content_type_header = Header {
                        header_name: HTTP_HEADERS.CONTENT_TYPE.to_string(),
                        header_value: content_type,
                    };

                    response = Response {
                        http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
                        status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE.to_string(),
                        reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE.to_string(),
                        headers: vec![content_type_header, App::get_x_content_type_options_header()],
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

    pub(crate) fn process_static_resources(request: &Request) -> std::io::Result<String> {
        let dir = env::current_dir().unwrap();
        let working_directory = dir.as_path().to_str().unwrap();
        let static_filepath = [working_directory, request.request_uri.as_str()].join(CONSTANTS.EMPTY_STRING);

        let unwrapped_contents = fs::read_to_string(static_filepath);
        unwrapped_contents
    }

    pub(crate) fn get_x_content_type_options_header() -> Header {
        Header {
            header_name: HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string(),
            header_value: CONSTANTS.NOSNIFF.to_string(),
        }
    }
}