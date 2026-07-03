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
    let fields = match FormUrlEncoded::parse(req.body.clone()) {
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
    let ct = req.get_header("content-type")
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
        .get_header("content-type")
        .map(|h| h.value.clone())
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
        if let Some(disposition) = part.get_header("content-disposition".to_string()) {
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

Call `part.get_header("header-name")` for case-insensitive lookup of a single header.

## Storing uploaded files

`FormMultipartData::parse` hands back raw bytes — it doesn't decide where they go. Use the [`Storage`](/features/storage/) trait (`storage-local` / `storage-s3` features) so the same handler code works against local disk in development and S3-compatible object storage in production:

```rust
use rust_web_server::storage::{LocalStorage, Storage};

let store = LocalStorage::new("/var/data/uploads");
let key = store.put("avatars/42.png", &part.body, "image/png")?;
```

See [File / Object Storage](/features/storage/) for the full API, including `S3Storage` for AWS S3, Cloudflare R2, and MinIO.

## Size limits

Request bodies are limited by `request_allocation_size` from `ConnectionInfo`. The default is configured via `RWS_CONFIG_REQUEST_ALLOCATION_SIZE`. Large uploads beyond this limit are rejected at the TCP read stage before any parser is called.

:::note[Streaming uploads]
The multipart parser reads the entire body into memory before returning. For very large files, consider streaming the upload to disk at the TCP layer or using an object-storage pre-signed URL workflow and directing the client to upload directly.
:::

## Generating multipart bodies (testing)

`FormMultipartData::generate` can build a multipart body for outbound requests or tests:

```rust
use rust_web_server::body::multipart_form_data::{FormMultipartData, Part};
use rust_web_server::header::Header;

let file_part = Part {
    headers: vec![
        Header {
            name: "Content-Disposition".to_string(),
            value: r#"form-data; name="avatar"; filename="photo.jpg""#.to_string(),
        },
        Header {
            name: "Content-Type".to_string(),
            value: "image/jpeg".to_string(),
        },
    ],
    body: include_bytes!("photo.jpg").to_vec(),
};

let boundary = "--WebKitFormBoundaryABC123";
let body_bytes = FormMultipartData::generate(vec![file_part], boundary).unwrap();
```
