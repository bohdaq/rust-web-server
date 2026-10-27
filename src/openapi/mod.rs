//! OpenAPI 3.1 documents generated from registered routes, with request and
//! response schemas from serde types.
//!
//! Every route [`Router::route_entries`](crate::router::Router::route_entries)
//! knows becomes an operation (method, path, path parameters). Describe one
//! further with an [`Operation`] — summary, JSON request body, responses,
//! query parameters — whose schemas come from [`JsonSchema`]
//! (`#[derive(JsonSchema)]`, the same derivation MCP tools use for
//! `inputSchema`/`outputSchema`, so it follows `#[serde]` renames). Rust has
//! no runtime reflection, so the types are named where the operation is
//! described rather than read off the handler. The document is OpenAPI 3.1,
//! whose schema dialect is JSON Schema 2020-12 — what `JsonSchema` emits.
//! No new dependency: the JSON is built by hand like the rest of the crate's.
//!
//! # Example
//!
//! ```rust
//! # #[cfg(all(feature = "openapi", feature = "macros"))]
//! # fn example() {
//! use rust_web_server::app::App;
//! use rust_web_server::openapi::{OpenApiConfig, Operation};
//! use rust_web_server::response::Response;
//! use rust_web_server::core::New;
//! use rust_web_server::JsonSchema;
//!
//! #[derive(JsonSchema)]
//! struct User { id: u64, name: String }
//! #[derive(JsonSchema)]
//! struct NewUser { name: String }
//!
//! let app = App::with_state(())
//!     .get("/users/:id", |_req, _params, _conn, _state| Response::new())
//!     .post("/users", |_req, _params, _conn, _state| Response::new())
//!     .openapi(
//!         OpenApiConfig::new("My API", "1.0.0")
//!             .operation("GET", "/users/:id", Operation::new("Look up a user").path_param::<u64>("id").response::<User>(200, "The user").status(404, "No such user"))
//!             .operation("POST", "/users", Operation::new("Create a user").body::<NewUser>().response::<User>(201, "Created")),
//!     );
//! // Serves GET /openapi.json (the document) and GET /docs (Swagger UI).
//! # }
//! ```

#[cfg(test)]
mod tests;

use crate::router::RouteInfo;
use crate::schema::JsonSchema;

/// Metadata for the generated OpenAPI document, and the described operations.
#[derive(Clone, Debug)]
pub struct OpenApiConfig {
    pub title: String,
    pub version: String,
    pub description: Option<String>,
    /// `(method, route pattern, operation)`, as given to [`OpenApiConfig::operation`].
    pub operations: Vec<(String, String, Operation)>,
}

impl OpenApiConfig {
    /// Create a config with the required `title` and `version` fields.
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        OpenApiConfig { title: title.into(), version: version.into(), description: None, operations: Vec::new() }
    }

    /// Set the OpenAPI document's `info.description`.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Describe the operation for `method` and route `pattern` (as
    /// registered: `/users/:id`). An operation for a pattern no route has is
    /// still documented — for routes served outside the router.
    pub fn operation(mut self, method: &str, pattern: &str, operation: Operation) -> Self {
        let method = method.to_ascii_uppercase();
        self.operations.retain(|(m, p, _)| !(*m == method && p == pattern));
        self.operations.push((method, pattern.to_string(), operation));
        self
    }
}

/// What a client needs to know about one operation beyond its method and
/// path. Schemas are JSON text, normally from [`JsonSchema`].
#[derive(Clone, Debug, Default)]
pub struct Operation {
    summary: String,
    description: Option<String>,
    operation_id: Option<String>,
    tags: Vec<String>,
    deprecated: bool,
    /// `(name, schema, description)` overriding the default string schema.
    path_params: Vec<(String, String, String)>,
    /// `(name, schema, required)`.
    query: Vec<(String, String, bool)>,
    /// `(content type, schema, required)`.
    body: Option<(String, String, bool)>,
    /// `(status, description, (content type, schema))`, in order added.
    responses: Vec<(u16, String, Option<(String, String)>)>,
}

impl Operation {
    /// An operation with a one-line `summary`.
    pub fn new(summary: impl Into<String>) -> Self {
        Operation { summary: summary.into(), ..Operation::default() }
    }

    /// A longer description (CommonMark).
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A unique `operationId` (client generators name methods after it).
    pub fn operation_id(mut self, id: impl Into<String>) -> Self {
        self.operation_id = Some(id.into());
        self
    }

    /// Group the operation under `tag`.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Mark the operation deprecated.
    pub fn deprecated(mut self) -> Self {
        self.deprecated = true;
        self
    }

