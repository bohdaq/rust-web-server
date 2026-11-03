//! Runtime state shared across replicas (18.22.0, #76).
//!
//! With `[cluster] redis = "host:port"` (else `[acme] redis`), changes made
//! on one replica through the admin API, `rws ctl` or the MCP tools — the
//! IP blocklist, maintenance mode, drained backends, feature flags and cache
//! purges — reach every replica within about a second.
//!
//! How: one JSON document per cluster (`<prefix>state`) holds the shared
//! state and a version. A change is applied locally first, then sent to
//! Redis as an operation that a Lua script applies to the document
//! atomically (Redis's `cjson`), so concurrent changes on different
//! replicas never overwrite each other. Every replica reads the document
//! once a second and, when its version moved, makes its own state match.
//! With Redis down, local changes still apply and are sent once it's back;
//! a replica that starts adopts the document. Each replica also writes a
//! heartbeat into `<prefix>nodes`, which `GET /admin/api/v1/status` lists.
//!
//! The admin operations ([`crate::admin::ops`]) publish; code that changes
//! [`crate::blocklist::global`] or the feature store directly stays local.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::core::json::escape as json_escape;

/// `[cluster]` settings.
pub const RWS_CONFIG_CLUSTER_REDIS: &str = "RWS_CONFIG_CLUSTER_REDIS";
pub const RWS_CONFIG_CLUSTER_REDIS_PASSWORD: &str = "RWS_CONFIG_CLUSTER_REDIS_PASSWORD";
pub const RWS_CONFIG_CLUSTER_PREFIX: &str = "RWS_CONFIG_CLUSTER_PREFIX";

/// How often the shared document is read.
pub const POLL: Duration = Duration::from_secs(1);
/// How often a heartbeat is written; nodes silent for 60 s drop out.
pub const HEARTBEAT: Duration = Duration::from_secs(5);
/// Purges kept in the document (older ones have been applied by then).
pub const PURGES_KEPT: u64 = 50;

/// One change to the shared state.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Maintenance(bool),
    Block(String),
    Unblock(String),
    Drain { upstream: String, backend: String, drained: bool },
    Flag { name: String, enabled: bool },
    Purge { cache: String, path: Option<String>, prefix: Option<String> },
}

impl Op {
    /// The Lua script's arguments.
    fn args(&self) -> Vec<String> {
        let bit = |b: bool| if b { "1" } else { "0" }.to_string();
        match self {
            Op::Maintenance(on) => vec!["maintenance".into(), bit(*on)],
            Op::Block(ip) => vec!["block".into(), ip.clone()],
            Op::Unblock(ip) => vec!["unblock".into(), ip.clone()],
            Op::Drain { upstream, backend, drained } => vec!["drain".into(), drain_key(upstream, backend), bit(*drained)],
            Op::Flag { name, enabled } => vec!["flag".into(), name.clone(), bit(*enabled)],
            Op::Purge { cache, path, prefix } => vec!["purge".into(), cache.clone(), path.clone().unwrap_or_default(), prefix.clone().unwrap_or_default()],
        }
    }
}

fn drain_key(upstream: &str, backend: &str) -> String {
    format!("{upstream}|{backend}")
}

/// The shared document.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub version: u64,
    pub maintenance: bool,
    pub blocked: BTreeSet<String>,
    /// `upstream|backend`.
    pub drained: BTreeSet<String>,
    pub flags: BTreeMap<String, bool>,
    /// Purge id → (cache, path, prefix); empty strings for "none".
    pub purges: BTreeMap<u64, (String, String, String)>,
}

impl State {
    /// The operations that rebuild this state in an empty document.
    pub fn to_ops(&self) -> Vec<Op> {
        let mut ops = Vec::new();
        if self.maintenance {
            ops.push(Op::Maintenance(true));
        }
        ops.extend(self.blocked.iter().map(|ip| Op::Block(ip.clone())));
        for key in &self.drained {
            if let Some((upstream, backend)) = key.split_once('|') {
                ops.push(Op::Drain { upstream: upstream.to_string(), backend: backend.to_string(), drained: true });
            }
        }
        ops.extend(self.flags.iter().map(|(name, enabled)| Op::Flag { name: name.clone(), enabled: *enabled }));
        ops
    }

