#[cfg(test)]
mod tests;

pub mod controller;

use std::{env};
use std::fs::{File, metadata};
use crate::app::controller::not_found::NotFoundController;
use crate::cors::Cors;
use crate::entry_point::Config;
use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};

use crate::request::{METHOD, Request};
use crate::response::{Error, Response, STATUS_CODE_REASON_PHRASE};
use crate::symbol::SYMBOL;


pub struct App {}

impl App {
    pub(crate) const INDEX_FILEPATH: &'static str = "index.html";

    pub(crate) fn handle_request(request: Request) -> (Response, Request) {

        let mut response: Response = Response::get_response(
            STATUS_CODE_REASON_PHRASE.n501_not_implemented,
            None,
            None
        ).unwrap();

        // by default we assume route or static asset is not found
        if NotFoundController::is_matching_request(&request) {
            response = NotFoundController::process_request(&request, response)
        }


        // index controller
        if request.request_uri == SYMBOL.slash {

            let boxed_content_range =
                Range::get_content_range_of_a_file(App::INDEX_FILEPATH);

            if boxed_content_range.is_ok() {
                let content_range = boxed_content_range.unwrap();
                let content_range_list = vec![content_range];
                let boxed_response = Response::get_response(
                    STATUS_CODE_REASON_PHRASE.n200_ok,
                    None,
                    Option::from(content_range_list)
                );
                if boxed_response.is_ok() {
                    response = boxed_response.unwrap();
                }
            } else {
                let error = boxed_content_range.err().unwrap();
                let mime_type = MimeType::TEXT_HTML.to_string();
                let content_range = Range::get_content_range(
                    Vec::from(error.as_bytes()),
                    mime_type
                );

                let content_range_list = vec![content_range];
                let boxed_response = Response::get_response(
                    STATUS_CODE_REASON_PHRASE.n500_internal_server_error,
                    None,
                    Option::from(content_range_list)
                );
                if boxed_response.is_ok() {
                    response = boxed_response.unwrap();
                }
            }
        }

        // static resources controller
        let is_get = request.method == METHOD.get;
        let is_head = request.method == METHOD.head;
        let is_options = request.method == METHOD.options;
        if is_get || is_head || is_options && request.request_uri != SYMBOL.slash {
            let boxed_content_range_list = App::process_static_resources(&request);
            if boxed_content_range_list.is_ok() {
                let content_range_list = boxed_content_range_list.unwrap();

                if content_range_list.len() != 0 {

                    let mut status_code_reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok;

                    let does_request_include_range_header = request.get_header(Header::RANGE.to_string()).is_some();
                    if does_request_include_range_header {
                        status_code_reason_phrase = STATUS_CODE_REASON_PHRASE.n206_partial_content;
                    }

                    let is_options_request = request.method == METHOD.options;
                    if is_options_request {
                        status_code_reason_phrase = STATUS_CODE_REASON_PHRASE.n204_no_content;
                    }

                    response = Response::get_response(
                        status_code_reason_phrase,
                        None,
                        Some(content_range_list)
                    ).unwrap();

                }
            } else {
                let error : Error = boxed_content_range_list.err().unwrap();
                let body = error.message;

                let content_range = Range::get_content_range(
                    Vec::from(body.as_bytes()),
                    MimeType::TEXT_HTML.to_string()
                );

                let content_range_list = vec![content_range];

                let boxed_response = Response::get_response(
                    error.status_code_reason_phrase,
                    None,
                    Some(content_range_list)
                );

                if boxed_response.is_ok() {
                    response = boxed_response.unwrap();
                }
            }

        }


        // cors wildcard controller
        if request.request_uri != SYMBOL.slash && request.method == METHOD.post {

            response = Response::get_response(
                STATUS_CODE_REASON_PHRASE.n200_ok,
                None,
                None
            ).unwrap();

        }

        let mut cors_header_list : Vec<Header>;
        let is_cors_set_to_allow_all_requests : bool = env::var(Config::RWS_CONFIG_CORS_ALLOW_ALL).unwrap().parse().unwrap();
        if is_cors_set_to_allow_all_requests {
            cors_header_list = Cors::allow_all(&request).unwrap();
        } else {
            cors_header_list = Cors::process_using_default_config(&request).unwrap();
        }

        response.headers.append(&mut cors_header_list);


        (response, request)
    }

    pub(crate) fn process_static_resources(request: &Request) -> Result<Vec<ContentRange>, Error> {
        let dir = env::current_dir().unwrap();
        let working_directory = dir.as_path().to_str().unwrap();
        let static_filepath = [working_directory, request.request_uri.as_str()].join(SYMBOL.empty_string);

        let mut content_range_list = Vec::new();

        let boxed_file = File::open(&static_filepath);
        if boxed_file.is_ok()  {
            let md = metadata(&static_filepath).unwrap();
            if md.is_file() {
                let mut range_header = &Header {
                    name: Header::RANGE.to_string(),
                    value: "bytes=0-".to_string()
                };

                let boxed_header = request.get_header(Header::RANGE.to_string());
                if boxed_header.is_some() {
                    range_header = boxed_header.unwrap();
                }

                let boxed_content_range_list = Range::get_content_range_list(&request.request_uri, range_header);
                if boxed_content_range_list.is_ok() {
                    content_range_list = boxed_content_range_list.unwrap();
                } else {
                    let error = boxed_content_range_list.err().unwrap();
                    return Err(error)
                }
            }
        }

        Ok(content_range_list)
    }
}