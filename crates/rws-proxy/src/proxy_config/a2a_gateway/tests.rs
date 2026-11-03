use super::*;
use crate::proxy_config::{ActionConfig, ProxyConfig};
use std::io::{Read, Write};
use std::sync::Arc;

/// A stand-in agent: its card points JSON-RPC at `/rpc`; a POST there
/// answers with the request it got (headers and body) as JSON text.
fn agent() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (log, card_base) = (Arc::clone(&seen), base.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut data = Vec::new();
            let mut buf = [0u8; 8192];
            let head_end = loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break None;
                }
                data.extend_from_slice(&buf[..n]);
                if let Some(p) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(p + 4);
                }
            };
            let Some(head_end) = head_end else { continue };
            let head = String::from_utf8_lossy(&data[..head_end]).to_string();
            let length: usize = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
            while data.len() < head_end + length {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
            }
            let body = String::from_utf8_lossy(&data[head_end..]).to_string();
            log.lock().unwrap().push(format!("{head}{body}"));
            let reply = if head.starts_with("GET /.well-known/agent-card.json") {
                format!(r#"{{"name":"Billing agent","description":"d","version":"1","supportedInterfaces":[{{"url":"{card_base}/rpc","protocolBinding":"JSONRPC","protocolVersion":"1.0"}},{{"url":"{card_base}/grpc","protocolBinding":"GRPC","protocolVersion":"1.0"}}],"capabilities":{{}},"defaultInputModes":["text"],"defaultOutputModes":["text"],"skills":[]}}"#)
            } else {
                r#"{"jsonrpc":"2.0","id":1,"result":{"message":{"messageId":"m","role":"ROLE_AGENT","parts":[{"text":"ok"}]}}}"#.to_string()
            };
            let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
        }
    });
    (base, seen)
}

fn config_of(toml: &str) -> A2aGatewayConfig {
    match &ProxyConfig::from_str(toml).routes[0].action {
        ActionConfig::A2a(c) => (**c).clone(),
        other => panic!("not an a2a action: {other:?}"),
    }
}

fn request(raw: &str) -> Request {
    Request::parse(raw.as_bytes()).unwrap()
}

#[test]
fn the_card_points_here_and_calls_reach_the_agent() {
    let _g = crate::test_env::lock();
    let (upstream, seen) = agent();
    std::env::set_var("RWS_TEST_A2A_AGENT_KEY", "Bearer agent-secret");
    let config = config_of(&format!(
        r#"
[[route]]
  [route.match]
  path = "/agents/billing/*"
  [route.action]
  type = "a2a"
  [route.action.a2a]
  upstream = "{upstream}/"
  headers_env = {{ Authorization = "RWS_TEST_A2A_AGENT_KEY" }}
"#
    ));
    assert_eq!((upstream.as_str(), 300_000, false), (config.upstream.as_str(), config.timeout_ms, config.audit_log));
    let gateway = A2aGateway::new(config, "/agents/billing/*");
    let conn = ConnectionInfo::default();

    let card = gateway.execute(&request("GET /agents/billing/.well-known/agent-card.json HTTP/1.1\r\nHost: gw.example\r\n\r\n"), &conn).unwrap();
    let card = json_lite::parse(std::str::from_utf8(&card.content_range_list[0].body).unwrap()).unwrap();
    let interfaces = card.get("supportedInterfaces").unwrap().as_array().unwrap();
    assert_eq!(1, interfaces.len(), "only the JSON-RPC interface, through here");
    assert_eq!(Some("http://gw.example/agents/billing"), interfaces[0].get("url").and_then(JsonValue::as_str));
    assert_eq!(Some("Billing agent"), card.get("name").and_then(JsonValue::as_str));
    assert!(seen.lock().unwrap()[0].contains("Authorization: Bearer agent-secret"), "the card is fetched with the configured header");

    let body = r#"{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{}}"#;
    let raw = format!("POST /agents/billing/ HTTP/1.1\r\nHost: gw.example\r\nAuthorization: Bearer caller-token\r\nA2A-Version: 1.0\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    let reply = gateway.execute(&request(&raw), &conn).unwrap();
    assert_eq!(200, reply.status_code);
    assert!(String::from_utf8_lossy(&reply.content_range_list[0].body).contains(r#""text":"ok""#));
    assert!(reply.headers.iter().any(|h| h.name.eq_ignore_ascii_case("proxy-status") && h.value.contains("received-status=200")));
    let forwarded = seen.lock().unwrap().last().unwrap().clone();
    assert!(forwarded.starts_with("POST /rpc "), "{forwarded}");
    assert!(forwarded.contains("A2A-Version: 1.0") && forwarded.ends_with(body), "{forwarded}");
    assert!(forwarded.contains("Authorization: Bearer agent-secret") && !forwarded.contains("caller-token"), "the caller's token stays here: {forwarded}");

    assert_eq!(405, gateway.execute(&request("GET /agents/billing HTTP/1.1\r\nHost: g\r\n\r\n"), &conn).unwrap().status_code);
    assert_eq!(404, gateway.execute(&request("GET /agents/billing/other HTTP/1.1\r\nHost: g\r\n\r\n"), &conn).unwrap().status_code);
    std::env::remove_var("RWS_TEST_A2A_AGENT_KEY");
}

#[test]
fn an_unreachable_agent_and_bad_configs() {
    let gateway = A2aGateway::new(A2aGatewayConfig { upstream: "http://127.0.0.1:9".into(), timeout_ms: 1000, ..Default::default() }, "/a/*");
    let reply = gateway.execute(&request("GET /a/.well-known/agent-card.json HTTP/1.1\r\nHost: g\r\n\r\n"), &ConnectionInfo::default()).unwrap();
    assert_eq!(503, reply.status_code);
    assert!(reply.headers.iter().any(|h| h.name == "Retry-After"));

    let mut report = ConfigReport::default();
    validate(&A2aGatewayConfig::default(), None, "route #1", &mut report);
    let text = format!("{report:?}");
    assert!(text.contains("needs [route.match] path") && text.contains("needs upstream"), "{text}");
    let mut report = ConfigReport::default();
    validate(&A2aGatewayConfig { upstream: "ftp://x".into(), ..Default::default() }, Some("/a"), "route #1", &mut report);
    assert!(format!("{report:?}").contains("isn't an http(s) URL"));
    let checked = crate::config_check::check_str_offline("[[route]]\n[route.match]\npath = \"/a/*\"\n[route.action]\ntype = \"a2a\"\n[route.action.a2a]\nupstream = \"http://10.0.0.7:9000\"\nbogus = 1\n");
    assert!(format!("{checked:?}").contains("bogus"), "unknown keys are reported");
}
