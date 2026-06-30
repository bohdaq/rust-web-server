use crate::sse::{Sse, SseEvent};

// ── SseEvent encoding ─────────────────────────────────────────────────────────

#[test]
fn data_only_event() {
    let encoded = SseEvent::data("hello").encode();
    assert_eq!(encoded, b"data: hello\n\n");
}

#[test]
fn empty_data_field_is_preserved() {
    let encoded = SseEvent::data("").encode();
    assert_eq!(encoded, b"data: \n\n");
}

#[test]
fn event_with_type() {
    let encoded = SseEvent::data("payload").event_type("update").encode();
    assert_eq!(encoded, b"event: update\ndata: payload\n\n");
}

#[test]
fn event_with_id() {
    let encoded = SseEvent::data("x").id("42").encode();
    assert_eq!(encoded, b"id: 42\ndata: x\n\n");
}

#[test]
fn event_with_all_fields() {
    let encoded = SseEvent::data("msg")
        .id("7")
        .event_type("ping")
        .retry(3000)
        .encode();
    assert_eq!(encoded, b"id: 7\nevent: ping\ndata: msg\nretry: 3000\n\n");
}

#[test]
fn multi_line_data_produces_multiple_data_lines() {
    let encoded = SseEvent::data("line1\nline2\nline3").encode();
    assert_eq!(encoded, b"data: line1\ndata: line2\ndata: line3\n\n");
}

// ── Sse builder ───────────────────────────────────────────────────────────────

#[test]
fn sse_event_convenience() {
    let response = Sse::new().event("update", "hello").into_response();
    let body = response.content_range_list[0].body.clone();
    assert_eq!(body, b"event: update\ndata: hello\n\n");
}

#[test]
fn sse_data_convenience() {
    let response = Sse::new().data("raw").into_response();
    let body = response.content_range_list[0].body.clone();
    assert_eq!(body, b"data: raw\n\n");
}

#[test]
fn sse_push_raw_event() {
    let response = Sse::new()
        .push(SseEvent::data("v").id("1").event_type("tick"))
        .into_response();
    let body = response.content_range_list[0].body.clone();
    assert_eq!(body, b"id: 1\nevent: tick\ndata: v\n\n");
}

#[test]
fn sse_retry_directive() {
    let response = Sse::new().retry(5000).into_response();
    let body = response.content_range_list[0].body.clone();
    assert_eq!(body, b"retry: 5000\n\n");
}

#[test]
fn sse_comment() {
    let response = Sse::new().comment("keep-alive").into_response();
    let body = response.content_range_list[0].body.clone();
    assert_eq!(body, b": keep-alive\n\n");
}

#[test]
fn sse_multiple_events_concatenated() {
    let response = Sse::new()
        .event("open", "")
        .data("first")
        .data("second")
        .into_response();
    let body = String::from_utf8(response.content_range_list[0].body.clone()).unwrap();
    assert_eq!(
        body,
        "event: open\ndata: \n\ndata: first\n\ndata: second\n\n"
    );
}

// ── Response headers ──────────────────────────────────────────────────────────

#[test]
fn into_response_status_200() {
    let response = Sse::new().into_response();
    assert_eq!(200, response.status_code);
}

#[test]
fn into_response_cache_control_no_cache() {
    let response = Sse::new().into_response();
    let cc = response.headers.iter().find(|h| h.name == "Cache-Control");
    assert!(cc.is_some());
    assert_eq!("no-cache", cc.unwrap().value);
}

#[test]
fn into_response_x_accel_buffering_no() {
    let response = Sse::new().into_response();
    let xab = response.headers.iter().find(|h| h.name == "X-Accel-Buffering");
    assert!(xab.is_some());
    assert_eq!("no", xab.unwrap().value);
}

#[test]
fn into_response_content_type_is_event_stream() {
    let response = Sse::new().data("x").into_response();
    let ct = response.content_range_list[0].content_type.clone();
    assert_eq!("text/event-stream", ct);
}
