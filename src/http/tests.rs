use crate::http::VERSION;

#[test]
fn test_version() {
    assert_eq!(VERSION.http_0_9, "HTTP/0.9");
    assert_eq!(VERSION.http_1_0, "HTTP/1.0");
    assert_eq!(VERSION.http_1_1, "HTTP/1.1");
    assert_eq!(VERSION.http_2_0, "HTTP/2.0");
}