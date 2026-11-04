//! A mock OpenAI-compatible LLM: the upstream for an rws `llm` route.

use std::time::{SystemTime, UNIX_EPOCH};

use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::sse::{Sse, SseEvent};

use crate::common::{json, text, S};

pub const MODEL: &str = "mock-1";

fn usage() -> serde_json::Value {
    serde_json::json!({ "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 })
}

/// The text of the last `user` message (string content, or the text parts
/// of an array content).
pub fn last_user_message(body: &serde_json::Value) -> Option<String> {
    let messages = body.get("messages")?.as_array()?;
    let message = messages.iter().rev().find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))?;
    match message.get("content")? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(parts) => Some(
            parts.iter().filter_map(|p| p.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join(""),
        ),
        _ => None,
    }
}

pub fn models() -> Response {
    json(S.n200_ok, &serde_json::json!({
        "object": "list",
        "data": [{ "id": MODEL, "object": "model", "created": 0, "owned_by": "rws-e2e" }]
    }))
}

pub fn chat_completions(request: &Request) -> Response {
    let body: serde_json::Value = match serde_json::from_slice(&request.body) {
        Ok(v) => v,
        Err(e) => return text(S.n400_bad_request, &format!("invalid JSON: {e}")),
    };
    let Some(prompt) = last_user_message(&body) else {
        return text(S.n400_bad_request, "no user message");
    };
    let model = body.get("model").and_then(|m| m.as_str()).unwrap_or(MODEL).to_string();
    let content = format!("echo: {prompt}");
    let created = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let id = format!("chatcmpl-e2e-{created}");

    if body.get("stream").and_then(|s| s.as_bool()) == Some(true) {
        let chunk = |delta: serde_json::Value, finish: Option<&str>, usage: Option<serde_json::Value>| {
            let mut c = serde_json::json!({
                "id": id, "object": "chat.completion.chunk", "created": created, "model": model,
                "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }],
            });
            if let Some(u) = usage {
                c["usage"] = u;
            }
            c.to_string()
        };
        // Split the reply in two content chunks so clients see a real stream.
        let mut mid = content.len() / 2;
        while !content.is_char_boundary(mid) {
            mid -= 1;
        }
        let (a, b) = content.split_at(mid);
        return Sse::new()
            .push(SseEvent::data(chunk(serde_json::json!({"role": "assistant", "content": ""}), None, None)))
            .push(SseEvent::data(chunk(serde_json::json!({"content": a}), None, None)))
            .push(SseEvent::data(chunk(serde_json::json!({"content": b}), None, None)))
            .push(SseEvent::data(chunk(serde_json::json!({}), Some("stop"), Some(usage()))))
            .push(SseEvent::data("[DONE]"))
            .into_response();
    }

    json(S.n200_ok, &serde_json::json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": "stop"
        }],
        "usage": usage(),
    }))
}
