use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

pub struct FormGetMethodController;

// TODO:
impl FormGetMethodController {

    pub fn is_matching_request(request: &Request) -> bool {
        request.request_uri == "/form-get-method" && request.method == METHOD.get
    }

    pub fn process_request(_request: &Request, mut response: Response) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();

        response
    }
}