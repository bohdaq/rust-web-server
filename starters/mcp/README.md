# {{project-name}}

An MCP server on [rust-web-server](https://rws8.tech).

```sh
cargo test
MCP_TOKEN=$(openssl rand -hex 32) cargo run   # MCP at http://127.0.0.1:7878/mcp
```

The layout is rws's conventional one (`rws.app.toml`): `src/main.rs` serves
the server that `src/lib.rs` builds, tools live in `src/tools/`, tests in
`tests/`. With `RWS_ENV=dev` a coding agent can also inspect the project at
`/_dev/mcp`.