    /// Apply `op` (the same rules as the Lua script).
    pub fn apply(&mut self, op: &Op) {
        match op {
            Op::Maintenance(on) => self.maintenance = *on,
            Op::Block(ip) => {
                self.blocked.insert(ip.clone());
            }
            Op::Unblock(ip) => {
                self.blocked.remove(ip);
            }
            Op::Drain { upstream, backend, drained } => {
                let key = drain_key(upstream, backend);
                if *drained {
                    self.drained.insert(key);
                } else {
                    self.drained.remove(&key);
                }
            }
            Op::Flag { name, enabled } => {
                self.flags.insert(name.clone(), *enabled);
            }
            Op::Purge { cache, path, prefix } => {
                let id = self.purges.keys().next_back().copied().unwrap_or(0) + 1;
                self.purges.insert(id, (cache.clone(), path.clone().unwrap_or_default(), prefix.clone().unwrap_or_default()));
                self.purges.retain(|k, _| *k + PURGES_KEPT > id);
            }
        }
        self.version += 1;
    }

    /// The document as the Lua script writes it (`cjson`).
    pub fn to_json(&self) -> String {
        let set = |s: &BTreeSet<String>| s.iter().map(|k| format!("\"{}\":true", json_escape(k))).collect::<Vec<_>>().join(",");
        let flags: Vec<String> = self.flags.iter().map(|(k, v)| format!("\"{}\":{v}", json_escape(k))).collect();
        let purges: Vec<String> = self
            .purges
            .iter()
            .map(|(id, (c, p, x))| format!(r#""{id}":{{"cache":"{}","path":"{}","prefix":"{}"}}"#, json_escape(c), json_escape(p), json_escape(x)))
            .collect();
        format!(
            r#"{{"v":{},"maintenance":{},"blocked":{{{}}},"drained":{{{}}},"flags":{{{}}},"purges":{{{}}}}}"#,
            self.version,
            self.maintenance,
            set(&self.blocked),
            set(&self.drained),
            flags.join(","),
            purges.join(",")
        )
    }

    /// Read the document; `None` when it isn't one.
    pub fn from_json(text: &str) -> Option<State> {
        use crate::mcp::json_rpc::{extract_str, object_members};
        let top = object_members(text.trim())?;
        let field = |k: &str| top.iter().find(|(key, _)| key == k).map(|(_, v)| v.trim().to_string());
        // cjson writes an empty table as {}; a non-empty one as an object.
        let keys = |raw: Option<String>| -> BTreeSet<String> { raw.and_then(|r| object_members(&r)).map(|m| m.into_iter().map(|(k, _)| k).collect()).unwrap_or_default() };
        let version = field("v")?.trim_end_matches(".0").parse().ok()?;
        let purges = field("purges")
            .and_then(|r| object_members(&r))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, spec)| {
                let get = |k: &str| extract_str(&spec, k).unwrap_or_default();
                Some((id.parse().ok()?, (get("cache"), get("path"), get("prefix"))))
            })
            .collect();
        Some(State {
            version,
            maintenance: field("maintenance").as_deref() == Some("true"),
            blocked: keys(field("blocked")),
            drained: keys(field("drained")),
            flags: field("flags").and_then(|r| object_members(&r)).unwrap_or_default().into_iter().map(|(k, v)| (k, v.trim() == "true")).collect(),
            purges,
        })
    }
}

/// Where the shared document lives.
pub trait Store: Send + Sync {
    /// Apply `op` to the document atomically.
    fn apply(&self, op: &Op) -> std::io::Result<()>;
    /// The document, if there is one.
    fn state(&self) -> std::io::Result<Option<State>>;
    /// Record `node` as alive now; the nodes seen in the last minute.
    fn heartbeat(&self, node: &str, version: u64) -> std::io::Result<Vec<String>>;
}

/// The document in Redis.
pub struct RedisStore {
    conn: crate::redis_protocol::RespConn,
    prefix: String,
}

const APPLY_SCRIPT: &str = r#"
local s = redis.call('GET', KEYS[1])
local t = {}
if s then t = cjson.decode(s) end
t.blocked = t.blocked or {}
t.drained = t.drained or {}
t.flags = t.flags or {}
t.purges = t.purges or {}
local op = ARGV[1]
if op == 'maintenance' then t.maintenance = (ARGV[2] == '1')
elseif op == 'block' then t.blocked[ARGV[2]] = true
elseif op == 'unblock' then t.blocked[ARGV[2]] = nil
elseif op == 'drain' then if ARGV[3] == '1' then t.drained[ARGV[2]] = true else t.drained[ARGV[2]] = nil end
elseif op == 'flag' then t.flags[ARGV[2]] = (ARGV[3] == '1')
elseif op == 'purge' then
  local id = 0
  for k, _ in pairs(t.purges) do if tonumber(k) > id then id = tonumber(k) end end
  id = id + 1
  t.purges[tostring(id)] = { cache = ARGV[2], path = ARGV[3], prefix = ARGV[4] }
  for k, _ in pairs(t.purges) do if tonumber(k) + tonumber(ARGV[5]) <= id then t.purges[k] = nil end end
