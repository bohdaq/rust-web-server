---
title: Compression
description: Automatic brotli, zstd and gzip compression for text responses, negotiated per request, with static files compressed once.
---

`rust-web-server` compresses text responses automatically, in the coding the
client prefers: `br` (brotli), `zstd` or `gzip` (brotli and zstd since
18.1.0). No middleware registration, no configuration.

## Which coding

The request's `Accept-Encoding` decides: the coding with the highest `q`
wins, `*` stands for any coding not listed, and `q=0` refuses one. When
several are equally acceptable — a browser sends
`gzip, deflate, br, zstd` — the server's order breaks the tie:

| Response | Order | Why |
|---|---|---|
| Static files (compressed once, then cached) | `br`, `zstd`, `gzip` | brotli's output is smallest, and it is paid for once |
| Everything else (compressed per response) | `zstd`, `br`, `gzip` | zstd costs the least CPU at a similar ratio |

A request without `Accept-Encoding` gets the response uncompressed.

Brotli and zstd are the `brotli` and `zstd` Cargo features, both on by
default (`zstd` builds the C libzstd). Without them only gzip is offered;
the `http1`-only build (`--no-default-features --features http1`) has
gzip only.

## What gets compressed

`compression::apply_compression(request, response)` runs after your
handler, before the response is written (HTTP/1.1, HTTP/2 and HTTP/3). It
compresses when:

1. The response has no `Content-Encoding` already — a backend's compressed
   response is passed through, never compressed twice.
2. The body is at least 512 bytes (`compression::MIN_COMPRESS_SIZE`, the
   same minimum Caddy uses).
3. The client accepts a coding (above).
4. The `Content-Type` is compressible (below).
5. The response isn't partial: a `206` is a slice of the file and is sent as
   is (since 17.113.2).

Levels are the fast ones peers use for per-response compression: brotli 4,
zstd 3, gzip 6 (`compression::Level::Dynamic`; brotli's default, 11, is
about a hundred times slower than gzip). If the result isn't smaller than
the original, the original is sent. A compressed response gets:

- `Content-Encoding: br`, `zstd` or `gzip`
- `Vary: Accept-Encoding` (appended to an existing `Vary`, or added)
- its own `ETag` when it had one: `"x"` becomes `"x-br"` (since 18.1.0;
  each representation needs a different strong ETag)

`compression::apply_gzip` still exists and only ever uses gzip.

## Compressible content types

Only responses whose `Content-Type` starts with one of these are compressed:

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

Images, audio, video, fonts and archives are already compressed.

## Static files are compressed once

A static file is compressed once per version per coding, not on every
request: the open-file cache keeps the brotli, zstd and gzip forms of each
file up to 1 MiB, keyed by its size and modification time, within the same
8 MiB memory budget, at the higher `Level::Static` (brotli 6, zstd 6,
gzip 6). An edit is picked up at once; a file modified less than 2 seconds
ago isn't cached yet. The `ETag` carries the coding (`"<mtime>-<size>-br"`),
so a client revalidating gets a `304` for the representation it holds.

### Precompressed files

If `app.js.br`, `app.js.zst` or `app.js.gz` sits next to `app.js`, is a
regular file (not a symlink) and is at least as new as `app.js`, its bytes
are sent for that coding instead of compressing — as nginx's `gzip_static`
does. Build them once with the best settings (`brotli -q 11`,
`zstd -19`):

```bash
brotli -q 11 -k static/app.js       # static/app.js.br
zstd -19 -k static/app.js           # static/app.js.zst
gzip -9 -k static/app.js            # static/app.js.gz
```

A sibling older than the file (the file was edited since) is ignored, and
the file is compressed instead. Like the cached forms, this applies to
files up to 1 MiB.

## Large files

Static files larger than **256 KiB** are sent without being read into
memory — `sendfile(2)` over plain HTTP, chunked otherwise — except that a
client accepting a coding gets a compressible file of up to **1 MiB**
compressed from the cache. Larger files are sent uncompressed.

:::note[Range requests]
Byte-range requests (`Range: bytes=N-M`) are answered with the slice,
uncompressed: compressing a slice would describe bytes that don't exist.
:::

## Checking it

```bash
curl -s -D - -o /dev/null -H "Accept-Encoding: br" http://localhost:7878/app.js
# content-encoding: br
# vary: Accept-Encoding
# etag: "1793007086216830266-26624-br"
```

## No configuration needed

There are no `rws.config.toml` keys, environment variables or middleware
layers for compression.
