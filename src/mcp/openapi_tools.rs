//! MCP tools from an OpenAPI 3.x document (18.19.0, #66): each operation
//! becomes a tool whose input schema holds its path, query and header
//! parameters and its JSON body, and [`OpenApiTool::request`] turns a
//! call's arguments back into the HTTP request.
//!
//! ```rust
//! use rust_web_server::mcp::openapi_tools::{tools, ToolOptions};
//!
//! let spec = r#"{"openapi":"3.1.0","info":{"title":"Billing","version":"1"},
//!   "paths":{"/invoices/{id}":{"get":{"operationId":"getInvoice","summary":"Get an invoice",
//!     "parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"integer"}}]}}}}"#;
//! let found = tools(spec, &ToolOptions::default()).unwrap();
//! assert_eq!("getInvoice", found[0].name);
//! let call = found[0].request(r#"{"id": 42}"#).unwrap();
//! assert_eq!(("GET", "/invoices/42"), (call.method.as_str(), call.path_and_query.as_str()));
//! ```
//!
//! Local `$ref`s (`#/components/...`) are inlined, a cycle or more than
//! [`MAX_REF_DEPTH`] levels becomes `{}`; remote refs are refused. Only
//! `GET` operations become tools unless [`ToolOptions::include`] names
//! others: writes are an explicit choice.

use std::collections::BTreeMap;

use super::ToolAnnotations;
use crate::service_discovery::json_lite::{self, JsonValue};

/// `$ref`s nested deeper than this become `{}`.
pub const MAX_REF_DEPTH: usize = 24;
/// Documents larger than this are refused.
pub const MAX_SPEC_BYTES: usize = 16 * 1024 * 1024;
/// Inlined schemas nest at most this deep and hold at most this many
/// values; past either the rest becomes `{}` (a document whose refs fan out
/// would otherwise grow exponentially).
pub const MAX_SCHEMA_DEPTH: usize = 64;
pub const MAX_SCHEMA_NODES: usize = 50_000;
/// Descriptions are cut to this many bytes.
const MAX_DESCRIPTION: usize = 1024;

/// Which operations become tools, and how they're named.
#[derive(Debug, Clone, Default)]
pub struct ToolOptions {
    /// Tool names get `<prefix>__` (the gateway's naming).
    pub prefix: Option<String>,
    /// operationIds (or generated names) to expose; empty: every `GET`.
    pub include: Vec<String>,
    /// operationIds never to expose.
    pub exclude: Vec<String>,
}

/// Where a parameter goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    Path,
    Query,
    Header,
}

/// One operation as an MCP tool.
#[derive(Debug, Clone)]
pub struct OpenApiTool {
    pub name: String,
    pub description: String,
    /// The tool's `inputSchema` (JSON text).
    pub input_schema: String,
    /// The 2xx JSON response's schema when it is an object (`outputSchema`).
    pub output_schema: Option<String>,
    pub annotations: ToolAnnotations,
    pub method: String,
    /// `/invoices/{id}`.
    pub path: String,
    pub parameters: Vec<(String, Location)>,
    /// The operation takes a JSON body (the `body` argument).
    pub has_body: bool,
}

/// The HTTP request for one call.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolRequest {
    pub method: String,
    /// Path with parameters filled and percent-encoded, plus the query.
    pub path_and_query: String,
    pub headers: Vec<(String, String)>,
    /// JSON text.
    pub body: Option<String>,
}

/// Every operation of `spec` (OpenAPI 3.0 or 3.1, JSON) as a tool.
pub fn tools(spec: &str, options: &ToolOptions) -> Result<Vec<OpenApiTool>, String> {
    if spec.len() > MAX_SPEC_BYTES {
        return Err(format!("the OpenAPI document is over {} MB", MAX_SPEC_BYTES >> 20));
    }
    let doc = json_lite::parse(spec).map_err(|e| format!("the OpenAPI document isn't JSON: {e}"))?;
    match doc.get("openapi").and_then(JsonValue::as_str) {
        Some(v) if v.starts_with("3.") => {}
        Some(v) => return Err(format!("OpenAPI {v} isn't supported (3.0 and 3.1 are)")),
        None if doc.get("swagger").is_some() => return Err("Swagger 2.0 isn't supported; convert it to OpenAPI 3".into()),
        None => return Err("not an OpenAPI document (no \"openapi\" field)".into()),
    }
    let Some(JsonValue::Object(paths)) = doc.get("paths") else { return Ok(Vec::new()) };
    let mut out: Vec<OpenApiTool> = Vec::new();
    let mut taken: BTreeMap<String, usize> = BTreeMap::new();
    for (path, item) in paths {
        let item = resolve(&doc, item);
        let shared = item.get("parameters").and_then(JsonValue::as_array).unwrap_or(&[]).to_vec();
        for method in ["get", "put", "post", "delete", "patch", "head", "options"] {
            let Some(op) = item.get(method) else { continue };
            let op = resolve(&doc, op);
            let generated = sanitize(&format!("{method}_{}", path.trim_matches('/')));
            let id = op.get("operationId").and_then(JsonValue::as_str).map(sanitize).filter(|s| !s.is_empty()).unwrap_or(generated);
            let listed = |list: &[String]| list.iter().any(|x| x == &id);
            if listed(&options.exclude) || (if options.include.is_empty() { method != "get" } else { !listed(&options.include) }) {
                continue;
            }
            let mut tool = tool(&doc, path, method, &op, &shared, &id);
            let mut name = match &options.prefix {
                Some(prefix) => format!("{prefix}__{}", tool.name),
                None => tool.name.clone(),
            };
            name.truncate(64);
            let n = taken.entry(name.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                let suffix = format!("_{n}");
                name.truncate(64 - suffix.len());
                name.push_str(&suffix);
            }
            tool.name = name;
            out.push(tool);
        }
    }
    Ok(out)
}

