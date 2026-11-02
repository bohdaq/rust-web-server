use super::{build_spec, swagger_ui_html, OpenApiConfig};
use crate::router::RouteInfo;

fn route(method: &str, pattern: &str) -> RouteInfo {
    RouteInfo { method: method.to_string(), pattern: pattern.to_string() }
}

#[test]
fn includes_title_and_version() {
    let spec = build_spec(&OpenApiConfig::new("My API", "1.0.0"), &[]);
    assert!(spec.contains(r#""openapi":"3.1.0""#));
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

// ── operations ────────────────────────────────────────────────────────────────

use super::Operation;
use crate::schema::{JsonSchema, ObjectSchema};

struct Item;
impl JsonSchema for Item {
    fn json_schema() -> String {
        let mut o = ObjectSchema::new("A stock item");
        o.property("sku", &String::json_schema(), "", true);
        o.property("on_hand", &i64::json_schema(), "", true);
        o.finish()
    }
}

struct Filter;
impl JsonSchema for Filter {
    fn json_schema() -> String {
        let mut o = ObjectSchema::new("");
        o.property("limit", &u32::json_schema(), "", true);
        o.property("q", &Option::<String>::json_schema(), "", false);
        o.finish()
    }
}

#[test]
fn described_operations_carry_bodies_responses_and_parameters() {
    let config = OpenApiConfig::new("Inventory", "1")
        .operation(
            "post",
            "/items/:sku",
            Operation::new("Adjust stock")
                .description("Adds `delta`.")
                .operation_id("adjustStock")
                .tag("items")
                .path_param::<u32>("sku")
                .body::<Item>()
                .response::<Item>(200, "The item")
                .status(404, "No such SKU"),
        )
        .operation("GET", "/items", Operation::new("List").query::<Filter>().response::<Vec<Item>>(200, "Every item").deprecated());
    let spec = build_spec(&config, &[route("POST", "/items/:sku"), route("GET", "/items")]);
    let item = Item::json_schema();
    assert!(spec.contains(r#""operationId":"adjustStock","tags":["items"]"#), "{spec}");
    assert!(spec.contains(r#"{"name":"sku","in":"path","required":true,"schema":{"type":"integer","minimum":0}}"#), "{spec}");
    assert!(spec.contains(&format!(r#""requestBody":{{"required":true,"content":{{"application/json":{{"schema":{item}}}}}}}"#)), "{spec}");
    assert!(spec.contains(&format!(r#""200":{{"description":"The item","content":{{"application/json":{{"schema":{item}}}}}}}"#)), "{spec}");
    assert!(spec.contains(r#""404":{"description":"No such SKU"}"#), "{spec}");
    assert!(spec.contains(r#"{"name":"limit","in":"query","required":true,"schema":{"type":"integer","minimum":0}}"#), "{spec}");
    assert!(spec.contains(r#"{"name":"q","in":"query","required":false,"schema":"#), "{spec}");
    assert!(spec.contains(r#""deprecated":true"#), "{spec}");
}

#[test]
fn an_undescribed_route_keeps_the_generic_200_and_a_described_unrouted_one_is_added() {
    let config = OpenApiConfig::new("t", "1").operation("DELETE", "/elsewhere/:id", Operation::new("Remove").status(204, "Gone"));
    let spec = build_spec(&config, &[route("GET", "/plain")]);
    assert!(spec.contains(r#""/plain":{"get":{"summary":"GET /plain","responses":{"200":{"description":"OK"}}}}"#), "{spec}");
    assert!(spec.contains(r#""/elsewhere/{id}":{"delete":{"summary":"Remove","parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string"}}],"responses":{"204":{"description":"Gone"}}}}"#), "{spec}");
}

#[test]
fn describing_an_operation_twice_keeps_the_last() {
    let config = OpenApiConfig::new("t", "1")
        .operation("GET", "/a", Operation::new("first"))
        .operation("get", "/a", Operation::new("second").status(200, "x").status(200, "y"));
    let spec = build_spec(&config, &[route("GET", "/a")]);
    assert!(spec.contains(r#""summary":"second""#) && !spec.contains("first"), "{spec}");
    assert!(spec.contains(r#""responses":{"200":{"description":"y"}}"#), "{spec}");
}

#[test]
fn brace_and_colon_patterns_describe_the_same_operation() {
    // The route registered as `/items/{sku}` is listed as `/items/:sku`; an
    // operation described either way applies to it.
    let config = OpenApiConfig::new("t", "1")
        .operation("GET", "/items/{sku}", Operation::new("Look up").status(404, "Missing"))
        .operation("GET", "/files/:rest", Operation::new("ignored, replaced"))
        .operation("GET", "/files/{*rest}", Operation::new("Files"));
    let spec = build_spec(&config, &[route("GET", "/items/:sku"), route("GET", "/files/*rest")]);
    assert_eq!(1, spec.matches(r#""/items/{sku}""#).count(), "{spec}");
    assert!(spec.contains(r#""summary":"Look up""#) && spec.contains(r#""404""#), "{spec}");
    assert!(spec.contains(r#""/files/{rest}":{"get":{"summary":"Files""#) && !spec.contains("ignored"), "{spec}");
    assert!(spec.contains(r#"{"name":"rest","in":"path""#), "{spec}");
}
