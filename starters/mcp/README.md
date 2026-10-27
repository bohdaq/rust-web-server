# {{project-name}}

An MCP server on [rust-web-server](https://rws8.tech).

```sh
cargo test
MCP_TOKEN=$(openssl rand -hex 32) cargo run   # MCP at http://127.0.0.1:7878/mcp
```
