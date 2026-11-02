use super::*;

const PETSTORE: &str = r##"{
  "openapi": "3.0.3",
  "info": {"title": "Petstore", "version": "1.0"},
  "paths": {
    "/pets": {
      "get": {"operationId": "listPets", "summary": "List pets",
        "parameters": [{"name": "limit", "in": "query", "schema": {"type": "integer"}, "description": "At most this many"},
                       {"name": "tag", "in": "query", "schema": {"type": "array", "items": {"type": "string"}}}],
        "responses": {"200": {"content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/Pet"}}}}}}},
      "post": {"operationId": "createPet", "summary": "Create a pet",
        "requestBody": {"$ref": "#/components/requestBodies/NewPet"},
        "responses": {"201": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/Pet"}}}}}}
    },
    "/pets/{petId}": {
      "parameters": [{"$ref": "#/components/parameters/PetId"}],
      "get": {"operationId": "showPet", "description": "One pet",
        "parameters": [{"name": "X-Request-Id", "in": "header", "schema": {"type": "string"}},
                       {"name": "session", "in": "cookie", "schema": {"type": "string"}}],
        "responses": {"200": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/Pet"}}}}}},
      "delete": {"operationId": "deletePet", "responses": {"204": {}}}
    },
    "/stores/{store id}/stock": {"get": {"responses": {"200": {}}}}
  },
  "components": {
    "parameters": {"PetId": {"name": "petId", "in": "path", "required": true, "schema": {"type": "string"}}},
    "requestBodies": {"NewPet": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Pet"}}}}},
    "schemas": {
      "Pet": {"type": "object", "required": ["name"], "properties": {"id": {"type": "integer"}, "name": {"type": "string"}, "owner": {"$ref": "#/components/schemas/Person"}}},
      "Person": {"type": "object", "properties": {"name": {"type": "string"}, "pets": {"type": "array", "items": {"$ref": "#/components/schemas/Pet"}}}}
    }
  }
}"##;

fn by_name<'a>(tools: &'a [OpenApiTool], name: &str) -> &'a OpenApiTool {
    tools.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no {name} in {:?}", tools.iter().map(|t| &t.name).collect::<Vec<_>>()))
}

fn schema(tool: &OpenApiTool) -> JsonValue {
    json_lite::parse(&tool.input_schema).unwrap()
}

#[test]
fn get_operations_become_tools_with_their_parameters() {
    let found = tools(PETSTORE, &ToolOptions::default()).unwrap();
    let names: Vec<&str> = found.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(vec!["listPets", "showPet", "get_stores_store_id_stock"], names, "GETs only by default; a generated name without an operationId");

    let list = by_name(&found, "listPets");
    let s = schema(list);
    assert_eq!(Some("At most this many"), s.get("properties").unwrap().get("limit").unwrap().get("description").and_then(JsonValue::as_str));
    assert!(s.get("required").is_none());
    assert!(list.output_schema.is_none(), "an array response isn't an object schema");
    assert_eq!((Some(true), None, Some(true)), (list.annotations.read_only_hint, list.annotations.destructive_hint, list.annotations.idempotent_hint));

    let show = by_name(&found, "showPet");
    let s = schema(show);
    assert!(s.get("properties").unwrap().get("petId").is_some(), "path-level parameter, through its $ref");
    assert!(s.get("properties").unwrap().get("session").is_none(), "cookies aren't sent");
    assert_eq!(Some(&JsonValue::Array(vec![JsonValue::String("petId".into())])), s.get("required"));
    assert_eq!("One pet", show.description);
    let output = json_lite::parse(show.output_schema.as_deref().unwrap()).unwrap();
    assert_eq!(Some("integer"), output.get("properties").unwrap().get("id").unwrap().get("type").and_then(JsonValue::as_str));
    // Pet → owner (Person) → pets → items: Pet again, a cycle → {}.
    let cycle = output.get("properties").unwrap().get("owner").unwrap().get("properties").unwrap().get("pets").unwrap().get("items").unwrap();
    assert_eq!(&JsonValue::Object(Vec::new()), cycle);
}

