use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::ValueCache;

const MIN: Duration = Duration::from_secs(60);

#[test]
fn fetch_or_computes_once_until_it_expires() {
    let cache: ValueCache<u32> = ValueCache::new(10);
    let calls = AtomicUsize::new(0);
    let compute = || {
        calls.fetch_add(1, Ordering::SeqCst);
        7
    };
    assert_eq!(7, cache.fetch_or("k", MIN, compute));
    assert_eq!(7, cache.fetch_or("k", MIN, || unreachable!()));
    assert_eq!(1, calls.load(Ordering::SeqCst));
    assert_eq!((1, 1), cache.stats());

    cache.put("short", 1, Duration::from_millis(1));
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(None, cache.get("short"));
    assert_eq!(2, cache.fetch_or("short", MIN, || 2));
}

#[test]
fn errors_are_not_cached() {
    let cache: ValueCache<String> = ValueCache::new(10);
    assert_eq!(Err("down"), cache.try_fetch_or("k", MIN, || Err("down")));
    assert!(cache.is_empty());
    assert_eq!(Ok::<_, &str>("up".to_string()), cache.try_fetch_or("k", MIN, || Ok("up".to_string())));
    assert_eq!(Ok::<_, &str>("up".to_string()), cache.try_fetch_or("k", MIN, || Err("not called")));
}

#[test]
fn the_oldest_goes_when_full_after_expired_ones() {
    let cache: ValueCache<u32> = ValueCache::new(3);
    cache.put("a", 1, MIN);
    cache.put("b", 2, Duration::from_millis(1));
    cache.put("c", 3, MIN);
    std::thread::sleep(Duration::from_millis(5));
    cache.put("d", 4, MIN); // b expired: it goes, a stays
    assert_eq!((Some(1), None), (cache.get("a"), cache.get("b")));
    cache.put("e", 5, MIN); // full of live ones: the oldest (a) goes
    assert_eq!((None, Some(3), Some(4), Some(5)), (cache.get("a"), cache.get("c"), cache.get("d"), cache.get("e")));
    // Replacing a key doesn't count twice.
    cache.put("e", 6, MIN);
    assert_eq!((3, Some(6)), (cache.len(), cache.get("e")));
    assert_eq!(1, ValueCache::<u8>::new(0).capacity, "at least one entry");
}

#[test]
fn invalidation() {
    let cache: ValueCache<u32> = ValueCache::new(10);
    let clone = cache.clone();
    cache.put("user:1:name", 1, MIN);
    cache.put("user:1:email", 2, MIN);
    cache.put("user:2:name", 3, MIN);
    assert!(clone.invalidate("user:2:name"));
    assert!(!clone.invalidate("user:2:name"));
    assert_eq!(2, cache.invalidate_prefix("user:1:"));
    assert!(cache.is_empty());
    cache.put("x", 1, MIN);
    cache.clear();
    assert_eq!(0, clone.len());
}

#[test]
fn async_fetch() {
    let cache: ValueCache<u32> = ValueCache::new(10);
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    runtime.block_on(async {
        assert_eq!(5, cache.fetch_or_async("k", MIN, || async { 5 }).await);
        assert_eq!(5, cache.fetch_or_async("k", MIN, || async { unreachable!() }).await);
        assert_eq!(Err::<u32, &str>("no"), cache.try_fetch_or_async("e", MIN, || async { Err("no") }).await);
        assert_eq!(Ok::<u32, &str>(9), cache.try_fetch_or_async("e", MIN, || async { Ok(9) }).await);
    });
}