    /// Path parameter `name` holds a `T` (default: a string).
    pub fn path_param<T: JsonSchema + ?Sized>(self, name: &str) -> Self {
        self.path_param_schema(name, &T::json_schema(), "")
    }

    /// Path parameter `name` with an explicit schema and description.
    pub fn path_param_schema(mut self, name: &str, schema: &str, description: &str) -> Self {
        self.path_params.retain(|(n, _, _)| n != name);
        self.path_params.push((name.to_string(), schema.to_string(), description.to_string()));
        self
    }

    /// Query parameters: one per property of `T` (a struct), required as the
    /// struct requires them — the type a handler parses the query into.
    pub fn query<T: JsonSchema + ?Sized>(mut self) -> Self {
        self.query.extend(object_properties(&T::json_schema()));
        self
    }

    /// One query parameter with an explicit schema.
    pub fn query_param(mut self, name: &str, schema: &str, required: bool) -> Self {
        self.query.push((name.to_string(), schema.to_string(), required));
        self
    }

    /// A required `application/json` request body holding a `T`.
    pub fn body<T: JsonSchema + ?Sized>(self) -> Self {
        self.body_schema("application/json", &T::json_schema(), true)
    }

    /// A request body of `content_type` with an explicit schema.
    pub fn body_schema(mut self, content_type: &str, schema: &str, required: bool) -> Self {
        self.body = Some((content_type.to_string(), schema.to_string(), required));
        self
    }

    /// Response `status` with an `application/json` body holding a `T`.
    pub fn response<T: JsonSchema + ?Sized>(self, status: u16, description: &str) -> Self {
        self.response_schema(status, description, "application/json", &T::json_schema())
    }

    /// Response `status` with a body of `content_type` and an explicit schema.
    pub fn response_schema(mut self, status: u16, description: &str, content_type: &str, schema: &str) -> Self {
        self.responses.retain(|(s, _, _)| *s != status);
        self.responses.push((status, description.to_string(), Some((content_type.to_string(), schema.to_string()))));
        self
    }

    /// Response `status` without a documented body (an error, `204`, …).
    pub fn status(mut self, status: u16, description: &str) -> Self {
        self.responses.retain(|(s, _, _)| *s != status);
        self.responses.push((status, description.to_string(), None));
        self
    }
}

/// The properties of an object schema as query parameters.
fn object_properties(schema: &str) -> Vec<(String, String, bool)> {
    use crate::mcp::json_rpc::object_members;
    let Some(members) = object_members(schema) else { return Vec::new() };
    let required: Vec<String> = members
        .iter()
        .find(|(k, _)| k == "required")
        .map(|(_, v)| {
            v.trim().trim_start_matches('[').trim_end_matches(']').split(',').map(|n| n.trim().trim_matches('"').to_string()).collect()
        })
        .unwrap_or_default();
    let Some(properties) = members.iter().find(|(k, _)| k == "properties").and_then(|(_, v)| object_members(v)) else { return Vec::new() };
    properties.into_iter().map(|(name, schema)| { let required = required.contains(&name); (name, schema, required) }).collect()
}

