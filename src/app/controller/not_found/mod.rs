use file_ext::FileExt;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

pub struct NotFoundController;

impl NotFoundController {
    pub const NOT_FOUND_FILEPATH: &'static str = "404.html";

    pub fn is_matching_request(_request: &Request) -> bool {
        true
    }

    pub fn process_request(_request: &Request, mut response: Response) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase.to_string();


        if FileExt::does_file_exist(NotFoundController::NOT_FOUND_FILEPATH) {
            let boxed_content_range =
                Range::get_content_range_of_a_file(NotFoundController::NOT_FOUND_FILEPATH);

            if boxed_content_range.is_ok() {
                let content_range = boxed_content_range.unwrap();
                let content_range_list = vec![content_range];
                response.content_range_list = content_range_list;
            } else {
                let error = boxed_content_range.err().unwrap();
                let mime_type = MimeType::TEXT_HTML.to_string();
                let content_range = Range::get_content_range(
                    Vec::from(error.as_bytes()),
                    mime_type
                );

                let content_range_list = vec![content_range];
                response.content_range_list = content_range_list;
                response.status_code = *STATUS_CODE_REASON_PHRASE.n500_internal_server_error.status_code;
                response.reason_phrase = STATUS_CODE_REASON_PHRASE.n500_internal_server_error.reason_phrase.to_string();
            }
        } else {
            let not_found_file = include_bytes!("404.html");

            let content_range =
                Range::get_content_range(not_found_file.to_vec(), MimeType::TEXT_HTML.to_string());


            let content_range_list = vec![content_range];
            response.content_range_list = content_range_list;

        }

        response
    }
}