//! A typed in-process event bus (18.35.0, #93): code that does something
//! publishes an event value; code that cares subscribes to its type.
//!
//! ```rust
//! # extern crate rws_server as rust_web_server;
//! use std::sync::{Arc, Mutex};
//! use rust_web_server::events::EventBus;
//!
//! #[derive(Debug, Clone)]
//! struct UserSignedUp { email: String }
//!
//! let bus = EventBus::new();
//! let welcomed = Arc::new(Mutex::new(Vec::new()));
//! let w = Arc::clone(&welcomed);
//! bus.subscribe(move |e: &UserSignedUp| w.lock().unwrap().push(e.email.clone()));
//!
//! assert_eq!(1, bus.publish(&UserSignedUp { email: "ann@example.com".into() }));
//! assert_eq!(vec!["ann@example.com"], *welcomed.lock().unwrap());
//! ```
//!
//! Subscribers run on the publishing thread, in the order they subscribed;
//! one that panics is skipped (and reported on stderr) without stopping the
//! others. Slow work belongs in a job the subscriber enqueues. Clones of a
//! bus share its subscribers. [`EventBus::forward_to_mcp`] turns an event
//! type into MCP notifications, [`EventBus::forward_to_channel`] into
//! [channel](crate::channels) messages.

#[cfg(test)]
mod tests;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

type Subscriber = Arc<dyn Fn(&dyn Any) + Send + Sync>;
type Subscribers = HashMap<TypeId, Vec<(SubscriptionId, Subscriber)>>;

/// What [`EventBus::subscribe`] returns: pass it to
/// [`EventBus::unsubscribe`] to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

/// Subscribers by event type. See the [module docs](self).
#[derive(Clone, Default)]
pub struct EventBus {
    subscribers: Arc<RwLock<Subscribers>>,
    next_id: Arc<AtomicU64>,
}

impl EventBus {
    pub fn new() -> Self {
        EventBus::default()
    }

    /// Call `handler` with every event of type `E` published from now on.
    pub fn subscribe<E, F>(&self, handler: F) -> SubscriptionId
    where
        E: Any,
        F: Fn(&E) + Send + Sync + 'static,
    {
        let id = SubscriptionId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let subscriber: Subscriber = Arc::new(move |event: &dyn Any| {
            if let Some(event) = event.downcast_ref::<E>() {
                handler(event);
            }
        });
        self.subscribers.write().unwrap_or_else(|e| e.into_inner()).entry(TypeId::of::<E>()).or_default().push((id, subscriber));
        id
    }

    /// Stop a subscription; `false` when it was already gone.
    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        let mut subscribers = self.subscribers.write().unwrap_or_else(|e| e.into_inner());
        for list in subscribers.values_mut() {
            if let Some(position) = list.iter().position(|(i, _)| *i == id) {
                list.remove(position);
                return true;
            }
        }
        false
    }

    /// Hand `event` to every subscriber of its type, in subscription
    /// order; returns how many ran to completion.
    pub fn publish<E: Any>(&self, event: &E) -> usize {
        // Cloned out, so a subscriber may subscribe or publish itself.
        let subscribers: Vec<Subscriber> = match self.subscribers.read().unwrap_or_else(|e| e.into_inner()).get(&TypeId::of::<E>()) {
            Some(list) => list.iter().map(|(_, s)| Arc::clone(s)).collect(),
            None => return 0,
        };
        let mut completed = 0;
        for subscriber in subscribers {
            match catch_unwind(AssertUnwindSafe(|| subscriber(event))) {
                Ok(()) => completed += 1,
                Err(_) => eprintln!("[events] a subscriber to {} panicked", std::any::type_name::<E>()),
            }
        }
        completed
    }

    /// How many subscribers events of type `E` have.
    pub fn subscribers<E: Any>(&self) -> usize {
        self.subscribers.read().unwrap_or_else(|e| e.into_inner()).get(&TypeId::of::<E>()).map_or(0, Vec::len)
    }

    /// Send each `E` to MCP clients as the notification `method`, with
    /// `params(event)` (a JSON object) as its params
    /// ([`McpServer::notify`](crate::mcp::McpServer::notify)).
    pub fn forward_to_mcp<E, F>(&self, server: crate::mcp::McpServer, method: &str, params: F) -> SubscriptionId
    where
        E: Any,
        F: Fn(&E) -> String + Send + Sync + 'static,
    {
        let method = method.to_string();
        self.subscribe(move |event: &E| server.notify(&method, Some(&params(event))))
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Publish each `E` to the channel topic `topic(event)` as
    /// `message(event)` ([`Hub::publish`](crate::channels::Hub::publish)).
    pub fn forward_to_channel<E, T, M>(&self, hub: crate::channels::Hub, topic: T, message: M) -> SubscriptionId
    where
        E: Any,
        T: Fn(&E) -> String + Send + Sync + 'static,
        M: Fn(&E) -> String + Send + Sync + 'static,
    {
        self.subscribe(move |event: &E| {
            hub.publish(&topic(event), &message(event));
        })
    }
}
