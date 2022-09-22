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
}