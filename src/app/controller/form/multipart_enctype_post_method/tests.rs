
#[test]
fn is_matching_only_for_multipart_post_to_its_path() {
    use crate::app::controller::form::multipart_enctype_post_method::FormMultipartEnctypePostMethodController as C;
    use crate::controller::Controller;
    use crate::header::Header;
    use crate::request::Request;
    use crate::server::{Address, ConnectionInfo};
    let conn = ConnectionInfo {
        client: Address { ip: "127.0.0.1".to_string(), port: 1 },
        server: Address { ip: "127.0.0.1".to_string(), port: 2 },
        request_size: 1,
        sni_hostname: None,
    };
    let req = |method: &str, uri: &str, content_type: Option<&str>| Request {
        method: method.to_string(),
        request_uri: uri.to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: content_type
            .map(|v| vec![Header { name: Header::_CONTENT_TYPE.into(), value: v.to_string().into() }])
            .unwrap_or_default(),
        body: Default::default(),
    };
    let path = "/form-multipart-enctype-post-method";
    assert!(C::is_matching(&req("POST", path, Some("multipart/form-data; boundary=x")), &conn));
    assert!(!C::is_matching(&req("GET", "/anything", None), &conn), "plain GET");
    assert!(!C::is_matching(&req("POST", path, None), &conn), "no Content-Type");
    assert!(!C::is_matching(&req("POST", path, Some("application/json")), &conn), "other Content-Type");
    assert!(!C::is_matching(&req("POST", "/other", Some("multipart/form-data; boundary=x")), &conn), "other path");
}
