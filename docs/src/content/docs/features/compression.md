---
title: Compression
description: Automatic gzip compression for text-based responses with no configuration required.
---

`rust-web-server` compresses HTTP responses automatically when the client signals support via the `Accept-Encoding: gzip` request header. No middleware registration, no feature flag, no configuration — it works out of the box for every build variant.

## How it works

`compression::apply_gzip(request, response)` is called by the HTTP/1.1 connection handler after `app.execute()` returns, before the response bytes are written to the socket. It compresses only when:

1. The response has no `Content-Encoding` already — a backend's gzipped or Brotli response is passed through, never compressed twice.
2. The body is at least 512 bytes (`compression::MIN_COMPRESS_SIZE`, the same minimum Caddy uses): below that, gzip's framing and CPU cost outweigh the savings.
3. The `Accept-Encoding` request header contains `gzip` (case-insensitive).
4. The response `Content-Type` matches a compressible MIME type (see below).

Every `ContentRange` is then compressed with `flate2`'s default level. If the
result isn't smaller than the original — random or already-compressed data
served as text — the original body is sent instead. A compressed response
receives:

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

## Static files are compressed once

A static file is compressed once per version, not on every request: the
open-file cache keeps the gzip form of each file up to 1 MiB next to its
contents, keyed by the file's size and modification time, within the same
8 MiB memory budget. An edit is picked up at once (the size or time
changes); a file modified less than 2 seconds ago isn't cached yet, as for
the contents. The bytes and headers are exactly what `apply_gzip` would
produce. Compressing a 26 KiB JavaScript file on every request cost about
80× the CPU of serving it; from the cache it costs about the same as an
uncompressed file.

Responses from your handlers and from proxied backends are still compressed
per response.

## Large file streaming

Static files larger than **256 KiB** are sent without being read into memory
— `sendfile(2)` over plain HTTP, chunked otherwise (`response.stream_file`)
— except that a client accepting gzip gets a compressible file of up to
**1 MiB** gzip-compressed from the cache (a 330 KiB JavaScript bundle went
out as 80 KiB, at less CPU than sending it whole). Larger files, and every
large file for a client without gzip, are sent uncompressed.

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
