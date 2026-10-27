use super::*;

#[test]
fn native_index_is_the_schema_3_bucket() {
    assert_eq!(0, native_index(1.0), "(2^-1/8, 1]");
    assert_eq!(8, native_index(2.0));
    assert_eq!(-8, native_index(0.5));
    assert_eq!(1, native_index(1.05));
    assert_eq!(1, native_index(2f64.powf(0.125)), "an upper bound belongs to its bucket");
    assert_eq!(-79, native_index(0.001));
    assert_eq!(MAX_INDEX, native_index(1e300));
    assert_eq!(MIN_INDEX, native_index(1e-300));
}

#[test]
fn series_follow_the_conventions() {
    let s = Series::new("GET", 200, true, "HTTP/2.0");
    assert_eq!(("GET", "https", "2"), (s.method(), s.scheme(), s.protocol_version()));
    let s = Series::new("PURGE", 404, false, "HTTP/1.1");
    assert_eq!(("_OTHER", "http", "1.1"), (s.method(), s.scheme(), s.protocol_version()));
    assert_eq!("3", Series::new("GET", 200, true, "HTTP/3").protocol_version());
    assert_eq!("1.0", Series::new("get", 200, false, "HTTP/1.0").protocol_version());
    assert_eq!("_OTHER", Series::new("get", 200, false, "HTTP/1.0").method(), "methods are case-sensitive");
}

#[test]
fn a_histogram_keeps_explicit_and_native_buckets() {
    let mut h = Histogram::default();
    for secs in [0.004, 0.006, 0.3, 30.0, 0.0] {
        h.observe(secs);
    }
    assert_eq!(5, h.count);
    assert!((h.sum - 30.31).abs() < 1e-9);
    assert_eq!(2, h.counts[0], "0 and 0.004 ≤ 0.005");
    assert_eq!(1, h.counts[1]);
    assert_eq!(1, h.counts[7], "0.3 in (0.25, 0.5]");
    assert_eq!(1, h.counts[14], "30 above 10");
    assert_eq!(1, h.zero_count);
    assert_eq!(4, h.native.iter().sum::<u64>());
    assert_eq!(native_index(0.004), h.native_offset, "grown downwards from the first");
}

#[test]
fn native_buckets_become_spans_and_deltas() {
    let mut h = Histogram::default();
    for secs in [1.0, 1.05, 1.05, 2.0] {
        h.observe(secs);
    }
    // Buckets 0 (1), 1 (2), 8 (1): a gap of 6 starts a second span.
    assert_eq!((vec![(0, 2), (6, 1)], vec![1, 1, -1]), h.native_spans());

    let mut h = Histogram::default();
    h.observe(1.0);
    h.observe(1.2); // bucket 3: a gap of two stays in the span as zeros
    assert_eq!((vec![(0, 4)], vec![1, -1, 0, 1]), h.native_spans());
}

#[test]
fn observations_from_many_threads_merge() {
    let series = Series::new("TRACE", 599, true, "HTTP/3");
    let threads: Vec<_> = (0..8).map(|_| std::thread::spawn(move || record(series, 0.02))).collect();
    threads.into_iter().for_each(|t| t.join().unwrap());
    let all = snapshot();
    let (_, h) = all.iter().find(|(s, _)| *s == series).unwrap();
    assert_eq!(8, h.count);
    assert_eq!(8, h.counts[2], "(0.01, 0.025]");

    let text = prometheus_text();
    let labels = r#"http_request_method="TRACE",http_response_status_code="599",network_protocol_version="3",url_scheme="https""#;
    assert!(text.contains(&format!("http_server_request_duration_seconds_bucket{{{labels},le=\"0.025\"}} 8")), "{text}");
    assert!(text.contains(&format!("http_server_request_duration_seconds_count{{{labels}}} 8")), "{text}");
    assert!(text.contains("# TYPE http_server_request_duration_seconds histogram"));
}