/// MCP tool names: `[A-Za-z0-9_-]`, at most 64.
fn sanitize(s: &str) -> String {
    let mut out: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').chars().take(64).collect()
}

/// Follow `$ref`s (local only) until a value without one.
fn resolve(doc: &JsonValue, value: &JsonValue) -> JsonValue {
    let mut current = value;
    for _ in 0..MAX_REF_DEPTH {
        let Some(reference) = current.get("$ref").and_then(JsonValue::as_str) else { return current.clone() };
        match pointer(doc, reference) {
            Some(target) => current = target,
            None => return JsonValue::Object(Vec::new()),
        }
    }
    JsonValue::Object(Vec::new())
}

/// A local JSON pointer (`#/components/schemas/Invoice`).
fn pointer<'a>(doc: &'a JsonValue, reference: &str) -> Option<&'a JsonValue> {
    let path = reference.strip_prefix("#/")?;
    let mut cur = doc;
    for token in path.split('/') {
        let token = token.replace("~1", "/").replace("~0", "~");
        cur = match cur {
            JsonValue::Object(_) => cur.get(&token)?,
            JsonValue::Array(items) => items.get(token.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// A schema with every local `$ref` inlined (cycles, depth, size → `{}`).
fn inline(doc: &JsonValue, value: &JsonValue) -> JsonValue {
    let mut budget = MAX_SCHEMA_NODES;
    inline_within(doc, value, &mut Vec::new(), 0, &mut budget)
}

fn inline_within(doc: &JsonValue, value: &JsonValue, stack: &mut Vec<String>, depth: usize, budget: &mut usize) -> JsonValue {
    if stack.len() >= MAX_REF_DEPTH || depth >= MAX_SCHEMA_DEPTH || *budget == 0 {
        return JsonValue::Object(Vec::new());
    }
    *budget -= 1;
    match value {
        JsonValue::Object(pairs) => {
            if let Some(reference) = value.get("$ref").and_then(JsonValue::as_str) {
                if stack.iter().any(|r| r == reference) {
                    return JsonValue::Object(Vec::new());
                }
                let Some(target) = pointer(doc, reference) else { return JsonValue::Object(Vec::new()) };
                stack.push(reference.to_string());
                let out = inline_within(doc, target, stack, depth + 1, budget);
                stack.pop();
                return out;
            }
            JsonValue::Object(pairs.iter().map(|(k, v)| (k.clone(), inline_within(doc, v, stack, depth + 1, budget))).collect())
        }
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(|v| inline_within(doc, v, stack, depth + 1, budget)).collect()),
        other => other.clone(),
    }
}

fn text(value: &JsonValue, key: &str) -> String {
    value.get(key).and_then(JsonValue::as_str).unwrap_or("").to_string()
}

fn tool(doc: &JsonValue, path: &str, method: &str, op: &JsonValue, shared: &[JsonValue], id: &str) -> OpenApiTool {
    // Operation parameters override path-level ones with the same name+in.
    let mut params: Vec<JsonValue> = Vec::new();
    for raw in shared.iter().chain(op.get("parameters").and_then(JsonValue::as_array).unwrap_or(&[]).iter()) {
        let p = resolve(doc, raw);
        let key = (text(&p, "name"), text(&p, "in"));
        params.retain(|q| (text(q, "name"), text(q, "in")) != key);
        params.push(p);
    }
    let mut properties: Vec<(String, JsonValue)> = Vec::new();
    let mut required: Vec<JsonValue> = Vec::new();
    let mut parameters = Vec::new();
    for p in &params {
        let location = match text(p, "in").as_str() {
            "path" => Location::Path,
            "query" => Location::Query,
            "header" => Location::Header,
            _ => continue, // cookies: not sent
        };
        let name = text(p, "name");
        if name.is_empty() {
            continue;
        }
        let mut schema = match p.get("schema") {
            Some(s) => inline(doc, s),
            None => JsonValue::Object(vec![("type".into(), JsonValue::String("string".into()))]),
        };
        let description = text(p, "description");
        if let (JsonValue::Object(fields), false) = (&mut schema, description.is_empty()) {
            if !fields.iter().any(|(k, _)| k == "description") {
                fields.push(("description".into(), JsonValue::String(description)));
            }
        }
        if location == Location::Path || p.get("required") == Some(&JsonValue::Bool(true)) {
            required.push(JsonValue::String(name.clone()));
        }
        properties.push((name.clone(), schema));
        parameters.push((name, location));
    }
    let body = op.get("requestBody").map(|b| resolve(doc, b));
    let body_schema = body
        .as_ref()
        .and_then(|b| b.get("content"))
        .and_then(|c| match c {
            JsonValue::Object(types) => types.iter().find(|(t, _)| t.starts_with("application/json") || t.ends_with("+json")).map(|(_, v)| v.clone()),
            _ => None,
        })
        .and_then(|media| media.get("schema").map(|s| inline(doc, s)));
    let has_body = body_schema.is_some();
    if let Some(schema) = body_schema {
        if body.as_ref().and_then(|b| b.get("required")) == Some(&JsonValue::Bool(true)) {
            required.push(JsonValue::String("body".into()));
        }
        properties.push(("body".into(), schema));
    }
    let mut input = vec![("type".to_string(), JsonValue::String("object".into())), ("properties".to_string(), JsonValue::Object(properties))];
    if !required.is_empty() {
        input.push(("required".to_string(), JsonValue::Array(required)));
    }
    let output_schema = op
        .get("responses")
        .and_then(|r| match r {
            JsonValue::Object(codes) => codes.iter().find(|(c, _)| c.starts_with('2')).map(|(_, v)| resolve(doc, v)),
            _ => None,
        })
        .and_then(|resp| resp.get("content").and_then(|c| c.get("application/json")).and_then(|m| m.get("schema")).map(|s| inline(doc, s)))
        .filter(|s| s.get("type").and_then(JsonValue::as_str) == Some("object"))
        .map(|s| json_text(&s));
    let mut description = [text(op, "summary"), text(op, "description")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n\n");
    if description.is_empty() {
        description = format!("{} {path}", method.to_ascii_uppercase());
    }
    if description.len() > MAX_DESCRIPTION {
        let mut cut = MAX_DESCRIPTION;
        while !description.is_char_boundary(cut) {
            cut -= 1;
        }
        description.truncate(cut);
    }
    let annotations = ToolAnnotations {
        read_only_hint: Some(matches!(method, "get" | "head" | "options")),
        destructive_hint: (method == "delete").then_some(true),
        idempotent_hint: matches!(method, "get" | "head" | "options" | "put" | "delete").then_some(true),
        open_world_hint: Some(true),
    };
    OpenApiTool {
        name: id.to_string(),
        description,
        input_schema: json_text(&JsonValue::Object(input)),
        output_schema,
        annotations,
        method: method.to_ascii_uppercase(),
        path: path.to_string(),
        parameters,
        has_body,
    }
}

fn json_text(value: &JsonValue) -> String {
    json_lite::to_text(value)
}

/// Percent-encode everything but unreserved characters.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A scalar argument as text; anything else as its JSON.
fn scalar(value: &JsonValue) -> String {
    match value {
        JsonValue::String(s) => s.clone(),
        other => json_text(other),
    }
}

impl OpenApiTool {
    /// The request for a call with `arguments` (a JSON object).
    pub fn request(&self, arguments: &str) -> Result<ToolRequest, String> {
        let args = if arguments.trim().is_empty() { JsonValue::Object(Vec::new()) } else { json_lite::parse(arguments).map_err(|e| format!("arguments aren't JSON: {e}"))? };
        if !matches!(args, JsonValue::Object(_)) {
            return Err("arguments must be an object".into());
        }
        let mut path = self.path.clone();
        let mut query: Vec<String> = Vec::new();
        let mut headers = Vec::new();
        for (name, location) in &self.parameters {
            let Some(value) = args.get(name).filter(|v| !matches!(v, JsonValue::Null)) else {
                if *location == Location::Path {
                    return Err(format!("missing path parameter {name}"));
                }
                continue;
            };
            match location {
                Location::Path => path = path.replace(&format!("{{{name}}}"), &encode(&scalar(value))),
                Location::Query => match value {
                    JsonValue::Array(items) => query.extend(items.iter().map(|v| format!("{}={}", encode(name), encode(&scalar(v))))),
                    other => query.push(format!("{}={}", encode(name), encode(&scalar(other)))),
                },
                Location::Header => {
                    let text = scalar(value);
                    if text.bytes().any(|b| b < 0x20 || b == 0x7f) {
                        return Err(format!("header {name} can't hold control characters"));
                    }
                    headers.push((name.clone(), text));
                }
            }
        }
        let body = match (self.has_body, args.get("body")) {
            (true, Some(v)) => Some(json_text(v)),
            _ => None,
        };
        let path_and_query = if query.is_empty() { path } else { format!("{path}?{}", query.join("&")) };
        Ok(ToolRequest { method: self.method.clone(), path_and_query, headers, body })
    }
}

impl super::McpServer {
    /// Make `app`'s operations, as `spec` (its OpenAPI document) describes
    /// them, tools of this server, run in-process: a call builds the
    /// request and hands it to `app` — no HTTP round trip, the app's own
    /// middleware applying as to any request. With [`AppWithState`] the
    /// document comes from [`AppWithState::openapi_spec`].
    ///
    /// [`AppWithState`]: crate::state::AppWithState
    /// [`AppWithState::openapi_spec`]: crate::state::AppWithState::openapi_spec
    pub fn openapi_tools<A>(mut self, app: A, spec: &str, options: &ToolOptions) -> Result<Self, String>
    where
        A: crate::application::Application + Send + Sync + 'static,
    {
        let app = std::sync::Arc::new(app);
        for tool in tools(spec, options)? {
            let (name, description, input) = (tool.name.clone(), tool.description.clone(), tool.input_schema.clone());
            let (output, annotations) = (tool.output_schema.clone(), tool.annotations);
            let (app, tool) = (std::sync::Arc::clone(&app), std::sync::Arc::new(tool));
            self = self.tool_with_context(&name, &description, &input, move |ctx, args| call_in_process(app.as_ref(), &tool, args, &ctx));
            if let Some(schema) = output {
                self = self.tool_output_schema(&name, &schema);
            }
            self = self.tool_annotations(&name, ToolAnnotations { open_world_hint: Some(false), ..annotations });
        }
        Ok(self)
    }
}

/// One call, handed to `app` as a request.
fn call_in_process(app: &dyn crate::application::Application, tool: &OpenApiTool, arguments: &str, ctx: &super::McpContext) -> Result<super::McpContent, String> {
    use crate::header::Header;
    let call = tool.request(arguments)?;
    let mut headers = vec![Header::new("Host", "localhost".to_string()), Header::new("Accept", "application/json".to_string())];
    // The MCP caller's identity, for the app's own checks.
    if let Some(sub) = ctx.auth_claims.as_deref().and_then(|c| json_lite::parse(c).ok()).and_then(|c| c.get("sub").and_then(JsonValue::as_str).map(str::to_string)) {
        headers.push(Header::new("X-Rws-Mcp-Caller", sub));
    }
    headers.extend(call.headers.iter().filter(|(n, _)| !n.eq_ignore_ascii_case("x-rws-mcp-caller")).map(|(n, v)| Header::new(n.clone(), v.clone())));
    let body = call.body.unwrap_or_default().into_bytes();
    if !body.is_empty() {
        headers.push(Header::new("Content-Type", "application/json".to_string()));
        headers.push(Header::new("Content-Length", body.len().to_string()));
    }
    let request = crate::request::Request { method: call.method.clone(), request_uri: call.path_and_query.clone(), http_version: "HTTP/1.1".to_string(), headers, body: body.into() };
    let connection = crate::server::ConnectionInfo::default();
    let response = app.execute(&request, &connection)?;
    let content_type = response.content_range_list.first().map(|c| c.content_type.clone()).unwrap_or_default();
    let text = response.content_range_list.iter().map(|c| String::from_utf8_lossy(&c.body).into_owned()).collect::<String>();
    if !(200..300).contains(&response.status_code) {
        return Err(format!("HTTP {}: {text}", response.status_code));
    }
    Ok(match (content_type.contains("json"), tool.output_schema.is_some()) {
        (true, true) if text.trim_start().starts_with('{') => super::McpContent::structured(text),
        (true, _) => super::McpContent::json(text),
        _ => super::McpContent::text(text),
    })
}

#[cfg(test)]
mod tests;