#[test]
fn writes_only_when_included_and_names_are_unique() {
    let options = ToolOptions { prefix: Some("pets".into()), include: vec!["createPet".into(), "deletePet".into(), "listPets".into()], exclude: vec!["listPets".into()] };
    let found = tools(PETSTORE, &options).unwrap();
    let names: Vec<&str> = found.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(vec!["pets__createPet", "pets__deletePet"], names, "include, minus exclude, prefixed");
    let create = by_name(&found, "pets__createPet");
    let s = schema(create);
    assert!(create.has_body);
    assert_eq!(Some(&JsonValue::Array(vec![JsonValue::String("body".into())])), s.get("required"));
    assert_eq!(Some("object"), s.get("properties").unwrap().get("body").unwrap().get("type").and_then(JsonValue::as_str));
    assert!(create.output_schema.is_some(), "201 counts as success");
    let delete = by_name(&found, "pets__deletePet");
    assert_eq!((Some(false), Some(true)), (delete.annotations.read_only_hint, delete.annotations.destructive_hint));

    let twice = r#"{"openapi":"3.1.0","paths":{"/a":{"get":{"operationId":"same"}},"/b":{"get":{"operationId":"same"}}}}"#;
    let names: Vec<String> = tools(twice, &ToolOptions::default()).unwrap().into_iter().map(|t| t.name).collect();
    assert_eq!(vec!["same", "same_2"], names);
}

