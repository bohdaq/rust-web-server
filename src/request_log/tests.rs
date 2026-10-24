use super::*;

fn entry(method: &str, path: &str, status: i16) -> LogEntry {
    LogEntry {
        timestamp: 0,
        method: method.to_string(),
        path: path.to_string(),
        status,
        client_ip: "127.0.0.1".to_string(),
        latency_ms: 1,
    }
}

#[test]
fn push_and_recent() {
    let log = RequestLog::new(10);
    log.push(entry("GET", "/a", 200));
    log.push(entry("POST", "/b", 201));
    let recent = log.recent(10);
    assert_eq!(2, recent.len());
    assert_eq!("/a", recent[0].path);
    assert_eq!("/b", recent[1].path);
}

#[test]
fn capacity_evicts_oldest() {
    let log = RequestLog::new(3);
    log.push(entry("GET", "/a", 200));
    log.push(entry("GET", "/b", 200));
    log.push(entry("GET", "/c", 200));
    log.push(entry("GET", "/d", 200));
    let recent = log.recent(10);
    assert_eq!(3, recent.len());
    assert_eq!("/b", recent[0].path);
    assert_eq!("/d", recent[2].path);
}

#[test]
fn recent_n_returns_tail() {
    let log = RequestLog::new(10);
    for i in 0..5 {
        log.push(entry("GET", &format!("/{}", i), 200));
    }
    let recent = log.recent(2);
    assert_eq!(2, recent.len());
    assert_eq!("/3", recent[0].path);
    assert_eq!("/4", recent[1].path);
}

#[test]
fn recent_errors_filters() {
    let log = RequestLog::new(10);
    log.push(entry("GET", "/ok", 200));
    log.push(entry("GET", "/not_found", 404));
    log.push(entry("GET", "/err", 500));
    let errors = log.recent_errors(10);
    assert_eq!(2, errors.len());
    assert!(errors.iter().all(|e| e.status >= 400));
}

#[test]
fn entries_from_many_threads_merge_in_order_and_respect_capacity() {
    let log = std::sync::Arc::new(RequestLog::new(50));
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let log = std::sync::Arc::clone(&log);
            std::thread::spawn(move || {
                for i in 0..20 {
                    log.push(entry("GET", &format!("/{t}/{i}"), 200));
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(50, log.len());
    let recent = log.recent(100);
    assert_eq!(50, recent.len());
    // Each thread's own entries stay in order.
    for t in 0..8 {
        let mine: Vec<usize> = recent
            .iter()
            .filter_map(|e| e.path.strip_prefix(&format!("/{t}/")).map(|i| i.parse().unwrap()))
            .collect();
        assert!(mine.windows(2).all(|w| w[0] < w[1]), "thread {t}: {mine:?}");
    }
}
