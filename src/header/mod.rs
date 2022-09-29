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
    pub const VARY: &'static str = "Vary";
    pub const ORIGIN: &'static str = "Origin";
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
    pub const _AUTHORIZATION: &'static str = "Authorization";
    pub const _CACHE_CONTROL: &'static str = "Cache-Control";
    pub const _CLEAR_SITE_DATA: &'static str = "Clear-Site-Data";
    pub const _CONTENT_DISPOSITION: &'static str = "Content-Disposition";
    pub const _CONTENT_ENCODING: &'static str = "Content-Encoding";
    pub const _CONTENT_LANGUAGE: &'static str = "Content-Language";
    pub const _CONTENT_LENGTH: &'static str = "Content-Length";
    pub const _CONTENT_LOCATION: &'static str = "Content-Location";
    pub const _CONTENT_SECURITY_POLICY: &'static str = "Content-Security-Policy";
    pub const _CONTENT_SECURITY_POLICY_REPORT_ONLY: &'static str = "Content-Security-Policy-Report-Only";
    pub const _COOKIE: &'static str = "Cookie";
    pub const _CROSS_ORIGIN_EMBEDDER_POLICY: &'static str = "Cross-Origin-Embedder-Policy";
    pub const _CROSS_ORIGIN_OPENER_POLICY: &'static str = "Cross-Origin-Opener-Policy";
    pub const _CROSS_ORIGIN_RESOURCE_POLICY: &'static str = "Cross-Origin-Resource-Policy";
    pub const _DATE: &'static str = "Date";
    pub const _DEVICE_MEMORY: &'static str = "Device-Memory";
    pub const _DIGEST: &'static str = "Digest";
    pub const _DOWNLINK: &'static str = "Downlink";
    pub const _EARLY_DATA: &'static str = "Early-Data";
    pub const _ECT: &'static str = "ECT";
    pub const _ETAG: &'static str = "ETag";
    pub const _EXPECT: &'static str = "Expect";
    pub const _EXPIRES: &'static str = "Expires";
    pub const _FEATURE_POLICY: &'static str = "Feature-Policy";
    pub const _PERMISSIONS_POLICY: &'static str = "Permissions-Policy";
    pub const _FORWARDED: &'static str = "Forwarded";
    pub const _FROM: &'static str = "From";
    pub const _HOST: &'static str = "Host";
    pub const _IF_MATCH: &'static str = "If-Match";
    pub const _IF_MODIFIED_SINCE: &'static str = "If-Modified-Since";
    pub const _IF_NONE_MATCH: &'static str = "If-None-Match";
    pub const _IF_RANGE: &'static str = "If-Range";
    pub const _IF_UNMODIFIED_SINCE: &'static str = "If-Unmodified-Since";
    pub const _LAST_MODIFIED: &'static str = "Last-Modified";
    pub const _LINK: &'static str = "Link";
    pub const _LOCATION: &'static str = "Location";
    pub const _MAX_FORWARDS: &'static str = "Max-Forwards";
    pub const _NEL: &'static str = "NEL";





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

