//! Counting traffic: every request (fed from the access-log path, so all
//! protocols and whether or not lines are written) lands in hourly counts
//! per host and group, daily unique visitors, and daily top lists.
//!
//! Bounded by construction: [`MAX_HOSTS`] hosts (more go to `other`),
//! hourly buckets for `retention_days`, top lists of [`TOP`] entries a day
//! (the rest added to `(other)`), [`MAX_PENDING`] undecided browsers per
//! shard, a classification cache of [`MAX_CACHE`] per shard. Unique
//! visitors are counted with `hash(secret, day, host, ip, user-agent)` —
//! the secret is made at start and never stored, nor are the hashes — so
//! nobody can be followed from one day to the next and no IP or User-Agent
//! is kept.
//!
//! A browser isn't a person until it loads a stylesheet, script or font as
//! a browser rendering the page does: its requests wait in a pending record
//! and are counted as a person's then, or as "browser, no assets" (a bot)
//! after [`UNDECIDED`] without one — or at once after 32 requests without
//! one.

use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, Hash, Hasher};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{classify, Class, Group, Kind};

/// Hosts counted separately; others are `other`.
pub const MAX_HOSTS: usize = 16;
/// Entries a daily top list keeps.
pub const TOP: usize = 200;
/// Entries of each top list kept on disk and reported.
pub const TOP_SAVED: usize = 100;
/// Browsers awaiting their first asset, per shard.
pub const MAX_PENDING: usize = 4096;
/// Classifications cached per shard.
pub const MAX_CACHE: usize = 16384;
/// A browser that loads no asset this long after its last request isn't
/// a person.
pub const UNDECIDED: Duration = Duration::from_secs(120);
/// A browser that makes this many requests without loading an asset is
/// decided at once: automation.
const MAX_PENDING_REQUESTS: usize = 32;
const SHARDS: usize = 16;

/// The groups counted, as array indexes.
const PEOPLE: usize = 0;
const AI: usize = 1;
const BOTS: usize = 2;

fn slot(group: Group) -> Option<usize> {
    match group {
        Group::Person => Some(PEOPLE),
        Group::Ai => Some(AI),
        Group::Bot => Some(BOTS),
        Group::Internal => None,
    }
}

/// One request, as counted.
#[derive(Clone, Debug)]
pub(crate) struct Hit<'a> {
    pub ip: IpAddr,
    pub host: &'a str,
    pub user_agent: Option<&'a str>,
    pub referer: Option<&'a str>,
    pub method: &'a str,
    pub path: &'a str,
    pub status: i16,
    /// The response's media type, when known.
    pub content_type: Option<&'a str>,
    pub now: u64,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) struct Counts {
    pub requests: u64,
    pub pages: u64,
}

/// A top list: counts by key, at most [`TOP`] keys and `(other)`.
#[derive(Default, Debug, Clone)]
pub(crate) struct Top(pub HashMap<String, u64>);

impl Top {
    fn add(&mut self, key: &str, n: u64) {
        if let Some(count) = self.0.get_mut(key) {
            *count += n;
        } else if self.0.len() < TOP {
            self.0.insert(key.chars().take(300).collect(), n);
        } else {
            *self.0.entry("(other)".to_string()).or_default() += n;
        }
    }

    pub(crate) fn merge(&mut self, other: &Top) {
        for (key, n) in &other.0 {
            *self.0.entry(key.clone()).or_default() += n;
        }
    }

    pub(crate) fn sorted(&self, n: usize) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = self.0.iter().map(|(k, v)| (k.clone(), *v)).collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out.truncate(n);
        out
    }
}

/// One day's lists and unique visitors (all hosts; keys carry the host).
#[derive(Default, Debug, Clone)]
pub(crate) struct Day {
    pub visitors: [u64; 3],
    pub people_pages: Top,
    pub ai_pages: Top,
    /// `name\toperator\tkind\tverified`.
    pub ai_agents: Top,
    /// `kind\tname`.
    pub bots: Top,
    pub referrers: Top,
    pub not_found: Top,
}

