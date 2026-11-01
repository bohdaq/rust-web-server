use super::*;
use std::sync::Arc;

/// A replica's local state, recorded instead of applied to the process.
#[derive(Default)]
struct Recorder {
    seen: Mutex<Vec<(State, u64)>>,
}

impl Local for Arc<Recorder> {
    fn apply(&self, state: &State, purges_after: u64) {
        self.seen.lock().unwrap().push((state.clone(), purges_after));
    }
    fn snapshot(&self) -> State {
        self.last()
    }
}

impl Recorder {
    fn last(&self) -> State {
        self.seen.lock().unwrap().last().map(|(s, _)| s.clone()).unwrap_or_default()
    }
}

fn replica(store: &Arc<MemoryStore>, node: &str) -> (ClusterSync, Arc<Recorder>) {
    let rec = Arc::new(Recorder::default());
    (ClusterSync::new(Box::new(Arc::clone(store)), Box::new(Arc::clone(&rec)), node), rec)
}

#[test]
fn the_document_round_trips_through_json() {
    let mut s = State::default();
    s.apply(&Op::Maintenance(true));
    s.apply(&Op::Block("203.0.113.9".into()));
    s.apply(&Op::Block("2001:db8::1".into()));
    s.apply(&Op::Unblock("203.0.113.9".into()));
    s.apply(&Op::Drain { upstream: "web".into(), backend: "http://10.0.0.5:8080".into(), drained: true });
    s.apply(&Op::Flag { name: "beta \"x\"".into(), enabled: false });
    s.apply(&Op::Purge { cache: "app".into(), path: Some("/a".into()), prefix: None });
    assert_eq!(s.version, 7);
    let back = State::from_json(&s.to_json()).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.blocked.iter().collect::<Vec<_>>(), ["2001:db8::1"]);
    assert!(back.drained.contains("web|http://10.0.0.5:8080"));
    // cjson writes empty tables as {} and may write 3 as 3.0: both read.
    assert_eq!(State::from_json(r#"{"v":3.0,"blocked":{},"drained":{},"flags":{},"purges":{}}"#).unwrap().version, 3);
    assert!(State::from_json("not json").is_none());
}

#[test]
fn purges_keep_only_the_newest() {
    let mut s = State::default();
    for i in 0..(PURGES_KEPT + 10) {
        s.apply(&Op::Purge { cache: format!("c{i}"), path: None, prefix: None });
    }
    assert_eq!(s.purges.len() as u64, PURGES_KEPT);
    assert_eq!(*s.purges.keys().next_back().unwrap(), PURGES_KEPT + 10);
}

#[test]
fn a_change_on_one_replica_reaches_the_other() {
    let store = Arc::new(MemoryStore::default());
    let (a, _rec_a) = replica(&store, "web-1");
    let (b, rec_b) = replica(&store, "web-2");
    a.tick(true);
    b.tick(true);
    a.publish(Op::Block("198.51.100.4".into()));
    a.publish(Op::Maintenance(true));
    b.tick(false);
    let state = rec_b.last();
    assert!(state.maintenance && state.blocked.contains("198.51.100.4"), "{state:?}");
    // Nothing new: no second apply.
    let applied = rec_b.seen.lock().unwrap().len();
    b.tick(false);
    assert_eq!(rec_b.seen.lock().unwrap().len(), applied);
    // Both nodes are listed.
    a.tick(true);
    assert!(a.status_json().contains(r#""nodes":["web-1","web-2"]"#), "{}", a.status_json());
    assert!(a.status_json().contains(r#""redis":"up""#));
}

#[test]
fn changes_made_while_the_store_is_down_are_sent_later() {
    let store = Arc::new(MemoryStore::default());
    let (a, _) = replica(&store, "web-1");
    let (b, rec_b) = replica(&store, "web-2");
    a.publish(Op::Maintenance(false)); // the document exists before Redis goes away
    store.down.store(true, std::sync::atomic::Ordering::SeqCst);
    a.publish(Op::Flag { name: "checkout-v2".into(), enabled: true });
    a.publish(Op::Block("192.0.2.77".into()));
    a.tick(false);
    assert!(a.status_json().contains(r#""redis":"down""#) && a.status_json().contains(r#""pending":2"#), "{}", a.status_json());
    store.down.store(false, std::sync::atomic::Ordering::SeqCst);
    a.tick(false);
    assert!(a.status_json().contains(r#""pending":0"#));
    b.tick(false);
    let state = rec_b.last();
    assert_eq!(state.flags.get("checkout-v2"), Some(&true));
    assert!(state.blocked.contains("192.0.2.77"));
}

#[test]
fn a_new_replica_adopts_the_state_but_not_old_purges() {
    let store = Arc::new(MemoryStore::default());
    let (a, _) = replica(&store, "web-1");
    a.publish(Op::Purge { cache: "app".into(), path: None, prefix: None });
    a.publish(Op::Maintenance(true));
    let (late, rec) = replica(&store, "web-3");
    late.tick(false);
    let (state, purges_after) = rec.seen.lock().unwrap().last().cloned().unwrap();
    assert!(state.maintenance);
    assert_eq!(purges_after, 1, "the existing purge is not replayed");
    a.publish(Op::Purge { cache: "app".into(), path: Some("/x".into()), prefix: None });
    late.tick(false);
    let (state, purges_after) = rec.seen.lock().unwrap().last().cloned().unwrap();
    assert_eq!((purges_after, state.purges.len()), (1, 2), "the new purge (id 2) is after 1");
}

#[test]
fn the_process_state_follows_the_document() {
    let _g = crate::test_env::lock();
    let ip = "192.0.2.201";
    let mut s = State::default();
    s.apply(&Op::Block(ip.into()));
    s.apply(&Op::Flag { name: "rws-cluster-test-flag".into(), enabled: true });
    ProcessState.apply(&s, 0);
    assert!(crate::blocklist::global().is_blocked(ip));
    assert!(crate::feature::global().is_enabled("rws-cluster-test-flag"));
    s.apply(&Op::Unblock(ip.into()));
    ProcessState.apply(&s, 0);
    assert!(!crate::blocklist::global().is_blocked(ip));
    crate::maintenance::MAINTENANCE_MODE.store(false, std::sync::atomic::Ordering::SeqCst);
}

#[test]
fn reads_what_redis_cjson_writes() {
    // Captured from Redis 7's cjson.encode (key order is Lua's table order).
    let s = State::from_json(r#"{"flags":{},"blocked":{"10.9.9.9":true},"drained":{},"purges":{},"v":1}"#).expect("parses");
    assert_eq!(s.version, 1);
    assert!(s.blocked.contains("10.9.9.9"));
}

#[test]
fn an_empty_store_is_seeded_from_a_replica_s_state() {
    let first = Arc::new(MemoryStore::default());
    let (a, rec_a) = replica(&first, "web-1");
    a.publish(Op::Block("192.0.2.9".into()));
    a.publish(Op::Drain { upstream: "web".into(), backend: "10.0.0.5:80".into(), drained: true });
    a.tick(false);
    assert!(rec_a.last().blocked.contains("192.0.2.9"));
    // Redis restarted empty: web-1 is pointed at a fresh store.
    let fresh = Arc::new(MemoryStore::default());
    let (a2, rec_a2) = (ClusterSync::new(Box::new(Arc::clone(&fresh)), Box::new(Arc::clone(&rec_a)), "web-1"), &rec_a);
    a2.tick(false);
    let seeded = fresh.state().unwrap().expect("seeded");
    assert!(seeded.blocked.contains("192.0.2.9") && seeded.drained.contains("web|10.0.0.5:80"), "{seeded:?}");
    assert!(rec_a2.last().blocked.contains("192.0.2.9"), "nothing was undone");
}
