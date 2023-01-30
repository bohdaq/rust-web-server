use std::collections::HashMap;
use crate::body::form_urlencoded::FormUrlEncoded;
use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::symbol::SYMBOL;

pub struct FormUrlEncodedEnctypePostMethodController;

impl FormUrlEncodedEnctypePostMethodController {
    pub const FORM_URL_ENCODED_CONTENT_TYPE: &'static str = "application/x-www-form-urlencoded";

    pub fn is_matching_request(request: &Request) -> bool {
        let boxed_content_type = request.get_header(Header::_CONTENT_TYPE.to_string());
        if boxed_content_type.is_none() { return false; }

        let content_type_header = boxed_content_type.unwrap();
        let is_form_url_encoded_content_type =
            content_type_header.value.to_lowercase()
                .eq(FormUrlEncodedEnctypePostMethodController::FORM_URL_ENCODED_CONTENT_TYPE);
        if !is_form_url_encoded_content_type { return false }

        request.request_uri == "/form-url-encoded-enctype-post-method" && request.method == METHOD.post
    }

    pub fn process_request(_request: &Request, mut response: Response) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();

        let boxed_body_string = String::from_utf8(_request.body.clone());
        if boxed_body_string.is_err() {
            let message = boxed_body_string.clone().err().unwrap().to_string();
            response.status_code = *STATUS_CODE_REASON_PHRASE.n400_bad_request.status_code;
            response.reason_phrase = STATUS_CODE_REASON_PHRASE.n400_bad_request.reason_phrase.to_string();
            response.content_range_list = vec![
              ContentRange{
                  unit: Range::BYTES.to_string(),
                  range: Range { start: 0, end: message.len() as u64 },
                  size: message.len().to_string(),
                  body: Vec::from(message.as_bytes()),
                  content_type: MimeType::TEXT_PLAIN.to_string(),
              }
            ];
        }

        // direct unwrap due to prior utf-8 encoding check
        // here is the form data, as an example here it is printed in the response body
        let form : HashMap<String, String> = FormUrlEncoded::parse(_request.body.clone()).unwrap();

        let mut formatted_list : Vec<String> = vec![];
        for (key, value) in form.into_iter() {
            let formatted_output = format!("{} is {}{}", key, value, SYMBOL.new_line_carriage_return);
            formatted_list.push(formatted_output);
        }

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