impl Day {
    pub(crate) fn merge(&mut self, other: &Day) {
        for g in 0..3 {
            self.visitors[g] += other.visitors[g];
        }
        self.people_pages.merge(&other.people_pages);
        self.ai_pages.merge(&other.ai_pages);
        self.ai_agents.merge(&other.ai_agents);
        self.bots.merge(&other.bots);
        self.referrers.merge(&other.referrers);
        self.not_found.merge(&other.not_found);
    }
}

/// A browser's requests while it's undecided.
struct Pending {
    last: u64,
    requests: Vec<Deferred>,
}

struct Deferred {
    hour: u64,
    host: u8,
    page: Option<String>,
    referrer: Option<String>,
    not_found: Option<String>,
}

#[derive(Default)]
struct Shard {
    /// `(hour, host)` → counts per group.
    hours: BTreeMap<(u64, u8), [Counts; 3]>,
    days: BTreeMap<u64, Day>,
    /// Today's visitors and the group each was counted in.
    seen: HashMap<u64, Group>,
    pending: HashMap<u64, Pending>,
    cache: HashMap<u64, Class>,
    today: u64,
}

struct Recorder {
    shards: Vec<Mutex<Shard>>,
    hosts: RwLock<Vec<String>>,
    /// A secret made at start and never saved; with the day it keys the
    /// visitor hash, so one day's visitors can't be matched to the next's.
    secret: RandomState,
    retention_days: u64,
    exclude: Vec<String>,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Bumped when host indexes change meaning (a restore, tests' resets).
static HOSTS_GENERATION: AtomicU64 = AtomicU64::new(1);
static CALLS: AtomicU64 = AtomicU64::new(0);

fn recorder() -> &'static Recorder {
    static RECORDER: OnceLock<Recorder> = OnceLock::new();
    RECORDER.get_or_init(|| Recorder {
        shards: (0..SHARDS).map(|_| Mutex::new(Shard::default())).collect(),
        hosts: RwLock::new(Vec::new()),
        secret: RandomState::new(),
        retention_days: super::config::retention_days(),
        exclude: super::config::exclude(),
    })
}

/// Start counting (the binary does this when the admin API is on).
pub fn enable() {
    let _ = recorder();
    ENABLED.store(true, Ordering::Relaxed);
}

/// Whether requests are being counted.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub(crate) fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A Host header as counted: lowercase, without the port.
fn normalize_host(host: &str) -> String {
    let host = host.trim();
    // `[::1]:8080` keeps its brackets; `example.com:443` loses the port.
    let host = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(rest),
        None => host.split(':').next().unwrap_or(host),
    };
    if host.is_empty() { "(none)".to_string() } else { host.to_ascii_lowercase() }
}

fn host_index(raw: &str) -> u8 {
    // The last host this thread saw: most requests repeat it.
    thread_local! {
        static LAST: std::cell::RefCell<(u64, String, u8)> = const { std::cell::RefCell::new((0, String::new(), 0)) };
    }
    let generation = HOSTS_GENERATION.load(Ordering::Relaxed);
    if let Ok(Some(i)) = LAST.try_with(|last| {
        let last = last.borrow();
        (last.0 == generation && !last.1.is_empty() && last.1 == raw).then_some(last.2)
    }) {
        return i;
    }
    let i = host_index_slow(raw);
    let _ = LAST.try_with(|last| *last.borrow_mut() = (generation, raw.to_string(), i));
    i
}

