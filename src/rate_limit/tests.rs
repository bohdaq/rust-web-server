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

// ── RedisRateLimiter ──────────────────────────────────────────────────────
//
// Spins up a tiny in-process fake Redis server (RESP v2) rather than
// requiring a real Redis instance in CI. It supports just enough of
// SET/INCR/GET/DEL to exercise RedisRateLimiter's logic.

use std::collections::HashMap as Map;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

fn start_fake_redis() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        let store: Arc<Mutex<Map<String, String>>> = Arc::new(Mutex::new(Map::new()));
        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };
            let store = Arc::clone(&store);
            std::thread::spawn(move || fake_redis_conn(&mut stream, &store));
        }
    });
    addr
}

fn fake_redis_conn(stream: &mut TcpStream, store: &Mutex<Map<String, String>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let line = line.trim_end();
        if !line.starts_with('*') {
            return;
        }
        let argc: usize = match line[1..].parse() {
            Ok(n) => n,
            Err(_) => return,
        };
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            let mut len_line = String::new();
            if reader.read_line(&mut len_line).unwrap_or(0) == 0 {
                return;
            }
            let len: usize = match len_line.trim_end()[1..].parse() {
                Ok(n) => n,
                Err(_) => return,
            };
            let mut buf = vec![0u8; len + 2];
            if reader.read_exact(&mut buf).is_err() {
                return;
            }
            buf.truncate(len);
            args.push(String::from_utf8_lossy(&buf).to_string());
        }
        let reply = fake_redis_execute(&args, store);
        if stream.write_all(reply.as_bytes()).is_err() {
            return;
        }
    }
}

fn fake_redis_execute(args: &[String], store: &Mutex<Map<String, String>>) -> String {
    let mut guard = store.lock().unwrap();
    match args[0].to_uppercase().as_str() {
        "SET" => {
            let key = &args[1];
            let value = &args[2];
            let nx = args.iter().any(|a| a.eq_ignore_ascii_case("NX"));
            if nx && guard.contains_key(key) {
                "$-1\r\n".to_string()
            } else {
                guard.insert(key.clone(), value.clone());
                "+OK\r\n".to_string()
            }
        }
        "INCR" => {
            let key = &args[1];
            let n: i64 = guard.get(key).and_then(|v| v.parse().ok()).unwrap_or(0) + 1;
            guard.insert(key.clone(), n.to_string());
            format!(":{}\r\n", n)
        }
        "GET" => match guard.get(&args[1]) {
            Some(v) => format!("${}\r\n{}\r\n", v.len(), v),
            None => "$-1\r\n".to_string(),
        },
        "DEL" => {
            let existed = guard.remove(&args[1]).is_some();
            format!(":{}\r\n", if existed { 1 } else { 0 })
        }
        _ => "-ERR unknown command\r\n".to_string(),
    }
}

#[test]
fn redis_first_request_is_allowed() {
    let addr = start_fake_redis();
    let limiter = RedisRateLimiter::new(addr, None, 5, 60);
    assert!(limiter.check("10.0.0.1").unwrap());
}

#[test]
fn redis_requests_within_limit_are_allowed() {
    let addr = start_fake_redis();
    let limiter = RedisRateLimiter::new(addr, None, 3, 60);
    assert!(limiter.check("10.0.0.1").unwrap());
    assert!(limiter.check("10.0.0.1").unwrap());
    assert!(limiter.check("10.0.0.1").unwrap());
}

#[test]
fn redis_request_exceeding_limit_is_denied() {
    let addr = start_fake_redis();
    let limiter = RedisRateLimiter::new(addr, None, 2, 60);
    assert!(limiter.check("10.0.0.1").unwrap());
    assert!(limiter.check("10.0.0.1").unwrap());
    assert!(!limiter.check("10.0.0.1").unwrap());
}

#[test]
fn redis_different_keys_are_tracked_independently() {
    let addr = start_fake_redis();
    let limiter = RedisRateLimiter::new(addr, None, 1, 60);
    assert!(limiter.check("10.0.0.1").unwrap());
    assert!(limiter.check("10.0.0.2").unwrap()); // different IP, fresh bucket
    assert!(!limiter.check("10.0.0.1").unwrap()); // first IP now exhausted
}

#[test]
fn redis_remaining_decrements_on_each_check() {
    let addr = start_fake_redis();
    let limiter = RedisRateLimiter::new(addr, None, 5, 60);
    assert_eq!(5, limiter.remaining("10.0.0.1").unwrap());
    limiter.check("10.0.0.1").unwrap();
    assert_eq!(4, limiter.remaining("10.0.0.1").unwrap());
    limiter.check("10.0.0.1").unwrap();
    assert_eq!(3, limiter.remaining("10.0.0.1").unwrap());
}

#[test]
fn redis_reset_clears_state_for_key() {
    let addr = start_fake_redis();
    let limiter = RedisRateLimiter::new(addr, None, 1, 60);
    limiter.check("10.0.0.1").unwrap();
    assert!(!limiter.check("10.0.0.1").unwrap()); // exhausted
    limiter.reset("10.0.0.1").unwrap();
    assert!(limiter.check("10.0.0.1").unwrap()); // allowed after reset
}

#[test]
fn redis_zero_max_requests_always_denies() {
    let addr = start_fake_redis();
    let limiter = RedisRateLimiter::new(addr, None, 0, 60);
    assert!(!limiter.check("10.0.0.1").unwrap());
}

#[test]
fn redis_set_limits_takes_effect_immediately() {
    let addr = start_fake_redis();
    let limiter = RedisRateLimiter::new(addr, None, 1, 60);
    assert!(limiter.check("10.0.0.1").unwrap());
    assert!(!limiter.check("10.0.0.1").unwrap()); // exhausted at limit 1
    limiter.set_limits(10, 60);
    limiter.reset("10.0.0.1").unwrap();
    assert!(limiter.check("10.0.0.1").unwrap()); // new limit applies
}

#[test]
fn redis_check_errors_when_server_unreachable() {
    // Port 1 is a privileged, essentially never-listening port on CI/dev
    // machines — connection should fail immediately (refused), giving
    // callers an `Err` to make a fail-open/fail-closed decision on.
    let limiter = RedisRateLimiter::new("127.0.0.1:1", None, 5, 60);
    assert!(limiter.check("10.0.0.1").is_err());
}
