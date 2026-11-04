use super::*;
use crate::core::New;
use crate::server_config::log_privacy::AnonymizeIp;
use std::io::{Read, Write};

fn request(uri: &str) -> Request {
    Request {
        method: "GET".to_string(),
        request_uri: uri.to_string(),
        http_version: "HTTP/1.1".to_string(),
        headers: vec![],
        body: Default::default(),
    }
}

fn response(status: i16) -> Response {
    let mut response = Response::new();
    response.status_code = status;
    response
}

const IDS: TraceIds = TraceIds { trace_id: [0x4b; 16], span_id: [0x0a; 8], sampled: true };

#[test]
fn the_trace_header_goes_from_otel_layer_to_the_log_and_not_further() {
    let mut r = response(200);
    // Export off: nothing put, nothing taken.
    put_trace(&mut r, IDS);
    assert!(r.headers.iter().all(|h| h.name != TRACE_HEADER));
    r.headers.push(crate::header::Header::new(TRACE_HEADER, "x".to_string()));
    assert_eq!(None, take_trace(&mut r));
    assert_eq!(1, r.headers.iter().filter(|h| h.name == TRACE_HEADER).count(), "left alone while off");

    enable_on_this_thread(true);
    let mut r = response(200);
    put_trace(&mut r, IDS);
    assert_eq!(Some(IDS), take_trace(&mut r));
    assert!(r.headers.iter().all(|h| !h.name.eq_ignore_ascii_case(TRACE_HEADER)), "removed before the response goes out");
    assert_eq!(None, take_trace(&mut r));
    let unsampled = TraceIds { sampled: false, ..IDS };
    put_trace(&mut r, unsampled);
    assert_eq!(Some(unsampled), take_trace(&mut r));
    enable_on_this_thread(false);
}

#[test]
fn malformed_trace_headers_are_ignored() {
    for bad in ["", "4b4b-0a0a-01", &format!("{}-{}", "4b".repeat(16), "0a".repeat(8)), &format!("{}-{}-01", "zz".repeat(16), "0a".repeat(8))] {
        assert_eq!(None, parse_trace(bad), "{bad}");
    }
    assert_eq!(Some(IDS), parse_trace(&format!("{}-{}-01", "4b".repeat(16), "0a".repeat(8))));
}

#[test]
fn a_record_is_an_otlp_log_record() {
    let privacy = LogPrivacy { anonymize_ip: AnonymizeIp::Truncate, ..LogPrivacy::default() };
    let mut out = Vec::new();
    write_record(&mut out, &request("/a?token=1"), &response(503), "203.0.113.9".parse().unwrap(), &privacy, Some(IDS), b"{\"status\":503,\"tab\":\"\t\x01\"}", 42);
    let record = String::from_utf8(out).unwrap();
    assert_eq!(
        format!(
            "{{\"timeUnixNano\":\"42\",\"observedTimeUnixNano\":\"42\",\"severityNumber\":17,\"severityText\":\"ERROR\",\
             \"body\":{{\"stringValue\":\"{{\\\"status\\\":503,\\\"tab\\\":\\\"\\t\\u0001\\\"}}\"}},\
             \"attributes\":[{{\"key\":\"http.request.method\",\"value\":{{\"stringValue\":\"GET\"}}}},\
             {{\"key\":\"url.path\",\"value\":{{\"stringValue\":\"/a\"}}}},\
             {{\"key\":\"http.response.status_code\",\"value\":{{\"intValue\":\"503\"}}}},\
             {{\"key\":\"client.address\",\"value\":{{\"stringValue\":\"203.0.113.0\"}}}},\
             {{\"key\":\"network.protocol.version\",\"value\":{{\"stringValue\":\"1.1\"}}}}],\
             \"traceId\":\"{}\",\"spanId\":\"{}\",\"flags\":1}}",
            "4b".repeat(16),
            "0a".repeat(8)
        ),
        record
    );
    // No span: no ids; 4xx is WARN, the rest INFO.
    for (status, severity) in [(404, "\"severityNumber\":13,\"severityText\":\"WARN\""), (200, "\"severityNumber\":9,\"severityText\":\"INFO\"")] {
        let mut out = Vec::new();
        write_record(&mut out, &request("/"), &response(status), "::1".parse().unwrap(), &LogPrivacy::default(), None, b"line", 1);
        let record = String::from_utf8(out).unwrap();
        assert!(record.contains(severity), "{record}");
        assert!(!record.contains("traceId"), "{record}");
        assert!(record.contains("\"stringValue\":\"::1\""), "{record}");
    }
}

#[test]
fn the_request_body_wraps_records_in_resource_logs() {
    let body = String::from_utf8(body(b"{\"a\":1},{\"b\":2}", "shop", "1.2")).unwrap();
    assert!(body.starts_with("{\"resourceLogs\":[{\"resource\":{\"attributes\":[{\"key\":\"service.name\",\"value\":{\"stringValue\":\"shop\"}},{\"key\":\"service.version\",\"value\":{\"stringValue\":\"1.2\"}}]}"), "{body}");
    assert!(body.ends_with("\"logRecords\":[{\"a\":1},{\"b\":2}]}]}]}"), "{body}");
}