fn host_index_slow(host: &str) -> u8 {
    let host = normalize_host(host);
    let recorder = recorder();
    if let Some(i) = recorder.hosts.read().unwrap_or_else(|e| e.into_inner()).iter().position(|h| *h == host) {
        return i as u8;
    }
    let mut hosts = recorder.hosts.write().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = hosts.iter().position(|h| *h == host) {
        return i as u8;
    }
    if hosts.len() < MAX_HOSTS {
        hosts.push(host);
        return (hosts.len() - 1) as u8;
    }
    if let Some(i) = hosts.iter().position(|h| h == "other") {
        return i as u8;
    }
    hosts.push("other".to_string());
    (hosts.len() - 1) as u8
}

pub(crate) fn host_names() -> Vec<String> {
    recorder().hosts.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `hash(secret, day, host, ip, user-agent)` — no lock.
fn visitor(day: u64, host: u8, ip: IpAddr, user_agent: Option<&str>) -> u64 {
    let mut hasher = recorder().secret.build_hasher();
    day.hash(&mut hasher);
    host.hash(&mut hasher);
    ip.hash(&mut hasher);
    user_agent.unwrap_or("").hash(&mut hasher);
    hasher.finish()
}

fn is_page(hit: &Hit<'_>) -> bool {
    if hit.method != "GET" || !matches!(hit.status, 200 | 304) {
        return false;
    }
    match hit.content_type {
        Some(ct) => ct.trim_start().as_bytes().get(..9).is_some_and(|p| p.eq_ignore_ascii_case(b"text/html")),
        None => {
            let last = hit.path.rsplit('/').next().unwrap_or("");
            last.is_empty() || !last.contains('.') || last.ends_with(".html")
        }
    }
}

fn is_asset(path: &str) -> bool {
    let path = path.split('?').next().unwrap_or(path).as_bytes();
    [&b".css"[..], b".js", b".mjs", b".woff2", b".woff"].iter().any(|ext| path.len() >= ext.len() && path[path.len() - ext.len()..].eq_ignore_ascii_case(ext))
}

fn page_key(host: &str, path: &str) -> String {
    format!("{}{}", normalize_host(host), path.split('?').next().unwrap_or(path))
}

fn referrer_host(referer: Option<&str>, host: &str) -> Option<String> {
    let rest = referer?.split_once("://")?.1;
    let other = rest.split(['/', '?', '#']).next()?.split(':').next()?.to_ascii_lowercase();
    (!other.is_empty() && other != normalize_host(host)).then_some(other)
}

/// Requests of visitors already decided, counted by the thread that served
/// them (merged by readers): most requests are assets and repeats, and they
/// then cost no shared lock. Pages, 404s and undecided browsers take the
/// shared path, which also feeds the lists.
#[derive(Default)]
struct Local {
    /// Visitor → (day, counted group slot), as decided by a shard.
    decided: HashMap<u64, (u64, usize)>,
    counts: BTreeMap<(u64, u8), [Counts; 3]>,
}

/// How many decided visitors a thread remembers.
const LOCAL_DECIDED: usize = 1024;

fn locals() -> &'static Mutex<Vec<std::sync::Weak<Mutex<Local>>>> {
    static LOCALS: OnceLock<Mutex<Vec<std::sync::Weak<Mutex<Local>>>>> = OnceLock::new();
    LOCALS.get_or_init(Mutex::default)
}

thread_local! {
    static LOCAL: std::sync::Arc<Mutex<Local>> = {
        let local = std::sync::Arc::new(Mutex::new(Local::default()));
        let mut all = locals().lock().unwrap_or_else(|e| e.into_inner());
        all.retain(|w| w.strong_count() > 0);
        all.push(std::sync::Arc::downgrade(&local));
        local
    };
}

/// Move every thread's buffered counts into shard 0.
fn drain_locals() {
    let all: Vec<_> = locals().lock().unwrap_or_else(|e| e.into_inner()).iter().filter_map(|w| w.upgrade()).collect();
    let mut target = recorder().shards[0].lock().unwrap_or_else(|e| e.into_inner());
    for local in all {
        let counts = std::mem::take(&mut local.lock().unwrap_or_else(|e| e.into_inner()).counts);
        for (key, c) in counts {
            let entry = target.hours.entry(key).or_default();
            for g in 0..3 {
                entry[g].requests += c[g].requests;
                entry[g].pages += c[g].pages;
            }
        }
    }
}