end
t.v = (t.v or 0) + 1
redis.call('SET', KEYS[1], cjson.encode(t))
return t.v
"#;

const HEARTBEAT_SCRIPT: &str = r#"
local s = redis.call('GET', KEYS[1])
local t = {}
if s then t = cjson.decode(s) end
local now = tonumber(ARGV[2])
t[ARGV[1]] = { at = now, v = tonumber(ARGV[3]) }
local names = {}
for k, n in pairs(t) do
  if n.at < now - 60 then t[k] = nil else table.insert(names, k) end
end
redis.call('SET', KEYS[1], cjson.encode(t))
table.sort(names)
return table.concat(names, ',')
"#;

impl RedisStore {
    pub fn new(addr: &str, password: Option<String>, prefix: &str) -> RedisStore {
        RedisStore { conn: crate::redis_protocol::RespConn::new(addr, password), prefix: prefix.to_string() }
    }
}

impl Store for RedisStore {
    fn apply(&self, op: &Op) -> std::io::Result<()> {
        let key = format!("{}state", self.prefix);
        let mut args = op.args();
        if matches!(op, Op::Purge { .. }) {
            args.push(PURGES_KEPT.to_string());
        }
        let mut cmd: Vec<&[u8]> = vec![b"EVAL", APPLY_SCRIPT.as_bytes(), b"1", key.as_bytes()];
        cmd.extend(args.iter().map(|a| a.as_bytes()));
        self.conn.cmd(&cmd).map(|_| ())
    }

    fn state(&self) -> std::io::Result<Option<State>> {
        let key = format!("{}state", self.prefix);
        match self.conn.cmd(&[b"GET", key.as_bytes()])? {
            crate::redis_protocol::RespReply::Bulk(Some(bytes)) => Ok(State::from_json(&String::from_utf8_lossy(&bytes))),
            _ => Ok(None),
        }
    }

    fn heartbeat(&self, node: &str, version: u64) -> std::io::Result<Vec<String>> {
        let key = format!("{}nodes", self.prefix);
        let now = unix_now().to_string();
        let v = version.to_string();
        match self.conn.cmd(&[b"EVAL", HEARTBEAT_SCRIPT.as_bytes(), b"1", key.as_bytes(), node.as_bytes(), now.as_bytes(), v.as_bytes()])? {
            crate::redis_protocol::RespReply::Bulk(Some(bytes)) => Ok(String::from_utf8_lossy(&bytes).split(',').filter(|n| !n.is_empty()).map(str::to_string).collect()),
            _ => Ok(Vec::new()),
        }
    }
}

impl<T: Store> Store for std::sync::Arc<T> {
    fn apply(&self, op: &Op) -> std::io::Result<()> {
        (**self).apply(op)
    }
    fn state(&self) -> std::io::Result<Option<State>> {
        (**self).state()
    }
    fn heartbeat(&self, node: &str, version: u64) -> std::io::Result<Vec<String>> {
        (**self).heartbeat(node, version)
    }
}

/// The document in memory: for tests (several replicas in one process) and
/// as a reference for the Lua script's rules.
#[derive(Default)]
pub struct MemoryStore {
    state: Mutex<Option<State>>,
    nodes: Mutex<BTreeMap<String, u64>>,
    /// When set, every call fails (Redis down).
    pub down: std::sync::atomic::AtomicBool,
}

impl MemoryStore {
    fn check(&self) -> std::io::Result<()> {
        if self.down.load(std::sync::atomic::Ordering::SeqCst) {
            Err(std::io::Error::other("store down"))
        } else {
            Ok(())
        }
    }
}

impl Store for MemoryStore {
    fn apply(&self, op: &Op) -> std::io::Result<()> {
        self.check()?;
        self.state.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(State::default).apply(op);
        Ok(())
    }

