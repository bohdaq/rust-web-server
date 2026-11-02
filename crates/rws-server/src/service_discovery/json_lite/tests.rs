use super::{parse, JsonValue};

#[test]
fn parses_scalars() {
    assert_eq!(JsonValue::Null, parse("null").unwrap());
    assert_eq!(JsonValue::Bool(true), parse("true").unwrap());
    assert_eq!(JsonValue::Bool(false), parse("false").unwrap());
    assert_eq!(JsonValue::Number(42.0), parse("42").unwrap());
    assert_eq!(JsonValue::Number(-3.5), parse("-3.5").unwrap());
    assert_eq!(JsonValue::Number(1.5e3), parse("1.5e3").unwrap());
    assert_eq!(JsonValue::String("hi".to_string()), parse("\"hi\"").unwrap());
}

#[test]
fn parses_string_escapes() {
    let parsed = parse(r#""a\n\t\"b\\c""#).unwrap();
    assert_eq!(JsonValue::String("a\n\t\"b\\c".to_string()), parsed);
}

#[test]
fn parses_unicode_escape() {
    let parsed = parse(r#""AB""#).unwrap();
    assert_eq!(JsonValue::String("AB".to_string()), parsed);
}

#[test]
fn parses_empty_object_and_array() {
    assert_eq!(JsonValue::Object(vec![]), parse("{}").unwrap());
    assert_eq!(JsonValue::Array(vec![]), parse("[]").unwrap());
}

#[test]
fn parses_nested_object_and_array() {
    let parsed = parse(r#"{"a": 1, "b": [1, 2, {"c": "d"}], "e": null}"#).unwrap();
    assert_eq!(JsonValue::Number(1.0), *parsed.get("a").unwrap());
    let arr = parsed.get("b").unwrap().as_array().unwrap();
    assert_eq!(3, arr.len());
    assert_eq!(JsonValue::Number(1.0), arr[0]);
    assert_eq!("d", arr[2].get("c").unwrap().as_str().unwrap());
    assert_eq!(JsonValue::Null, *parsed.get("e").unwrap());
}

#[test]
fn parses_whitespace_between_tokens() {
    let parsed = parse(" { \"a\" : 1 ,\n\"b\" : 2 } ").unwrap();
    assert_eq!(JsonValue::Number(1.0), *parsed.get("a").unwrap());
    assert_eq!(JsonValue::Number(2.0), *parsed.get("b").unwrap());
}

#[test]
fn get_on_non_object_returns_none() {
    let parsed = parse("[1,2,3]").unwrap();
    assert!(parsed.get("a").is_none());
    let parsed = parse("\"hi\"").unwrap();
    assert!(parsed.get("a").is_none());
}

#[test]
fn as_str_as_f64_as_array_type_mismatches_return_none() {
    let parsed = parse(r#"{"n": 1, "s": "x"}"#).unwrap();
    assert!(parsed.get("n").unwrap().as_str().is_none());
    assert!(parsed.get("s").unwrap().as_f64().is_none());
    assert!(parsed.get("s").unwrap().as_array().is_none());
}

#[test]
fn rejects_malformed_json() {
    assert!(parse("{").is_err());
    assert!(parse("[1,2").is_err());
    assert!(parse(r#"{"a" 1}"#).is_err());
    assert!(parse("").is_err());
    assert!(parse("nul").is_err());
}

#[test]
fn consul_like_response_parses() {
    let body = r#"[
        {
            "Node": {"Address": "10.0.0.9"},
            "Service": {"Address": "10.0.0.5", "Port": 8080},
            "Checks": [{"Status": "passing"}]
        },
        {
            "Node": {"Address": "10.0.0.10"},
            "Service": {"Address": "", "Port": 9090},
            "Checks": [{"Status": "passing"}]
        }
    ]"#;
    let parsed = parse(body).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(2, arr.len());
    assert_eq!("10.0.0.5", arr[0].get("Service").unwrap().get("Address").unwrap().as_str().unwrap());
    assert_eq!(8080.0, arr[0].get("Service").unwrap().get("Port").unwrap().as_f64().unwrap());
    assert_eq!("", arr[1].get("Service").unwrap().get("Address").unwrap().as_str().unwrap());
    assert_eq!("10.0.0.10", arr[1].get("Node").unwrap().get("Address").unwrap().as_str().unwrap());
}
