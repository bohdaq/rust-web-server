use crate::client_hint::ClientHint;
use crate::cors::Cors;
use crate::ext::date_time_ext::DateTimeExt;
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
    pub const X_FRAME_OPTIONS: &'static str = "X-Frame-Options";
    pub const _X_FRAME_OPTIONS_VALUE_DENY: &'static str = "DENY";
    pub const X_FRAME_OPTIONS_VALUE_SAME_ORIGIN: &'static str = "SAMEORIGIN";
    pub const RANGE: &'static str = "Range";
    pub const ACCEPT_RANGES: &'static str = "Accept-Ranges";
    pub const CONTENT_RANGE: &'static str = "Content-Range";
    pub const _HOST: &'static str = "Host";
    pub const VARY: &'static str = "Vary";
    pub const ORIGIN: &'static str = "Origin";
    pub const _DATE: &'static str = "Date";
    pub const DATE_ISO_8601: &'static str = "Date-ISO-8601";

    pub const ACCESS_CONTROL_REQUEST_METHOD: &'static str = "Access-Control-Request-Method";
    pub const ACCESS_CONTROL_REQUEST_HEADERS: &'static str = "Access-Control-Request-Headers";
    pub const ACCESS_CONTROL_ALLOW_ORIGIN: &'static str = "Access-Control-Allow-Origin";
    pub const ACCESS_CONTROL_ALLOW_METHODS: &'static str = "Access-Control-Allow-Methods";
    pub const ACCESS_CONTROL_ALLOW_HEADERS: &'static str = "Access-Control-Allow-Headers";
    pub const ACCESS_CONTROL_ALLOW_CREDENTIALS: &'static str = "Access-Control-Allow-Credentials";
    pub const ACCESS_CONTROL_MAX_AGE: &'static str = "Access-Control-Max-Age";
    pub const ACCESS_CONTROL_EXPOSE_HEADERS: &'static str = "Access-Control-Expose-Headers";

    pub const _ACCEPT: &'static str = "Accept";
    pub const _ACCEPT_CH: &'static str = "Accept-CH";
    pub const _ACCEPT_ENCODING: &'static str = "Accept-Encoding";
    pub const _ACCEPT_LANGUAGE: &'static str = "Accept-Language";
    pub const _ACCEPT_PATCH: &'static str = "Accept-Patch";
    pub const _ACCEPT_POST: &'static str = "Accept-Post";
    pub const _AGE: &'static str = "Age";
    pub const _ALLOW: &'static str = "Allow";
    pub const _ALT_SVC: &'static str = "Alt-Svc";



    pub const NAME_VALUE_SEPARATOR: &'static str = ": ";



    pub fn get_header_list(request: &Request) -> Vec<Header> {
        let mut header_list : Vec<Header>;
        let mut vary_value : Vec<String>;

        let cors_vary = Cors::get_vary_header_value();
        vary_value = vec![cors_vary];
        let cors_header_list: Vec<Header> = Cors::get_headers(&request);
        header_list = cors_header_list;

        let client_hint_header = ClientHint::get_accept_client_hints_header();
        header_list.push(client_hint_header);

        let client_hint_vary = ClientHint::get_vary_header_value();
        vary_value.push(client_hint_vary);

        let vary_header = Header { name: Header::VARY.to_string(), value: vary_value.join(", ") };
        header_list.push(vary_header);

        let x_content_type_options_header = Header::get_x_content_type_options_header();
        header_list.push(x_content_type_options_header);

        let accept_ranges_header = Header::get_accept_ranges_header();
        header_list.push(accept_ranges_header);

        let x_frame_options_header = Header::get_x_frame_options_header();
        header_list.push(x_frame_options_header);

        let date_iso_8601_header = Header::get_date_iso_8601_header();
        header_list.push(date_iso_8601_header);

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

    pub fn get_x_frame_options_header() -> Header {
        Header {
            name: Header::X_FRAME_OPTIONS.to_string(),
            value: Header::X_FRAME_OPTIONS_VALUE_SAME_ORIGIN.to_string(),
        }
    }

    pub fn get_date_iso_8601_header() -> Header {
        Header {
            name: Header::DATE_ISO_8601.to_string(),
            value: DateTimeExt::_now_utc().to_string(),
        }
    }

}