    fn state(&self) -> std::io::Result<Option<State>> {
        self.check()?;
        // Round-trip through JSON, as Redis does.
        Ok(self.state.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|s| State::from_json(&s.to_json())))
    }

    fn heartbeat(&self, node: &str, _version: u64) -> std::io::Result<Vec<String>> {
        self.check()?;
        let mut nodes = self.nodes.lock().unwrap_or_else(|e| e.into_inner());
        let now = unix_now();
        nodes.insert(node.to_string(), now);
        nodes.retain(|_, at| *at + 60 >= now);
        Ok(nodes.keys().cloned().collect())
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Makes this process's runtime state match a [`State`]; the default
/// applies to the process-wide stores.
pub trait Local: Send + Sync {
    fn apply(&self, state: &State, purges_after: u64);
    /// This process's current state, to seed an empty store (a Redis that
    /// restarted without its data, or a new cluster).
    fn snapshot(&self) -> State;
}

/// The process-wide blocklist, maintenance switch, upstream drains, feature
/// flags and named caches.
pub struct ProcessState;

impl Local for ProcessState {
    fn snapshot(&self) -> State {
        let mut state = State {
            maintenance: crate::maintenance::MAINTENANCE_MODE.load(std::sync::atomic::Ordering::SeqCst),
            blocked: crate::blocklist::global().list().into_iter().collect(),
            flags: crate::feature::global().list().into_iter().collect(),
            ..State::default()
        };
        #[cfg(not(target_arch = "wasm32"))]
        for (upstream, backends) in crate::proxy_config::upstream::snapshot() {
            for b in backends.iter().filter(|b| b.drained) {
                state.drained.insert(drain_key(&upstream, &b.url));
            }
        }
        state
    }

    fn apply(&self, state: &State, purges_after: u64) {
        crate::maintenance::MAINTENANCE_MODE.store(state.maintenance, std::sync::atomic::Ordering::SeqCst);
        let blocklist = crate::blocklist::global();
        let current: BTreeSet<String> = blocklist.list().into_iter().collect();
        for ip in state.blocked.difference(&current) {
            blocklist.block(ip);
        }
        for ip in current.difference(&state.blocked) {
            blocklist.unblock(ip);
        }
        #[cfg(not(target_arch = "wasm32"))]
        for (upstream, backends) in crate::proxy_config::upstream::snapshot() {
            for b in backends {
                let drained = state.drained.contains(&drain_key(&upstream, &b.url));
                if b.drained != drained {
                    crate::proxy_config::upstream::set_drained(&upstream, &b.url, drained);
                }
            }
        }
        for (name, enabled) in &state.flags {
            crate::feature::global().set(name, *enabled);
        }
        for (id, (cache, path, prefix)) in &state.purges {
            if *id > purges_after {
                if let Some(c) = crate::cache::registry::get(cache) {
                    let target = if !path.is_empty() {
                        crate::cache::Purge::Exact(path.clone())
                    } else if !prefix.is_empty() {
                        crate::cache::Purge::Prefix(prefix.clone())
                    } else {
                        crate::cache::Purge::All
                    };
                    c.purge(&target);
                }
            }
        }
    }
}

/// One replica's view of the cluster: the store, what it applied last, and
/// operations waiting for the store to come back.
pub struct ClusterSync {
    store: Box<dyn Store>,
    local: Box<dyn Local>,
    node: String,
    inner: Mutex<SyncState>,
    /// Changes made here, waiting for the sync thread (18.24.0: `publish`
    /// never waits on Redis, so an admin change doesn't hang when it's
    /// down); `wake` starts a round at once.
    queue: Mutex<Vec<Op>>,
    wake: std::sync::Condvar,
}

#[derive(Default)]
struct SyncState {
    applied: u64,
    purges_applied: Option<u64>,
    pending: Vec<Op>,
    nodes: Vec<String>,
    up: bool,
    reported_down: bool,
}

impl ClusterSync {
    pub fn new(store: Box<dyn Store>, local: Box<dyn Local>, node: &str) -> ClusterSync {
        ClusterSync { store, local, node: node.to_string(), inner: Mutex::new(SyncState::default()), queue: Mutex::new(Vec::new()), wake: std::sync::Condvar::new() }
    }

    /// Send a change made here (already applied locally).
    pub fn publish(&self, op: Op) {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).push(op);
        self.wake.notify_one();
    }

    /// Wait up to `timeout` for a published change (the sync thread's pause
    /// between rounds).
    fn wait(&self, timeout: Duration) {
        let queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        if queue.is_empty() {
            let _ = self.wake.wait_timeout(queue, timeout);
        }
    }

    /// One round: send what's pending, read the document, apply a newer
    /// one. `heartbeat` also writes this node's heartbeat.
    pub fn tick(&self, heartbeat: bool) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let published = std::mem::take(&mut *self.queue.lock().unwrap_or_else(|e| e.into_inner()));
        inner.pending.extend(published);
        let result = (|| -> std::io::Result<()> {
            if self.store.state()?.is_none() {
                // An empty store: seed it with what this replica has, so a
                // Redis that lost its data doesn't undo drains and blocks.
                for op in self.local.snapshot().to_ops() {
                    self.store.apply(&op)?;
                }
                // The queued changes are sent after it: each one sets a
                // value, so replaying what the snapshot already has is harmless.
            }
            while let Some(op) = inner.pending.first().cloned() {
                self.store.apply(&op)?;
                inner.pending.remove(0);
            }
            if let Some(state) = self.store.state()? {
                // The first read adopts the document without replaying old purges.
                let purges_after = *inner.purges_applied.get_or_insert_with(|| state.purges.keys().next_back().copied().unwrap_or(0));
                if state.version != inner.applied {
                    self.local.apply(&state, purges_after);
                    inner.applied = state.version;
                    inner.purges_applied = Some(state.purges.keys().next_back().copied().unwrap_or(purges_after).max(purges_after));
                }
            }
            if heartbeat {
                inner.nodes = self.store.heartbeat(&self.node, inner.applied)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                if inner.reported_down {
                    println!("[cluster] Redis reachable again; this replica is in sync");
                }
                inner.up = true;
                inner.reported_down = false;
            }
            Err(e) => {
                if !inner.reported_down {
                    eprintln!("[cluster] Redis unreachable ({e}); changes apply on this replica and are sent when it's back");
                    inner.reported_down = true;
                }
                inner.up = false;
            }
        }
    }

    /// `{"node":…,"redis":"up"|"down","version":N,"nodes":[…],"pending":N}`.
    pub fn status_json(&self) -> String {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let nodes: Vec<String> = inner.nodes.iter().map(|n| format!("\"{}\"", json_escape(n))).collect();
        format!(
            r#"{{"node":"{}","redis":"{}","version":{},"nodes":[{}],"pending":{}}}"#,
            json_escape(&self.node),
            if inner.up { "up" } else { "down" },
            inner.applied,
            nodes.join(","),
            inner.pending.len()
        )
    }
}

