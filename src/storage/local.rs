use super::{Storage, StorageError};
use std::fs;
use std::path::PathBuf;

/// Stores objects as files under a root directory on local disk.
///
/// Keys are treated as relative paths under `root`; parent directories are
/// created automatically on `put`. A key containing a `..` segment is
/// rejected to prevent escaping `root` — the same defense used by the
/// config-driven proxy's static-file action.
///
/// Content type is not persisted — plain files on disk have no metadata
/// slot for it. Pair this with a static-file route (which detects MIME type
/// from the file extension) if you need to serve uploaded files back over
/// HTTP; see [`with_base_url`](Self::with_base_url).
pub struct LocalStorage {
    root: PathBuf,
    base_url: String,
}

impl LocalStorage {
    /// Create a store rooted at `root`. The directory itself does not need
    /// to exist yet — it (and any key subdirectories) are created on `put`.
    pub fn new(root: impl Into<String>) -> Self {
        LocalStorage { root: PathBuf::from(root.into()), base_url: String::new() }
    }

    /// Set the URL prefix returned by [`Storage::url`] — e.g. `/uploads` if
    /// `root` is also served as a static directory under that path. Without
    /// a base URL, `url()` falls back to the object's filesystem path.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    fn resolve(&self, key: &str) -> Result<PathBuf, StorageError> {
        if key.split('/').any(|segment| segment == "..") {
            return Err(StorageError::new(format!("key '{key}' must not contain '..' segments")));
        }
        Ok(self.root.join(key.trim_start_matches('/')))
    }
}

impl Storage for LocalStorage {
    fn put(&self, key: &str, data: &[u8], _content_type: &str) -> Result<String, StorageError> {
        let path = self.resolve(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| StorageError::new(format!("failed to create '{}': {e}", parent.display())))?;
        }
        fs::write(&path, data).map_err(|e| StorageError::new(format!("failed to write '{key}': {e}")))?;
        Ok(key.to_string())
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.resolve(key)?;
        fs::read(&path).map_err(|e| StorageError::new(format!("failed to read '{key}': {e}")))
    }

    fn delete(&self, key: &str) -> Result<(), StorageError> {
        let path = self.resolve(key)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StorageError::new(format!("failed to delete '{key}': {e}"))),
        }
    }

    fn url(&self, key: &str) -> String {
        if self.base_url.is_empty() {
            match self.resolve(key) {
                Ok(path) => path.to_string_lossy().to_string(),
                Err(_) => key.to_string(),
            }
        } else {
            format!("{}/{}", self.base_url.trim_end_matches('/'), key.trim_start_matches('/'))
        }
    }
}

#[cfg(test)]
mod tests;
