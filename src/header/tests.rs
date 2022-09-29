use crate::header::Header;

#[test]
fn header_test() {
    let header = Header { name: Header::ORIGIN.to_string(), value: "some string".to_string() };

    assert_eq!(header.name, Header::ORIGIN.to_string());
    assert_eq!(header.value, "some string".to_string());
}

#[test]
fn header_constants() {
    let header = Header { name: Header::CONTENT_TYPE.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::CONTENT_TYPE.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::CONTENT_LENGTH.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::CONTENT_LENGTH.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::X_CONTENT_TYPE_OPTIONS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::X_CONTENT_TYPE_OPTIONS.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::RANGE.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::RANGE.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::ACCEPT_RANGES.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::ACCEPT_RANGES.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::CONTENT_RANGE.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::CONTENT_RANGE.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::_HOST.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::_HOST.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::VARY.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::VARY.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::ORIGIN.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::ORIGIN.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::ACCESS_CONTROL_REQUEST_METHOD.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::ACCESS_CONTROL_REQUEST_METHOD.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::ACCESS_CONTROL_REQUEST_HEADERS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::ACCESS_CONTROL_REQUEST_HEADERS.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::ACCESS_CONTROL_ALLOW_ORIGIN.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::ACCESS_CONTROL_ALLOW_METHODS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::ACCESS_CONTROL_ALLOW_METHODS.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::ACCESS_CONTROL_ALLOW_HEADERS.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::ACCESS_CONTROL_ALLOW_CREDENTIALS.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::ACCESS_CONTROL_MAX_AGE.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::ACCESS_CONTROL_MAX_AGE.to_string());
    assert_eq!(header.value, "some string".to_string());

    let header = Header { name: Header::ACCESS_CONTROL_EXPOSE_HEADERS.to_string(), value: "some string".to_string() };
    assert_eq!(header.name, Header::ACCESS_CONTROL_EXPOSE_HEADERS.to_string());
    assert_eq!(header.value, "some string".to_string());

    assert_eq!(Header::_ACCEPT, "Accept");
    assert_eq!(Header::_ACCEPT_CH, "Accept-CH");
    assert_eq!(Header::_ACCEPT_ENCODING, "Accept-Encoding");
    assert_eq!(Header::_ACCEPT_LANGUAGE, "Accept-Language");
    assert_eq!(Header::_ACCEPT_PATCH, "Accept-Patch");
    assert_eq!(Header::_AGE, "Age");
    assert_eq!(Header::DATE_ISO_8601, "Date-ISO-8601");
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

}