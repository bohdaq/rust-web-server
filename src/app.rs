use std::{env, fs};
use std::fs::{File, metadata};
use std::io::Read;
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
        let mut file_content = Vec::new();
        let mut file = File::open(&App::NOT_FOUND_PAGE_FILEPATH).expect("Unable to open file");
        file.read_to_end(&mut file_content).expect("Unable to read");

        let mut contents = file_content;
        let content_type = MimeType::detect_mime_type(App::NOT_FOUND_PAGE_FILEPATH);

        let content_type_header = Header {
            header_name: HTTP_HEADERS.CONTENT_TYPE.to_string(),
            header_value: content_type,
        };

        let mut response = Response {
            http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
            status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.STATUS_CODE.to_string(),
            reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N404_NOT_FOUND.REASON_PHRASE.to_string(),
            headers: vec![
                content_type_header,
                App::get_x_content_type_options_header(),
                App::get_accept_ranges_header(),
            ],
            message_body: contents
        };

        if request.request_uri == CONSTANTS.SLASH {
            let mut file_content = Vec::new();
            let mut file = File::open(&App::INDEX_FILEPATH).expect("Unable to open file");
            file.read_to_end(&mut file_content).expect("Unable to read");

            let mut contents = file_content;
            let content_type = MimeType::detect_mime_type(App::INDEX_FILEPATH);

            let content_type_header = Header {
                header_name: HTTP_HEADERS.CONTENT_TYPE.to_string(),
                header_value: content_type,
            };

            response = Response {
                http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
                status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE.to_string(),
                reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE.to_string(),
                headers: vec![
                    content_type_header,
                    App::get_x_content_type_options_header(),
                    App::get_accept_ranges_header(),
                ],
                message_body: contents
            };
        }

        if request.method == REQUEST_METHODS.GET && request.request_uri != CONSTANTS.SLASH {
            let result = App::process_static_resources(&request);

            if result.len() != 0 {
                let content_type = MimeType::detect_mime_type(&request.request_uri);

                let content_type_header = Header {
                    header_name: HTTP_HEADERS.CONTENT_TYPE.to_string(),
                    header_value: content_type,
                };

                response = Response {
                    http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
                    status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.STATUS_CODE.to_string(),
                    reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N200_OK.REASON_PHRASE.to_string(),
                    headers: vec![
                        content_type_header,
                        App::get_x_content_type_options_header(),
                        App::get_accept_ranges_header(),
                    ],
                    message_body: result
                };
            }



        }

        response
    }

    pub(crate) fn process_static_resources(request: &Request) -> Vec<u8> {
        let dir = env::current_dir().unwrap();
        let working_directory = dir.as_path().to_str().unwrap();
        let static_filepath = [working_directory, request.request_uri.as_str()].join(CONSTANTS.EMPTY_STRING);

        let mut contents = Vec::new();

        let boxed_file = File::open(&static_filepath);
        if boxed_file.is_ok()  {
            let md = metadata(&static_filepath).unwrap();
            if md.is_file() {
                let mut file = boxed_file.unwrap();
                file.read_to_end(&mut contents).expect("Unable to read");
            }
        }

        contents
    }

    pub(crate) fn get_x_content_type_options_header() -> Header {
        Header {
            header_name: HTTP_HEADERS.X_CONTENT_TYPE_OPTIONS.to_string(),
            header_value: CONSTANTS.NOSNIFF.to_string(),
        }
    }

    pub(crate) fn get_accept_ranges_header() -> Header {
        Header {
            header_name: HTTP_HEADERS.ACCEPT_RANGES.to_string(),
            header_value: CONSTANTS.BYTES.to_string(),
        }
    }
}