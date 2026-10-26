---
title: Forms & File Uploads
description: Parse URL-encoded forms and multipart file uploads from incoming request bodies.
---

`rust-web-server` provides two parsers for HTML form submissions, both living under `rust_web_server::body`:

- `FormUrlEncoded` — for `application/x-www-form-urlencoded` bodies (plain text fields)
- `FormMultipartData` — for `multipart/form-data` bodies (files and binary data)

## URL-encoded forms

```rust
use rust_web_server::body::form_urlencoded::FormUrlEncoded;
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;

fn handle_login(
    req: &Request,
    _params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &(),
) -> Response {
    let fields = match FormUrlEncoded::parse(req.body.to_vec()) {
        Ok(map) => map,
        Err(e)  => {
            // body was not valid UTF-8
            let mut r = Response::new();
            r.status_code = 400;
            return r;
        }
    };

    let username = fields.get("username").map(String::as_str).unwrap_or("");
    let password = fields.get("password").map(String::as_str).unwrap_or("");

    // ... authenticate ...
    Response::new()
}
```

`FormUrlEncoded::parse` takes the raw body bytes (`Vec<u8>`) and returns `Result<HashMap<String, String>, String>`. Percent-encoding is decoded and ASCII control characters are stripped automatically.

To serialise a map back to URL-encoded format:

```rust
use std::collections::HashMap;
use rust_web_server::body::form_urlencoded::FormUrlEncoded;

let mut map = HashMap::new();
map.insert("q".to_string(), "hello world".to_string());
let encoded = FormUrlEncoded::generate(map);
// encoded == "q=hello+world" (or similar)
```

## Multipart form data

HTML file-upload forms use `enctype="multipart/form-data"`. The body consists of multiple **parts**, each with its own headers and a binary body. The parts are separated by a **boundary** string that appears in the `Content-Type` header.

### Extracting the boundary

```rust
use rust_web_server::body::multipart_form_data::FormMultipartData;
use rust_web_server::header::Header;

fn get_boundary(req: &Request) -> Result<String, String> {
    let ct = req.header("content-type")
        .ok_or_else(|| "missing Content-Type".to_string())?;

    FormMultipartData::extract_boundary(&ct.value)
}
```

`extract_boundary` splits the `Content-Type` value on `boundary=` and returns everything after it.

### Parsing the body

```rust
use rust_web_server::body::multipart_form_data::{FormMultipartData, Part};
use rust_web_server::request::Request;
use rust_web_server::response::Response;
use rust_web_server::router::PathParams;
use rust_web_server::server::ConnectionInfo;

fn upload(
    req: &Request,
    _params: &PathParams,
    _conn: &ConnectionInfo,
    _state: &(),
) -> Response {
    // 1. Get boundary from Content-Type
    let content_type = req
        .header("content-type")
        .map(|h| h.value.to_string())
        .unwrap_or_default();

    let boundary = match FormMultipartData::extract_boundary(&content_type) {
        Ok(b)  => b,
        Err(_) => {
            let mut r = Response::new();
            r.status_code = 400;
            return r;
        }
    };

    // 2. Parse all parts
    let parts = match FormMultipartData::parse(&req.body, boundary) {
        Ok(p)  => p,
        Err(_) => {
            let mut r = Response::new();
            r.status_code = 400;
            return r;
        }
    };

    // 3. Iterate over parts
    for part in &parts {
        // Each part has headers (e.g. Content-Disposition, Content-Type)
        if let Some(disposition) = part.header("content-disposition") {
            let name = extract_field_name(&disposition.value);
            let file_name = extract_filename(&disposition.value);

            if let Some(fname) = file_name {
                // This part is a file upload
                let file_bytes: &[u8] = &part.body;
                println!("Received file '{}' ({} bytes)", fname, file_bytes.len());
                // Save file_bytes to disk, S3, etc.
            } else {
                // Plain text field
                let value = String::from_utf8_lossy(&part.body);
                println!("Field '{}' = '{}'", name, value);
            }
        }
    }

    Response::new()
}

fn extract_field_name(disposition: &str) -> &str {
    // Content-Disposition: form-data; name="field_name"; filename="file.txt"
    disposition
        .split(';')
        .find_map(|seg| seg.trim().strip_prefix("name=\"").and_then(|s| s.strip_suffix('"')))
        .unwrap_or("")
}

fn extract_filename(disposition: &str) -> Option<&str> {
    disposition
        .split(';')
        .find_map(|seg| seg.trim().strip_prefix("filename=\"").and_then(|s| s.strip_suffix('"')))
}
```

### The `Part` type

Each `Part` returned by `FormMultipartData::parse` has:

| Field | Type | Description |
|---|---|---|
| `headers` | `Vec<Header>` | Per-part headers (Content-Disposition, Content-Type, etc.) |
| `body` | `Vec<u8>` | Raw bytes of this part's body |

Call `part.header("header-name")` (or `part.get_header(String)`) for case-insensitive lookup of a single header.

## Storing uploaded files

`FormMultipartData::parse` hands back raw bytes — it doesn't decide where they go. Use the [`Storage`](/features/storage/) trait (`storage-local` / `storage-s3` / `storage-azure` features) so the same handler code works against local disk in development and S3-compatible or Azure Blob object storage in production:

```rust
use rust_web_server::storage::{LocalStorage, Storage};

let store = LocalStorage::new("/var/data/uploads");
let key = store.put("avatars/42.png", &part.body, "image/png")?;
```

See [File / Object Storage](/features/storage/) for the full API, including `S3Storage` for AWS S3, Cloudflare R2, and MinIO, and `AzureBlobStorage` for Azure Blob Storage.

