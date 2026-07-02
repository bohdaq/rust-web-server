---
title: Compression
description: Automatic gzip compression for text-based responses with no configuration required.
---

`rust-web-server` compresses HTTP responses automatically when the client signals support via the `Accept-Encoding: gzip` request header. No middleware registration, no feature flag, no configuration — it works out of the box for every build variant.

## How it works

`compression::apply_gzip(request, response)` is called in `Server::process()` after `app.execute()` returns, before the response bytes are written to the socket. It performs three checks in sequence:

1. The response body is non-empty.
2. The `Accept-Encoding` request header contains `gzip` (case-insensitive).
3. The response `Content-Type` matches a compressible MIME type (see below).

If all three pass, every `ContentRange` in the response body is compressed in-place with `flate2`'s default compression level. The response then receives:

- `Content-Encoding: gzip`
- `Vary: Accept-Encoding` (appended to any existing `Vary` value, or added if absent)

## Compressible content types

Only responses whose `Content-Type` starts with one of the following MIME types are compressed:

| MIME type |
|-----------|
| `text/html` |
| `text/css` |
| `text/javascript` |
| `text/plain` |
| `text/xml` |
| `application/json` |
| `application/javascript` |
| `application/xml` |
| `application/xhtml+xml` |
| `image/svg+xml` |

Binary formats (images, audio, video, fonts, archives) are not compressed because they are already compressed or compression yields no benefit.

## Large file streaming

Static files larger than **8 MB** are served via HTTP/1.1 chunked transfer encoding without loading the entire file into memory (`response.stream_file`). Because the file body never passes through the in-memory `ContentRange` list, `apply_gzip` is not called for these responses. Clients receive the raw (uncompressed) file bytes for large assets.

:::note[Range requests]
Byte-range requests (`Range: bytes=N-M`) always bypass chunked streaming and receive the slice directly. Gzip compression applies normally to range responses when the content type is compressible.
:::

## Example: verifying compression

```bash
curl -s -H "Accept-Encoding: gzip" http://localhost:7878/ \
  --output - | file -
# output: gzip compressed data
```

## No configuration needed

There are no `rws.config.toml` keys, environment variables, or middleware layers to enable compression. It is always active at the server-core level.
