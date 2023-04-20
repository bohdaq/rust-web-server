use crate::client_hint::ClientHint;
use crate::cors::Cors;
use crate::ext::date_time_ext::DateTimeExt;
use crate::ext::string_ext::StringExt;
use crate::range::Range;
use crate::request::Request;
use crate::symbol::SYMBOL;

#[cfg(test)]
mod tests;

pub mod content_disposition;

#[cfg(test)]
mod example;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Header {
    pub name: String,
    pub value: String,
}

impl Header {

    pub fn as_string(&self) -> String {
        let formatted = format!("{}: {}", self.name, self.value);
        formatted
    }

    pub const _ACCESS_CONTROL_REQUEST_METHOD: &'static str = "Access-Control-Request-Method";
    pub const _ACCESS_CONTROL_REQUEST_HEADERS: &'static str = "Access-Control-Request-Headers";
    pub const _ACCESS_CONTROL_ALLOW_ORIGIN: &'static str = "Access-Control-Allow-Origin";
    pub const _ACCESS_CONTROL_ALLOW_METHODS: &'static str = "Access-Control-Allow-Methods";
    pub const _ACCESS_CONTROL_ALLOW_HEADERS: &'static str = "Access-Control-Allow-Headers";
    pub const _ACCESS_CONTROL_ALLOW_CREDENTIALS: &'static str = "Access-Control-Allow-Credentials";
    pub const _ACCESS_CONTROL_MAX_AGE: &'static str = "Access-Control-Max-Age";
    pub const _ACCESS_CONTROL_EXPOSE_HEADERS: &'static str = "Access-Control-Expose-Headers";

    pub const _ACCEPT: &'static str = "Accept";
    pub const _ACCEPT_RANGES: &'static str = "Accept-Ranges";
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
    pub const _CONTENT_TYPE: &'static str = "Content-Type";
    pub const _CONTENT_LENGTH: &'static str = "Content-Length";
    pub const _CONTENT_RANGE: &'static str = "Content-Range";
    pub const _CONTENT_DISPOSITION: &'static str = "Content-Disposition";
    pub const _CONTENT_ENCODING: &'static str = "Content-Encoding";
    pub const _CONTENT_LANGUAGE: &'static str = "Content-Language";
    pub const _CONTENT_LOCATION: &'static str = "Content-Location";
    pub const _CONTENT_SECURITY_POLICY: &'static str = "Content-Security-Policy";
    pub const _CONTENT_SECURITY_POLICY_REPORT_ONLY: &'static str = "Content-Security-Policy-Report-Only";
    pub const _COOKIE: &'static str = "Cookie";
    pub const _CROSS_ORIGIN_EMBEDDER_POLICY: &'static str = "Cross-Origin-Embedder-Policy";
    pub const _CROSS_ORIGIN_OPENER_POLICY: &'static str = "Cross-Origin-Opener-Policy";
    pub const _CROSS_ORIGIN_RESOURCE_POLICY: &'static str = "Cross-Origin-Resource-Policy";
    pub const _DATE: &'static str = "Date";
    pub const _DATE_UNIX_EPOCH_NANOS: &'static str = "Date-Unix-Epoch-Nanos";
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
    pub const _LAST_MODIFIED_UNIX_EPOCH_NANOS: &'static str = "Last-Modified-Unix-Epoch-Nanos";
    pub const _LINK: &'static str = "Link";
    pub const _LOCATION: &'static str = "Location";
    pub const _MAX_FORWARDS: &'static str = "Max-Forwards";
    pub const _NEL: &'static str = "NEL";
    pub const _ORIGIN: &'static str = "Origin";
    pub const _PROXY_AUTHENTICATE: &'static str = "Proxy-Authenticate";
    pub const _PROXY_AUTHORIZATION: &'static str = "Proxy-Authorization";
    pub const _RANGE: &'static str = "Range";
    pub const _REFERER: &'static str = "Referer";
    pub const _REFERRER_POLICY: &'static str = "Referrer-Policy";
    pub const _RETRY_AFTER: &'static str = "Retry-After";
    pub const _RTT: &'static str = "RTT";
    pub const _SAVE_DATA: &'static str = "Save-Data";
    pub const _SEC_CH_UA: &'static str = "Sec-CH-UA";
    pub const _SEC_CH_UA_ARCH: &'static str = "Sec-CH-UA-Arch";
    pub const _SEC_CH_UA_BITNESS: &'static str = "Sec-CH-UA-Bitness";
    pub const _SEC_CH_UA_FULL_VERSION_LIST: &'static str = "Sec-CH-UA-Full-Version-List";
    pub const _SEC_CH_UA_MOBILE: &'static str = "Sec-CH-UA-Mobile";
    pub const _SEC_CH_UA_MODEL: &'static str = "Sec-CH-UA-Model";
    pub const _SEC_CH_UA_PLATFORM: &'static str = "Sec-CH-UA-Platform";
    pub const _SEC_CH_UA_PLATFORM_VERSION: &'static str = "Sec-CH-UA-Platform-Version";
    pub const _SEC_FETCH_DEST: &'static str = "Sec-Fetch-Dest";
    pub const _SEC_FETCH_MODE: &'static str = "Sec-Fetch-Mode";
    pub const _SEC_FETCH_SITE: &'static str = "Sec-Fetch-Site";
    pub const _SEC_FETCH_USER: &'static str = "Sec-Fetch-User";
    pub const _SEC_GPC: &'static str = "Sec-GPC";
    pub const _SERVER: &'static str = "Server";
    pub const _SERVER_TIMING: &'static str = "Server-Timing";
    pub const _SERVICE_WORKER_NAVIGATION_PRELOAD: &'static str = "Service-Worker-Navigation-Preload";
    pub const _SET_COOKIE: &'static str = "Set-Cookie";
    pub const _SOURCE_MAP: &'static str = "SourceMap";
    pub const _STRICT_TRANSPORT_SECURITY: &'static str = "Strict-Transport-Security";
    pub const _TE: &'static str = "TE";
    pub const _TIMING_ALLOW_ORIGIN: &'static str = "Timing-Allow-Origin";
    pub const _TRAILER: &'static str = "Trailer";
    pub const _TRANSFER_ENCODING: &'static str = "Transfer-Encoding";
    pub const _UPGRADE: &'static str = "Upgrade";
    pub const _UPGRADE_INSECURE_REQUESTS: &'static str = "Upgrade-Insecure-Requests";
    pub const _USER_AGENT: &'static str = "User-Agent";
    pub const _VARY: &'static str = "Vary";
    pub const _VIA: &'static str = "Via";
    pub const _WANT_DIGEST: &'static str = "Want-Digest";
    pub const _WWW_AUTHENTICATE: &'static str = "WWW-Authenticate";
    pub const _X_CONTENT_TYPE_OPTIONS: &'static str = "X-Content-Type-Options";
    pub const _X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF: &'static str = "nosniff";
    pub const _X_FRAME_OPTIONS: &'static str = "X-Frame-Options";
    pub const _X_FRAME_OPTIONS_VALUE_DENY: &'static str = "DENY";
    pub const _X_FRAME_OPTIONS_VALUE_SAME_ORIGIN: &'static str = "SAMEORIGIN";




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

