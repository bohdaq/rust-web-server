use super::*;
use crate::application::Application;
use crate::config_check::{check_str, check_str_offline};
use crate::proxy_config::{ActionConfig, ProxyConfig};
use crate::request::Request;
use crate::server::ConnectionInfo;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

const EXAMPLE: &str = r#"
[[route]]
  name = "llm"
  [route.match]
  path = "/v1/*"
  [route.action]
  type = "llm"
  [route.action.llm]
  audit_log = true
  redis = "127.0.0.1:6379"
  redis_password_env = "RWS_TEST_LLM_REDIS_PW"
  read_timeout_ms = 60000

  [[route.action.llm.provider]]
  name = "openai"
  api = "openai"
  url = "https://api.openai.com"
  key_env = "RWS_TEST_LLM_OPENAI"

  [[route.action.llm.provider]]
  name = "local"
  api = "openai"
  url = "http://127.0.0.1:11434"

  [[route.action.llm.key]]
  identity = "alice"
  key_env = "RWS_TEST_LLM_ALICE"
  budget_tokens = 100000
  window_secs = 86400
"#;

fn llm_of(toml: &str) -> LlmConfig {
    match &ProxyConfig::from_str(toml).routes[0].action {
        ActionConfig::Llm(c) => (**c).clone(),
        other => panic!("not an llm action: {other:?}"),
    }
}

#[test]
fn the_whole_action_parses() {
    let c = llm_of(EXAMPLE);
    assert!(c.audit_log && !c.allow_anonymous);
    assert_eq!((c.redis.as_str(), c.redis_password_env.as_str(), c.redis_prefix.as_str()), ("127.0.0.1:6379", "RWS_TEST_LLM_REDIS_PW", "rws:llm:"));
    assert_eq!((c.connect_timeout_ms, c.read_timeout_ms), (10_000, 60_000));
    assert_eq!(c.providers.len(), 2);
    assert_eq!(c.providers[0], LlmProviderConfig { name: "openai".into(), api: "openai".into(), url: "https://api.openai.com".into(), key_env: "RWS_TEST_LLM_OPENAI".into() });
    assert_eq!(c.providers[1].key_env, "");
    assert_eq!(c.keys, [LlmKeyConfig { identity: "alice".into(), key_env: "RWS_TEST_LLM_ALICE".into(), budget_tokens: 100_000, window_secs: 86_400 }]);
}

#[test]
fn the_example_is_valid_offline_and_unset_secrets_fail_online() {
    let offline = check_str_offline(EXAMPLE);
    assert!(offline.errors.is_empty(), "{:?}", offline.errors);
    let joined = check_str(EXAMPLE).errors.join("\n");
    assert!(joined.contains("RWS_TEST_LLM_OPENAI") && joined.contains("RWS_TEST_LLM_ALICE"), "{joined}");
}

#[test]
fn mistakes_are_reported() {
    let bad = EXAMPLE
        .replace(r#"api = "openai"
  url = "https://api.openai.com""#, r#"api = "gemini"
  url = "https://api.openai.com""#)
        .replace(r#"name = "local""#, r#"name = "openai""#)
        .replace("http://127.0.0.1:11434", "127.0.0.1:11434")
        .replace("window_secs = 86400", "window = 86400");
    let joined = check_str_offline(&bad).errors.join("\n");
    for expected in ["api \"gemini\"", "\"openai\" is used twice", "must start with http", "window: unknown key", "budget_tokens and window_secs go together"] {
        assert!(joined.contains(expected), "missing {expected:?} in:\n{joined}");
    }
    let nobody = EXAMPLE.replace("[[route.action.llm.key]]\n  identity = \"alice\"\n  key_env = \"RWS_TEST_LLM_ALICE\"\n  budget_tokens = 100000\n  window_secs = 86400", "");
    assert!(check_str_offline(&nobody).errors.iter().any(|e| e.contains("nobody can call")));
    let anonymous = nobody.replace("audit_log = true", "audit_log = true\n  allow_anonymous = true");
    assert!(check_str_offline(&anonymous).errors.is_empty(), "{:?}", check_str_offline(&anonymous).errors);
}

#[test]
fn a_config_file_llm_route_forwards_with_the_provider_key_and_counts_the_budget() {
    let _g = crate::test_env::lock();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap_or(0);
                }
                head.push_str(&line);
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let reply = r#"{"usage":{"prompt_tokens":30,"completion_tokens":20}}"#;
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len());
            let _ = tx.send(head);
        }
    });
    std::env::set_var("RWS_TEST_LLM_E2E_PROVIDER", "sk-provider");
    std::env::set_var("RWS_TEST_LLM_E2E_CAROL", "vk-carol");
    let toml = format!(
        r#"
[[route]]
  [route.match]
  path = "/v1/*"
  [route.action]
  type = "llm"
  [[route.action.llm.provider]]
  name = "mock"
  api = "openai"
  url = "{url}"
  key_env = "RWS_TEST_LLM_E2E_PROVIDER"
  [[route.action.llm.key]]
  identity = "carol"
  key_env = "RWS_TEST_LLM_E2E_CAROL"
  budget_tokens = 60
  window_secs = 3600
"#
    );
    assert!(check_str(&toml).errors.is_empty(), "{:?}", check_str(&toml).errors);
    let (app, _handles) = crate::proxy_config::builder::build(ProxyConfig::from_str(&toml));
    let post = |key: &str| {
        let body = r#"{"model":"m"}"#;
        let raw = format!("POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {key}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        app.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap().status_code
    };
    assert_eq!(post("wrong"), 401);
    assert_eq!(post("vk-carol"), 200);
    let head = rx.recv().unwrap();
    assert!(head.contains("Authorization: Bearer sk-provider") && !head.contains("vk-carol"), "{head}");
    assert_eq!(post("vk-carol"), 200, "50 of 60 spent: still allowed");
    assert_eq!(post("vk-carol"), 429, "100 of 60 spent");
    std::env::remove_var("RWS_TEST_LLM_E2E_PROVIDER");
    std::env::remove_var("RWS_TEST_LLM_E2E_CAROL");
}
