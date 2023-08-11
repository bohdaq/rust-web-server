use crate::url::path::UrlPath;

#[test]
fn is_matching() {
    let url = "/some/path/1234";
    let pattern = "/some/path/[[id]]";

    let is_matching = UrlPath::is_matching(url, pattern).unwrap();

    assert!(is_matching);
}