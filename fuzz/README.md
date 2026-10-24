# Fuzzing

[cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) (libFuzzer) targets for
every parser that sees untrusted input. Needs nightly Rust:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

| Target | Parser | Input comes from |
|---|---|---|
| `http_request` | `Request::parse` | any client |
| `http1_framing` | HTTP/1.1 head detection, body framing, chunked request bodies (via `fuzz_api`) | any client |
| `http_response` | `Response::parse` | proxied backends |
| `range` | `Range::parse_range_in_content_range`, `Content-Range` | clients / backends |
| `json` | `JSON::parse_as_properties`, `JSONProperty::parse` | request bodies |
| `mcp` | `McpServer::handle_request` (JSON-RPC, batches) | MCP clients |
| `form` | url-encoded + multipart form bodies | request bodies |
| `url` | `URL::parse_request_target` | request targets |
| `jwt` | `auth::verify_jwt` | bearer tokens |
| `saml_xml` | the SAML XML parser (via `fuzz_api`) | `POST /saml/acs`, unauthenticated |
| `config_file` | `rws.config.toml` settings loader | operator files |
| `proxy_config` | proxy route parser | operator files |

## Running

```bash
fuzz/run.sh            # every target for 60 s, 4 in parallel
fuzz/run.sh 600 8      # 10 minutes each, 8 in parallel
cargo +nightly fuzz run http_request corpus/http_request seeds/http_request   # one target, until stopped
```

`seeds/<target>/` holds hand-written starting inputs (committed); the fuzzer
grows `corpus/<target>/` from them (not committed). A crash is written to
`artifacts/<target>/crash-*` — reproduce it with
`cargo +nightly fuzz run <target> artifacts/<target>/crash-...`, fix it, and
add a regression test next to the parser.

CI runs every target for 30 s on each push and for 10 minutes each in the
weekly scheduled run (`.github/workflows/ci.yml`, `fuzz` job).

## What fuzzing found so far

All fixed, each with a regression test:

- A request whose target didn't start with `/` (`GET :x HTTP/1.1`) panicked
  the worker thread inside `url-build-parse`; with the thread pool's fixed
  size, that many requests stopped the HTTP/1.1 server.
- Request and response parsing recursed once per header line, and multipart
  parsing once per part: a single request with ~2,000 header lines overflowed
  the stack and aborted the process. The SAML XML parser recursed per nesting
  level with no limit.
- `Content-Length: <not a number>` panicked (`parse().unwrap()`) in both the
  request and the response parser.
- A response header line without `: ` indexed past the end of a split.
- A trailing `\` in an MCP JSON-RPC string sliced past the end of the input.
- A config value that is a lone quote (`key = '`) panicked the proxy-route
  parser; a NUL byte in any config value panicked `env::set_var`.
