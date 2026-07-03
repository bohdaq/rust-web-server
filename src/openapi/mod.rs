//! OpenAPI 3.0 schema generation from registered routes.
//!
//! Generates a minimal, valid OpenAPI 3.0.3 document — `openapi`, `info`,
//! and `paths` (HTTP method, path, and path parameters) — directly from the
//! same `(method, pattern)` data [`Router::route_entries`](crate::router::Router::route_entries)
//! already exposes. No new dependency: the JSON is hand-built the same way
//! the MCP server builds its JSON-RPC responses.
//!
//! **Scope**: paths, methods, and path parameters only. Request/response
//! *body* schemas (from `#[derive(Validate)]` or serde types) are not
//! generated — Rust has no runtime type reflection, so that would require
//! deriving schema information at the macro level, which is a larger,
//! separate feature. Every operation's response is documented generically
//! as `200 OK` with no schema.
//!
//! # Example
//!
//! ```rust
//! # #[cfg(feature = "openapi")]
//! # fn example() {
//! use rust_web_server::app::App;
//! use rust_web_server::openapi::OpenApiConfig;
//! use rust_web_server::response::Response;
//! use rust_web_server::core::New;
//!
//! let app = App::with_state(())
//!     .get("/users", |_req, _params, _conn, _state| Response::new())
//!     .get("/users/:id", |_req, _params, _conn, _state| Response::new())
//!     .openapi(OpenApiConfig::new("My API", "1.0.0"));
//! // Now serves GET /openapi.json (the generated spec) and GET /docs (Swagger UI).
//! # }
//! ```

#[cfg(test)]
mod tests;

use crate::router::RouteInfo;

/// Metadata for the generated OpenAPI document.
#[derive(Clone, Debug)]
pub struct OpenApiConfig {
    pub title: String,
    pub version: String,
    pub description: Option<String>,
}

impl OpenApiConfig {
    /// Create a config with the required `title` and `version` fields.
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        OpenApiConfig { title: title.into(), version: version.into(), description: None }
    }

    /// Set the OpenAPI document's `info.description`.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Builds an OpenAPI 3.0.3 JSON document from `routes`.
///
/// Routes sharing the same path (different methods) are merged into one
/// `paths` entry, matching the OpenAPI spec's structure. `:name` and
/// `*name` segments both become `{name}` in the path template with a
/// corresponding `parameters` entry — OpenAPI has no native "rest of path"
/// wildcard concept, so `*name` is represented the same way as `:name`, on
/// a best-effort basis.
pub fn build_spec(config: &OpenApiConfig, routes: &[RouteInfo]) -> String {
    let mut paths: Vec<(String, Vec<&RouteInfo>)> = Vec::new();
    for route in routes {
        let openapi_path = to_openapi_path(&route.pattern);
        match paths.iter_mut().find(|(p, _)| *p == openapi_path) {
            Some(entry) => entry.1.push(route),
            None => paths.push((openapi_path, vec![route])),
        }
    }

    let paths_json: Vec<String> = paths
        .iter()
        .map(|(path, methods)| {
            let operations: Vec<String> = methods
                .iter()
                .map(|route| build_operation_json(route))
                .collect();
            format!(r#""{}":{{{}}}"#, json_escape(path), operations.join(","))
        })
        .collect();

    let description_json = match &config.description {
        Some(d) => format!(r#","description":"{}""#, json_escape(d)),
        None => String::new(),
    };

    format!(
        r#"{{"openapi":"3.0.3","info":{{"title":"{}","version":"{}"{}}},"paths":{{{}}}}}"#,
        json_escape(&config.title),
        json_escape(&config.version),
        description_json,
        paths_json.join(",")
    )
}

fn build_operation_json(route: &RouteInfo) -> String {
    let params = path_param_names(&route.pattern);
    let parameters_json = if params.is_empty() {
        String::new()
    } else {
        let entries: Vec<String> = params
            .iter()
            .map(|name| {
                format!(
                    r#"{{"name":"{}","in":"path","required":true,"schema":{{"type":"string"}}}}"#,
                    json_escape(name)
                )
            })
            .collect();
        format!(r#","parameters":[{}]"#, entries.join(","))
    };

    format!(
        r#""{}":{{"summary":"{} {}","responses":{{"200":{{"description":"OK"}}}}{}}}"#,
        route.method.to_lowercase(),
        json_escape(&route.method),
        json_escape(&route.pattern),
        parameters_json
    )
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
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = std::fmt::Write::write_fmt(&mut out, format_args!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
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
