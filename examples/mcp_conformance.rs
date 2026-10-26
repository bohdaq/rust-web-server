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
//! Fixtures rws can't express yet (elicitation) are left out, so their
//! scenarios fail and are listed in the baseline rather than faked.

use rust_web_server::mcp::{LogLevel, McpContent, McpServer, PromptArgDef, PromptMessage, SamplingRequest};
use rust_web_server::server::Server;

const TEST_IMAGE_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

const TEST_AUDIO_BASE64: &str = "UklGRiYAAABXQVZFZm10IBAAAAABAAEAQB8AAAB9AAACABAAZGF0YQIAAAA=";

fn arg(args: &str, name: &str) -> String {
    rust_web_server::mcp::extract_arg(args, name).unwrap_or_default()
}

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
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"name":{"type":"string"},"address":{"type":"object","properties":{"street":{"type":"string"},"city":{"type":"string"}},"additionalProperties":false}},"additionalProperties":false}"#,
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
        .completion("ref/prompt", "test_prompt_with_arguments", |_, _| Ok(Vec::new()));

    // Logging goes through the server, so register it on the built server.
    let logger = server.clone();
    server.register_tool("test_tool_with_logging", "Tests tool that emits log messages during execution", r#"{"type":"object"}"#, move |_| {
        for message in ["Tool execution started", "Tool processing data", "Tool execution completed"] {
            logger.log(LogLevel::Info, None, &format!(r#""{message}""#));
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Ok(McpContent::text("Tool with logging executed successfully"))
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
