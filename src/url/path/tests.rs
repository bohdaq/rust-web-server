use crate::url::path::{Part, UrlPath};

#[test]
fn parts() {
    let pattern = "[[name]]/some/path/[[id]]/another/part/[[param]]";
    let parts : Vec<Part> = UrlPath::extract_parts_from_pattern(pattern).unwrap();
    assert_eq!(parts.len(), 5);

    let name_param = parts.get(0).unwrap();
    assert_eq!(name_param.is_static, false);
    assert_eq!(name_param.name.clone().unwrap(), "name");
    println!("123")

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