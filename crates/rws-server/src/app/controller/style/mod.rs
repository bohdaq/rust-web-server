use file_ext::FileExt;
use crate::controller::Controller;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

pub struct StyleController;

impl Controller for StyleController {
    fn is_matching(request: &Request, _connection: &ConnectionInfo) -> bool {
        request.method == METHOD.get &&request.request_uri == "/style.css"
    }

    fn process(_request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();


        if FileExt::does_file_exist(StyleController::STYLE_FILEPATH) {
            let boxed_content_range =
                Range::get_content_range_of_a_file(StyleController::STYLE_FILEPATH);

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
            let style_file = include_bytes!("style.css");

            let content_range =
                Range::get_content_range(style_file.to_vec(), MimeType::TEXT_CSS.to_string());


            let content_range_list = vec![content_range];
            response.content_range_list = content_range_list;

        }

        response
    }
}

// backward compatability
impl StyleController {
    pub const STYLE_FILEPATH: &'static str = "style.css";

    pub fn is_matching_request(request: &Request) -> bool {
        request.request_uri == "/style.css"
    }

    pub fn process_request(_request: &Request, mut response: Response) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();


        if FileExt::does_file_exist(StyleController::STYLE_FILEPATH) {
            let boxed_content_range =
                Range::get_content_range_of_a_file(StyleController::STYLE_FILEPATH);

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
            let style_file = include_bytes!("style.css");

            let content_range =
                Range::get_content_range(style_file.to_vec(), MimeType::TEXT_CSS.to_string());


            let content_range_list = vec![content_range];
            response.content_range_list = content_range_list;

        }

        response
    }
}