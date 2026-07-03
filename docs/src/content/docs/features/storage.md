---
title: File / Object Storage
description: A single Storage trait for local disk and S3-compatible object storage (AWS S3, Cloudflare R2, MinIO), with no AWS SDK dependency.
---

`FormMultipartData::parse()` (see [Forms & File Uploads](/building-apps/forms-uploads/)) hands back raw bytes with no place to put them. The `storage` module gives handlers a single `Storage` trait so the same upload code works against local disk in development and an S3-compatible bucket in production.

```rust
pub trait Storage: Send + Sync {
    fn put(&self, key: &str, data: &[u8], content_type: &str) -> Result<String, StorageError>;
    fn get(&self, key: &str) -> Result<Vec<u8>, StorageError>;
    fn delete(&self, key: &str) -> Result<(), StorageError>;
    fn url(&self, key: &str) -> String;
}
```

`url()` performs no I/O and never fails — it just formats a string.

## Local disk (`storage-local` feature)

No new dependencies.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["storage-local"] }
```

```rust
use rust_web_server::storage::{LocalStorage, Storage};

let store = LocalStorage::new("/var/data/uploads");

let key = store.put("avatars/42.png", &file_bytes, "image/png")?;
let bytes = store.get(&key)?;
store.delete(&key)?;
```

Keys are relative paths under the root directory; parent directories (e.g. `avatars/`) are created automatically. A key containing a `..` segment is rejected — the same defense used by the config-driven proxy's static-file action.

Content type is **not** persisted — plain files on disk have no metadata slot for it.

### Serving uploaded files back over HTTP

`LocalStorage::url()` returns the object's filesystem path by default, which isn't directly useful to a browser. If the storage root is also served as a static directory, call `.with_base_url()` so `url()` returns an HTTP path instead:

```rust
let store = LocalStorage::new("/var/data/uploads").with_base_url("/uploads");
let key = store.put("avatars/42.png", &file_bytes, "image/png")?;
assert_eq!("/uploads/avatars/42.png", store.url(&key));
```

Pair this with a [`Router`](/building-apps/routing/) route (or the config-driven proxy's `type = "static"` action — see [Config-Driven Proxy](/proxy/config-driven/)) that serves `/var/data/uploads` at the `/uploads` prefix.

## S3-compatible object storage (`storage-s3` feature)

Works with AWS S3, Cloudflare R2, MinIO, or any other S3-compatible provider. Signs every request with AWS Signature Version 4 using the existing outbound HTTP client (`hmac` + `sha2`) — **no AWS SDK dependency**.

```toml
[dependencies]
rust-web-server = { version = "17", features = ["storage-s3"] }
```

```rust,no_run
use rust_web_server::storage::{S3Storage, Storage};

let store = S3Storage::from_env()?;

let key = store.put("avatars/42.png", &file_bytes, "image/png")?;
let bytes = store.get(&key)?;
store.delete(&key)?;
let public_url = store.url(&key);
# Ok::<(), rust_web_server::storage::StorageError>(())
```

### Configuration

`S3Storage::from_env()` reads:

| Variable | Default |
|---|---|
| `RWS_S3_BUCKET` | **(required)** |
| `RWS_S3_REGION` | `us-east-1` |
| `RWS_S3_ACCESS_KEY` | **(required)** |
| `RWS_S3_SECRET_KEY` | **(required)** |
| `RWS_S3_ENDPOINT` | `https://s3.{region}.amazonaws.com` |

Point `RWS_S3_ENDPOINT` at a custom host to use a non-AWS provider:

```bash
# Cloudflare R2
RWS_S3_ENDPOINT=https://<account-id>.r2.cloudflarestorage.com

# MinIO (local development)
RWS_S3_ENDPOINT=http://localhost:9000
```

Or construct `S3Config` directly instead of reading the environment:

```rust
use rust_web_server::storage::{S3Config, S3Storage};

let store = S3Storage::new(S3Config {
    bucket: "my-bucket".to_string(),
    region: "us-east-1".to_string(),
    access_key: "...".to_string(),
    secret_key: "...".to_string(),
    endpoint: "https://s3.us-east-1.amazonaws.com".to_string(),
});
```

### Addressing style

`S3Storage` always uses **path-style** addressing — `{endpoint}/{bucket}/{key}` — rather than virtual-hosted-style (`{bucket}.{host}/{key}`). Path-style works against every S3-compatible provider, including custom endpoints (R2, MinIO) where a wildcard DNS entry for virtual-hosted-style isn't set up.

:::note[Signing scope]
Every request signs exactly three headers: `host`, `x-amz-content-sha256`, and `x-amz-date`. This covers standard single-object `PUT`/`GET`/`DELETE` — there's no support for presigned URLs, multipart (chunked) uploads, or query-string signing.
:::

## Writing handler code against `Storage`

Depend on the trait, not a concrete type, so the same function works with either backend:

```rust
use rust_web_server::storage::{Storage, StorageError};

fn save_avatar(store: &dyn Storage, user_id: u64, bytes: &[u8]) -> Result<String, StorageError> {
    let key = format!("avatars/{user_id}.png");
    store.put(&key, bytes, "image/png")
}
```

Wire `store` in via [dependency injection](/building-apps/dependency-injection/) or [app state](/building-apps/state/) so handlers don't construct a new `LocalStorage`/`S3Storage` per request.