/// Builds an OpenAPI 3.1 JSON document from `routes` and the operations in
/// `config`.
///
/// Routes sharing a path (different methods) are merged into one `paths`
/// entry. `:name` and `*name` segments both become `{name}` with a
/// `parameters` entry — OpenAPI has no "rest of the path" parameter, so
/// `*name` is represented like `:name`.
pub fn build_spec(config: &OpenApiConfig, routes: &[RouteInfo]) -> String {
    let mut all: Vec<(String, String)> = routes.iter().map(|r| (r.method.to_ascii_uppercase(), r.pattern.clone())).collect();
    for (method, pattern, _) in &config.operations {
        if !all.iter().any(|(m, p)| m == method && p == pattern) {
            all.push((method.clone(), pattern.clone()));
        }
    }
    let mut paths: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (method, pattern) in all {
        let openapi_path = to_openapi_path(&pattern);
        match paths.iter_mut().find(|(p, _)| *p == openapi_path) {
            Some(entry) => entry.1.push((method, pattern)),
            None => paths.push((openapi_path, vec![(method, pattern)])),
        }
    }

    let paths_json: Vec<String> = paths
        .iter()
        .map(|(path, methods)| {
            let operations: Vec<String> = methods
                .iter()
                .map(|(method, pattern)| {
                    let described = config.operations.iter().find(|(m, p, _)| m == method && p == pattern).map(|(_, _, o)| o);
                    build_operation_json(method, pattern, described)
                })
                .collect();
            format!(r#""{}":{{{}}}"#, json_escape(path), operations.join(","))
        })
        .collect();

    let description_json = match &config.description {
        Some(d) => format!(r#","description":"{}""#, json_escape(d)),
        None => String::new(),
    };

    format!(
        r#"{{"openapi":"3.1.0","info":{{"title":"{}","version":"{}"{}}},"paths":{{{}}}}}"#,
        json_escape(&config.title),
        json_escape(&config.version),
        description_json,
        paths_json.join(",")
    )
}

fn build_operation_json(method: &str, pattern: &str, operation: Option<&Operation>) -> String {
    let default = Operation::new(format!("{method} {pattern}"));
    let operation = operation.unwrap_or(&default);
    let mut fields: Vec<String> = Vec::new();
    let summary = if operation.summary.is_empty() { format!("{method} {pattern}") } else { operation.summary.clone() };
    fields.push(format!(r#""summary":"{}""#, json_escape(&summary)));
    if let Some(description) = &operation.description {
        fields.push(format!(r#""description":"{}""#, json_escape(description)));
    }
    if let Some(id) = &operation.operation_id {
        fields.push(format!(r#""operationId":"{}""#, json_escape(id)));
    }
    if !operation.tags.is_empty() {
        let tags: Vec<String> = operation.tags.iter().map(|t| format!(r#""{}""#, json_escape(t))).collect();
        fields.push(format!(r#""tags":[{}]"#, tags.join(",")));
    }
    if operation.deprecated {
        fields.push(r#""deprecated":true"#.to_string());
    }

    let mut parameters: Vec<String> = path_param_names(pattern)
        .iter()
        .map(|name| {
            let (schema, description) = operation
                .path_params
                .iter()
                .find(|(n, _, _)| n == name)
                .map(|(_, schema, description)| (schema.as_str(), description.as_str()))
                .unwrap_or((r#"{"type":"string"}"#, ""));
            let description = if description.is_empty() { String::new() } else { format!(r#","description":"{}""#, json_escape(description)) };
            format!(r#"{{"name":"{}","in":"path","required":true{description},"schema":{schema}}}"#, json_escape(name))
        })
        .collect();
    parameters.extend(operation.query.iter().map(|(name, schema, required)| {
        format!(r#"{{"name":"{}","in":"query","required":{required},"schema":{schema}}}"#, json_escape(name))
    }));
    if !parameters.is_empty() {
        fields.push(format!(r#""parameters":[{}]"#, parameters.join(",")));
    }

    if let Some((content_type, schema, required)) = &operation.body {
        fields.push(format!(
            r#""requestBody":{{"required":{required},"content":{{"{}":{{"schema":{schema}}}}}}}"#,
            json_escape(content_type)
        ));
    }

    let responses: Vec<String> = if operation.responses.is_empty() {
        vec![r#""200":{"description":"OK"}"#.to_string()]
    } else {
        operation
            .responses
            .iter()
            .map(|(status, description, content)| {
                let content = match content {
                    Some((content_type, schema)) => format!(r#","content":{{"{}":{{"schema":{schema}}}}}"#, json_escape(content_type)),
                    None => String::new(),
                };
                format!(r#""{status}":{{"description":"{}"{content}}}"#, json_escape(description))
            })
            .collect()
    };
    fields.push(format!(r#""responses":{{{}}}"#, responses.join(",")));

    format!(r#""{}":{{{}}}"#, method.to_lowercase(), fields.join(","))
}

/// Converts a router pattern (`/users/:id`) to an OpenAPI path template
/// (`/users/{id}`).
fn to_openapi_path(pattern: &str) -> String {
    pattern
        .split('/')
        .map(|seg| {
            if let Some(name) = seg.strip_prefix(':').or_else(|| seg.strip_prefix('*')) {
                format!("{{{}}}", name)
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn path_param_names(pattern: &str) -> Vec<String> {
    pattern
        .split('/')
        .filter_map(|seg| seg.strip_prefix(':').or_else(|| seg.strip_prefix('*')))
        .map(|s| s.to_string())
        .collect()
}

fn json_escape(s: &str) -> String {
    crate::mcp::json_escape(s)
}

/// Self-contained Swagger UI HTML page, loading the `swagger-ui-dist` bundle
/// from a CDN and pointing it at `spec_url` (typically `/openapi.json`).
pub fn swagger_ui_html(spec_url: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>API Docs</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.onload = function() {{
      window.ui = SwaggerUIBundle({{
        url: '{}',
        dom_id: '#swagger-ui',
      }});
    }};
  </script>
</body>
</html>"#,
        spec_url
    )
}
