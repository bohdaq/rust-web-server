use std::borrow::Cow;
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

/// An HTTP header name/value pair.
///
/// Use the associated string constants (e.g. [`Header::_CONTENT_TYPE`]) for header names.
/// [`Header::get_header_list`] returns the standard set of response headers (client hints,
/// CORS, cache control, security headers) that should be included on every response.
///
/// Name and value are `Cow<'static, str>` (18.0.0; `String` before): the
/// built-in names and fixed values are borrowed `&'static str`s, so building
/// and cloning the standard header set allocates nothing. Construct with
/// [`Header::new`] (or `.into()` on each field); read through `Deref<Target =
/// str>` (`&*header.value`, `header.value.trim()`, `header.value == "x"`).
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Header {
    /// Header name, e.g. `"Content-Type"`.
    pub name: Cow<'static, str>,
    /// Header value, e.g. `"application/json"`.
    pub value: Cow<'static, str>,
}

/// Headers every built-in response carries, around the per-request `Date`.
struct FixedHeaders {
    before_date: Vec<Header>,
    after_date: Vec<Header>,
}

impl Header {

    /// A header from anything that converts into `Cow<'static, str>`: a
    /// `&'static str` (borrowed, no allocation) or a `String` (owned).
    ///
    /// ```
    /// use rust_web_server::header::Header;
    /// let h = Header::new(Header::_CONTENT_TYPE, format!("text/{}", "plain"));
    /// assert_eq!("text/plain", h.value);
    /// ```
    pub fn new(name: impl Into<Cow<'static, str>>, value: impl Into<Cow<'static, str>>) -> Header {
        Header { name: name.into(), value: value.into() }
    }

