use std::sync::{Arc, Mutex};

use super::*;

#[derive(Debug, Clone, PartialEq)]
struct Created(u32);

#[derive(Debug)]
struct Deleted;

#[test]
fn subscribers_get_only_their_type_in_order() {
    let bus = EventBus::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (a, b) = (Arc::clone(&seen), Arc::clone(&seen));
    bus.subscribe(move |e: &Created| a.lock().unwrap().push(format!("first {}", e.0)));
    bus.subscribe(move |e: &Created| b.lock().unwrap().push(format!("second {}", e.0)));
    assert_eq!(2, bus.subscribers::<Created>());
    assert_eq!(0, bus.publish(&Deleted));
    assert_eq!(2, bus.clone().publish(&Created(7)));
    assert_eq!(vec!["first 7", "second 7"], *seen.lock().unwrap());
}

#[test]
fn unsubscribe_stops_one_subscription() {
    let bus = EventBus::new();
    let count = Arc::new(Mutex::new(0));
    let c = Arc::clone(&count);
    let id = bus.subscribe(move |_: &Created| *c.lock().unwrap() += 1);
    bus.subscribe(|_: &Created| {});
    assert!(bus.unsubscribe(id));
    assert!(!bus.unsubscribe(id));
    assert_eq!(1, bus.publish(&Created(1)));
    assert_eq!(0, *count.lock().unwrap());
}

#[test]
fn a_panicking_subscriber_doesnt_stop_the_rest() {
    let bus = EventBus::new();
    let ran = Arc::new(Mutex::new(false));
    let r = Arc::clone(&ran);
    bus.subscribe(|_: &Created| panic!("boom"));
    bus.subscribe(move |_: &Created| *r.lock().unwrap() = true);
    assert_eq!(1, bus.publish(&Created(1)));
    assert!(*ran.lock().unwrap());
}

#[test]
fn subscribers_may_publish() {
    let bus = EventBus::new();
    let inner = bus.clone();
    let deleted = Arc::new(Mutex::new(0));
    let d = Arc::clone(&deleted);
    bus.subscribe(move |_: &Created| {
        inner.publish(&Deleted);
    });
    bus.subscribe(move |_: &Deleted| *d.lock().unwrap() += 1);
    bus.publish(&Created(1));
    assert_eq!(1, *deleted.lock().unwrap());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn events_forward_to_channels() {
    let bus = EventBus::new();
    let hub = crate::channels::Hub::new();
    let rx = hub.subscribe("item-3");
    bus.forward_to_channel(hub.clone(), |e: &Created| format!("item-{}", e.0), |e: &Created| format!("created {}", e.0));
    bus.publish(&Created(3));
    bus.publish(&Created(4));
    assert_eq!("created 3", rx.recv().unwrap());
    assert!(rx.try_recv().is_err());
}

#[test]
fn events_forward_to_mcp_notifications() {
    let server = crate::mcp::McpServer::new("t", "1");
    let bus = EventBus::new();
    bus.forward_to_mcp(server, "notifications/items/created", |e: &Created| format!(r#"{{"id":{}}}"#, e.0));
    // No client is listening: the notification goes nowhere, the subscriber completes.
    assert_eq!(1, bus.publish(&Created(5)));
}