/// Count one request.
pub(crate) fn record(hit: &Hit<'_>) {
    if !enabled() {
        return;
    }
    let recorder = recorder();
    let path = hit.path.split('?').next().unwrap_or(hit.path);
    if recorder.exclude.iter().any(|e| path == e) {
        return;
    }
    let day = hit.now / 86_400;
    let hour = hit.now / 3_600;
    let host = host_index(hit.host);
    let visitor = visitor(day, host, hit.ip, hit.user_agent);
    // A decided visitor's non-page request: counted by this thread (no
    // scanner check — the visitor keeps the group it was decided in).
    if hit.status != 404 && !is_page(hit) {
        let counted = LOCAL.try_with(|local| {
            let mut local = local.lock().unwrap_or_else(|e| e.into_inner());
            match local.decided.get(&visitor) {
                Some((d, g)) if *d == day => {
                    let g = *g;
                    local.counts.entry((hour, host)).or_default()[g].requests += 1;
                    true
                }
                _ => false,
            }
        });
        if counted == Ok(true) {
            return;
        }
    }
    let decided = record_shared(hit, day, hour, host, visitor, path);
    if let Some(g) = decided {
        let _ = LOCAL.try_with(|local| {
            let mut local = local.lock().unwrap_or_else(|e| e.into_inner());
            if local.decided.len() >= LOCAL_DECIDED {
                local.decided.clear();
            }
            local.decided.insert(visitor, (day, g));
        });
    }
    if CALLS.fetch_add(1, Ordering::Relaxed) % 1024 == 0 {
        settle(hit.now);
    }
}

/// The shared path; returns the visitor's group slot once it's decided.
fn record_shared(hit: &Hit<'_>, day: u64, hour: u64, host: u8, visitor: u64, path: &str) -> Option<usize> {
    let recorder = recorder();
    let mut shard = recorder.shards[(visitor % SHARDS as u64) as usize].lock().unwrap_or_else(|e| e.into_inner());
    shard.roll(day, recorder.retention_days);

    let class = match shard.cache.get(&visitor) {
        Some(class) => class.clone(),
        None => {
            let class = classify(hit.ip, hit.user_agent, "/");
            if shard.cache.len() >= MAX_CACHE {
                shard.cache.clear();
            }
            shard.cache.insert(visitor, class.clone());
            class
        }
    };
    let scanner = super::classify::scanner_path(hit.path) && matches!(class.kind, Kind::Browser | Kind::Tool | Kind::NoUserAgent);
    let class = if scanner { Class { kind: Kind::Scanner, name: "", operator: "", verified: false } } else { class };
    let page = is_page(hit).then(|| page_key(hit.host, path));
    let not_found = (hit.status == 404).then(|| page_key(hit.host, path));
    let referrer = referrer_host(hit.referer, hit.host);

    let decided = match class.group() {
        Group::Internal => None,
        Group::Person => {
            match shard.seen.get(&visitor).copied() {
                Some(Group::Person) => {
                    shard.count_person(hour, host, page, referrer, not_found);
                    Some(PEOPLE)
                }
                Some(_) => {
                    shard.count(BOTS, hour, host, &browser_without_assets(), page, not_found, visitor);
                    Some(BOTS)
                }
                None if is_asset(path) => {
                    shard.seen.insert(visitor, Group::Person);
                    shard.day(day).visitors[PEOPLE] += 1;
                    if let Some(pending) = shard.pending.remove(&visitor) {
                        for d in pending.requests {
                            shard.count_person(d.hour, d.host, d.page, d.referrer, d.not_found);
                        }
                    }
                    shard.count_person(hour, host, page, referrer, not_found);
                    Some(PEOPLE)
                }
                None => {
                    if shard.pending.len() >= MAX_PENDING && !shard.pending.contains_key(&visitor) {
                        // Too many undecided at once: count this one as a bot.
                        shard.seen.insert(visitor, Group::Bot);
                        shard.day(day).visitors[BOTS] += 1;
                        shard.count(BOTS, hour, host, &browser_without_assets(), page, not_found, visitor);
                        Some(BOTS)
                    } else {
                        let pending = shard.pending.entry(visitor).or_insert(Pending { last: hit.now, requests: Vec::new() });
                        pending.last = hit.now;
                        pending.requests.push(Deferred { hour, host, page, referrer, not_found });
                        if pending.requests.len() < MAX_PENDING_REQUESTS {
                            None
                        } else {
                            // This many requests and no stylesheet, script or
                            // font: automation, not someone reading pages.
                            let pending = shard.pending.remove(&visitor).expect("just added");
                            shard.seen.insert(visitor, Group::Bot);
                            shard.day(day).visitors[BOTS] += 1;
                            for d in pending.requests {
                                shard.count(BOTS, d.hour, d.host, &browser_without_assets(), d.page, d.not_found, visitor);
                            }
                            Some(BOTS)
                        }
                    }
                }
            }
        }
        group => {
            let g = slot(group).expect("counted group");
            if shard.seen.insert(visitor, group).is_none() {
                shard.day(day).visitors[g] += 1;
            }
            shard.count(g, hour, host, &class, page, not_found, visitor);
            // A scanner request isn't the visitor's usual class: not cached.
            (!scanner).then_some(g)
        }
    };
    decided
}

