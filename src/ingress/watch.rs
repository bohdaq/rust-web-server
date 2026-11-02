//! Kubernetes "watch" stream reader.
//!
//! Turns a `?watch=true` streaming response into a low-latency "something
//! changed" signal that triggers a full re-list, rather than
//! incrementally applying `ADDED`/`MODIFIED`/`DELETED` events to an
//! in-memory per-object cache.
//!
//! A fully correct watch client tracks a `resourceVersion` bookmark,
//! applies each event as a delta, and re-lists from scratch on a
//! `410 Gone` (the bookmark expired). That's meaningfully more surface
//! area to get subtly wrong (missed deletes, duplicate or stale entries,
//! resourceVersion bookkeeping across reconnects) than this crate's
//! existing polling loop ever had to handle. Reading every event line as
//! a plain trigger for [`super::KubernetesIngressWatcher::poll`] still
//! delivers both benefits the gap this closes named: a quiet cluster
//! leaves this thread blocked on a read with no polling work at all, and
//! a real change unblocks that read — and triggers a re-list — as soon as
//! the API server sends it, instead of waiting up to `poll_interval_secs`.
//! The existing interval-based resync loop keeps running unchanged
//! alongside this as a safety net (and as the sole mechanism if the watch
//! connection can't be established at all, e.g. the API server doesn't
//! support it for some reason).

// The reader is shared with etcd's watch (service_discovery), the layer below.
pub(crate) use crate::service_discovery::chunked_lines::read_chunked_lines;