## Size limits

Two independent knobs apply to request bodies:

- **`RWS_CONFIG_REQUEST_ALLOCATION_SIZE_IN_BYTES`** (default `10000`) — the size of each individual socket read. Bodies larger than this are accumulated across multiple reads automatically as long as the request declares `Content-Length`; this is a buffer-chunking size, not a hard cap on upload size.
- **`RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES`** (default `0`, unlimited) — the actual maximum accepted body size. When set, a request whose declared `Content-Length` exceeds it is rejected with `413 Payload Too Large` **before** any of its body is read off the socket, and the connection is closed rather than kept alive.

For an upload endpoint, set `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` to the largest file you intend to accept; leave it at `0` if uploads are already bounded some other way (e.g. a reverse proxy in front of `rws`). On HTTP/2 and HTTP/3 the cap is checked as `DATA` frames arrive, so an oversized body gets `413` as soon as it crosses the limit.

The limit is global: routes are matched only after the whole request, body included, has been read, so no per-route limit can run before buffering. For one route that must be stricter, the config-driven proxy has [`[route.middleware] max_body_size`](/proxy/config-driven/), and a `Router`/`AppWithState` handler can check `request.body.len()` and return `AppError::PayloadTooLarge`. Both run after the body is in memory, so they protect that route's logic, not server memory.

## Streaming large uploads

By default a request's body is read into memory before the handler runs.
For large uploads a handler can instead read the body **as it arrives**
(since 18.0.0): opt in per route, and `request.body` is a stream —
memory stays at a few chunks whatever the size.

```rust
use std::io::Read;
use rust_web_server::app::App;
use rust_web_server::response::{Response, STATUS_CODE_REASON_PHRASE};

let app = App::with_state(())
    .put("/upload/:name", |request, params, _conn, _state| {
        let path = format!("/var/data/uploads/{}", params.get("name").unwrap_or("upload"));
        let mut file = match std::fs::File::create(&path) {
            Ok(file) => file,
            Err(e) => return Response::text(STATUS_CODE_REASON_PHRASE.n500_internal_server_error, &e.to_string()),
        };
        // `&request.body` implements std::io::Read.
        match std::io::copy(&mut &request.body, &mut file) {
            Ok(bytes) => Response::text(STATUS_CODE_REASON_PHRASE.n200_ok, &format!("{bytes} bytes")),
            Err(e) => Response::text(STATUS_CODE_REASON_PHRASE.n400_bad_request, &e.to_string()),
        }
    })
    .stream_body("PUT", "/upload/:name");
```

- Any `Application` can opt in by implementing
  `streams_body(&self, request, connection) -> bool`; it is asked after the
  head is read, so decide from the method, path and headers.
- The handler runs on a blocking-pool thread while the body arrives; a
  client that stops, stalls past the read timeout, or sends more than
  `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES` makes the read fail with an error,
  rather than the body just ending early.
- If the handler answers without reading the whole body, the response goes
  out with `Connection: close` and the connection is closed.
- Middleware must agree: a stack streams only if every layer's
  `accepts_streamed_body` says so. The built-in header-only layers (access
  log, rate limit, blocklist, IP filter, request id, metrics, maintenance,
  cache, basic/JWT/forward auth) do; a layer that reads the body must not.
- Served over HTTP/1.1 (plain and TLS) and HTTP/2. On HTTP/2 the stream's
  flow-control window is given back only as the handler reads, so a client
  is never more than one window (64 KiB) ahead of it. HTTP/3 and the
  HTTP/1.1-only build read the body first as before.
- `FormMultipartData::parse` still needs the whole body; for multipart
  uploads of large files, read the body with `body_bytes()` only up to a
  size you accept, or upload the raw file instead of a form.

## `Expect: 100-continue` for large uploads

If a client sends `Expect: 100-continue` with an upload (many HTTP clients — curl, most HTTP libraries — do this automatically for large request bodies), `rws` responds with the `100 Continue` interim status immediately after parsing the headers, before it reads any of the body. This is handled automatically on HTTP/1.1 — no application code needed:

```bash
curl -v -X PUT --data-binary @large-file.bin \
  -H "Expect: 100-continue" http://localhost:7878/upload
# curl waits for "< HTTP/1.1 100 Continue" before it starts uploading
```

The `413`/`417` checks above run *before* the `100 Continue` is sent, not after:

- If the declared `Content-Length` exceeds `RWS_CONFIG_MAX_BODY_SIZE_IN_BYTES`, the client gets `413 Payload Too Large` instead of `100 Continue` — it's never invited to upload a body that's already going to be rejected.
- An `Expect` value other than `100-continue` (the only one this server understands) gets `417 Expectation Failed`, also without reading any body.

This applies to HTTP/1.1 only (plain and TLS). HTTP/2 and HTTP/3 read request bodies as separate `DATA` frames after the headers, rather than one blocking read off a raw byte stream, so they don't have the head-of-line blocking risk `100-continue` exists to avoid.

## Generating multipart bodies (testing)

`FormMultipartData::generate` can build a multipart body for outbound requests or tests:

```rust
use rust_web_server::body::multipart_form_data::{FormMultipartData, Part};
use rust_web_server::header::Header;

let file_part = Part {
    headers: vec![
        Header::new("Content-Disposition", r#"form-data; name="avatar"; filename="photo.jpg""#),
        Header::new("Content-Type", "image/jpeg"),
    ],
    body: include_bytes!("photo.jpg").to_vec(),
};

let boundary = "--WebKitFormBoundaryABC123";
let body_bytes = FormMultipartData::generate(vec![file_part], boundary).unwrap();
```
