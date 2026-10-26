use super::*;
use crate::test_env::TempPath;
use std::path::PathBuf;

fn touch(path: &str, text: &str, secs: u64) {
    std::fs::write(path, text).unwrap();
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(secs))
        .unwrap();
}

#[test]
fn a_change_reloads_once_it_has_settled() {
    let config = TempPath::new("watch_config.toml");
    touch(config.as_str(), "port = 1", 1_767_222_000);
    let path = PathBuf::from(config.as_str());
    let mut watcher = Watcher::new(move || vec![path.clone()]);
    assert!(!watcher.poll(), "nothing changed");

    touch(config.as_str(), "port = 2", 1_767_222_001);
    assert!(!watcher.poll(), "first sight of a change waits a poll");
    assert!(watcher.poll(), "unchanged since: reload");
    assert!(!watcher.poll(), "and only once");

    // Still being written: each poll sees something new, so no reload yet.
    touch(config.as_str(), "port = 3", 1_767_222_002);
    assert!(!watcher.poll());
    touch(config.as_str(), "port = 33", 1_767_222_003);
    assert!(!watcher.poll());
    assert!(watcher.poll());
}

#[test]
fn a_file_that_appears_or_disappears_is_a_change() {
    let cert = TempPath::new("watch_cert.pem");
    let path = PathBuf::from(cert.as_str());
    let mut watcher = Watcher::new(move || vec![path.clone()]);
    touch(cert.as_str(), "cert", 1_767_222_000);
    assert!(!watcher.poll());
    assert!(watcher.poll(), "appeared");
    std::fs::remove_file(cert.as_str()).unwrap();
    assert!(!watcher.poll());
    assert!(watcher.poll(), "disappeared");
}

#[test]
fn watched_paths_cover_the_config_and_every_certificate() {
    let _g = crate::test_env::lock();
    let keys = [
        "RWS_CONFIG_FILE",
        crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE,
        crate::entry_point::Config::RWS_CONFIG_TLS_KEY_FILE,
        "RWS_CONFIG_VIRTUAL_HOST_0_DOMAIN",
        "RWS_CONFIG_VIRTUAL_HOST_0_CERT_FILE",
        "RWS_CONFIG_VIRTUAL_HOST_0_KEY_FILE",
        "RWS_CONFIG_VIRTUAL_HOST_1_DOMAIN",
        RWS_CONFIG_WATCH,
    ];
    let saved: Vec<_> = keys.iter().map(|k| (k, std::env::var(k).ok())).collect();
    for k in keys {
        std::env::remove_var(k);
    }
    assert_eq!(vec![PathBuf::from("rws.config.toml")], watched_paths());
    assert!(!enabled());

    std::env::set_var("RWS_CONFIG_FILE", "/etc/rws/rws.config.toml");
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_TLS_CERT_FILE, "/etc/rws/cert.pem");
    std::env::set_var(crate::entry_point::Config::RWS_CONFIG_TLS_KEY_FILE, "/etc/rws/key.pem");
    std::env::set_var("RWS_CONFIG_VIRTUAL_HOST_0_DOMAIN", "a.example");
    std::env::set_var("RWS_CONFIG_VIRTUAL_HOST_0_CERT_FILE", "/etc/rws/a.pem");
    std::env::set_var("RWS_CONFIG_VIRTUAL_HOST_0_KEY_FILE", "/etc/rws/a.key");
    std::env::set_var(RWS_CONFIG_WATCH, "true");
    let paths: Vec<String> = watched_paths().iter().map(|p| p.display().to_string()).collect();
    assert_eq!(vec!["/etc/rws/a.key", "/etc/rws/a.pem", "/etc/rws/cert.pem", "/etc/rws/key.pem", "/etc/rws/rws.config.toml"], paths);
    assert!(enabled());

    for (k, v) in saved {
        match v {
            Some(v) => std::env::set_var(k, v),
            None => std::env::remove_var(k),
        }
    }
}