        let critical_client_hint_header = ClientHint::get_critical_client_hints_header();
        header_list.push(critical_client_hint_header);

        let client_hint_vary = ClientHint::get_vary_header_value();
        vary_value.push(client_hint_vary);

        let vary_header = Header { name: Header::_VARY.to_string(), value: vary_value.join(", ") };
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
            name: Header::_X_CONTENT_TYPE_OPTIONS.to_string(),
            value: Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF.to_string(),
        }
    }

    pub fn get_accept_ranges_header() -> Header {
        Header {
            name: Header::_ACCEPT_RANGES.to_string(),
            value: Range::BYTES.to_string(),
        }
    }

    pub fn get_x_frame_options_header() -> Header {
        Header {
            name: Header::_X_FRAME_OPTIONS.to_string(),
            value: Header::_X_FRAME_OPTIONS_VALUE_SAME_ORIGIN.to_string(),
        }
    }

    pub fn get_date_iso_8601_header() -> Header {
        Header {
            name: Header::_DATE_UNIX_EPOCH_NANOS.to_string(),
            value: DateTimeExt::_now_unix_epoch_nanos().to_string(),
        }
    }

    pub fn parse_header(raw_header: &str) -> Result<Header, String> {
        let escaped_header = StringExt::filter_ascii_control_characters(raw_header);
        let escaped_header = StringExt::truncate_new_line_carriage_return(&escaped_header);

        let boxed_split = escaped_header.split_once(SYMBOL.colon);
        if boxed_split.is_none() {
            let message = format!("Unable to parse header: {}", escaped_header);
            return Err(message)
        }

        let (name, value) = boxed_split.unwrap();

        let header = Header {
            name: name.trim().to_string(),
            value: value.trim().to_string(),
        };

        Ok(header)
    }

    pub fn parse(raw_header: &str) -> Result<Header, String> {
        Header::parse_header(raw_header)
    }

    pub fn generate(&self) -> String {
        self.as_string()
    }

}