    /// A header name taken from a parsed message: borrowed when it is one
    /// of the common names spelled exactly as listed (canonical or lower
    /// case — the forms clients and servers actually send), owned otherwise.
    /// Same text either way; it only saves the allocation.
    pub(crate) fn name_from(name: &str) -> Cow<'static, str> {
        const COMMON: &[&str] = &[
            "Host", "host", "User-Agent", "user-agent", "Accept", "accept",
            "Accept-Encoding", "accept-encoding", "Accept-Language", "accept-language",
            "Connection", "connection", "Content-Type", "content-type",
            "Content-Length", "content-length", "Date", "date", "Server", "server",
            "Last-Modified", "last-modified", "ETag", "etag", "Accept-Ranges", "accept-ranges",
            "Cache-Control", "cache-control", "Cookie", "cookie", "Set-Cookie", "set-cookie",
            "Referer", "referer", "Origin", "origin", "Authorization", "authorization",
            "If-None-Match", "if-none-match", "If-Modified-Since", "if-modified-since",
            "Transfer-Encoding", "transfer-encoding", "Keep-Alive", "keep-alive",
            "Vary", "vary", "Upgrade-Insecure-Requests", "upgrade-insecure-requests",
            "X-Forwarded-For", "x-forwarded-for", "Via", "via",
        ];
        match COMMON.iter().find(|common| **common == name) {
            Some(common) => Cow::Borrowed(common),
            None => Cow::Owned(name.to_string()),
        }
    }

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
    pub const _CONNECTION: &'static str = "Connection";
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

    pub const _STRICT_TRANSPORT_SECURITY_VALUE_DEFAULT: &'static str = "max-age=31536000; includeSubDomains";
    pub const _REFERRER_POLICY_VALUE_DEFAULT: &'static str = "strict-origin-when-cross-origin";
    pub const _PERMISSIONS_POLICY_VALUE_DEFAULT: &'static str = "geolocation=(), microphone=(), camera=()";
    pub const _CONTENT_SECURITY_POLICY_VALUE_DEFAULT: &'static str = "default-src 'self'";




    pub const NAME_VALUE_SEPARATOR: &'static str = ": ";

    pub const _DO_NOT_STORE_CACHE: &'static str = "no-store, no-cache, private, max-age=0, must-revalidate, proxy-revalidate";



    /// Build the standard response header list using settings from
    /// [`ServerConfig`].
    ///
    /// Prefer this method inside [`App::execute`] and any call-site that has
    /// an `App`-level config available. It avoids reading environment variables
    /// at request time, which enables proper test isolation via
    /// [`App::with_config`].
    pub fn get_header_list_with_config(
        request: &Request,
        config: &crate::server_config::ServerConfig,
    ) -> Vec<Header> {
        // CORS depends on the request and the date changes; everything else
        // depends only on the CSP setting, so it's built once per CSP value
        // and cloned (joining the client-hint and Vary lists per request was
        // a measurable share of a small response's CPU).
        let fixed = Header::fixed_headers(&config.csp, config.client_hints);
        let mut header_list = Cors::get_headers_from_config(request, config);
        header_list.reserve(fixed.before_date.len() + 2 + fixed.after_date.len());
        header_list.extend(fixed.before_date.iter().cloned());
        header_list.push(Header { name: Header::_DATE.into(), value: crate::http::date::now().into() });
        header_list.push(Header::get_date_iso_8601_header());
        header_list.extend(fixed.after_date.iter().cloned());
        header_list
    }

    /// The request-independent part of [`Header::get_header_list_with_config`]
    /// for a CSP value and client-hint setting, cached for the last seen.
    fn fixed_headers(csp: &str, client_hints: bool) -> std::sync::Arc<FixedHeaders> {
        use std::sync::{Arc, RwLock};
        static CACHE: RwLock<Option<(String, bool, Arc<FixedHeaders>)>> = RwLock::new(None);
        if let Some((cached_csp, cached_hints, fixed)) = CACHE.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if cached_csp == csp && *cached_hints == client_hints {
                return Arc::clone(fixed);
            }
        }
        // Client hints are opt-in (RWS_CONFIG_CLIENT_HINTS): ~700 bytes on
        // every response otherwise, and Critical-CH makes Chromium retry.
        let mut before_date = Vec::with_capacity(6);
        let vary = if client_hints {
            before_date.push(ClientHint::get_accept_client_hints_header());
            before_date.push(ClientHint::get_critical_client_hints_header());
            [Cors::get_vary_header_value(), ClientHint::get_vary_header_value()].join(", ")
        } else {
            Cors::get_vary_header_value()
        };
        before_date.push(Header { name: Header::_VARY.into(), value: vary.into() });
        before_date.push(Header::get_x_content_type_options_header());
        before_date.push(Header::get_accept_ranges_header());
        before_date.push(Header::get_x_frame_options_header());
        let mut after_date = vec![
            Header::get_no_cache_header(),
            Header { name: Header::_REFERRER_POLICY.into(), value: Header::_REFERRER_POLICY_VALUE_DEFAULT.into() },
            Header { name: Header::_PERMISSIONS_POLICY.into(), value: Header::_PERMISSIONS_POLICY_VALUE_DEFAULT.into() },
        ];
        if !csp.is_empty() {
            after_date.push(Header { name: Header::_CONTENT_SECURITY_POLICY.into(), value: csp.to_string().into() });
        }
        // Borrowed values clone for free: everything here is per CSP value,
        // not per request, so the built strings are interned for good.
        let before_date = before_date.into_iter().map(Header::interned).collect();
        let after_date = after_date.into_iter().map(Header::interned).collect();
        let fixed = Arc::new(FixedHeaders { before_date, after_date });
        *CACHE.write().unwrap_or_else(|e| e.into_inner()) = Some((csp.to_string(), client_hints, Arc::clone(&fixed)));
        fixed
    }

    /// `self` with an owned name or value turned into a `&'static str`
    /// leaked once per distinct string (a process-wide set), so cloning it
    /// never allocates. Only for values from a small, bounded set — the
    /// fixed response headers, which change only with the CSP setting.
    fn interned(self) -> Header {
        fn intern(s: Cow<'static, str>) -> Cow<'static, str> {
            use std::collections::HashSet;
            use std::sync::{Mutex, OnceLock};
            let owned = match s {
                Cow::Borrowed(_) => return s,
                Cow::Owned(owned) => owned,
            };
            static INTERNED: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
            let mut set = INTERNED.get_or_init(|| Mutex::new(HashSet::new())).lock().unwrap_or_else(|e| e.into_inner());
            if let Some(existing) = set.get(owned.as_str()) {
                return Cow::Borrowed(existing);
            }
            let leaked: &'static str = Box::leak(owned.into_boxed_str());
            set.insert(leaked);
            Cow::Borrowed(leaked)
        }
        Header { name: intern(self.name), value: intern(self.value) }
    }

    /// Build the standard response header list, reading CORS and CSP settings
    /// from environment variables.
    ///
    /// This is the legacy entry point retained for call-sites that do not yet
    /// have access to a [`ServerConfig`] (e.g. error helpers in
    /// `src/extract/mod.rs`). For all call-sites inside [`App::execute`] use
    /// [`get_header_list_with_config`] instead.
    pub fn get_header_list(request: &Request) -> Vec<Header> {
        Self::get_header_list_with_config(request, &crate::server_config::ServerConfig::current())
    }


    pub fn get_x_content_type_options_header() -> Header {
        Header {
            name: Header::_X_CONTENT_TYPE_OPTIONS.into(),
            value: Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF.into(),
        }
    }

    pub fn get_accept_ranges_header() -> Header {
        Header {
            name: Header::_ACCEPT_RANGES.into(),
            value: Range::BYTES.into(),
        }
    }

    pub fn get_x_frame_options_header() -> Header {
        Header {
            name: Header::_X_FRAME_OPTIONS.into(),
            value: Header::_X_FRAME_OPTIONS_VALUE_SAME_ORIGIN.into(),
        }
    }

    pub fn get_date_iso_8601_header() -> Header {
        Header {
            name: Header::_DATE_UNIX_EPOCH_NANOS.into(),
            value: crate::ext::decimal::Decimal::new(DateTimeExt::_now_unix_epoch_nanos()).as_str().to_string().into(),
        }
    }

    pub fn get_no_cache_header() -> Header {
        Header {
            name: Header::_CACHE_CONTROL.into(),
            value: Header::_DO_NOT_STORE_CACHE.into(),
        }
    }

    pub fn get_hsts_header() -> Header {
        Header {
            name: Header::_STRICT_TRANSPORT_SECURITY.into(),
            value: Header::_STRICT_TRANSPORT_SECURITY_VALUE_DEFAULT.into(),
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
            name: name.trim().to_string().into(),
            value: value.trim().to_string().into(),
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

