use super::*;

#[test]
fn first_request_is_allowed() {
    let limiter = RateLimiter::new(5, 60);
    assert!(limiter.check("10.0.0.1"));
}

#[test]
fn requests_within_limit_are_allowed() {
    let limiter = RateLimiter::new(3, 60);
    assert!(limiter.check("10.0.0.1"));
    assert!(limiter.check("10.0.0.1"));
    assert!(limiter.check("10.0.0.1"));
}

#[test]
fn request_exceeding_limit_is_denied() {
    let limiter = RateLimiter::new(2, 60);
    assert!(limiter.check("10.0.0.1"));
    assert!(limiter.check("10.0.0.1"));
    assert!(!limiter.check("10.0.0.1"));
}

#[test]
fn different_keys_are_tracked_independently() {
    let limiter = RateLimiter::new(1, 60);
    assert!(limiter.check("10.0.0.1"));
    assert!(limiter.check("10.0.0.2")); // different IP, fresh bucket
    assert!(!limiter.check("10.0.0.1")); // first IP now exhausted
}

#[test]
fn remaining_decrements_on_each_check() {
    let limiter = RateLimiter::new(5, 60);
    assert_eq!(5, limiter.remaining("10.0.0.1"));
    limiter.check("10.0.0.1");
    assert_eq!(4, limiter.remaining("10.0.0.1"));
    limiter.check("10.0.0.1");
    assert_eq!(3, limiter.remaining("10.0.0.1"));
}

#[test]
fn remaining_never_underflows() {
    let limiter = RateLimiter::new(1, 60);
    limiter.check("10.0.0.1");
    limiter.check("10.0.0.1");
    assert_eq!(0, limiter.remaining("10.0.0.1"));
}

#[test]
fn reset_clears_state_for_key() {
    let limiter = RateLimiter::new(1, 60);
    limiter.check("10.0.0.1");
    assert!(!limiter.check("10.0.0.1")); // exhausted
    limiter.reset("10.0.0.1");
    assert!(limiter.check("10.0.0.1")); // allowed after reset
}

#[test]
fn expired_requests_do_not_count() {
    // Use a 0-second window so requests expire immediately.
    let limiter = RateLimiter::new(1, 0);
    limiter.check("10.0.0.1");
    // Spin briefly so the Instant advances past the 0-second window.
    let deadline = std::time::Instant::now() + Duration::from_millis(5);
    while std::time::Instant::now() < deadline {}
    assert!(limiter.check("10.0.0.1")); // old entry expired
}

#[test]
fn zero_max_requests_always_denies() {
    let limiter = RateLimiter::new(0, 60);
    assert!(!limiter.check("10.0.0.1"));
}