#[test]
fn calls_become_requests() {
    let found = tools(PETSTORE, &ToolOptions { include: vec!["listPets".into(), "showPet".into(), "createPet".into()], ..Default::default() }).unwrap();
    let list = by_name(&found, "listPets").request(r#"{"limit": 5, "tag": ["a b", "c&d"]}"#).unwrap();
    assert_eq!(("GET", "/pets?limit=5&tag=a%20b&tag=c%26d"), (list.method.as_str(), list.path_and_query.as_str()));
    let show = by_name(&found, "showPet").request(r#"{"petId": "x/../y", "X-Request-Id": "r1"}"#).unwrap();
    assert_eq!("/pets/x%2F..%2Fy", show.path_and_query, "path parameters can't escape their segment");
    assert_eq!(vec![("X-Request-Id".to_string(), "r1".to_string())], show.headers);
    let create = by_name(&found, "createPet").request(r#"{"body": {"name": "Rex"}}"#).unwrap();
    assert_eq!((Some(r#"{"name":"Rex"}"#), "/pets"), (create.body.as_deref(), create.path_and_query.as_str()));
    let show = by_name(&found, "showPet");
    assert!(show.request("{}").unwrap_err().contains("missing path parameter petId"));
    assert!(show.request(r#"{"petId": "1", "X-Request-Id": "a\r\nX: y"}"#).unwrap_err().contains("control characters"));
    assert!(show.request("[1]").is_err());
}

#[test]
fn documents_rws_refuses_and_hostile_refs() {
    assert!(tools(r#"{"swagger":"2.0"}"#, &ToolOptions::default()).unwrap_err().contains("Swagger 2.0"));
    assert!(tools(r#"{"openapi":"2.5"}"#, &ToolOptions::default()).unwrap_err().contains("isn't supported"));
    assert!(tools("{}", &ToolOptions::default()).unwrap_err().contains("not an OpenAPI"));
    assert!(tools("[", &ToolOptions::default()).is_err());
    assert!(tools(r#"{"openapi":"3.0.0"}"#, &ToolOptions::default()).unwrap().is_empty());

    // Each level refers to the next twice: 2^30 values if inlined naively.
    let mut schemas = String::new();
    for i in 0..30 {
        schemas.push_str(&format!(r##""S{i}":{{"type":"object","properties":{{"a":{{"$ref":"#/components/schemas/S{n}"}},"b":{{"$ref":"#/components/schemas/S{n}"}}}}}},"##, n = i + 1));
    }
    schemas.push_str(r#""S30":{"type":"string"}"#);
    let bomb = format!(
        r##"{{"openapi":"3.1.0","paths":{{"/x":{{"get":{{"operationId":"x","responses":{{"200":{{"content":{{"application/json":{{"schema":{{"$ref":"#/components/schemas/S0"}}}}}}}}}}}}}}}},"components":{{"schemas":{{{schemas}}}}}}}"##
    );
    let started = std::time::Instant::now();
    let found = tools(&bomb, &ToolOptions::default()).unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "bounded by MAX_SCHEMA_NODES");
    assert!(found[0].output_schema.as_ref().unwrap().len() < 10_000_000);
    let remote = r##"{"openapi":"3.1.0","paths":{"/r":{"get":{"operationId":"r","parameters":[{"$ref":"https://evil.example/p.json"}]}}}}"##;
    assert_eq!(r#"{"type":"object","properties":{}}"#, tools(remote, &ToolOptions::default()).unwrap()[0].input_schema, "remote refs are not fetched");
}

fn invoices_app() -> crate::state::AppWithState<()> {
    use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
    crate::app::App::with_state(())
        .get("/invoices/:id", |req, params, _conn, _state| {
            let id = params.get("id").unwrap_or("");
            if id == "0" {
                return Response::text(STATUS_CODE_REASON_PHRASE.n404_not_found, "no invoice 0");
            }
            let caller = req.header("X-Rws-Mcp-Caller").map(|h| h.value.to_string()).unwrap_or_default();
            Response::json(STATUS_CODE_REASON_PHRASE.n200_ok, format!(r#"{{"id":"{id}","total":42,"caller":"{caller}"}}"#).into_bytes())
        })
        .post("/invoices", |req, _params, _conn, _state| {
            Response::json(STATUS_CODE_REASON_PHRASE.n201_created, format!(r#"{{"created":{}}}"#, String::from_utf8_lossy(&req.body)).into_bytes())
        })
}

const INVOICES: &str = r#"{"openapi":"3.1.0","info":{"title":"Invoices","version":"1"},"paths":{
  "/invoices/{id}":{"get":{"operationId":"getInvoice","summary":"One invoice","parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string"}}],
    "responses":{"200":{"content":{"application/json":{"schema":{"type":"object","properties":{"total":{"type":"integer"}}}}}}}}},
  "/invoices":{"post":{"operationId":"createInvoice","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object"}}}}}}}}"#;

fn rpc(client: &crate::test_client::TestClient<crate::mcp::McpServer>, body: &str) -> String {
    client.post("/mcp").header("Authorization", "Bearer t-alice").header("Content-Type", "application/json").body_text(body).send().body_text().to_string()
}

#[test]
fn an_apps_own_operations_run_in_process() {
    let server = crate::mcp::McpServer::new("invoices", "1")
        .openapi_tools(invoices_app(), INVOICES, &ToolOptions { include: vec!["getInvoice".into(), "createInvoice".into()], ..Default::default() })
        .unwrap()
        .require_bearer_tokens([("t-alice", "alice")]);
    let client = crate::test_client::TestClient::new(server);
    let listed = rpc(&client, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
    assert!(listed.contains(r#""name":"getInvoice""#) && listed.contains(r#""name":"createInvoice""#), "{listed}");
    assert!(listed.contains(r#""openWorldHint":false"#), "in-process tools aren't open-world: {listed}");

    let got = rpc(&client, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"getInvoice","arguments":{"id":"7"}}}"#);
    assert!(got.contains("structuredContent") && got.contains(r#"\"total\":42"#), "{got}");
    assert!(got.contains(r#"\"caller\":\"alice\""#), "the MCP caller reaches the handler: {got}");
    let created = rpc(&client, r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"createInvoice","arguments":{"body":{"amount":5}}}}"#);
    assert!(created.contains(r#"created\":{\"amount\":5}"#), "{created}");
    let missing = rpc(&client, r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"getInvoice","arguments":{"id":"0"}}}"#);
    assert!(missing.contains(r#""isError":true"#) && missing.contains("HTTP 404") && missing.contains("no invoice 0"), "{missing}");
}

#[cfg(feature = "openapi")]
#[test]
fn the_document_can_come_from_the_app() {
    use crate::openapi::{OpenApiConfig, Operation};
    let app = invoices_app();
    let config = OpenApiConfig::new("Invoices", "1").operation("GET", "/invoices/:id", Operation::new("One invoice").operation_id("getInvoice").path_param_schema("id", r#"{"type":"string"}"#, "The invoice"));
    let spec = app.openapi_spec(&config);
    let found = tools(&spec, &ToolOptions::default()).unwrap();
    assert!(found.iter().any(|t| t.name == "getInvoice" && t.path == "/invoices/{id}"), "{:?}", found.iter().map(|t| (&t.name, &t.path)).collect::<Vec<_>>());
    let server = crate::mcp::McpServer::new("invoices", "1").openapi_tools(app, &spec, &ToolOptions::default()).unwrap();
    let client = crate::test_client::TestClient::new(server.require_bearer_tokens([("t-alice", "alice")]));
    let got = rpc(&client, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"getInvoice","arguments":{"id":"9"}}}"#);
    assert!(got.contains(r#"\"id\":\"9\""#), "{got}");
}
