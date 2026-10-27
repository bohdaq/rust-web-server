use super::*;

#[test]
fn the_body_is_an_otlp_cumulative_histogram() {
    http::record(http::Series::new("DELETE", 204, true, "HTTP/2.0"), 0.03);
    http::record(http::Series::new("DELETE", 204, true, "HTTP/2.0"), 20.0);
    let body = body("svc \"q\"", "1.2.3", 100, 200);
    assert!(body.starts_with(r#"{"resourceMetrics":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"svc \"q\""}}"#), "{body}");
    assert!(body.contains(r#""name":"http.server.request.duration""#) && body.contains(r#""unit":"s""#), "{body}");
    assert!(body.contains(r#""aggregationTemporality":2"#), "cumulative: {body}");
    let point = body
        .split(r#"{"attributes":["#)
        .find(|p| p.contains(r#""stringValue":"DELETE""#) && p.contains(r#""intValue":"204""#))
        .unwrap_or_else(|| panic!("{body}"));
    assert!(point.contains(r#"{"key":"url.scheme","value":{"stringValue":"https"}}"#), "{point}");
    assert!(point.contains(r#"{"key":"network.protocol.version","value":{"stringValue":"2"}}"#), "{point}");
    assert!(point.contains(r#""startTimeUnixNano":"100","timeUnixNano":"200","count":"2","sum":20.03"#), "{point}");
    assert!(point.contains(r#""bucketCounts":["0","0","0","1","0","0","0","0","0","0","0","0","0","0","1"]"#), "15 counts for 14 bounds: {point}");
    assert!(point.contains(r#""explicitBounds":[0.005,0.01,0.025,0.05,0.075,0.1,0.25,0.5,0.75,1,2.5,5,7.5,10]"#), "{point}");
}

#[test]
fn the_body_is_valid_json() {
    http::record(http::Series::new("GET", 200, false, "HTTP/1.1"), 0.001);
    let body = body("svc", "1", 1, 2);
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for c in body.chars() {
        match (in_string, escaped, c) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (_, false, '"') => in_string = !in_string,
            (false, _, '{' | '[') => depth += 1,
            (false, _, '}' | ']') => depth -= 1,
            _ => {}
        }
        assert!(depth >= 0, "{body}");
    }
    assert_eq!((0, false), (depth, in_string), "{body}");
}
