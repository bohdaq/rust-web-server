use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

pub struct NotFoundController;

impl NotFoundController {
    pub const NOT_FOUND_PAGE_FILEPATH: &'static str = "404.html";

    pub fn is_matching_request(_request: &Request) -> bool {
        //not found controller is called when other url matches failed
        true
    }

    pub fn process_request(_request: &Request, mut response: Response) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n404_not_found.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n404_not_found.reason_phrase.to_string();

        let boxed_content_range =
            Range::get_content_range_of_a_file(NotFoundController::NOT_FOUND_PAGE_FILEPATH);

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
        }
        response
    }
}