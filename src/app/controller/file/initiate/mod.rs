use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};

pub struct FileUploadInitiateController;

//TODO:
impl FileUploadInitiateController {

    pub fn is_matching_request(request: &Request) -> bool {

        let boxed_path = request.get_uri_path();
        if boxed_path.is_err() {
            let message = format!("unable to get path {} {} {}", request.method, request.request_uri, boxed_path.err().unwrap());
            eprintln!("{}", message);
            return false
        }

        let path = boxed_path.unwrap();

        path == "/file-upload/initiate" && request.method == METHOD.post
    }

    pub fn process_request(_request: &Request, mut response: Response) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();


        response
    }
}