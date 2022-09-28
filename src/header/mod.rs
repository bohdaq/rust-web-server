use crate::client_hint::ClientHint;
use crate::cors::Cors;
use crate::range::Range;
use crate::request::Request;

#[cfg(test)]
mod tests;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Header {
    pub name: String,
    pub value: String,
}

impl Header {
    pub const CONTENT_TYPE: &'static str = "Content-Type";
    pub const CONTENT_LENGTH: &'static str = "Content-Length";
    pub const X_CONTENT_TYPE_OPTIONS: &'static str = "X-Content-Type-Options";
    pub const X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF: &'static str = "nosniff";
    pub const RANGE: &'static str = "Range";
    pub const ACCEPT_RANGES: &'static str = "Accept-Ranges";
    pub const CONTENT_RANGE: &'static str = "Content-Range";
    pub const _HOST: &'static str = "Host";
    pub const VARY: &'static str = "Vary";
    pub const ORIGIN: &'static str = "Origin";

    pub const ACCESS_CONTROL_REQUEST_METHOD: &'static str = "Access-Control-Request-Method";
    pub const ACCESS_CONTROL_REQUEST_HEADERS: &'static str = "Access-Control-Request-Headers";
    pub const ACCESS_CONTROL_ALLOW_ORIGIN: &'static str = "Access-Control-Allow-Origin";
    pub const ACCESS_CONTROL_ALLOW_METHODS: &'static str = "Access-Control-Allow-Methods";
    pub const ACCESS_CONTROL_ALLOW_HEADERS: &'static str = "Access-Control-Allow-Headers";
    pub const ACCESS_CONTROL_ALLOW_CREDENTIALS: &'static str = "Access-Control-Allow-Credentials";
    pub const ACCESS_CONTROL_MAX_AGE: &'static str = "Access-Control-Max-Age";
    pub const ACCESS_CONTROL_EXPOSE_HEADERS: &'static str = "Access-Control-Expose-Headers";


    pub const NAME_VALUE_SEPARATOR: &'static str = ": ";



    pub fn get_header_list(request: &Request) -> Vec<Header> {
        let mut header_list : Vec<Header>;
        let mut vary_value : Vec<String>;

        let cors_vary = Cors::get_vary_header_value();
        vary_value = vec![cors_vary];
        let cors_header_list: Vec<Header> = Cors::get_headers(&request);
        header_list = cors_header_list;

        let boxed_client_hint_header = ClientHint::get_accept_client_hints_header();
        if boxed_client_hint_header.is_some() {
            let client_hint_vary = ClientHint::get_vary_header_value();
            vary_value.push(client_hint_vary);
            let client_hint_header = boxed_client_hint_header.unwrap();
            header_list.push(client_hint_header);
        }

        let vary_header = Header { name: Header::VARY.to_string(), value: vary_value.join(", ") };
        header_list.push(vary_header);

        let x_content_type_options_header = Header::get_x_content_type_options_header();
        header_list.push(x_content_type_options_header);

        let accept_ranges_header = Header::get_accept_ranges_header();
        header_list.push(accept_ranges_header);

        header_list
    }


    pub fn get_x_content_type_options_header() -> Header {
        Header {
            name: Header::X_CONTENT_TYPE_OPTIONS.to_string(),
            value: Header::X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF.to_string(),
        }
    }

    pub fn get_accept_ranges_header() -> Header {
        Header {
            name: Header::ACCEPT_RANGES.to_string(),
            value: Range::BYTES.to_string(),
        }
    }

}

