---
title: LLM Proxy
description: One endpoint in front of OpenAI-compatible and Anthropic providers: virtual keys, token usage from JSON and streams, budgets shared through Redis, provider fallback and rws_llm_* metrics.
---

The LLM proxy (since 18.9.0) puts one endpoint in front of LLM providers. Applications keep their OpenAI or Anthropic SDK and point its base URL at rws; rws swaps the caller's virtual key for the provider's key, counts the tokens each response reports, enforces token budgets, and fails over to the next provider when one is down or rate limited.

It speaks two APIs:

| API | Requests | Usage read from |
|---|---|---|
| `openai` (OpenAI and every OpenAI-compatible server: Azure OpenAI, vLLM, Ollama, LiteLLM, …) | any path not ending in `/messages`: `/v1/chat/completions`, `/v1/responses`, `/v1/embeddings`, `/v1/models` | `usage.prompt_tokens` / `completion_tokens` (Chat Completions, also in the final chunk of a stream), `usage.input_tokens` / `output_tokens` (Responses API, `response.completed` event) |
| `anthropic` (the Messages API) | paths ending in `/messages` or `/messages/count_tokens` | `usage` in the body; in a stream `message_start` (`message.usage`) and `message_delta` (cumulative `output_tokens`) |

Anthropic's `cache_creation_input_tokens` and `cache_read_input_tokens` count as input tokens: they are billed, so they count toward budgets.

## In the config file

```toml
[[route]]
  [route.match]
  path = "/v1/*"

  [route.action]
  type = "llm"

  [route.action.llm]
  audit_log          = true               # one JSON line per request in the access log
  redis              = "127.0.0.1:6379"   # share budgets between replicas (default: in memory)
  redis_password_env = "REDIS_PASSWORD"
  redis_prefix       = "rws:llm:"
  connect_timeout_ms = 10000
  read_timeout_ms    = 300000             # per read of the provider's response
  allow_anonymous    = false              # true: requests without a known key pass as "anonymous"

  [[route.action.llm.provider]]           # tried in this order, per API
  name    = "openai"
  api     = "openai"
  url     = "https://api.openai.com"
  key_env = "OPENAI_API_KEY"

  [[route.action.llm.provider]]           # the fallback for openai requests
  name = "local"
  api  = "openai"
  url  = "http://127.0.0.1:11434"         # Ollama: no key

  [[route.action.llm.provider]]
  name    = "anthropic"
  api     = "anthropic"
  url     = "https://api.anthropic.com"
  key_env = "ANTHROPIC_API_KEY"

  [[route.action.llm.key]]                # a virtual key clients send
  identity      = "alice"
  key_env       = "ALICE_LLM_KEY"
  budget_tokens = 2000000                 # optional: tokens per window
  window_secs   = 86400
```

Secrets never sit in the file: provider keys, virtual keys and the Redis password come from the environment variables named. `rws --check` reports unknown keys, bad APIs and URLs, duplicate provider names, a budget without a window, a proxy nobody can call, and unset variables.

The request path goes to the provider after the provider URL's own path: with `url = "https://example.com/openai"`, `/v1/chat/completions` becomes `/openai/v1/chat/completions`.

Clients use their SDK as usual:

```sh
export OPENAI_BASE_URL=https://llm.example.com/v1  OPENAI_API_KEY=$ALICE_LLM_KEY
export ANTHROPIC_BASE_URL=https://llm.example.com  ANTHROPIC_API_KEY=$ALICE_LLM_KEY
```

## Virtual keys

A client authenticates with `Authorization: Bearer <key>` or `x-api-key: <key>` (OpenAI's and Anthropic's SDKs send one each). The key names an identity; an unknown key gets `401` with an error body in the providers' shape. The virtual key never reaches a provider: rws removes `Authorization` and `x-api-key` and sends the provider's own key, as `Authorization: Bearer` for `openai` and as `x-api-key` for `anthropic` (with `anthropic-version: 2023-06-01` unless the client sent one). It also removes `Accept-Encoding`, so it can read the response (rws compresses to the client itself).

