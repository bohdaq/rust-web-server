use std::collections::HashMap;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::symbol::SYMBOL;
use crate::entry_point::get_request_allocation_size;

pub struct FileUpload {
    pub name: String,
    pub last_modified: u128,
    pub size: u128
}

impl FileUpload {

}

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
        response.status_code = *STATUS_CODE_REASON_PHRASE.n400_bad_request.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n400_bad_request.reason_phrase.to_string();


        let boxed_query_option = _request.get_uri_query();
        if boxed_query_option.is_err() {
            let error_message = boxed_query_option.clone().err().unwrap().to_string();
            eprintln!("unable to extract query from url: {}", error_message)
        }
        let query_option = boxed_query_option.unwrap();
        if query_option.is_none() {
            return response;
        }

        let form: HashMap<String, String> = query_option.unwrap();
        if form.get("name").is_none() {
            return response;
        }
        if form.get("lastModified").is_none() {
            return response;
        }
        if form.get("size").is_none() {
            return response;
        }


        let mut formatted_list : Vec<String> = vec![];
        for (key, value) in form.into_iter() {
            let formatted_output = format!("{} is {}{}", key, value, SYMBOL.new_line_carriage_return);
            formatted_list.push(formatted_output);
        }

        let mut request_allocation_size = get_request_allocation_size();
        let offset = 4000;
        if request_allocation_size > offset {
            request_allocation_size = get_request_allocation_size() - offset;
        }
        let formatted_output = format!("{} is {}{}", "request_allocation_size_in_bytes", request_allocation_size, SYMBOL.new_line_carriage_return);
        formatted_list.push(formatted_output);


        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();

        let response_body = formatted_list.join(SYMBOL.empty_string);
        response.content_range_list = vec![
            ContentRange{
                unit: Range::BYTES.to_string(),
                range: Range { start: 0, end: response_body.len() as u64 },
                size: response_body.len().to_string(),
                body: Vec::from(response_body.as_bytes()),
                content_type: MimeType::TEXT_PLAIN.to_string(),
            }
        ];

        response
    }
}