use super::*;

#[test]
fn references_default_as_the_spec_says() {
    let parent: ParentReference = serde_json::from_str(r#"{"name":"gw"}"#).unwrap();
    assert_eq!((GATEWAY_GROUP, "Gateway"), (parent.group(), parent.kind()));
    let backend: BackendRef = serde_json::from_str(r#"{"name":"svc","port":8080,"weight":3}"#).unwrap();
    assert_eq!(("", "Service", Some(8080), Some(3)), (backend.object.group(), backend.object.kind(), backend.object.port, backend.weight));
}

#[test]
fn status_is_written_with_the_api_field_names() {
    let status = GatewayStatus {
        addresses: vec![GatewayStatusAddress { address_type: Some("IPAddress".to_string()), value: "10.0.0.1".to_string() }],
        listeners: vec![ListenerStatus { name: "http".to_string(), supported_kinds: vec![], attached_routes: 2, conditions: vec![] }],
        ..Default::default()
    };
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(serde_json::json!({"type": "IPAddress", "value": "10.0.0.1"}), json["addresses"][0]);
    assert_eq!(2, json["listeners"][0]["attachedRoutes"]);
    assert!(json.get("attachedListenerSets").is_none() && json.get("conditions").is_none(), "unset fields aren't written: {json}");
}

#[test]
fn http_route_rules_read_every_field_rws_uses() {
    let rule: HttpRouteRule = serde_json::from_value(serde_json::json!({
        "name": "r1",
        "matches": [{"path": {"type": "Exact", "value": "/a"}, "headers": [{"name": "x", "value": "1"}],
                     "queryParams": [{"type": "RegularExpression", "name": "q", "value": "^v"}], "method": "POST"}],
        "filters": [{"type": "RequestRedirect", "requestRedirect": {"scheme": "https", "statusCode": 301}},
                    {"type": "RequestMirror", "requestMirror": {"backendRef": {"name": "m", "port": 80}, "fraction": {"numerator": 1, "denominator": 4}}}],
        "backendRefs": [{"name": "svc", "port": 80, "weight": 2, "filters": []}],
        "timeouts": {"request": "10s", "backendRequest": "2s"},
        "retry": {"codes": [503], "attempts": 2, "backoff": "100ms"}
    }))
    .unwrap();
    assert_eq!(Some("r1"), rule.name.as_deref());
    assert_eq!(Some("Exact".to_string()), rule.matches[0].path.as_ref().unwrap().match_type);
    assert_eq!("RegularExpression", rule.matches[0].query_params[0].match_type.as_deref().unwrap());
    assert_eq!(Some(301), rule.filters[0].request_redirect.as_ref().unwrap().status_code);
    assert_eq!(Some(4), rule.filters[1].request_mirror.as_ref().unwrap().fraction.as_ref().unwrap().denominator);
    assert_eq!(Some(2), rule.backend_refs[0].backend_ref.weight);
    assert_eq!(Some("2s".to_string()), rule.timeouts.unwrap().backend_request);
    assert_eq!(vec![503], rule.retry.unwrap().codes);
}