## Fallback

Providers of the request's API are tried in the order listed. One that doesn't answer, or answers `429` or a `5xx`, is skipped for the next; the last one's answer is returned whatever it is. All of this happens before the first byte reaches the client, so a streamed answer never switches provider halfway. Each skip counts in `rws_llm_fallbacks_total{provider}`. When no provider answers, the client gets `502`.

## Streams

Event streams pass through as they arrive; rws reads the usage from them on the way. OpenAI only reports usage in a stream when asked, so for an `openai` request with `"stream": true` and no `stream_options`, rws adds `"stream_options":{"include_usage":true}`: the stream then ends with one extra chunk with empty `choices` that carries the usage. A request that sets `stream_options` itself is sent unchanged.

A stream the client abandons still counts: the usage seen so far is recorded when the connection goes away (for Anthropic that includes the input tokens from `message_start`).

## Budgets

`budget_tokens` per `window_secs` is a fixed window per identity, counting input plus output tokens. It is checked before each request and charged after it with what the provider reported, so the request that crosses the budget completes and the next one gets `429` with `Retry-After` (seconds to the next window) and counts in `rws_llm_budget_rejections_total{identity}`.

Without `redis` the counts live in the process. With it, every replica pointing at the same Redis shares them (`INCRBY` on `<redis_prefix><identity>:<window_secs>:<window number>`, expiring a minute after the window). Redis errors are logged and let requests through: an outage of Redis doesn't take the proxy down with it.

## Metrics and audit log

On `/metrics`:

| Series | Labels |
|---|---|
| `rws_llm_requests_total` | `provider`, `model`, `status` (`error` when the provider didn't answer) |
| `rws_llm_tokens_total` | `provider`, `model`, `identity`, `type` (`input`, `output`) |
| `rws_llm_time_to_first_token_seconds` (histogram) | `provider`, `model` |
| `rws_llm_fallbacks_total` | `provider` |
| `rws_llm_budget_rejections_total` | `identity` |

`model` is the request's `model`. Because clients choose it, the label is bounded: past 64 distinct names, or for a name with characters other than letters, digits and `-_.:/@`, it reads `_other`.

With `audit_log = true` each request writes one line through the access log:

```json
{"time":"2026-10-29T12:00:00Z","event":"llm_request","sub":"alice","provider":"openai","model":"gpt-4.1-mini","status":200,"input_tokens":31,"output_tokens":12,"ttft_ms":412.204,"duration_ms":1630.511}
```

## In code

`rust_web_server::llm::LlmProxy` is an `Application`, so it can be served on its own or mounted behind a route:

```rust,no_run
use rust_web_server::llm::{Api, LlmProxy, Provider};
use rust_web_server::prelude::*;

#[tokio::main]
async fn main() {
    let key = |name: &str| std::env::var(name).unwrap_or_default();
    let proxy = LlmProxy::new()
        .provider(Provider::new("openai", Api::OpenAi, "https://api.openai.com").unwrap().api_key(key("OPENAI_API_KEY")))
        .provider(Provider::new("anthropic", Api::Anthropic, "https://api.anthropic.com").unwrap().api_key(key("ANTHROPIC_API_KEY")))
        .virtual_key("vk-alice", "alice")
        .budget("alice", 1_000_000, 86_400)
        .redis("127.0.0.1:6379", None, "rws:llm:")
        .audit_log(true);
    let (listener, pool) = Server::setup().unwrap();
    Server::run_tls(listener, pool, proxy).await;
}
```

`UsageParser` is public too, for reading usage out of a provider response elsewhere: feed it the body in pieces with the response's content type and call `finish()`.

The full example is `examples/llm_proxy.rs`; `ci/e2e/llm_proxy.sh` runs the proxy against stand-in providers and a real Redis.
