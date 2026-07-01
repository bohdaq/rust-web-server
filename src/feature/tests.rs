use super::*;

#[test]
fn unknown_flag_is_disabled() {
    let store = FeatureStore::new();
    assert!(!store.is_enabled("nonexistent"));
}

#[test]
fn set_and_read() {
    let store = FeatureStore::new();
    store.set("my_flag", true);
    assert!(store.is_enabled("my_flag"));
}

#[test]
fn disable_flag() {
    let store = FeatureStore::new();
    store.set("flag", true);
    store.set("flag", false);
    assert!(!store.is_enabled("flag"));
}

#[test]
fn list_sorted() {
    let store = FeatureStore::new();
    store.set("z_flag", true);
    store.set("a_flag", false);
    let list = store.list();
    assert_eq!(list[0].0, "a_flag");
    assert_eq!(list[1].0, "z_flag");
}
