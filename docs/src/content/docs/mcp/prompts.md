---
title: MCP Prompts
description: Register reusable prompt templates on an McpServer that AI clients can invoke by name with arguments.
---

Prompts are reusable message templates that clients can retrieve from the server and inject directly into a conversation. Unlike tools (which the AI calls autonomously), prompts are user-initiated: the client fetches the template, fills in arguments, and inserts the resulting messages.

## Registering a prompt

Call `.prompt(name, description, handler)` on `McpServer`:

```rust
use rust_web_server::mcp::{McpServer, PromptMessage, extract_arg};

let mcp = McpServer::new("my-server", "1.0")
    .prompt(
        "summarize",
        "Summarize the given text",
        |args| {
            let text = extract_arg(args, "text")
                .unwrap_or_else(|| "some text".to_string());
            Ok(vec![PromptMessage::user(format!("Please summarize: {text}"))])
        },
    );
```

The handler signature is:

```rust
Fn(&str) -> Result<Vec<PromptMessage>, String>
```

The `&str` argument is the raw `arguments` JSON object sent by the client (e.g. `{"text":"hello"}`). Return `Err(String)` to surface an error to the client.

## PromptMessage

Build individual messages with the two constructor methods:

```rust
PromptMessage::user("content")       // role: "user"
PromptMessage::assistant("content")  // role: "assistant"
```

A handler can return any mix of user and assistant messages. The MCP client inserts them into the conversation in order.

## Extracting arguments with `extract_arg`

`extract_arg(args_json, "param_name")` does a lightweight key lookup on the raw JSON string without pulling in a JSON library:

```rust
use rust_web_server::mcp::extract_arg;

let args = r#"{"language":"rust","code":"fn main(){}"}"#;
let lang = extract_arg(args, "language"); // Some("rust")
let missing = extract_arg(args, "other"); // None
```

Return a default or an error when the argument is absent:

```rust
let lang = extract_arg(args, "language")
    .ok_or_else(|| "missing required argument: language".to_string())?;
```

## Declaring argument metadata with `prompt_with_args`

Use `.prompt_with_args` to attach typed argument definitions. These are returned in `prompts/list` so clients can render a form:

```rust
use rust_web_server::mcp::{McpServer, PromptArgDef, PromptMessage, extract_arg};

let mcp = McpServer::new("my-server", "1.0")
    .prompt_with_args(
        "code_review",
        "Request a code review for a snippet",
        vec![
            PromptArgDef::required("language", "Programming language of the snippet"),
            PromptArgDef::required("code",     "Source code to review"),
        ],
        |args| {
            let language = extract_arg(args, "language")
                .unwrap_or_else(|| "unknown".to_string());
            let code = extract_arg(args, "code")
                .unwrap_or_else(|| "(no code provided)".to_string());
            Ok(vec![PromptMessage::user(format!(
                "Please review the following {language} code and point out any bugs, \
                 style issues, or improvements:\n\n```{language}\n{code}\n```"
            ))])
        },
    );
```

`PromptArgDef::required(name, description)` marks the argument as required. Use `PromptArgDef::optional(name, description)` for optional ones.

## JSON-RPC methods

### `prompts/list`

The client sends:

```json
{"jsonrpc":"2.0","id":1,"method":"prompts/list"}
```

The server responds with every registered prompt including its argument definitions:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "prompts": [
      {
        "name": "code_review",
        "description": "Request a code review for a snippet",
        "arguments": [
          {"name":"language","description":"Programming language of the snippet","required":true},
          {"name":"code",    "description":"Source code to review",              "required":true}
        ]
      }
    ]
  },
  "id": 1
}
```

### `prompts/get`

The client sends the prompt name and the filled-in arguments:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "prompts/get",
  "params": {
    "name": "code_review",
    "arguments": {
      "language": "rust",
      "code": "fn add(a: i32, b: i32) -> i32 { a + b }"
    }
  }
}
```

The server calls the handler with the serialised `arguments` object and responds:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "description": "Request a code review for a snippet",
    "messages": [
      {
        "role": "user",
        "content": {
          "type": "text",
          "text": "Please review the following rust code ..."
        }
      }
    ]
  },
  "id": 2
}
```

## Complete example: code-review prompt

```rust
use rust_web_server::server::Server;
use rust_web_server::mcp::{McpServer, PromptArgDef, PromptMessage, extract_arg};

# #[cfg(not(feature = "http2"))]
# fn main() {
let mcp = McpServer::new("code-tools", "1.0")
    .prompt_with_args(
        "code_review",
        "Generate a code-review request message",
        vec![
            PromptArgDef::required("language", "Programming language (e.g. rust, python)"),
            PromptArgDef::required("code",     "Source code to review"),
            PromptArgDef::optional("focus",    "Optional focus area, e.g. performance or safety"),
        ],
        |args| {
            let language = extract_arg(args, "language")
                .ok_or_else(|| "argument 'language' is required".to_string())?;
            let code = extract_arg(args, "code")
                .ok_or_else(|| "argument 'code' is required".to_string())?;
            let focus_clause = match extract_arg(args, "focus") {
                Some(f) => format!(" Pay particular attention to {f}."),
                None    => String::new(),
            };

            Ok(vec![PromptMessage::user(format!(
                "Please review the following {language} code for correctness, \
                 style, and potential improvements.{focus_clause}\n\n\
                 ```{language}\n{code}\n```"
            ))])
        },
    );

let (listener, pool) = Server::setup().unwrap();
Server::run(listener, pool, mcp);
# }
```

:::note[Notifications]
`notifications/initialized` is a one-way notification the client sends after the handshake. The server acknowledges it with `202 Accepted` and no body. You do not need to handle it explicitly.
:::
