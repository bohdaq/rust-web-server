use std::collections::HashMap;
use crate::symbol::SYMBOL;
use crate::url::path::{Part, UrlPath};

#[test]
fn parts() {
    let pattern = "[[name]]/some/path/[[id]]/another/part/[[param]]/ending";
    let parts : Vec<Part> = UrlPath::extract_parts_from_pattern(pattern).unwrap();
    assert_eq!(parts.len(), 6);

    let name_param = parts.get(0).unwrap();
    assert_eq!(name_param.is_static, false);
    assert_eq!(name_param.name.clone().unwrap(), "name");
    assert!(name_param.value.clone().is_none());
    assert!(name_param.static_pattern.clone().is_none());

    let name_param = parts.get(1).unwrap();
    assert_eq!(name_param.is_static, true);
    assert!(name_param.name.clone().is_none());
    assert!(name_param.value.clone().is_none());
    assert_eq!(name_param.static_pattern.clone().unwrap(), "/some/path/");

    let name_param = parts.get(2).unwrap();
    assert_eq!(name_param.is_static, false);
    assert_eq!(name_param.name.clone().unwrap(), "id");
    assert!(name_param.value.clone().is_none());
    assert!(name_param.static_pattern.clone().is_none());

    let name_param = parts.get(3).unwrap();
    assert_eq!(name_param.is_static, true);
    assert!(name_param.name.clone().is_none());
    assert!(name_param.value.clone().is_none());
    assert_eq!(name_param.static_pattern.clone().unwrap(), "/another/part/");

    let name_param = parts.get(4).unwrap();
    assert_eq!(name_param.is_static, false);
    assert_eq!(name_param.name.clone().unwrap(), "param");
    assert!(name_param.value.clone().is_none());
    assert!(name_param.static_pattern.clone().is_none());

    let name_param = parts.get(5).unwrap();
    assert_eq!(name_param.is_static, true);
    assert!(name_param.name.clone().is_none());
    assert!(name_param.value.clone().is_none());
    assert_eq!(name_param.static_pattern.clone().unwrap(), "/ending");

}

#[test]
fn parts_extra_opening_bracket() {
    let pattern = "[[[[[name]]";
    let reason : String = UrlPath::extract_parts_from_pattern(pattern).err().unwrap();
    assert_eq!(reason, "at least one extra [ char");
}

#[test]
fn parts_extra_nested_token() {
    let pattern = "[[[[password]]name]]";
    let reason : String = UrlPath::extract_parts_from_pattern(pattern).err().unwrap();
    assert_eq!(reason, "at least one extra [ char");

}

#[test]
fn parts_malformed() {
    let pattern = "[[name]][[other_param]]/some/path/[[id]]/another/part/[[param]]";
    let reason : String = UrlPath::extract_parts_from_pattern(pattern).err().unwrap();
    assert_eq!(reason, "two consecutive tokens one after another")

}

#[test]
fn parts_malformed_whitespace() {
    let pattern = " [[name]][[other_param]]/some/path/[[id]]/another/part/[[param]]";
    let reason : String = UrlPath::extract_parts_from_pattern(pattern).err().unwrap();
    assert_eq!(reason, "path contains control character or whitespace")

}

#[test]
fn is_matching() {
    let url = "/some/path/1234";
    let pattern = "/some/path/[[id]]";

    let is_matching = UrlPath::is_matching(url, pattern).unwrap();

    assert!(is_matching);
}

#[test]
fn is_matching_multiple_tokens() {
    let url = "/some/path/1234/another/path/asd";
    let pattern = "/some/path/[[id]]/another/path/[[name]]";

    let is_matching = UrlPath::is_matching(url, pattern).unwrap();

    assert!(is_matching);
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
fn is_matching_control_char_path() {
    let url = format!("/so{}me/path/1234", SYMBOL.control_char_string_terminator);
    let pattern = "/some/path/[[id]]";

    let is_matching_boxed = UrlPath::is_matching(url.as_str(), pattern);

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

#[test]
fn build() {
    let mut params = HashMap::new();
    params.insert("name".to_string(), "SomeName".to_string());
    params.insert("number".to_string(), "12345".to_string());

    let pattern = "[[name]]/somename/[[number]]/somenumber";

    let built_token_string = UrlPath::build(params, pattern).unwrap();
    assert_eq!("SomeName/somename/12345/somenumber".to_string(), built_token_string);
}

#[test]
fn build_non_url() {
    let mut params = HashMap::new();
    params.insert("name".to_string(), "SomeName".to_string());
    params.insert("number".to_string(), "12345".to_string());

    let pattern = "[[name]]somename[[number]]somenumber";

    let built_token_string = UrlPath::build(params, pattern).unwrap();
    assert_eq!("SomeNamesomename12345somenumber".to_string(), built_token_string);
}

#[test]
fn extract() {
    let mut params = HashMap::new();
    params.insert("name".to_string(), "SomeName".to_string());
    params.insert("number".to_string(), "12345".to_string());

    let pattern = "[[name]]/somename/[[number]]/somenumber";
    let text = "SomeName/somename/12345/somenumber";

    let resulting_map = UrlPath::extract(text, pattern).unwrap();
    let name = resulting_map.get("name").unwrap();
    let number = resulting_map.get("number").unwrap();
    assert_eq!("SomeName", name);
    assert_eq!("12345", number);
}

#[test]
fn extract_v2() {
    let mut params = HashMap::new();
    params.insert("name".to_string(), "SomeName".to_string());
    params.insert("number".to_string(), "12345".to_string());

    let pattern = "somename/[[name]]/somenumber/[[number]]";
    let text = "somename/SomeName/somenumber/12345";

    let resulting_map = UrlPath::extract(text, pattern).unwrap();
    let name = resulting_map.get("name").unwrap();
    // let number = resulting_map.get("number").unwrap();
    assert_eq!("SomeName", name);
    // assert_eq!("12345", number);
}