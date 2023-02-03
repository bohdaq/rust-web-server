use crate::header::Header;

#[test]
fn parse() {
    let raw_header = "Some-Header: some value";
    let header = Header::parse_header(raw_header).unwrap();

    assert_eq!(header.name, "Some-Header".to_string());
    assert_eq!(header.value, "some value".to_string());
}

#[test]
fn parse_not_valid_header() {
    let raw_header = "some random characters";
    let boxed_header = Header::parse_header(raw_header);

    assert!(boxed_header.is_err());
    let err_msg = boxed_header.err().unwrap();
    assert_eq!("Unable to parse header: some random characters", err_msg);
}

#[test]
fn header_test() {
    let header = Header { name: Header::_ORIGIN.to_string(), value: "some string".to_string() };

    assert_eq!(header.name, Header::_ORIGIN.to_string());
    assert_eq!(header.value, "some string".to_string());
}

#[test]
fn header_constants() {
    let header = Header { name: Header::_CONTENT_TYPE.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_CONTENT_TYPE.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_CONTENT_LENGTH.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_CONTENT_LENGTH.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_X_CONTENT_TYPE_OPTIONS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_X_CONTENT_TYPE_OPTIONS.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_RANGE.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_RANGE.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_ACCEPT_RANGES.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_ACCEPT_RANGES.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_CONTENT_RANGE.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_CONTENT_RANGE.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_HOST.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_HOST.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_VARY.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_VARY.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_ORIGIN.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_ORIGIN.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_ACCESS_CONTROL_REQUEST_METHOD.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_ACCESS_CONTROL_REQUEST_METHOD.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_ACCESS_CONTROL_REQUEST_HEADERS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_ACCESS_CONTROL_REQUEST_HEADERS.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_ACCESS_CONTROL_ALLOW_ORIGIN.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_ACCESS_CONTROL_ALLOW_METHODS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_ACCESS_CONTROL_ALLOW_METHODS.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_ACCESS_CONTROL_ALLOW_HEADERS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_ACCESS_CONTROL_ALLOW_HEADERS.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_ACCESS_CONTROL_MAX_AGE.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_ACCESS_CONTROL_MAX_AGE.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_ACCESS_CONTROL_EXPOSE_HEADERS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_ACCESS_CONTROL_EXPOSE_HEADERS.to_string());
    assert_eq!(header.value, "some string".to_string());

    assert_eq!(Header::_ACCEPT, "Accept");
    assert_eq!(Header::_ACCEPT_CH, "Accept-CH");
    assert_eq!(Header::_ACCEPT_ENCODING, "Accept-Encoding");
    assert_eq!(Header::_ACCEPT_LANGUAGE, "Accept-Language");
    assert_eq!(Header::_ACCEPT_PATCH, "Accept-Patch");
    assert_eq!(Header::_AGE, "Age");
    assert_eq!(Header::_DATE_UNIX_EPOCH_NANOS, "Date-Unix-Epoch-Nanos");
    assert_eq!(Header::_ALT_SVC, "Alt-Svc");
    assert_eq!(Header::_AUTHORIZATION, "Authorization");
    assert_eq!(Header::_CACHE_CONTROL, "Cache-Control");
    assert_eq!(Header::_CLEAR_SITE_DATA, "Clear-Site-Data");
    assert_eq!(Header::_CONTENT_DISPOSITION, "Content-Disposition");
    assert_eq!(Header::_CONTENT_LANGUAGE, "Content-Language");
    assert_eq!(Header::_CONTENT_LENGTH, "Content-Length");
    assert_eq!(Header::_CONTENT_LOCATION, "Content-Location");
    assert_eq!(Header::_CONTENT_SECURITY_POLICY, "Content-Security-Policy");
    assert_eq!(Header::_CONTENT_SECURITY_POLICY_REPORT_ONLY, "Content-Security-Policy-Report-Only");
    assert_eq!(Header::_COOKIE, "Cookie");
    assert_eq!(Header::_CROSS_ORIGIN_EMBEDDER_POLICY, "Cross-Origin-Embedder-Policy");
    assert_eq!(Header::_CROSS_ORIGIN_OPENER_POLICY, "Cross-Origin-Opener-Policy");
    assert_eq!(Header::_CROSS_ORIGIN_RESOURCE_POLICY, "Cross-Origin-Resource-Policy");
    assert_eq!(Header::_DATE, "Date");
    assert_eq!(Header::_DEVICE_MEMORY, "Device-Memory");
    assert_eq!(Header::_DIGEST, "Digest");
    assert_eq!(Header::_DOWNLINK, "Downlink");
    assert_eq!(Header::_EARLY_DATA, "Early-Data");
    assert_eq!(Header::_ECT, "ECT");
    assert_eq!(Header::_ETAG, "ETag");
    assert_eq!(Header::_EXPECT, "Expect");
    assert_eq!(Header::_EXPIRES, "Expires");
    assert_eq!(Header::_FEATURE_POLICY, "Feature-Policy");
    assert_eq!(Header::_PERMISSIONS_POLICY, "Permissions-Policy");
    assert_eq!(Header::_FROM, "From");
    assert_eq!(Header::_IF_MODIFIED_SINCE, "If-Modified-Since");
    assert_eq!(Header::_IF_NONE_MATCH, "If-None-Match");
    assert_eq!(Header::_IF_RANGE, "If-Range");
    assert_eq!(Header::_IF_UNMODIFIED_SINCE, "If-Unmodified-Since");
    assert_eq!(Header::_LINK, "Link");
    assert_eq!(Header::_LOCATION, "Location");
    assert_eq!(Header::_MAX_FORWARDS, "Max-Forwards");
    assert_eq!(Header::_NEL, "NEL");
    assert_eq!(Header::_PROXY_AUTHENTICATE, "Proxy-Authenticate");
    assert_eq!(Header::_PROXY_AUTHORIZATION, "Proxy-Authorization");
    assert_eq!(Header::_REFERER, "Referer");
    assert_eq!(Header::_REFERRER_POLICY, "Referrer-Policy");
    assert_eq!(Header::_RTT, "RTT");
    assert_eq!(Header::_SAVE_DATA, "Save-Data");
    assert_eq!(Header::_SEC_CH_UA, "Sec-CH-UA");
    assert_eq!(Header::_SEC_CH_UA_ARCH, "Sec-CH-UA-Arch");
    assert_eq!(Header::_SEC_CH_UA_BITNESS, "Sec-CH-UA-Bitness");
    assert_eq!(Header::_SEC_CH_UA_FULL_VERSION_LIST, "Sec-CH-UA-Full-Version-List");
    assert_eq!(Header::_SEC_CH_UA_MOBILE, "Sec-CH-UA-Mobile");
    assert_eq!(Header::_SEC_CH_UA_MODEL, "Sec-CH-UA-Model");
    assert_eq!(Header::_SEC_CH_UA_PLATFORM, "Sec-CH-UA-Platform");
    assert_eq!(Header::_SEC_CH_UA_PLATFORM_VERSION, "Sec-CH-UA-Platform-Version");
    assert_eq!(Header::_SEC_FETCH_DEST, "Sec-Fetch-Dest");
    assert_eq!(Header::_SEC_FETCH_MODE, "Sec-Fetch-Mode");
    assert_eq!(Header::_SEC_FETCH_SITE, "Sec-Fetch-Site");
    assert_eq!(Header::_SEC_FETCH_USER, "Sec-Fetch-User");
    assert_eq!(Header::_SERVER, "Server");
    assert_eq!(Header::_SET_COOKIE, "Set-Cookie");
    assert_eq!(Header::_SET_COOKIE, "Set-Cookie");
    assert_eq!(Header::_SOURCE_MAP, "SourceMap");
    assert_eq!(Header::_STRICT_TRANSPORT_SECURITY, "Strict-Transport-Security");
    assert_eq!(Header::_TE, "TE");
    assert_eq!(Header::_TIMING_ALLOW_ORIGIN, "Timing-Allow-Origin");
    assert_eq!(Header::_TRAILER, "Trailer");
    assert_eq!(Header::_TRANSFER_ENCODING, "Transfer-Encoding");
    assert_eq!(Header::_UPGRADE, "Upgrade");
    assert_eq!(Header::_USER_AGENT, "User-Agent");
    assert_eq!(Header::_VARY, "Vary");
    assert_eq!(Header::_VIA, "Via");
    assert_eq!(Header::_WANT_DIGEST, "Want-Digest");
    assert_eq!(Header::_WWW_AUTHENTICATE, "WWW-Authenticate");
    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS, "X-Content-Type-Options");
    assert_eq!(Header::_X_CONTENT_TYPE_OPTIONS_VALUE_NOSNIFF, "nosniff");
    assert_eq!(Header::_X_FRAME_OPTIONS, "X-Frame-Options");
    assert_eq!(Header::_X_FRAME_OPTIONS_VALUE_DENY, "DENY");
    assert_eq!(Header::_X_FRAME_OPTIONS_VALUE_SAME_ORIGIN, "SAMEORIGIN");
    assert_eq!(Header::_LAST_MODIFIED, "Last-Modified");
    assert_eq!(Header::_LAST_MODIFIED_UNIX_EPOCH_NANOS, "Last-Modified-Unix-Epoch-Nanos");

}