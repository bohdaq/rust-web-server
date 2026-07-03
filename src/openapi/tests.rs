use super::{build_spec, swagger_ui_html, OpenApiConfig};
use crate::router::RouteInfo;

fn route(method: &str, pattern: &str) -> RouteInfo {
    RouteInfo { method: method.to_string(), pattern: pattern.to_string() }
}

#[test]
fn includes_title_and_version() {
    let spec = build_spec(&OpenApiConfig::new("My API", "1.0.0"), &[]);
    assert!(spec.contains(r#""openapi":"3.0.3""#));
    assert!(spec.contains(r#""title":"My API""#));
    assert!(spec.contains(r#""version":"1.0.0""#));
}

#[test]
fn includes_description_when_set() {
    let config = OpenApiConfig::new("My API", "1.0.0").description("Does things.");
    let spec = build_spec(&config, &[]);
    assert!(spec.contains(r#""description":"Does things.""#));
}

#[test]
fn omits_description_when_unset() {
    let spec = build_spec(&OpenApiConfig::new("My API", "1.0.0"), &[]);
    assert!(!spec.contains("description"));
}

#[test]
fn literal_path_has_no_parameters() {
    let routes = vec![route("GET", "/users")];
    let spec = build_spec(&OpenApiConfig::new("t", "1"), &routes);
    assert!(spec.contains(r#""/users":{"get":"#));
    assert!(!spec.contains("\"parameters\""));
}

#[test]
fn named_param_becomes_openapi_brace_syntax_with_parameters_entry() {
    let routes = vec![route("GET", "/users/:id")];
    let spec = build_spec(&OpenApiConfig::new("t", "1"), &routes);
    assert!(spec.contains(r#""/users/{id}":"#));
    assert!(spec.contains(r#""name":"id""#));
    assert!(spec.contains(r#""in":"path""#));
    assert!(spec.contains(r#""required":true"#));
}

#[test]
fn wildcard_param_becomes_openapi_brace_syntax_too() {
    let routes = vec![route("GET", "/files/*path")];
    let spec = build_spec(&OpenApiConfig::new("t", "1"), &routes);
    assert!(spec.contains(r#""/files/{path}":"#));
    assert!(spec.contains(r#""name":"path""#));
}

#[test]
fn multiple_methods_on_the_same_path_are_merged_into_one_entry() {
    let routes = vec![route("GET", "/users"), route("POST", "/users")];
    let spec = build_spec(&OpenApiConfig::new("t", "1"), &routes);

    // Exactly one "/users" path key, containing both "get" and "post".
    assert_eq!(1, spec.matches(r#""/users":"#).count());
    assert!(spec.contains(r#""get":"#));
    assert!(spec.contains(r#""post":"#));
}

#[test]
fn different_paths_produce_separate_entries() {
    let routes = vec![route("GET", "/users"), route("GET", "/posts")];
    let spec = build_spec(&OpenApiConfig::new("t", "1"), &routes);
    assert!(spec.contains(r#""/users":"#));
    assert!(spec.contains(r#""/posts":"#));
}

#[test]
fn root_path_is_supported() {
    let routes = vec![route("GET", "/")];
    let spec = build_spec(&OpenApiConfig::new("t", "1"), &routes);
    assert!(spec.contains(r#""/":{"get":"#));
}

#[test]
fn special_characters_in_title_are_escaped() {
    let spec = build_spec(&OpenApiConfig::new("My \"Cool\" API", "1.0.0"), &[]);
    assert!(spec.contains(r#""title":"My \"Cool\" API""#));
}

#[test]
fn every_operation_has_a_200_response() {
    let routes = vec![route("DELETE", "/users/:id")];
    let spec = build_spec(&OpenApiConfig::new("t", "1"), &routes);
    assert!(spec.contains(r#""responses":{"200":{"description":"OK"}}"#));
}

#[test]
fn swagger_ui_html_points_at_the_given_spec_url() {
    let html = swagger_ui_html("/openapi.json");
    assert!(html.contains("url: '/openapi.json'"));
    assert!(html.contains("swagger-ui-bundle.js"));
    assert!(html.contains("<div id=\"swagger-ui\">"));
}