fn browser_without_assets() -> Class {
    Class { kind: Kind::Browser, name: "browser, no assets", operator: "", verified: false }
}

impl Shard {
    fn day(&mut self, day: u64) -> &mut Day {
        self.days.entry(day).or_default()
    }

    /// A new day: forget today's visitors and cache; drop what's past retention.
    fn roll(&mut self, day: u64, retention_days: u64) {
        if self.today == day {
            return;
        }
        self.today = day;
        self.seen.clear();
        self.cache.clear();
        let oldest_day = day.saturating_sub(retention_days.saturating_sub(1));
        self.days.retain(|d, _| *d >= oldest_day);
        self.hours.retain(|(h, _), _| *h / 24 >= oldest_day);
    }

    fn count_person(&mut self, hour: u64, host: u8, page: Option<String>, referrer: Option<String>, not_found: Option<String>) {
        let counts = &mut self.hours.entry((hour, host)).or_default()[PEOPLE];
        counts.requests += 1;
        counts.pages += page.is_some() as u64;
        let day = self.day(hour / 24);
        if let Some(page) = page {
            day.people_pages.add(&page, 1);
            if let Some(referrer) = referrer {
                day.referrers.add(&referrer, 1);
            }
        }
        if let Some(path) = not_found {
            day.not_found.add(&path, 1);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn count(&mut self, g: usize, hour: u64, host: u8, class: &Class, page: Option<String>, not_found: Option<String>, _visitor: u64) {
        let counts = &mut self.hours.entry((hour, host)).or_default()[g];
        counts.requests += 1;
        counts.pages += page.is_some() as u64;
        let day = self.day(hour / 24);
        if g == AI {
            day.ai_agents.add(&format!("{}\t{}\t{}\t{}", or_unknown(class.name), class.operator, class.kind.label(), class.verified), 1);
            if let Some(page) = page {
                day.ai_pages.add(&page, 1);
            }
        } else {
            let kind = if class.kind == Kind::Browser { "browser-no-assets" } else { class.kind.label() };
            day.bots.add(&format!("{kind}\t{}", class.name), 1);
        }
        if let Some(path) = not_found {
            day.not_found.add(&path, 1);
        }
    }
}

fn or_unknown(name: &str) -> &str {
    if name.is_empty() { "(unnamed)" } else { name }
}

/// Decide browsers that went quiet without loading an asset: bots.
pub(crate) fn settle(now: u64) {
    for shard in &recorder().shards {
        let mut shard = shard.lock().unwrap_or_else(|e| e.into_inner());
        let done: Vec<u64> = shard.pending.iter().filter(|(_, p)| now.saturating_sub(p.last) >= UNDECIDED.as_secs()).map(|(v, _)| *v).collect();
        for visitor in done {
            let Some(pending) = shard.pending.remove(&visitor) else { continue };
            let day = pending.requests.first().map_or(now / 86_400, |d| d.hour / 24);
            if shard.seen.insert(visitor, Group::Bot).is_none() {
                shard.day(day).visitors[BOTS] += 1;
            }
            for d in pending.requests {
                shard.count(BOTS, d.hour, d.host, &browser_without_assets(), d.page, d.not_found, visitor);
            }
        }
    }
}

/// Everything counted, merged across shards: `(hour, host) → counts` and
/// `day → lists` (after settling quiet browsers as of `now`).
pub(crate) fn snapshot(now: u64) -> (BTreeMap<(u64, u8), [Counts; 3]>, BTreeMap<u64, Day>) {
    settle(now);
    drain_locals();
    let mut hours: BTreeMap<(u64, u8), [Counts; 3]> = BTreeMap::new();
    let mut days: BTreeMap<u64, Day> = BTreeMap::new();
    for shard in &recorder().shards {
        let shard = shard.lock().unwrap_or_else(|e| e.into_inner());
        for (key, counts) in &shard.hours {
            let entry = hours.entry(*key).or_default();
            for g in 0..3 {
                entry[g].requests += counts[g].requests;
                entry[g].pages += counts[g].pages;
            }
        }
        for (day, lists) in &shard.days {
            days.entry(*day).or_default().merge(lists);
        }
    }
    (hours, days)
}

/// Put back what [`snapshot`] gave (loading a saved file): counts are added
/// to shard 0.
pub(crate) fn restore(hosts: Vec<String>, hours: BTreeMap<(u64, u8), [Counts; 3]>, days: BTreeMap<u64, Day>) {
    let recorder = recorder();
    let mut known = recorder.hosts.write().unwrap_or_else(|e| e.into_inner());
    let map: Vec<u8> = hosts
        .iter()
        .map(|h| match known.iter().position(|k| k == h) {
            Some(i) => i as u8,
            None if known.len() < MAX_HOSTS + 1 => {
                known.push(h.clone());
                (known.len() - 1) as u8
            }
            None => 0,
        })
        .collect();
    drop(known);
    HOSTS_GENERATION.fetch_add(1, Ordering::Relaxed);
    let mut shard = recorder.shards[0].lock().unwrap_or_else(|e| e.into_inner());
    for ((hour, host), counts) in hours {
        let host = map.get(host as usize).copied().unwrap_or(0);
        let entry = shard.hours.entry((hour, host)).or_default();
        for g in 0..3 {
            entry[g].requests += counts[g].requests;
            entry[g].pages += counts[g].pages;
        }
    }
    for (day, lists) in days {
        shard.days.entry(day).or_default().merge(&lists);
    }
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    let recorder = recorder();
    HOSTS_GENERATION.fetch_add(1, Ordering::Relaxed);
    for local in locals().lock().unwrap_or_else(|e| e.into_inner()).iter().filter_map(|w| w.upgrade()) {
        *local.lock().unwrap_or_else(|e| e.into_inner()) = Local::default();
    }
    for shard in &recorder.shards {
        *shard.lock().unwrap_or_else(|e| e.into_inner()) = Shard::default();
    }
    recorder.hosts.write().unwrap_or_else(|e| e.into_inner()).clear();
}
