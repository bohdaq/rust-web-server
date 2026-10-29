//! An LLM proxy in code: OpenAI and Anthropic behind one endpoint, with a
//! virtual key per caller, a daily token budget and fallback to a second
//! OpenAI-compatible provider. (For the same in `rws.config.toml`, see the
//! docs' "LLM proxy".)
//!
//! ```sh
//! OPENAI_API_KEY=sk-… ANTHROPIC_API_KEY=sk-ant-… cargo run --example llm_proxy
//! curl localhost:7878/v1/chat/completions -H 'Authorization: Bearer vk-alice' \
//!   -H 'Content-Type: application/json' -d '{"model":"gpt-4.1-mini","messages":[{"role":"user","content":"hi"}]}'
//! curl localhost:7878/metrics | grep rws_llm_
//! ```

use rust_web_server::llm::{Api, LlmProxy, Provider};
use rust_web_server::prelude::*;

/// The proxy: `OPENAI_API_KEY`'s OpenAI first, then a local
/// OpenAI-compatible server (Ollama, vLLM) at `fallback_url`, and Anthropic.
pub fn proxy(openai_url: &str, fallback_url: &str, anthropic_url: &str) -> LlmProxy {
    let key = |name: &str| std::env::var(name).unwrap_or_default();
    LlmProxy::new()
        .provider(Provider::new("openai", Api::OpenAi, openai_url).unwrap().api_key(key("OPENAI_API_KEY")))
        .provider(Provider::new("local", Api::OpenAi, fallback_url).unwrap())
        .provider(Provider::new("anthropic", Api::Anthropic, anthropic_url).unwrap().api_key(key("ANTHROPIC_API_KEY")))
        .virtual_key("vk-alice", "alice")
        .budget("alice", 1_000_000, 86_400)
        .audit_log(true)
}

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().unwrap();
    let app = proxy("https://api.openai.com", "http://127.0.0.1:11434", "https://api.anthropic.com");
    Server::run_tls(listener, pool, app).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_web_server::application::Application;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    /// A provider stand-in answering every request with `status`.
    fn provider(status: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; length];
                let _ = reader.read_exact(&mut body);
                let reply = r#"{"usage":{"prompt_tokens":3,"completion_tokens":4}}"#;
                let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len());
            }
        });
        url
    }

    #[test]
    fn a_failing_provider_falls_back_and_unknown_keys_are_refused() {
        let app = proxy(&provider("503 Service Unavailable"), &provider("200 OK"), &provider("200 OK"));
        let call = |key: &str| {
            let body = r#"{"model":"m","messages":[]}"#;
            let raw = format!("POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {key}\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            app.execute(&Request::parse(raw.as_bytes()).unwrap(), &ConnectionInfo::default()).unwrap().status_code
        };
        assert_eq!(call("vk-alice"), 200);
        assert_eq!(call("vk-nobody"), 401);
    }
}
