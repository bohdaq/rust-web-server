use crate::url::path::{Part, UrlPath};

#[test]
fn parts() {
    let pattern = "/some/path/[[id]]";
    let parts : Vec<Part> = UrlPath::extract_parts_from_pattern(pattern).unwrap();


}

#[test]
fn is_matching() {
    let url = "/some/path/1234";
    let pattern = "/some/path/[[id]]";

    // let is_matching = UrlPath::is_matching(url, pattern).unwrap();

    // assert!(is_matching);
}


#[test]
fn is_matching_whitespace_path() {
    let url = "/so me/path/1234";
    let pattern = "/some/path/[[id]]";

    let is_matching_boxed = UrlPath::is_matching(url, pattern);

    assert!(is_matching_boxed.is_err());
    assert_eq!("path contains control character or whitespace", is_matching_boxed.err().unwrap());
}

#[test]
fn is_matching_whitespace_pattern() {
    let url = "/some/path/1234";
    let pattern = "/so me/path/[[id]]";

    let is_matching_boxed = UrlPath::is_matching(url, pattern);

    assert!(is_matching_boxed.is_err());
    assert_eq!("path contains control character or whitespace", is_matching_boxed.err().unwrap());
}