#[test]
fn full_buffers_drop_records_instead_of_growing() {
    let mut buf = Vec::new();
    append(&mut buf, b"{\"timeUnixNano\":\"1\"}");
    append(&mut buf, b"");
    assert_eq!(b"{\"timeUnixNano\":\"1\"}".as_slice(), buf.as_slice());
    append(&mut buf, b"{\"timeUnixNano\":\"2\"}");
    assert_eq!(b"{\"timeUnixNano\":\"1\"},{\"timeUnixNano\":\"2\"}".as_slice(), buf.as_slice());
    let before = dropped();
    append(&mut buf, &vec![b' '; MAX_PENDING]);
    assert_eq!(before + 1, dropped());
    assert!(buf.len() < 100);
}

#[test]
fn configuration_comes_from_the_standard_variables() {
    let vars = |pairs: &'static [(&'static str, &'static str)]| move |name: &str| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string());
    assert!(config_from(vars(&[])).is_none(), "off unless asked for");
    assert!(config_from(vars(&[("OTEL_LOGS_EXPORTER", "none"), ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://c:4318")])).is_none());
    let config = config_from(vars(&[("OTEL_LOGS_EXPORTER", "console,otlp")])).unwrap();
    assert_eq!("http://localhost:4318/v1/logs", config.endpoint);
    assert_eq!(Duration::from_millis(1000), config.interval);
    assert_eq!("rws", config.service_name);
    let config = config_from(vars(&[
        ("OTEL_LOGS_EXPORTER", "otlp"),
        ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://c:4318/"),
        ("OTEL_BLRP_SCHEDULE_DELAY", "250"),
        ("OTEL_SERVICE_NAME", "shop"),
        ("OTEL_EXPORTER_OTLP_HEADERS", "a=1"),
        ("OTEL_EXPORTER_OTLP_LOGS_HEADERS", "b=2"),
    ]))
    .unwrap();
    assert_eq!("http://c:4318/v1/logs", config.endpoint);
    assert_eq!(Duration::from_millis(250), config.interval);
    assert_eq!("shop", config.service_name);
    assert_eq!(vec![("b".to_string(), "2".to_string())], config.headers);
    let config = config_from(vars(&[("OTEL_LOGS_EXPORTER", "otlp"), ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://c:4318"), ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "http://l:1/x")])).unwrap();
    assert_eq!("http://l:1/x", config.endpoint);
}

/// A collector that answers one POST and hands back its body.
fn collector() -> (String, std::thread::JoinHandle<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/logs", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = stream.read(&mut buf).unwrap();
            got.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&got).to_string();
            if let Some(end) = text.find("\r\n\r\n") {
                let length: usize = text[..end]
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap()))
                    .unwrap_or(0);
                if got.len() >= end + 4 + length {
                    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
                    return text;
                }
            }
            assert!(n > 0, "closed early");
        }
    });
    (url, handle)
}

#[test]
fn logged_requests_reach_the_collector_with_their_trace() {
    enable_on_this_thread(true);
    let marker = format!("/otlp-logs-test-{}", std::process::id());
    let privacy = LogPrivacy { anonymize_ip: AnonymizeIp::Truncate, ..LogPrivacy::default() };
    record(&request(&marker), &response(200), "198.51.100.23".parse().unwrap(), &privacy, Some(IDS), |out| out.extend_from_slice(b"the line"));
    // Through the access log itself: the trace header comes off the response.
    let mut served = response(404);
    put_trace(&mut served, IDS);
    crate::log::Log::log_served(&request(&format!("{marker}/served")), &mut served, &"192.0.2.1:1".parse().unwrap());
    assert!(served.headers.iter().all(|h| h.name != TRACE_HEADER));
    enable_on_this_thread(false);
    // Off again: not recorded.
    record(&request("/not-recorded"), &response(200), "192.0.2.1".parse().unwrap(), &privacy, None, |out| out.extend_from_slice(b"x"));

    let (url, handle) = collector();
    let exporter = Exporter {
        config: LogsExportConfig { endpoint: url, interval: Duration::from_secs(60), service_name: "logs-test".into(), service_version: "0".into(), headers: vec![("x-api-key".into(), "k".into())] },
        stop: AtomicBool::new(false),
        wake: (Mutex::new(()), Condvar::new()),
        sending: Mutex::new(()),
    };
    export(&exporter);
    let posted = handle.join().unwrap();
    assert!(posted.starts_with("POST /v1/logs HTTP/1.1\r\n"), "{posted}");
    assert!(posted.to_ascii_lowercase().contains("content-type: application/json"), "{posted}");
    assert!(posted.to_ascii_lowercase().contains("x-api-key: k"), "{posted}");
    assert!(posted.contains("\"stringValue\":\"logs-test\""), "{posted}");
    assert!(posted.contains(&format!("{{\"key\":\"url.path\",\"value\":{{\"stringValue\":\"{marker}\"}}}}")), "{posted}");
    assert!(posted.contains("\"body\":{\"stringValue\":\"the line\"}"), "{posted}");
    assert!(posted.contains("\"stringValue\":\"198.51.100.0\""), "anonymised: {posted}");
    assert!(posted.contains(&format!("\"traceId\":\"{}\",\"spanId\":\"{}\",\"flags\":1", "4b".repeat(16), "0a".repeat(8))), "{posted}");
    assert!(posted.contains(&format!("{marker}/served")), "the access log's record: {posted}");
    assert!(!posted.contains("/not-recorded"), "{posted}");
}
