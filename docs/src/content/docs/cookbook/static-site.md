---
title: "Cookbook: static site"
description: Serve a directory of files with automatic HTTPS from Let's Encrypt, HTTP→HTTPS redirects, compression and browser caching.
---

rws serves the directory it runs in. With a domain pointed at the server,
this is the whole configuration:

```toml
# /var/www/example/rws.config.toml — run rws in /var/www/example
port = 443
http_redirect_port = 80          # redirects to HTTPS and answers the ACME challenge

[acme]
domains = ["example.com", "www.example.com"]
email   = "ops@example.com"
```

What you get without more settings: a Let's Encrypt certificate ordered at
startup and renewed before it expires, HTTP/1.1, HTTP/2 and HTTP/3, gzip /
Brotli / zstd for text files (precompressed `.br`/`.gz` siblings are used
when present), `ETag` and `Last-Modified` with `304`s, `index.html` for
directories, and dot-files (`.git/`, `.env`) never served.

Long-lived caching for fingerprinted assets, a custom 404 page and no
directory listings:

```toml
port = 443
http_redirect_port = 80
directory_listing = false        # 404 instead of listing a directory without index.html

[acme]
domains = ["example.com"]
email   = "ops@example.com"

[[route]]
name = "assets"
[route.match]
path = "/assets/*"
[route.action]
type = "static"
root = "/var/www/example"
[route.middleware]
early_hints = ["</assets/app.css>; rel=preload; as=style"]
```

Hugo, Jekyll, Astro, Eleventy and other generators: build into a directory
and run rws there (`cd public && rws`). With the [packages](/deployment/systemd/)
put the files in `/var/www/rws` and the configuration in
`/etc/rws/rws.config.toml`.
