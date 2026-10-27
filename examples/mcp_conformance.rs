//! The fixture server the official MCP conformance suite
//! (github.com/modelcontextprotocol/conformance) tests: its `test_*` tools,
//! `test://` resources and `test_*` prompts, built on rws's MCP API.
//!
//! ```sh
//! RWS_CONFIG_PORT=3001 cargo run --example mcp_conformance &
//! npx @modelcontextprotocol/conformance@<version> server --url http://127.0.0.1:3001/mcp \
//!   --expected-failures ci/mcp-conformance-baseline.yml
//! ```
//!
//! Fixtures rws can't express yet (server-initiated elicitation on
//! 2025-11-25) are left out, so their scenarios fail and are listed in the
//! baseline rather than faked. The `test_input_required_result_*` tools are
//! MCP 2026-07-28's multi round-trip requests.

use rust_web_server::mcp::{InputRequired, LogLevel, McpContent, McpContext, McpServer, PromptArgDef, PromptMessage, PromptReply, SamplingRequest};
use rust_web_server::server::Server;

const TEST_IMAGE_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

const TEST_AUDIO_BASE64: &str = "UklGRiYAAABXQVZFZm10IBAAAAABAAEAQB8AAAB9AAACABAAZGF0YQIAAAA=";

fn arg(args: &str, name: &str) -> String {
    rust_web_server::mcp::extract_arg(args, name).unwrap_or_default()
}

