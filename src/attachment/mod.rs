//! Files on models (18.35.0, #94): an [`Attachment`] is a model field
//! whose bytes live in a [`Storage`] backend (local disk, S3, Azure, GCS)
//! and whose description — key, file name, content type, size — lives in
//! one `TEXT` column.
//!
//! ```rust
//! use rust_web_server::attachment::Attachment;
//! use rust_web_server::storage::{LocalStorage, Storage};
//!
//! # let dir = std::env::temp_dir().join(format!("rws-attachment-doc-{}", std::process::id()));
//! let storage = LocalStorage::new(dir.to_str().unwrap()).with_base_url("/uploads");
//! let avatar = Attachment::store(&storage, "avatars", "me.png", "image/png", b"\x89PNG...").unwrap();
//! assert!(avatar.key.starts_with("avatars/"));
//! assert_eq!(b"\x89PNG...".to_vec(), avatar.read(&storage).unwrap());
//!
//! // In a model: `pub avatar: Option<Attachment>` — stored as text.
//! let column = avatar.encode();
//! assert_eq!(Some(avatar.clone()), Attachment::decode(&column));
//! avatar.delete(&storage).unwrap();
//! # let _ = std::fs::remove_dir_all(&dir);
//! ```
//!
//! Keys are `<prefix>/<id>/<file name>` (`id`: 32 hex digits, time first) (the name with anything but
//! letters, digits, `.`, `-` and `_` replaced by `_`), so uploads never
//! overwrite each other. Deleting a row doesn't delete its file: call
//! [`Attachment::delete`] where you delete the row.
//!
//! The column holds `key=…&name=…&type=…&size=…`, percent-encoded. In a
//! `#[derive(Model)]` struct the field is `Attachment` or
//! `Option<Attachment>` (`model-*` features), with a `TEXT` column.

#[cfg(test)]
mod tests;

use crate::request::Request;
use crate::storage::{Storage, StorageError};

/// A stored file's description. See the [module docs](self).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// Where the bytes are in the storage backend.
    pub key: String,
    /// The original file name, as uploaded.
    pub filename: String,
    pub content_type: String,
    /// Bytes.
    pub size: u64,
}

impl Attachment {
    fn new_key(prefix: &str, filename: &str) -> String {
        let safe: String = filename
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
            .collect();
        let safe = safe.trim_start_matches('.');
        let safe = if safe.is_empty() { "file" } else { safe };
        let id = unique_id();
        let prefix = prefix.trim_matches('/');
        if prefix.is_empty() {
            format!("{id}/{safe}")
        } else {
            format!("{prefix}/{id}/{safe}")
        }
    }

    /// Store `data` under a new key below `prefix`.
    pub fn store(storage: &dyn Storage, prefix: &str, filename: &str, content_type: &str, data: &[u8]) -> Result<Attachment, StorageError> {
        let key = storage.put(&Self::new_key(prefix, filename), data, content_type)?;
        Ok(Attachment { key, filename: filename.to_string(), content_type: content_type.to_string(), size: data.len() as u64 })
    }

    /// Store everything `reader` yields (a streamed upload) under a new key
    /// below `prefix`; the size is counted as it's read.
    pub fn store_stream(storage: &dyn Storage, prefix: &str, filename: &str, content_type: &str, reader: &mut dyn std::io::Read) -> Result<Attachment, StorageError> {
        let mut counting = Counting { inner: reader, count: 0 };
        let key = storage.put_stream(&Self::new_key(prefix, filename), &mut counting, content_type)?;
        Ok(Attachment { key, filename: filename.to_string(), content_type: content_type.to_string(), size: counting.count })
    }

