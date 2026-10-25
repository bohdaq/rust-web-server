use super::*;

#[test]
fn block_and_check() {
    let bl = Blocklist::new();
    bl.block("1.2.3.4");
    assert!(bl.is_blocked("1.2.3.4"));
    assert!(!bl.is_blocked("5.6.7.8"));
}

#[test]
fn unblock() {
    let bl = Blocklist::new();
    bl.block("1.2.3.4");
    bl.unblock("1.2.3.4");
    assert!(!bl.is_blocked("1.2.3.4"));
}

#[test]
fn no_duplicate_block() {
    let bl = Blocklist::new();
    bl.block("1.2.3.4");
    bl.block("1.2.3.4");
    assert_eq!(1, bl.list().len());
}

#[test]
fn list_order() {
    let bl = Blocklist::new();
    bl.block("1.2.3.4");
    bl.block("5.6.7.8");
    let list = bl.list();
    assert_eq!(list[0], "1.2.3.4");
    assert_eq!(list[1], "5.6.7.8");
}

#[test]
fn clear() {
    let bl = Blocklist::new();
    bl.block("1.2.3.4");
    bl.clear();
    assert!(bl.list().is_empty());
}

// is_blocked skips the lock while nothing is blocked; the count must
// follow every change.
#[test]
fn lock_free_empty_check_tracks_block_unblock_and_clear() {
    let list = Blocklist::new();
    assert!(!list.is_blocked("10.0.0.1"));
    list.block("10.0.0.1");
    list.block("10.0.0.1");
    list.block("10.0.0.2");
    assert!(list.is_blocked("10.0.0.1") && list.is_blocked("10.0.0.2"));
    list.unblock("10.0.0.1");
    assert!(!list.is_blocked("10.0.0.1") && list.is_blocked("10.0.0.2"));
    list.clear();
    assert!(!list.is_blocked("10.0.0.2"));
    list.block("10.0.0.3");
    assert!(list.is_blocked("10.0.0.3"));
}