/// `{"type":"object","properties":{"<field>":{"type":"<kind>"}},"required":["<field>"]}`
fn form(field: &str, kind: &str) -> String {
    format!(r#"{{"type":"object","properties":{{"{field}":{{"type":"{kind}"}}}},"required":["{field}"]}}"#)
}

/// A field of an accepted elicitation's content.
fn elicited(ctx: &McpContext, key: &str, field: &str) -> Option<String> {
    rust_web_server::mcp::extract_arg(&ctx.elicitation_content(key)?, field)
}

fn sampling(prompt: &str, max_tokens: u32) -> SamplingRequest {
    SamplingRequest { messages: vec![PromptMessage::user(prompt)], max_tokens, system_prompt: None }
}

const NO_ARGS: &str = r#"{"type":"object"}"#;

/// The fixtures.
pub fn server() -> McpServer {
    // A browser page on this machine may call the server; any other origin
    // gets 403 (the suite's DNS-rebinding check).
    let port = std::env::var("RWS_CONFIG_PORT").unwrap_or_else(|_| "7878".into());
    let server = McpServer::new("rws-conformance", env!("CARGO_PKG_VERSION"))
        .allowed_origins([format!("http://127.0.0.1:{port}"), format!("http://localhost:{port}")])
        .logging_enabled()
        // ── tools ──
        .tool("test_simple_text", "Tests simple text content response", r#"{"type":"object"}"#, |_| {
            Ok(McpContent::text("This is a simple text response for testing."))
        })
        .tool("test_image_content", "Tests image content response", r#"{"type":"object"}"#, |_| {
            Ok(McpContent::image(TEST_IMAGE_BASE64, "image/png"))
        })
        .tool("test_audio_content", "Tests audio content response", r#"{"type":"object"}"#, |_| {
            Ok(McpContent::audio(TEST_AUDIO_BASE64, "audio/wav"))
        })
        .tool("test_multiple_content_types", "Tests response with multiple content types (text, image, resource)", r#"{"type":"object"}"#, |_| {
            Ok(McpContent::blocks([
                McpContent::text("Multiple content types test:"),
                McpContent::image(TEST_IMAGE_BASE64, "image/png"),
                McpContent::embedded("test://mixed-content-resource", r#"{"test":"data","value":123}"#, "application/json"),
            ]))
        })
        .tool("test_embedded_resource", "Tests embedded resource content response", r#"{"type":"object"}"#, |_| {
            Ok(McpContent::embedded("test://embedded-resource", "This is an embedded resource content.", "text/plain"))
        })
        .tool_with_context(
            "test_tool_with_progress",
            "Tests tool that reports progress notifications",
            r#"{"type":"object"}"#,
            |ctx, _| {
                for step in [0.0, 50.0, 100.0] {
                    ctx.report_progress(step, Some(100.0), Some(&format!("Completed step {step} of 100")));
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Ok(McpContent::text(ctx.progress_token.clone().unwrap_or_else(|| "0".into()).trim_matches('"').to_string()))
            },
        )
        .tool("test_error_handling", "Tests error response handling", r#"{"type":"object"}"#, |_| {
            Err("This tool intentionally returns an error for testing".into())
        })
        .tool_with_context(
            "test_sampling",
            "Tests server-initiated sampling (LLM completion request)",
            r#"{"type":"object","properties":{"prompt":{"type":"string","description":"The prompt to send to the LLM"}},"required":["prompt"]}"#,
            |ctx, args| {
                let request = SamplingRequest { messages: vec![PromptMessage::user(arg(args, "prompt"))], max_tokens: 100, system_prompt: None };
                Ok(McpContent::text(match ctx.sample(request, std::time::Duration::from_secs(10)) {
                    Ok(reply) => format!("LLM response: {}", reply.content.text),
                    Err(e) => format!("Sampling not supported or error: {e}"),
                }))
            },
        )
        .tool(
            "json_schema_2020_12_tool",
            "Tool with JSON Schema 2020-12 features for conformance testing (SEP-1613)",
            concat!(
                r##"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","##,
                r##""$defs":{"address":{"$anchor":"addressDef","type":"object","properties":{"street":{"type":"string"},"city":{"type":"string"}}}},"##,
                r##""properties":{"name":{"type":"string"},"address":{"$ref":"#/$defs/address"},"contactMethod":{"type":"string","enum":["phone","email"]},"##,
                r##""phone":{"type":"string"},"email":{"type":"string"}},"##,
                r##""allOf":[{"anyOf":[{"required":["phone"]},{"required":["email"]}]}],"##,
                r##""if":{"properties":{"contactMethod":{"const":"phone"}},"required":["contactMethod"]},"then":{"required":["phone"]},"else":{"required":["email"]},"##,
                r##""additionalProperties":false}"##
            ),
            |args| Ok(McpContent::text(format!("JSON Schema 2020-12 tool called with: {args}"))),
        )
        // ── resources ──
        .resource("test://static-text", "static-text", "A static text resource for testing", |_| {
            Ok(McpContent::text("This is the content of the static text resource."))
        })
        .resource("test://static-binary", "static-binary", "A static binary resource (image) for testing", |_| {
            Ok(McpContent::blob(TEST_IMAGE_BASE64, "image/png"))
        })
        .resource("test://template/{id}/data", "template", "A resource template with parameter substitution", |uri| {
            let id = uri.trim_start_matches("test://template/").trim_end_matches("/data");
            Ok(McpContent::json(format!(r#"{{"id":"{id}","templateTest":true,"data":"Data for ID: {id}"}}"#)))
        })
        .resource("test://watched-resource", "watched-resource", "A resource that auto-updates every 3 seconds", |_| {
            Ok(McpContent::text("Watched resource content"))
        })
        // ── prompts ──
        .prompt("test_simple_prompt", "A simple prompt without arguments", |_| {
            Ok(vec![PromptMessage::user("This is a simple prompt for testing.")])
        })
        .prompt_with_args(
            "test_prompt_with_arguments",
            "A prompt with required arguments",
            vec![PromptArgDef::required("arg1", "First test argument"), PromptArgDef::required("arg2", "Second test argument")],
            |args| Ok(vec![PromptMessage::user(format!("Prompt with arguments: arg1='{}', arg2='{}'", arg(args, "arg1"), arg(args, "arg2")))]),
        )
        .prompt_with_args(
            "test_prompt_with_embedded_resource",
            "A prompt that includes an embedded resource",
            vec![PromptArgDef::required("resourceUri", "URI of the resource to embed")],
            |args| {
                Ok(vec![
                    PromptMessage {
                        role: "user",
                        content: McpContent::embedded(arg(args, "resourceUri"), "Embedded resource content for testing.", "text/plain"),
                    },
                    PromptMessage::user("Please process the embedded resource above."),
                ])
            },
        )
        .prompt("test_prompt_with_image", "A prompt that includes image content", |_| {
            Ok(vec![
                PromptMessage { role: "user", content: McpContent::image(TEST_IMAGE_BASE64, "image/png") },
                PromptMessage::user("Please analyze the image above."),
            ])
        })
        .completion("ref/prompt", "test_prompt_with_arguments", |_, _| Ok(Vec::new()))
        // ── MCP 2026-07-28: multi round-trip requests ──
        .tool_with_context("test_input_required_result_elicitation", "Asks for your name, then greets you", NO_ARGS, |ctx, _| {
            Ok(match elicited(&ctx, "user_name", "name") {
                Some(name) => McpContent::text(format!("Hello, {name}!")),
                None => McpContent::input_required(InputRequired::new().elicit("user_name", "What is your name?", &form("name", "string"))),
            })
        })
        .tool_with_context("test_input_required_result_sampling", "Asks the client's LLM a question", NO_ARGS, |ctx, _| {
            Ok(match ctx.input_response("capital_question") {
                Some(reply) => McpContent::text(format!("The LLM said: {}", arg(&reply, "text"))),
                None => McpContent::input_required(InputRequired::new().sample("capital_question", &sampling("What is the capital of France?", 100))),
            })
        })
        .tool_with_context("test_input_required_result_list_roots", "Asks for the client's roots", NO_ARGS, |ctx, _| {
            Ok(match ctx.input_response("client_roots") {
                Some(roots) => McpContent::text(format!("Client roots: {roots}")),
                None => McpContent::input_required(InputRequired::new().list_roots("client_roots")),
            })
        })
        .tool_with_context("test_input_required_result_request_state", "Round-trips a requestState", NO_ARGS, |ctx, _| {
            Ok(match (ctx.request_state(), ctx.input_response("confirm")) {
                (Some("confirm-state"), Some(_)) => McpContent::text("state-ok: the requestState came back intact"),
                _ => McpContent::input_required(InputRequired::new().elicit("confirm", "Please confirm", &form("ok", "boolean")).state("confirm-state")),
            })
        })
        .tool_with_context("test_input_required_result_tampered_state", "Signs its requestState; an altered one is refused", NO_ARGS, |ctx, _| {
            Ok(match (ctx.request_state(), ctx.input_response("confirm")) {
                (Some("signed-state"), Some(_)) => McpContent::text("state-ok"),
                _ => McpContent::input_required(InputRequired::new().elicit("confirm", "Please confirm", &form("ok", "boolean")).state("signed-state")),
            })
        })
        .tool_with_context("test_input_required_result_multi_round", "Asks two questions, one per round", NO_ARGS, |ctx, _| {
            Ok(match ctx.request_state() {
                Some("round-2") if ctx.input_response("step2").is_some() => McpContent::text("Both steps complete"),
                Some("round-1") if ctx.input_response("step1").is_some() => McpContent::input_required(
                    InputRequired::new().elicit("step2", "Step 2: What is your favorite color?", &form("color", "string")).state("round-2"),
                ),
                _ => McpContent::input_required(InputRequired::new().elicit("step1", "Step 1: What is your name?", &form("name", "string")).state("round-1")),
            })
        })
        .tool_with_context("test_input_required_result_multiple_inputs", "Asks for a name, a greeting and the roots at once", NO_ARGS, |ctx, _| {
            let answered = ["user_name", "greeting", "client_roots"].iter().all(|key| ctx.input_response(key).is_some());
            Ok(if answered && ctx.request_state() == Some("multi") {
                McpContent::text("All three inputs received")
            } else {
                McpContent::input_required(
                    InputRequired::new()
                        .elicit("user_name", "What is your name?", &form("name", "string"))
                        .sample("greeting", &sampling("Generate a greeting", 50))
                        .list_roots("client_roots")
                        .state("multi"),
                )
            })
        })
        .tool_with_context("test_input_required_result_capabilities", "Asks only for what the client declared", NO_ARGS, |ctx, _| {
            let answered = ctx.input_response("question").is_some() || ctx.input_response("name").is_some();
            Ok(if answered {
                McpContent::text("Thanks")
            } else {
                let mut input = InputRequired::new();
                if ctx.client_supports("sampling") {
                    input = input.sample("question", &sampling("Say hi", 10));
                }
                if ctx.client_supports("elicitation") {
                    input = input.elicit("name", "What is your name?", &form("name", "string"));
                }
                McpContent::input_required(input)
            })
        })
        .tool_with_context("test_missing_capability", "Needs the client's sampling capability", NO_ARGS, |ctx, _| {
            Ok(match ctx.input_response("question") {
                Some(_) => McpContent::text("sampled"),
                None => McpContent::input_required(InputRequired::new().sample("question", &sampling("Say hi", 10))),
            })
        })
        .tool_with_context("test_streaming_elicitation", "Asks for input (on 2026-07-28, as a result, never as a request)", NO_ARGS, |ctx, _| {
            Ok(match elicited(&ctx, "answer", "value") {
                Some(value) => McpContent::text(format!("You said {value}")),
                None => McpContent::input_required(InputRequired::new().elicit("answer", "Say something", &form("value", "string"))),
            })
        })
        .tool_with_context("test_logging_tool", "Logs three messages about this request", NO_ARGS, |ctx, _| {
            for message in ["Tool execution started", "Tool processing data", "Tool execution completed"] {
                ctx.log(LogLevel::Info, &format!(r#""{message}""#));
            }
            Ok(McpContent::text("Logged"))
        })
        .prompt_with_context("test_input_required_result_prompt", "A prompt that asks for context first", vec![], |ctx, _| {
            Ok(match elicited(&ctx, "user_context", "context") {
                Some(context) => PromptReply::Messages(vec![PromptMessage::user(format!("Use this context: {context}"))]),
                None => PromptReply::InputRequired(InputRequired::new().elicit("user_context", "What context should the prompt use?", &form("context", "string"))),
            })
        });

    // Logging goes through the server, so register it on the built server.
    let logger = server.clone();
    server.register_tool("test_tool_with_logging", "Tests tool that emits log messages during execution", r#"{"type":"object"}"#, move |_| {
        for message in ["Tool execution started", "Tool processing data", "Tool execution completed"] {
            logger.log(LogLevel::Info, None, &format!(r#""{message}""#));
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Ok(McpContent::text("Tool with logging executed successfully"))
    });
    // Change the tool and prompt lists, so list_changed notifications go out.
    let tools = server.clone();
    server.register_tool("test_trigger_tool_change", "Adds and removes a tool", NO_ARGS, move |_| {
        tools.register_tool("test_transient_tool", "Here for a moment", NO_ARGS, |_| Ok(McpContent::text("transient")));
        tools.remove_tool("test_transient_tool");
        Ok(McpContent::text("The tool list changed"))
    });
    let prompts = server.clone();
    server.register_tool("test_trigger_prompt_change", "Adds and removes a prompt", NO_ARGS, move |_| {
        prompts.register_prompt("test_transient_prompt", "Here for a moment", |_| Ok(vec![PromptMessage::user("transient")]));
        prompts.remove_prompt("test_transient_prompt");
        Ok(McpContent::text("The prompt list changed"))
    });
    server
}

#[tokio::main]
async fn main() {
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, server()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(server: &McpServer, request: &str) -> String {
        String::from_utf8(server.handle_request(request).content_range_list[0].body.clone()).unwrap()
    }

    #[test]
    fn the_fixtures_answer() {
        let server = server();
        let tools = body(&server, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
        for tool in ["test_simple_text", "test_image_content", "test_error_handling", "test_tool_with_logging", "json_schema_2020_12_tool"] {
            assert!(tools.contains(tool), "{tool}: {tools}");
        }
        let text = body(&server, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"test_simple_text","arguments":{}}}"#);
        assert!(text.contains("This is a simple text response for testing."), "{text}");
        let template = body(&server, r#"{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"test://template/42/data"}}"#);
        assert!(template.contains(r#"\"id\":\"42\""#), "{template}");
        let prompt = body(&server, r#"{"jsonrpc":"2.0","id":4,"method":"prompts/get","params":{"name":"test_prompt_with_arguments","arguments":{"arg1":"a","arg2":"b"}}}"#);
        assert!(prompt.contains("arg1='a', arg2='b'"), "{prompt}");
    }
}