fn global() -> &'static OnceLock<ClusterSync> {
    static SYNC: OnceLock<ClusterSync> = OnceLock::new();
    &SYNC
}

/// Send a change made on this replica to the others (no-op without
/// `[cluster]`).
pub fn publish(op: Op) {
    if let Some(sync) = global().get() {
        sync.publish(op);
    }
}

/// The cluster status for the admin API, or `None` without `[cluster]`.
pub fn status_json() -> Option<String> {
    global().get().map(ClusterSync::status_json)
}

/// The binary's entry point: with `[cluster] redis` (or `[acme] redis`),
/// start syncing on a thread of its own.
pub fn start_from_env() {
    let env = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let Some(addr) = env(RWS_CONFIG_CLUSTER_REDIS).or_else(|| env(crate::entry_point::Config::RWS_CONFIG_ACME_REDIS)) else { return };
    let password = env(RWS_CONFIG_CLUSTER_REDIS_PASSWORD).or_else(|| env(crate::entry_point::Config::RWS_CONFIG_ACME_REDIS_PASSWORD));
    let prefix = env(RWS_CONFIG_CLUSTER_PREFIX).unwrap_or_else(|| "rws:cluster:".to_string());
    let node = crate::admin::access::node_name();
    let sync = ClusterSync::new(Box::new(RedisStore::new(&addr, password, &prefix)), Box::new(ProcessState), &node);
    if global().set(sync).is_err() {
        return;
    }
    println!("[cluster] sharing runtime state through Redis at {addr} as node {node}");
    let _ = std::thread::Builder::new().name("rws-cluster".into()).spawn(|| {
        let mut last_heartbeat: Option<std::time::Instant> = None;
        loop {
            if let Some(sync) = global().get() {
                let heartbeat = last_heartbeat.is_none_or(|t| t.elapsed() >= HEARTBEAT);
                sync.tick(heartbeat);
                if heartbeat {
                    last_heartbeat = Some(std::time::Instant::now());
                }
                sync.wait(POLL);
            } else {
                std::thread::sleep(POLL);
            }
        }
    });
}

#[cfg(test)]
mod tests;
