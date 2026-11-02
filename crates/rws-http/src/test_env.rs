//! Test helpers, as in rust-web-server's `test_env`: the lock around
//! `RWS_CONFIG_*` writes (this crate's tests run in their own process, so
//! the lock is their own) and self-deleting temp paths.
#![allow(dead_code)]

use std::sync::{Mutex, OnceLock};
static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
pub fn lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// A per-process path under the OS temp dir that is deleted (file or
/// directory) when dropped — so a test cleans up even when an assertion
/// fails before its last line.
pub struct TempPath(std::path::PathBuf);

impl TempPath {
    pub fn new(name: &str) -> Self {
        TempPath(std::env::temp_dir().join(format!("rws_test_{}_{}", std::process::id(), name)))
    }

    /// The same in the working directory (the static-file root), for
    /// tests that need a file the server would serve.
    pub fn new_in_cwd(name: &str) -> Self {
        TempPath(std::env::current_dir().expect("working directory").join(format!("rws_test_{}_{}", std::process::id(), name)))
    }

    pub fn as_str(&self) -> &str {
        self.0.to_str().expect("temp dir path is UTF-8")
    }
}

impl std::ops::Deref for TempPath {
    type Target = std::path::Path;
    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        // A symlink is removed itself, never followed (a test may link
        // to a system directory).
        let is_link = std::fs::symlink_metadata(&self.0).is_ok_and(|m| m.file_type().is_symlink());
        if is_link {
            let _ = std::fs::remove_file(&self.0);
        } else if self.0.is_dir() {
            let _ = std::fs::remove_dir_all(&self.0);
        } else {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}
