# {{project-name}}

A JSON API on [rust-web-server](https://rws8.tech).

```sh
cargo test
cargo run                 # http://127.0.0.1:7878/todos, API docs at /docs
RWS_ENV=dev cargo run     # plus the dev MCP endpoint for coding agents at /mcp
```

The layout is rws's conventional one (`rws.app.toml`): `src/main.rs` serves
the app that `src/lib.rs` builds, handlers live in `src/routes/`, tests in
`tests/`.