    /// Store the file uploaded in the `multipart/form-data` field `field`
    /// of `request`. `Ok(None)` when the request has no such field or it
    /// carries no file (no file name: an empty file input); `Err` for a body that isn't
    /// `multipart/form-data`.
    pub fn from_multipart(storage: &dyn Storage, prefix: &str, request: &Request, field: &str) -> Result<Option<Attachment>, StorageError> {
        use crate::body::multipart_form_data::FormMultipartData;
        use crate::header::content_disposition::ContentDisposition;

        let content_type = request.header("content-type").map(|h| h.value.to_string()).unwrap_or_default();
        if !content_type.to_ascii_lowercase().starts_with("multipart/form-data") {
            return Err(StorageError::new("the request body isn't multipart/form-data"));
        }
        let boundary = FormMultipartData::extract_boundary(&content_type).map_err(StorageError::new)?;
        let parts = FormMultipartData::parse(&request.body, boundary).map_err(StorageError::new)?;
        for part in parts {
            let Some(disposition) = part.header("content-disposition") else { continue };
            let Ok(disposition) = ContentDisposition::parse(&disposition.value) else {
                continue;
            };
            if disposition.field_name.as_deref() != Some(field) {
                continue;
            }
            // An empty file input sends `filename=""`.
            let filename = disposition.file_name.unwrap_or_default();
            if filename.is_empty() {
                return Ok(None);
            }
            let part_type = part.header("content-type").map(|h| h.value.to_string()).unwrap_or_else(|| "application/octet-stream".to_string());
            return Self::store(storage, prefix, &filename, &part_type, &part.body).map(Some);
        }
        Ok(None)
    }

    /// The file's URL ([`Storage::url`]).
    pub fn url(&self, storage: &dyn Storage) -> String {
        storage.url(&self.key)
    }

    /// The file's bytes.
    pub fn read(&self, storage: &dyn Storage) -> Result<Vec<u8>, StorageError> {
        storage.get(&self.key)
    }

    /// Delete the file.
    pub fn delete(&self, storage: &dyn Storage) -> Result<(), StorageError> {
        storage.delete(&self.key)
    }

    /// The column text.
    pub fn encode(&self) -> String {
        use crate::router::build_path;
        // `build_path` on an empty pattern is `/?<query>`: the query alone.
        let size = self.size.to_string();
        let query = build_path("/", &[("key", &self.key), ("name", &self.filename), ("type", &self.content_type), ("size", &size)]).unwrap_or_default();
        query.trim_start_matches("/?").to_string()
    }

    /// Parse column text written by [`encode`](Self::encode); `None` when
    /// it isn't one (no key, a bad size or escape).
    pub fn decode(text: &str) -> Option<Attachment> {
        let mut attachment = Attachment { key: String::new(), filename: String::new(), content_type: String::new(), size: 0 };
        for pair in text.split('&').filter(|p| !p.is_empty()) {
            let (name, value) = pair.split_once('=')?;
            let value = percent_decode(value)?;
            match name {
                "key" => attachment.key = value,
                "name" => attachment.filename = value,
                "type" => attachment.content_type = value,
                "size" => attachment.size = value.parse().ok()?,
                _ => {}
            }
        }
        (!attachment.key.is_empty()).then_some(attachment)
    }
}

/// 32 hex digits: the time in nanoseconds, then 64 random bits (a
/// process-seeded hash of a counter), so keys sort by upload time.
fn unique_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;
    static SEED: OnceLock<std::collections::hash_map::RandomState> = OnceLock::new();
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    let mut hasher = SEED.get_or_init(Default::default).build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write_u64(nanos);
    format!("{nanos:016x}{:016x}", hasher.finish())
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            out.push(u8::from_str_radix(text.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

struct Counting<'a> {
    inner: &'a mut dyn std::io::Read,
    count: u64,
}

impl std::io::Read for Counting<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count += n as u64;
        Ok(n)
    }
}

#[cfg(any(feature = "model-sqlite", feature = "model-postgres", feature = "model-mysql"))]
mod column {
    use super::Attachment;
    use crate::model::{DbError, FromColumn, ToColumn, Value};

    impl FromColumn for Attachment {
        fn from_column(v: Value) -> Result<Self, DbError> {
            match v {
                Value::Text(text) => Attachment::decode(&text).ok_or_else(|| DbError::new(format!("not an attachment: {text:?}"))),
                Value::Null => Err(DbError::new("unexpected NULL for Attachment")),
                other => Err(DbError::new(format!("cannot convert {other:?} to Attachment"))),
            }
        }
    }

    impl ToColumn for Attachment {
        fn to_column(&self) -> Value {
            Value::Text(self.encode())
        }
    }
}
