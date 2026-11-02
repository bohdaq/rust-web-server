use file_ext::FileExt;
use crate::controller::Controller;
use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::Range;
use crate::request::Request;
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::server::ConnectionInfo;

pub struct NotFoundController;

impl Controller for NotFoundController {
    fn is_matching(_request: &Request, _connection: &ConnectionInfo) -> bool {
        true
    }

    fn process(request: &Request, mut response: Response, _connection: &ConnectionInfo) -> Response {
        // A file that exists, asked for with a method static files don't
        // take (POST, PUT, DELETE…): 405 with the methods that work, not
        // 404 (RFC 9110 §15.5.6, #56).
        if NotFoundController::is_static_path_with_other_method(request) {
            response.status_code = *STATUS_CODE_REASON_PHRASE.n405_method_not_allowed.status_code;
            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n405_method_not_allowed.reason_phrase.to_string();
            response.headers.push(Header { name: Header::_ALLOW.into(), value: NotFoundController::STATIC_ALLOW.into() });
            response.content_range_list = vec![Range::get_content_range(
                b"405 Method Not Allowed".to_vec(),
                MimeType::TEXT_PLAIN.to_string(),
            )];
            return response;
        }

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

impl NotFoundController {
    /// The `Allow` value for a static file.
    pub const STATIC_ALLOW: &'static str = "GET, HEAD, OPTIONS";

    /// `true` when the request names a file or directory the static server
    /// would serve, with a method other than GET, HEAD or OPTIONS.
    fn is_static_path_with_other_method(request: &Request) -> bool {
        use crate::request::METHOD;
        if request.method == METHOD.get || request.method == METHOD.head || request.method == METHOD.options {
            return false;
        }
        let Ok(components) = crate::url::URL::parse_request_target(&request.request_uri) else { return false };
        if components.path == "/" {
            return false;
        }
        let Ok(fs_path) = crate::app::controller::static_resource::static_fs_path(&components.path) else { return false };
        std::fs::metadata(&fs_path).is_ok() && crate::app::controller::static_resource::inside_root(&fs_path)
    }
}

// backward compatability
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