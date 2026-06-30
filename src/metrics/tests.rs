use std::sync::atomic::Ordering;

use crate::metrics::{
    ACTIVE_CONNECTIONS, ERRORS_TOTAL, REQUESTS_TOTAL,
    connection_close, connection_open, prometheus_text, record_error, record_request,
};

// ── counter behaviour ─────────────────────────────────────────────────────────
//
// Static atomics persist across tests that run in the same process.  We cannot
// assert absolute values, but we CAN assert relative increments: after our call
// the counter must be at least N higher than it was before our call.  Even if
// another test calls the same function concurrently, the constraint `after >=
// before + 1` still holds because atomics serialise the fetch_add.

#[test]
fn record_request_increments_requests_total() {
    let before = REQUESTS_TOTAL.load(Ordering::SeqCst);
    record_request();
    let after = REQUESTS_TOTAL.load(Ordering::SeqCst);
    assert!(after >= before + 1, "expected requests_total to increase by at least 1");
}

#[test]
fn record_request_called_multiple_times_accumulates() {
    let before = REQUESTS_TOTAL.load(Ordering::SeqCst);
    record_request();
    record_request();
    record_request();
    let after = REQUESTS_TOTAL.load(Ordering::SeqCst);
    assert!(after >= before + 3);
}

#[test]
fn record_error_increments_errors_total() {
    let before = ERRORS_TOTAL.load(Ordering::SeqCst);
    record_error();
    let after = ERRORS_TOTAL.load(Ordering::SeqCst);
    assert!(after >= before + 1);
}

#[test]
fn connection_open_increments_active_connections() {
    let before = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    connection_open();
    let after = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    assert!(after >= before + 1);
    connection_close(); // restore balance
}

#[test]
fn connection_close_decrements_active_connections() {
    connection_open(); // ensure there is something to close
    let before = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    connection_close();
    let after = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    assert!(after <= before - 1);
}

#[test]
fn open_and_close_net_zero() {
    let before = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    connection_open();
    connection_open();
    connection_close();
    connection_close();
    let after = ACTIVE_CONNECTIONS.load(Ordering::SeqCst);
    // Net delta from our two pairs must be 0; other tests may shift the
    // absolute value, so we only check our own delta.
    assert_eq!(after - before, 0);
}

// ── prometheus_text format ────────────────────────────────────────────────────

#[test]
fn prometheus_text_contains_required_metric_names() {
    let text = prometheus_text();
    assert!(text.contains("rws_requests_total"),    "missing rws_requests_total");
    assert!(text.contains("rws_errors_total"),       "missing rws_errors_total");
    assert!(text.contains("rws_active_connections"), "missing rws_active_connections");
}

#[test]
fn prometheus_text_contains_help_and_type_lines() {
    let text = prometheus_text();
    let help_count  = text.lines().filter(|l| l.starts_with("# HELP")).count();
    let type_count  = text.lines().filter(|l| l.starts_with("# TYPE")).count();
    assert_eq!(3, help_count, "expected 3 # HELP lines");
    assert_eq!(3, type_count, "expected 3 # TYPE lines");
}

#[test]
fn prometheus_text_type_annotations_are_correct() {
    let text = prometheus_text();
    assert!(text.contains("# TYPE rws_requests_total counter"));
    assert!(text.contains("# TYPE rws_errors_total counter"));
    assert!(text.contains("# TYPE rws_active_connections gauge"));
}

#[test]
fn prometheus_text_metric_values_are_parseable_numbers() {
    record_request(); // ensure at least one value > 0
    let text = prometheus_text();
    for line in text.lines() {
        if line.starts_with("rws_") {
            let mut parts = line.splitn(2, ' ');
            let _name = parts.next().expect("metric name missing");
            let value = parts.next().expect("metric value missing");
            value.parse::<i64>().unwrap_or_else(|_| {
                panic!("metric value is not a number: '{}'", line)
            });
        }
    }
}

#[test]
fn prometheus_text_requests_reflect_recorded_requests() {
    let before_text = prometheus_text();
    let before_val: u64 = before_text
        .lines()
        .find(|l| l.starts_with("rws_requests_total "))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .expect("rws_requests_total line not found");

    record_request();
    record_request();

    let after_text = prometheus_text();
    let after_val: u64 = after_text
        .lines()
        .find(|l| l.starts_with("rws_requests_total "))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .expect("rws_requests_total line not found");

    assert!(after_val >= before_val + 2, "prometheus_text did not reflect two additional records");
}
