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
| `form` | url-encoded + multipart form bodies; the `Query<T>`/`Form<T>` deserializer | request bodies, query strings |
| `url` | `URL::parse_request_target` | request targets |
| `jwt` | `auth::verify_jwt` | bearer tokens |
| `saml_xml` | the SAML XML parser (via `fuzz_api`) | `POST /saml/acs`, unauthenticated |
| `rsa_key` | RSA key parsing (DER, PEM, JWK) and RS256 / rsa-sha256 verification | IdP metadata, JWKS, SAML `SignatureValue` |
| `h2_hpack` | HPACK header-block decoder (`h2_server::hpack`) | HTTP/2 clients |
| `h2_connection` | rws's HTTP/2 server: frames, stream states, flow control (`h2_server::fuzz`) | HTTP/2 clients |
| `config_file` | `rws.config.toml` settings loader | operator files |
| `app_manifest` | `rws.app.toml` app manifest parser (`AppManifest::parse`) | project files |
| `generator` | `rws gen`'s readers of lib.rs, mod.rs, Cargo.toml, route and test files, and its name and field parsers (`generator::fuzz`) | project files, `gen_*` MCP tool arguments |
| `proxy_config` | proxy route parser | operator files |
| `websocket_frame` | `WebSocket::read_frame_limited` | WebSocket clients |
| `dns_srv_response` | the DNS SRV response parser (`service_discovery`) | DNS servers |
| `dns_message` | the shared DNS message parser (`dns::parse`), TXT values, DNS UPDATE replies (ACME DNS-01) | DNS servers |
| `redis_reply` | the RESP reply reader (`redis_protocol`) | Redis servers |
| `cookie_header` | `CookieJar::parse` | any client |
| `otel_baggage` | the W3C `baggage` header parser (`otel::Baggage::parse`) | any client, with tracing on |
| `accept_encoding` | `Accept-Encoding` negotiation for gzip / br / zstd (via `fuzz_api`) | any client |
| `openapi_to_mcp` | OpenAPI documents to MCP tools: `$ref` inlining, schemas, request building (via `fuzz_api`) | an upstream API's OpenAPI document |
| `waf_inspect` | the WAF's decoding and detections, JSON and multipart field extraction (`waf`) | any client, with the WAF on |
| `base64_decode` | the shared base64 decoder (`core::base64::Decoder`): JWTs, PEM, PKCE, webhook secrets, gRPC-Web text, fleet keys | any client |
| `fleet_signature` | fleet `.sig` files and Ed25519 verification (`fleet`) | whoever can serve the fleet source |
| `pkcs11_uri` | `pkcs11:` URIs and PIN masking (`tls::pkcs11`) | config authors |
| `fastcgi_record` | FastCGI records, name-value pairs and CGI response headers (`fastcgi`) | FastCGI servers (php-fpm) |
| `a2a_request` | A2A JSON-RPC requests: parsing, messages, parts, tasks (via `fuzz_api`) | any A2A client |
| `accept_language` | `Accept-Language` parsing and language negotiation (via `fuzz_api`) | any client |
| `i18n_catalog` | i18n catalog files and message formatting (via `fuzz_api`) | catalog authors |
| `proxy_protocol` | the PROXY protocol v1/v2 header parser (`server::proxy_protocol::parse`) | L4 load balancers, or anyone when it's enabled |
| `mcp_gateway_reply` | what the MCP gateway reads from an upstream: JSON or event-stream replies, `tools/list` results (via `fuzz_api`) | upstream MCP servers |
| `llm_usage` | what the LLM proxy reads from a provider: token usage in JSON bodies and event streams fed in pieces, chunked framing (via `fuzz_api`) | LLM providers |
| `traffic_classify` | a User-Agent and path through the traffic classifier (via `fuzz_api`) | request headers |
| `authz_parse` | an authorization tuple, subject and relation definition (via `fuzz_api`) | admin API bodies; the config file |
| `available_dictionary` | an `Available-Dictionary` value, and `dcz`/`dcb` bodies through both decoders (via `fuzz_api`) | request headers; dictionary-compressed bodies |
| `cache_entry` | a disk-cache entry file read back: header line, key, stored response (via `fuzz_api`) | the cache directory |
| `wasm_plugin_output` | what a WebAssembly plugin returns: request and response directives (via `fuzz_api`) | plugins (`[route.middleware.wasm]`) |
| `sse_stream` | the test client's Server-Sent Events parser (`test_client::SseParser`), whole and in pieces | the app under test |

## Running

```bash
fuzz/run.sh            # every target for 60 s, 4 in parallel
fuzz/run.sh 600 8      # 10 minutes each, 8 in parallel
cargo +nightly fuzz run http_request corpus/http_request seeds/http_request   # one target, until stopped
```

`seeds/http1_framing/` and `seeds/http_request/` include a request-smuggling corpus (`smuggle_*`: CL+TE either way, duplicate or listed `Content-Length`, obfuscated or folded `Transfer-Encoding`, bare-LF chunks, oversized chunk sizes, trailers); `server::framing`'s `the_smuggling_corpus_is_refused_or_framed_consistently` test pins how each one is handled.

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
