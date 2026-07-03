use super::LocalStorage;
use crate::storage::Storage;

fn temp_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("rws_local_storage_{name}_{}", std::process::id()))
}

#[test]
fn put_then_get_roundtrips() {
    let dir = temp_dir("roundtrip");
    let store = LocalStorage::new(dir.to_str().unwrap());

    let key = store.put("file.txt", b"hello world", "text/plain").unwrap();
    assert_eq!("file.txt", key);
    assert_eq!(b"hello world".to_vec(), store.get("file.txt").unwrap());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn put_creates_nested_directories() {
    let dir = temp_dir("nested");
    let store = LocalStorage::new(dir.to_str().unwrap());

    store.put("avatars/2026/42.png", b"...png...", "image/png").unwrap();
    assert_eq!(b"...png...".to_vec(), store.get("avatars/2026/42.png").unwrap());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn delete_removes_file_and_is_idempotent() {
    let dir = temp_dir("delete");
    let store = LocalStorage::new(dir.to_str().unwrap());

    store.put("file.txt", b"data", "text/plain").unwrap();
    store.delete("file.txt").unwrap();
    assert!(store.get("file.txt").is_err());

    // Deleting an already-missing key is a no-op success (matches S3 semantics).
    store.delete("file.txt").unwrap();

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn get_missing_key_is_an_error() {
    let dir = temp_dir("missing");
    let store = LocalStorage::new(dir.to_str().unwrap());
    assert!(store.get("nope.txt").is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rejects_path_traversal() {
    let dir = temp_dir("traversal");
    let store = LocalStorage::new(dir.to_str().unwrap());

    assert!(store.put("../escape.txt", b"data", "text/plain").is_err());
    assert!(store.get("../escape.txt").is_err());
    assert!(store.delete("../escape.txt").is_err());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn url_without_base_url_falls_back_to_filesystem_path() {
    let dir = temp_dir("url_fallback");
    let store = LocalStorage::new(dir.to_str().unwrap());
    let url = store.url("file.txt");
    assert!(url.ends_with("file.txt"));
    assert!(url.contains(dir.to_str().unwrap()));
}

#[test]
fn url_with_base_url_uses_prefix() {
    let dir = temp_dir("url_prefix");
    let store = LocalStorage::new(dir.to_str().unwrap()).with_base_url("/uploads");
    assert_eq!("/uploads/avatars/42.png", store.url("avatars/42.png"));
}
