use crate::body::multipart_form_data::FormMultipartData;
use crate::ext::string_ext::StringExt;
use crate::header::Header;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};
use crate::request::{METHOD, Request};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::symbol::SYMBOL;

// TODO: wip
pub struct FormMultipartEnctypePostMethodController;

impl FormMultipartEnctypePostMethodController {
    pub const MULTIPART_FORM_DATA_CONTENT_TYPE: &'static str = "multipart/form-data; boundary=";

    pub fn is_matching_request(request: &Request) -> bool {
        let boxed_content_type = request.get_header(Header::_CONTENT_TYPE.to_string());
        if boxed_content_type.is_none() { return false; }

        let boxed_path = request.get_uri_path();
        if boxed_path.is_err() {
            let message = format!("unable to get path {}", boxed_path.err().unwrap());
            eprintln!("{}", message);
            return false
        }

        let path = boxed_path.unwrap();

        let content_type_header = boxed_content_type.unwrap();
        let is_form_multipart_content_type =
            StringExt::filter_ascii_control_characters(&content_type_header.value.to_lowercase())
                .starts_with(FormMultipartEnctypePostMethodController::MULTIPART_FORM_DATA_CONTENT_TYPE);
        if !is_form_multipart_content_type { return false }

        path == "/form-multipart-enctype-post-method" && request.method == METHOD.post
    }

    pub fn process_request(_request: &Request, mut response: Response) -> Response {
        response.status_code = *STATUS_CODE_REASON_PHRASE.n200_ok.status_code;
        response.reason_phrase = STATUS_CODE_REASON_PHRASE.n200_ok.reason_phrase.to_string();


        // here is the list of parts, as an example here it is printed in the response body
        // TODO: error-handling
        let content_type = _request.get_header(Header::_CONTENT_TYPE.to_string()).unwrap();
        let boundary = FormMultipartData::extract_boundary(&content_type.value).unwrap();
        let part_list = FormMultipartData::parse(&_request.body.clone(), boundary).unwrap();

        let mut formatted_list : Vec<String> = vec![];
        for part in part_list.into_iter() {
            let content_disposition = _request.get_header(Header::_CONTENT_DISPOSITION.to_string()).unwrap();
            let formatted_output = format!("Content-Disposition: {} | Length:  {}{}", content_disposition.value, part.body.len(), SYMBOL.new_line_carriage_return);
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