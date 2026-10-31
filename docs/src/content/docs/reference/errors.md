---
title: Error responses
description: The errors rws itself produces, as RFC 9457 problem details (application/problem+json), and how to get the plain-text bodies back.
---

Since 18.17.0 the errors rws produces itself, rather than your application,
answer with [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457) problem
details:

```http
HTTP/1.1 503 Service Unavailable
Content-Type: application/problem+json
Retry-After: 5

{"type":"about:blank","title":"Service Unavailable","status":503,"detail":"every backend is out of rotation"}
```

`type` is `about:blank`, so `title` is the status's reason phrase, and
`detail` (when there is one) says what happened. Status codes and other
headers (`Retry-After`, `WWW-Authenticate`, `Allow`) are unchanged.

## Which errors

| Status | When | `detail` |
|---|---|---|
| `400` | A request rws can't parse; a body an extractor rejects | The parse error |
| `401` | Basic / JWT / bearer auth, `PURGE` without the purge token, authorization without an identity | — or the reason |
| `403` | IP filter, blocklist, CSRF, authorization denied | — |
| `404` | No matching ingress rule; no matching Gateway API route | — |
| `408` | The request head took longer than 30 s | How long it may take |
| `413` | A body over `max_body_size` | The declared or reached size and the limit |
| `417` | An `Expect` value other than `100-continue` | The value |
| `421` | A Gateway API request for a host the connection isn't for | — |
| `429` | A rate limit | — |
| `431` | A request head over 64 KiB | The limit |
| `500` | An SSO callback that fails, a failing WASM plugin, a handler that errs | The reason |
| `501` | A method no built-in handles | The method |
| `502` | An upstream that can't be reached or answers garbage; an unreachable forward-auth service | — or the reason |
| `503` | Every backend out of rotation; maintenance mode; a route whose auth or plugin is misconfigured | The reason |
| `504` | An upstream or a handler that took too long | — |

Not affected:

- The 404 page for a missing static file (your `404.html`, or the
  built-in one): that's your site's content.
- APIs with their own error format: the admin API, MCP (JSON-RPC errors),
  the LLM proxy (OpenAI-style errors), the OAuth server (RFC 6749 errors),
  validation errors (`422` with the field list).
- HTTP/2 `413`/`431` sent while the request is still arriving: those are
  headers only.

`AppError::into_response()` uses the same shape, so your own errors match
rws's.

## Plain text instead

```toml
error_format = "text"   # RWS_CONFIG_ERROR_FORMAT
```

brings back the bodies of earlier releases: `502 Bad Gateway`,
`503 Service Unavailable: every backend is out of rotation`, and so on, as
`text/plain`.
