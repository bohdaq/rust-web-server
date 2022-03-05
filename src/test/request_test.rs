use regex::Regex;
use crate::constant::{HTTP_VERSIONS, REQUEST_METHODS};
use crate::{CONSTANTS, Request};

#[test]
fn method_and_request_uri_and_http_version_regex() {
    let re = Regex::new(Request::METHOD_AND_REQUEST_URI_AND_HTTP_VERSION_REGEX).unwrap();
    let caps = re.captures("GET / HTTP/1.1").unwrap();

    assert_eq!(HTTP_VERSIONS.HTTP_VERSION_1_1, &caps["http_version"]);
    assert_eq!(REQUEST_METHODS.GET, &caps["method"]);
    assert_eq!(CONSTANTS.SLASH, &caps["request_uri"]);


    let re = Regex::new(Request::METHOD_AND_REQUEST_URI_AND_HTTP_VERSION_REGEX).unwrap();
    let caps = re.captures("GET /draho-brat_pt2/drahobrat_pt2_ver2.mp4 HTTP/1.1").unwrap();

    assert_eq!(HTTP_VERSIONS.HTTP_VERSION_1_1, &caps["http_version"]);
    assert_eq!(REQUEST_METHODS.GET, &caps["method"]);
    assert_eq!("/draho-brat_pt2/drahobrat_pt2_ver2.mp4", &caps["request_uri"]);

}