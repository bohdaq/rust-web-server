//! Counting traffic: every request (fed from the access-log path, so all
//! protocols and whether or not lines are written) lands in hourly counts
//! per host and group, daily unique visitors, and daily top lists.
//!
//! Bounded by construction: every site the configuration routes
//! ([`set_configured_hosts`], up to [`MAX_SLOTS`]) and [`MAX_HOSTS`] other
//! Host headers (more go to `other`),
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
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::{BuildHasher, Hash, Hasher};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{classify, Class, Group, Kind};

/// Host headers that aren't a configured site counted separately; others
/// are `other`.
pub const MAX_HOSTS: usize = 16;
/// Host slots in all (the index is a `u8`; one is kept for `other`).
pub const MAX_SLOTS: usize = 255;
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

/// The groups counted, as array indexes: people, AI agents, bots, the
/// site's apps (18.35) and the owner (18.35).
const PEOPLE: usize = 0;
const AI: usize = 1;
const BOTS: usize = 2;
const APPS: usize = 3;
const OWNER: usize = 4;
/// How many groups are counted.
pub const GROUPS: usize = 5;

fn slot(group: Group) -> Option<usize> {
    match group {
        Group::Person => Some(PEOPLE),
        Group::Ai => Some(AI),
        Group::Bot => Some(BOTS),
        Group::App => Some(APPS),
        Group::Owner => Some(OWNER),
        Group::Internal => None,
    }
}

/// One request, as counted.
#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct Hit<'a> {
    pub ip: IpAddr,
    pub host: &'a str,
    pub user_agent: Option<&'a str>,
    pub referer: Option<&'a str>,
    pub method: &'a str,
    pub path: &'a str,
    pub status: i16,
    /// The response's media type, when known.
    pub content_type: Option<&'a str>,
    /// The request's headers: read (for the `Sec-CH-UA-Platform` and
    /// `Sec-CH-UA-Mobile` client hints) only when a visitor is first seen,
    /// so other requests don't pay for the lookup.
    pub headers: &'a [crate::header::Header],
    pub now: u64,
}

/// The per-day lists of people's audience and sources, keyed
/// `host\tvalue`: countries, browsers, OSes and device classes count
/// visitors; channels and UTM values count visits (page views arriving
/// from outside the site).
pub const DIMS: [&str; 8] = ["countries", "browsers", "os", "devices", "channels", "utm_source", "utm_medium", "utm_campaign"];
const COUNTRIES: usize = 0;
const CHANNELS: usize = 4;
const UTM: usize = 5;
/// A person's page views more than this apart are separate sessions (as
/// Google Analytics counts them).
pub const SESSION_GAP: u64 = 30 * 60;
/// Sessions open at once, per shard; past it the quietest is closed.
#[cfg(not(test))]
pub const MAX_OPEN_SESSIONS: usize = 8192;
#[cfg(test)]
pub const MAX_OPEN_SESSIONS: usize = 64;

/// The per-day lists the script feeds (`[traffic] script`): seconds and
/// page views with a report, keyed `host/path`; scroll depth, screen
/// sizes, events and event properties, keyed `host\tvalue`; goal
/// conversions (sessions that reached the goal), keyed `host\tgoal`.
pub const BEHAVIOUR: [&str; 7] = ["engaged_secs", "engaged_views", "scroll", "screens", "events", "event_props", "goals"];
const ENGAGED_SECS: usize = 0;
const ENGAGED_VIEWS: usize = 1;
const SCROLL: usize = 2;
const SCREENS: usize = 3;
const EVENTS: usize = 4;
const EVENT_PROPS: usize = 5;
const GOALS: usize = 6;
/// Reports a session may make; the rest are ignored.
pub const MAX_REPORTS: u32 = 500;
/// The longest time on one page counted (a tab left open).
pub const MAX_ENGAGED_SECS: u64 = 6 * 3600;

/// How far back the live view looks.
pub const LIVE_WINDOW: u64 = 30 * 60;
/// People's recent page views kept for it, per shard.
pub const MAX_RECENT: usize = 4096;

/// A person's page view, for the live view.
#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct Recent {
    pub at: u64,
    pub visitor: u64,
    /// `host/path`.
    pub page: String,
    pub country: Option<[u8; 2]>,
}

/// Entries a day's audience/source list keeps (all hosts together).
pub const DIM_TOP: usize = 400;
/// Entries of each audience/source list kept on disk.
pub const DIM_SAVED: usize = 300;

/// A person's country and device, worked out once a day.
#[derive(Clone, Debug)]
struct Audience {
    country: Option<[u8; 2]>,
    device: super::device::Device,
}

/// How a visit arrived, for a page view from outside the site.
#[derive(Clone, Debug, Default)]
struct Arrival {
    channel: &'static str,
    /// `utm_source`, `utm_medium`, `utm_campaign`.
    utm: [Option<String>; 3],
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[doc(hidden)]
pub struct Counts {
    pub requests: u64,
    pub pages: u64,
}

/// A top list: counts by key, at most [`TOP`] keys and `(other)`.
#[derive(Default, Debug, Clone)]
#[doc(hidden)]
pub struct Top(pub HashMap<String, u64>);

impl Top {
    fn add(&mut self, key: &str, n: u64) {
        self.add_capped(key, n, TOP);
    }

    fn add_capped(&mut self, key: &str, n: u64, cap: usize) {
        if let Some(count) = self.0.get_mut(key) {
            *count += n;
        } else if self.0.len() < cap {
            self.0.insert(key.chars().take(300).collect(), n);
        } else {
            *self.0.entry("(other)".to_string()).or_default() += n;
        }
    }

    #[doc(hidden)]
    pub fn merge(&mut self, other: &Top) {
        for (key, n) in &other.0 {
            *self.0.entry(key.clone()).or_default() += n;
        }
    }

    #[doc(hidden)]
    pub fn sorted(&self, n: usize) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = self.0.iter().map(|(k, v)| (k.clone(), *v)).collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out.truncate(n);
        out
    }
}

/// One day's lists and unique visitors (all hosts; keys carry the host).
#[derive(Default, Debug, Clone)]
#[doc(hidden)]
pub struct Day {
    pub visitors: [u64; GROUPS],
    pub people_pages: Top,
    pub ai_pages: Top,
    /// `name\toperator\tkind\tverified`, all sites: days saved before
    /// 18.35 only (now [`Day::site_ai_agents`]).
    pub ai_agents: Top,
    /// `kind\tname`, all sites: days saved before 18.35 only.
    pub bots: Top,
    /// Referring hosts, all sites: days saved before 18.35 only.
    pub referrers: Top,
    /// Unique visitors per site: `host\tgroup` (`0` people, `1` AI, `2` bots).
    pub site_visitors: Top,
    /// `host\tname\toperator\tkind\tverified`.
    pub site_ai_agents: Top,
    /// `host\tkind\tname`.
    pub site_bots: Top,
    /// `host\treferring host`.
    pub site_referrers: Top,
    pub not_found: Top,
    /// The [`DIMS`] lists, in that order.
    pub dims: [Top; 8],
    /// People's sessions per host (by the day they started).
    pub sessions: BTreeMap<String, Sessions>,
    /// The first and last page of people's sessions (`host/path`).
    pub entry_pages: Top,
    pub exit_pages: Top,
    /// The [`BEHAVIOUR`] lists, in that order.
    pub behaviour: [Top; 7],
    /// The [`EXTRA`] lists (18.35), in that order.
    pub extra: [Top; EXTRA.len()],
    /// People's visits that ended this day (18.35): the recent-visitors
    /// feed. Only the last [`MAX_VISITS`] are kept across all days.
    pub visits: Vec<Visit>,
}

/// The per-day lists added in 18.35, all keyed by host first:
/// - `site_apps`: app users, `host\tapp\tplatform`;
/// - `app_versions`: app users, `host\tapp\tversion`;
/// - `app_events`: events apps sent to `/_rws/e`, `host\tapp\tevent`;
/// - `site_tiers`: visitors by how sure rws is they're a person,
///   `host\ttier` (`verified` ran the script, `page-only` loaded no asset,
///   `hosting` came from a server network);
/// - `site_probes`: scanner requests, `host\tpath`;
/// - `probe_sources`: scanner requests, `host\tcountry\tnetwork` (and the
///   address with `[traffic] probe_addresses`);
/// - `site_crawl`: named bots' and AI agents' page fetches and 404s,
///   `host\tname\tpages|404`;
/// - `crawl_404`: the paths named bots got a 404 for, `host/path\tname`;
/// - `site_entries`: the first page of people's visits by channel,
///   `host/path\tchannel`;
/// - `downloads`: files downloaded (installers, packages, archives,
///   `install.sh`), `host/path\tgroup`;
/// - `owner_pages`: the owner's page views, `host/path`.
pub const EXTRA: [&str; 11] =
    ["site_apps", "app_versions", "app_events", "site_tiers", "site_probes", "probe_sources", "site_crawl", "crawl_404", "site_entries", "downloads", "owner_pages"];
const SITE_APPS: usize = 0;
const APP_VERSIONS: usize = 1;
const APP_EVENTS: usize = 2;
const SITE_TIERS: usize = 3;
const SITE_PROBES: usize = 4;
const PROBE_SOURCES: usize = 5;
const SITE_CRAWL: usize = 6;
const CRAWL_404: usize = 7;
const SITE_ENTRIES: usize = 8;
const DOWNLOADS: usize = 9;
const OWNER_PAGES: usize = 10;

/// Visits kept for the recent-visitors feed (across all days).
pub const MAX_VISITS: usize = 300;
/// Pages a visit lists.
pub const VISIT_PAGES: usize = 20;

/// One person's visit (a closed session, or an open one in a snapshot),
/// for the recent-visitors feed: what they read and how they came, with
/// no address, User-Agent or visitor key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[doc(hidden)]
pub struct Visit {
    pub start: u64,
    pub end: u64,
    pub host: String,
    /// Paths, in order (at most [`VISIT_PAGES`]).
    pub pages: Vec<String>,
    /// Page views in all (more than `pages` when it's long).
    pub views: u64,
    pub country: String,
    pub browser: String,
    pub os: String,
    pub device: String,
    pub channel: String,
    pub referrer: String,
    pub campaign: String,
    /// The visitor ran the script (`[traffic] script`).
    pub verified: bool,
    /// This visitor's visit number today (2 = came back).
    pub number: u32,
    pub events: Vec<String>,
}

/// Sessions added up.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[doc(hidden)]
pub struct Sessions {
    pub sessions: u64,
    /// Sessions of one page view.
    pub bounces: u64,
    /// Seconds from each session's first page view to its last.
    pub duration: u64,
    /// Page views in sessions.
    pub pages: u64,
}

impl Sessions {
    #[doc(hidden)]
    pub fn add(&mut self, other: &Sessions) {
        self.sessions += other.sessions;
        self.bounces += other.bounces;
        self.duration += other.duration;
        self.pages += other.pages;
    }
}

/// A person's session while it's open.
struct Session {
    start: u64,
    last: u64,
    pages: u64,
    /// The page key (`host/path`) last viewed.
    exit: String,
    /// Goals reached, by index into the configured goals.
    goals: u32,
    /// How the session arrived (for the database's rows).
    channel: Option<&'static str>,
    utm: [Option<String>; 3],
    /// Script reports so far.
    reports: u32,
    /// For the visit feed: paths in order (at most [`VISIT_PAGES`]), the
    /// visitor's audience, the referring site, events, the visit number.
    paths: Vec<String>,
    visitor: u64,
    audience: Option<Audience>,
    referrer: Option<String>,
    events: Vec<String>,
    number: u32,
    verified: bool,
}

impl Session {
    fn visit(&self) -> Visit {
        let host = self.exit.split('/').next().unwrap_or("").to_string();
        let audience = self.audience.as_ref();
        Visit {
            start: self.start,
            end: self.last,
            host,
            pages: self.paths.clone(),
            views: self.pages,
            country: audience.and_then(|a| a.country).map_or_else(String::new, |c| String::from_utf8_lossy(&c).into_owned()),
            browser: audience.map_or("", |a| a.device.browser).to_string(),
            os: audience.map_or("", |a| a.device.os).to_string(),
            device: audience.map_or("", |a| a.device.class).to_string(),
            channel: self.channel.unwrap_or("").to_string(),
            referrer: self.referrer.clone().unwrap_or_default(),
            campaign: self.utm[2].clone().or_else(|| self.utm[0].clone()).unwrap_or_default(),
            verified: self.verified,
            number: self.number,
            events: self.events.clone(),
        }
    }

    fn totals(&self) -> Sessions {
        Sessions { sessions: 1, bounces: (self.pages == 1) as u64, duration: self.last - self.start, pages: self.pages }
    }
}

impl Day {
    #[doc(hidden)]
    pub fn merge(&mut self, other: &Day) {
        for g in 0..GROUPS {
            self.visitors[g] += other.visitors[g];
        }
        for (mine, theirs) in self.extra.iter_mut().zip(&other.extra) {
            mine.merge(theirs);
        }
        self.visits.extend(other.visits.iter().cloned());
        for (mine, theirs) in self.dims.iter_mut().zip(&other.dims) {
            mine.merge(theirs);
        }
        self.people_pages.merge(&other.people_pages);
        self.ai_pages.merge(&other.ai_pages);
        self.ai_agents.merge(&other.ai_agents);
        self.bots.merge(&other.bots);
        self.referrers.merge(&other.referrers);
        self.site_visitors.merge(&other.site_visitors);
        self.site_ai_agents.merge(&other.site_ai_agents);
        self.site_bots.merge(&other.site_bots);
        self.site_referrers.merge(&other.site_referrers);
        self.not_found.merge(&other.not_found);
        for (mine, theirs) in self.behaviour.iter_mut().zip(&other.behaviour) {
            mine.merge(theirs);
        }
        self.entry_pages.merge(&other.entry_pages);
        self.exit_pages.merge(&other.exit_pages);
        for (host, s) in &other.sessions {
            self.sessions.entry(host.clone()).or_default().add(s);
        }
    }
}

/// A browser's requests while it's undecided.
struct Pending {
    last: u64,
    requests: Vec<Deferred>,
}

struct Deferred {
    /// When it was made (seconds).
    at: u64,
    hour: u64,
    host: u8,
    page: Option<String>,
    referrer: Option<String>,
    not_found: Option<String>,
    arrival: Option<Arrival>,
}

/// A visitor's classification, and for browsers their audience.
#[derive(Clone)]
struct Cached {
    class: Class,
    audience: Option<Audience>,
    /// For an app client: the version its User-Agent names.
    app_version: Option<String>,
}

#[derive(Default)]
struct Shard {
    /// `(hour, host)` → counts per group.
    hours: BTreeMap<(u64, u8), [Counts; GROUPS]>,
    days: BTreeMap<u64, Day>,
    /// Today's visitors and the group each was counted in.
    seen: HashMap<u64, Group>,
    pending: HashMap<u64, Pending>,
    cache: HashMap<u64, Cached>,
    /// People's open sessions by visitor.
    open: HashMap<u64, Session>,
    /// People's page views of the last [`LIVE_WINDOW`], oldest first.
    recent: VecDeque<Recent>,
    today: u64,
    /// Today's people who ran the script (18.35).
    verified: std::collections::HashSet<u64>,
    /// Visits each person started today.
    visit_numbers: HashMap<u64, u32>,
    /// Events each app client sent today (`/_rws/e`), to cap them.
    app_reports: HashMap<u64, u32>,
}

struct Recorder {
    shards: Vec<Mutex<Shard>>,
    hosts: RwLock<Vec<String>>,
    /// The sites the configuration routes (normalized, sorted).
    configured: RwLock<Vec<String>>,
    /// A secret made at start and never saved; with the day it keys the
    /// visitor hash, so one day's visitors can't be matched to the next's.
    secret: RandomState,
    retention_days: u64,
    exclude: Vec<String>,
    /// `[traffic] hosting_networks`.
    hosting: bool,
    /// Today's owner fingerprints (`hash(secret, day, ip, user-agent)`,
    /// no host): `(day, set)`.
    owners: Mutex<(u64, std::collections::HashSet<u64>)>,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Bumped when host indexes change meaning (a restore, tests' resets).
static HOSTS_GENERATION: AtomicU64 = AtomicU64::new(1);
static CALLS: AtomicU64 = AtomicU64::new(0);
/// Bumped when a client becomes the owner, so threads forget the group
/// they had decided for it.
static OWNER_GENERATION: AtomicU64 = AtomicU64::new(1);

fn recorder() -> &'static Recorder {
    static RECORDER: OnceLock<Recorder> = OnceLock::new();
    RECORDER.get_or_init(|| Recorder {
        shards: (0..SHARDS).map(|_| Mutex::new(Shard::default())).collect(),
        hosts: RwLock::new(Vec::new()),
        configured: RwLock::new(Vec::new()),
        secret: RandomState::new(),
        retention_days: super::config::retention_days(),
        exclude: super::config::exclude(),
        hosting: super::config::hosting_networks(),
        owners: Mutex::new((0, std::collections::HashSet::new())),
    })
}

/// Start counting (the binary does this when the admin API is on).
pub fn enable() {
    // The access log (no `log` module on wasm32) reports each request here.
    #[cfg(not(target_arch = "wasm32"))]
    crate::log::set_access_observer(super::observe);
    let _ = recorder();
    ENABLED.store(true, Ordering::Relaxed);
}

/// Whether requests are being counted.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Seconds since the epoch, as the counts use.
pub fn now() -> u64 {
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
    let configured = recorder.configured.read().unwrap_or_else(|e| e.into_inner());
    let mut hosts = recorder.hosts.write().unwrap_or_else(|e| e.into_inner());
    host_slot(&mut hosts, &configured, host)
}

/// The slot of `host` (normalized), added when there's room: a configured
/// site always gets one; another Host header while fewer than
/// [`MAX_HOSTS`] such are counted (with no configured sites, the first
/// [`MAX_HOSTS`] hosts seen); the rest share `other`.
fn host_slot(hosts: &mut Vec<String>, configured: &[String], host: String) -> u8 {
    let host = canonical(host, configured);
    if let Some(i) = hosts.iter().position(|h| *h == host) {
        return i as u8;
    }
    let room = host != "other"
        && hosts.len() < MAX_SLOTS
        && if configured.is_empty() {
            hosts.len() < MAX_HOSTS
        } else {
            configured.binary_search(&host).is_ok() || hosts.iter().filter(|h| *h != "other" && configured.binary_search(h).is_err()).count() < MAX_HOSTS
        };
    if room {
        hosts.push(host);
        return (hosts.len() - 1) as u8;
    }
    if let Some(i) = hosts.iter().position(|h| h == "other") {
        return i as u8;
    }
    hosts.push("other".to_string());
    (hosts.len() - 1) as u8
}

/// The most configured sites counted separately.
const MAX_CONFIGURED: usize = MAX_SLOTS - MAX_HOSTS - 1;

/// The sites the configuration routes (`[[route]] match.host`): each gets
/// its own counts, ahead of whatever else arrives in a Host header, and is
/// listed first on the Traffic page. Set at start and on every reload.
pub fn set_configured_hosts<I: IntoIterator<Item = String>>(hosts: I) {
    let hidden: Vec<String> = super::config::hide_hosts().iter().map(|h| normalize_host(h)).collect();
    let mut list: Vec<String> =
        hosts.into_iter().map(|h| normalize_host(&h)).filter(|h| h != "(none)" && h != "other" && !h.contains('*') && !hidden.contains(h)).collect();
    list.sort();
    list.dedup();
    // `www.<site>` counts as `<site>`.
    let apexes = list.clone();
    list.retain(|h| h.strip_prefix("www.").is_none_or(|apex| apexes.binary_search_by(|c| c.as_str().cmp(apex)).is_err()));
    list.truncate(MAX_CONFIGURED);
    let mut configured = recorder().configured.write().unwrap_or_else(|e| e.into_inner());
    if *configured != list {
        *configured = list;
        // Hosts that went to `other` may have a slot now.
        HOSTS_GENERATION.fetch_add(1, Ordering::Relaxed);
    }
}

/// The configured sites ([`set_configured_hosts`]), sorted.
#[doc(hidden)]
pub fn configured_hosts() -> Vec<String> {
    recorder().configured.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The name of host slot `i`.
fn host_name(i: u8) -> String {
    recorder().hosts.read().unwrap_or_else(|e| e.into_inner()).get(i as usize).cloned().unwrap_or_else(|| "other".to_string())
}

#[doc(hidden)]
pub fn host_names() -> Vec<String> {
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
    format!("{}{}", site_of(host), path.split('?').next().unwrap_or(path))
}

/// The site a Host header is counted under (18.35.2): with configured sites,
/// a configured one as is, `www.<site>` as `<site>`, anything else (addresses,
/// `(none)`, names no route serves, `[traffic] hide_hosts`) as `other`;
/// without, the host itself.
fn canonical(host: String, configured: &[String]) -> String {
    if configured.is_empty() || configured.binary_search(&host).is_ok() || host == "other" {
        return host;
    }
    match host.strip_prefix("www.") {
        Some(apex) if configured.binary_search_by(|c| c.as_str().cmp(apex)).is_ok() => apex.to_string(),
        _ => "other".to_string(),
    }
}

/// [`canonical`] of a raw Host header.
fn site_of(raw: &str) -> String {
    canonical(normalize_host(raw), &recorder().configured.read().unwrap_or_else(|e| e.into_inner()))
}

fn referrer_host(referer: Option<&str>, host: &str) -> Option<String> {
    let rest = referer?.split_once("://")?.1;
    let other = rest.split(['/', '?', '#']).next()?.split(':').next()?.to_ascii_lowercase();
    // `www.example.com` linking to `example.com` is the same site.
    (!other.is_empty() && other != normalize_host(host) && site_of(&other) != site_of(host)).then_some(other)
}

/// The channel a referring site belongs to.
#[doc(hidden)]
pub fn channel_of(referrer_host: &str) -> &'static str {
    let h = referrer_host.strip_prefix("www.").unwrap_or(referrer_host);
    let is = |d: &str| h == d || h.ends_with(&format!(".{d}"));
    const AI: &[&str] = &["chatgpt.com", "chat.openai.com", "perplexity.ai", "claude.ai", "gemini.google.com", "copilot.microsoft.com", "chat.deepseek.com", "chat.mistral.ai", "you.com", "phind.com", "kagi.com"];
    const SEARCH: &[&str] = &["bing.com", "duckduckgo.com", "search.yahoo.com", "yandex.ru", "yandex.com", "baidu.com", "ecosia.org", "search.brave.com", "startpage.com", "qwant.com", "naver.com", "seznam.cz"];
    const SOCIAL: &[&str] = &[
        "facebook.com", "m.facebook.com", "l.facebook.com", "lm.facebook.com", "t.co", "twitter.com", "x.com", "linkedin.com", "lnkd.in", "reddit.com", "instagram.com",
        "youtube.com", "tiktok.com", "pinterest.com", "news.ycombinator.com", "mastodon.social", "bsky.app", "threads.net", "vk.com", "telegram.org", "t.me", "discord.com",
        "whatsapp.com", "lobste.rs",
    ];
    // App stores, before search: play.google.com isn't Google search.
    const STORE: &[&str] = &[
        "apps.apple.com", "itunes.apple.com", "play.google.com", "chromewebstore.google.com", "addons.mozilla.org", "microsoftedge.microsoft.com",
        "apps.microsoft.com", "snapcraft.io", "flathub.org", "f-droid.org", "appgallery.huawei.com",
    ];
    if AI.iter().any(|d| is(d)) {
        "ai"
    } else if STORE.iter().any(|d| is(d)) || (h == "chrome.google.com") {
        "store"
    } else if SEARCH.iter().any(|d| is(d)) || h.starts_with("google.") || h.contains(".google.") {
        "search"
    } else if SOCIAL.iter().any(|d| is(d)) {
        "social"
    } else {
        "referral"
    }
}

/// `%XX` and `+` decoded, at most 100 characters; `None` when empty.
fn query_value(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len().min(300));
    let mut i = 0;
    while i < bytes.len() && out.len() < 300 {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    let value: String = String::from_utf8_lossy(&out).trim().chars().filter(|c| !c.is_control()).take(100).collect();
    (!value.is_empty()).then_some(value)
}

/// `utm_source`, `utm_medium` and `utm_campaign` from a request target.
#[doc(hidden)]
pub fn utm(target: &str) -> [Option<String>; 3] {
    let mut out: [Option<String>; 3] = Default::default();
    let Some(query) = target.split_once('?').map(|(_, q)| q.split('#').next().unwrap_or(q)) else { return out };
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let slot = match name {
            "utm_source" => 0,
            "utm_medium" => 1,
            "utm_campaign" => 2,
            _ => continue,
        };
        if out[slot].is_none() {
            out[slot] = query_value(value);
        }
    }
    out
}

/// How a page view arrived: `None` for navigation within the site (a
/// same-site `Referer`), else the channel and any UTM values.
fn arrival(hit: &Hit<'_>) -> Option<Arrival> {
    let utm = utm(hit.path);
    let channel = if utm[0].is_some() || utm[1].is_some() || utm[2].is_some() {
        "campaign"
    } else {
        match hit.referer {
            None => "direct",
            Some(_) => channel_of(&referrer_host(hit.referer, hit.host)?),
        }
    };
    Some(Arrival { channel, utm })
}

/// Requests of visitors already decided, counted by the thread that served
/// them (merged by readers): most requests are assets and repeats, and they
/// then cost no shared lock. Pages, 404s and undecided browsers take the
/// shared path, which also feeds the lists.
#[derive(Default)]
struct Local {
    /// Visitor → (day, counted group slot), as decided by a shard.
    /// Visitor → (day, owner generation, counted group slot).
    decided: HashMap<u64, (u64, u64, usize)>,
    counts: BTreeMap<(u64, u8), [Counts; GROUPS]>,
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
            for g in 0..GROUPS {
                entry[g].requests += c[g].requests;
                entry[g].pages += c[g].pages;
            }
        }
    }
}

/// Count one request.
#[doc(hidden)]
pub fn record(hit: &Hit<'_>) {
    if !enabled() {
        return;
    }
    let recorder = recorder();
    let path = hit.path.split('?').next().unwrap_or(hit.path);
    // rws's own endpoints (the script's reports, app events, the owner
    // link) are counted by what they report, not as requests. The script
    // itself is an asset like any: loading it shows a browser.
    if recorder.exclude.iter().any(|e| path == e)
        || (path.starts_with("/_rws/") && matches!(path, super::script::BEACON_PATH | super::script::APP_EVENTS_PATH | super::owner::PATH))
    {
        return;
    }
    let day = hit.now / 86_400;
    // A successful admin API call: this client is the owner from now on.
    // (JSON first: it rules out almost every request without a search.)
    let json = hit.content_type.is_some_and(|ct| ct.as_bytes().get(..16).is_some_and(|p| p.eq_ignore_ascii_case(b"application/json")));
    if json && path.contains("/api/") && super::owner::is_admin_call(hit.headers, hit.status) {
        mark_owner(hit.ip, hit.user_agent, hit.now);
    }
    let hour = hit.now / 3_600;
    let host = host_index(hit.host);
    let visitor = visitor(day, host, hit.ip, hit.user_agent);
    let generation = OWNER_GENERATION.load(Ordering::Relaxed);
    let download = is_download(hit, path);
    let request_goal = request_goals_configured() && (200..400).contains(&hit.status);
    // A decided visitor's non-page request: counted by this thread (no
    // scanner check — the visitor keeps the group it was decided in).
    if hit.status != 404 && !download && !request_goal && !is_page(hit) {
        let counted = LOCAL.try_with(|local| {
            let mut local = local.lock().unwrap_or_else(|e| e.into_inner());
            match local.decided.get(&visitor) {
                Some((d, gen, g)) if *d == day && *gen == generation => {
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
    let decided = record_shared(hit, day, hour, host, visitor, path, download);
    if let Some(g) = decided {
        let _ = LOCAL.try_with(|local| {
            let mut local = local.lock().unwrap_or_else(|e| e.into_inner());
            if local.decided.len() >= LOCAL_DECIDED {
                local.decided.clear();
            }
            local.decided.insert(visitor, (day, generation, g));
        });
    }
    if CALLS.fetch_add(1, Ordering::Relaxed) % 1024 == 0 {
        settle(hit.now);
    }
}

/// The shared path; returns the visitor's group slot once it's decided.
fn record_shared(hit: &Hit<'_>, day: u64, hour: u64, host: u8, visitor: u64, path: &str, download: bool) -> Option<usize> {
    let recorder = recorder();
    let mut shard = recorder.shards[(visitor % SHARDS as u64) as usize].lock().unwrap_or_else(|e| e.into_inner());
    shard.roll(day, recorder.retention_days);

    let cached = match shard.cache.get(&visitor) {
        Some(cached) => cached.clone(),
        None => {
            let class = classify(hit.ip, hit.user_agent, "/");
            // The owner, then the site's apps, before the network checks:
            // an app on a phone behind a cloud VPN is still the app.
            let header = |name: &str| hit.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.as_ref());
            let owner = class.kind != Kind::Local
                && (super::owner::in_networks(hit.ip) || super::owner::has_cookie(hit.headers) || is_owner(day, hit.ip, hit.user_agent));
            let app = if owner || class.verified || class.kind == Kind::Local { None } else { super::apps::detect(hit.user_agent, header("origin")) };
            let app_version = app.as_ref().and_then(|a| a.version.clone());
            let class = match (owner, app) {
                (true, _) => owner_class(),
                (false, Some(app)) => Class { kind: Kind::App, name: app.name, operator: app.platform, verified: false },
                (false, None) => class,
            };
            // A browser from a hosting network is a program (looked up once
            // per visitor: the class is cached).
            let class = match (class.kind, recorder.hosting) {
                (Kind::Browser, true) => match super::hosting::provider(hit.ip) {
                    Some(provider) => Class { kind: Kind::Hosting, name: provider, operator: "", verified: false },
                    None => class,
                },
                _ => class,
            };
            // Country and device only for browsers: they're shown for people.
            let audience = (class.kind == Kind::Browser).then(|| Audience {
                country: super::geo::country(hit.ip),
                device: {
                    let hint = |name: &str| hit.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.as_ref());
                    super::device::parse(hit.user_agent.unwrap_or(""), hint("sec-ch-ua-platform"), hint("sec-ch-ua-mobile"))
                },
            });
            let cached = Cached { class, audience, app_version };
            if shard.cache.len() >= MAX_CACHE {
                shard.cache.clear();
            }
            shard.cache.insert(visitor, cached.clone());
            cached
        }
    };
    let class = cached.class;
    let scanner = super::classify::scanner_path(hit.path) && matches!(class.kind, Kind::Browser | Kind::Hosting | Kind::Tool | Kind::NoUserAgent);
    let class = if scanner { Class { kind: Kind::Scanner, name: "", operator: "", verified: false } } else { class };
    let page = is_page(hit).then(|| page_key(hit.host, path));
    let not_found = (hit.status == 404).then(|| page_key(hit.host, path));
    let referrer = referrer_host(hit.referer, hit.host);
    // Where a person's page view came from (only browsers become people).
    let arrival = if page.is_some() && class.kind == Kind::Browser { arrival(hit) } else { None };

    let decided = match class.group() {
        Group::Internal => None,
        Group::Person => {
            match shard.seen.get(&visitor).copied() {
                Some(Group::Person) => {
                    shard.count_person(visitor, hit.now, host, page, referrer, not_found, arrival);
                    Some(PEOPLE)
                }
                Some(_) => {
                    shard.count(BOTS, hour, host, &browser_without_assets(), page, not_found, visitor);
                    Some(BOTS)
                }
                None if is_asset(path) => {
                    shard.seen.insert(visitor, Group::Person);
                    shard.new_visitor(day, host, PEOPLE);
                    if let Some(audience) = &cached.audience {
                        shard.count_audience(day, &site_of(hit.host), audience);
                    }
                    if let Some(pending) = shard.pending.remove(&visitor) {
                        for d in pending.requests {
                            shard.count_person(visitor, d.at, d.host, d.page, d.referrer, d.not_found, d.arrival);
                        }
                    }
                    shard.count_person(visitor, hit.now, host, page, referrer, not_found, arrival);
                    Some(PEOPLE)
                }
                None => {
                    if shard.pending.len() >= MAX_PENDING && !shard.pending.contains_key(&visitor) {
                        // Too many undecided at once: count this one as a bot.
                        shard.seen.insert(visitor, Group::Bot);
                        shard.new_visitor(day, host, BOTS);
                        shard.count(BOTS, hour, host, &browser_without_assets(), page, not_found, visitor);
                        Some(BOTS)
                    } else {
                        let pending = shard.pending.entry(visitor).or_insert(Pending { last: hit.now, requests: Vec::new() });
                        pending.last = hit.now;
                        pending.requests.push(Deferred { at: hit.now, hour, host, page, referrer, not_found, arrival });
                        if pending.requests.len() < MAX_PENDING_REQUESTS {
                            None
                        } else {
                            // This many requests and no stylesheet, script or
                            // font: automation, not someone reading pages.
                            let pending = shard.pending.remove(&visitor).expect("just added");
                            shard.seen.insert(visitor, Group::Bot);
                            shard.new_visitor(day, host, BOTS);
                            shard.count_page_only(day, host);
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
                shard.new_visitor(day, host, g);
                let site = host_name(host);
                if g == APPS {
                    shard.count_app_user(day, &site, class.name, class.operator, cached.app_version.as_deref());
                } else if class.kind == Kind::Hosting {
                    shard.day(day).extra[SITE_TIERS].add_capped(&format!("{site}\thosting"), 1, DIM_TOP);
                }
            }
            if scanner {
                shard.count_probe(day, &host_name(host), path, hit.ip);
            }
            shard.count(g, hour, host, &class, page, not_found, visitor);
            // A scanner request isn't the visitor's usual class: not cached.
            (!scanner).then_some(g)
        }
    };
    if download {
        let label = decided.map_or("undecided", group_label);
        shard.day(day).extra[DOWNLOADS].add_capped(&format!("{}\t{label}", page_key(hit.host, path)), 1, DIM_TOP);
    }
    if decided == Some(PEOPLE) && request_goals_configured() {
        let reached = request_goals(path);
        if let (true, Some(session)) = (reached != 0, shard.open.get_mut(&visitor)) {
            session.goals |= reached;
        }
    }
    decided
}

/// The name of a counted group slot.
fn group_label(g: usize) -> &'static str {
    ["people", "ai", "bots", "apps", "owner"].get(g).copied().unwrap_or("other")
}

fn owner_class() -> Class {
    Class { kind: Kind::Owner, name: "", operator: "", verified: false }
}

/// `hash(secret, day, ip, user-agent)`: a client on every site, for the
/// owner's fingerprint.
fn fingerprint(day: u64, ip: IpAddr, user_agent: Option<&str>) -> u64 {
    let mut hasher = recorder().secret.build_hasher();
    "owner".hash(&mut hasher);
    day.hash(&mut hasher);
    ip.hash(&mut hasher);
    user_agent.unwrap_or("").hash(&mut hasher);
    hasher.finish()
}

fn is_owner(day: u64, ip: IpAddr, user_agent: Option<&str>) -> bool {
    let owners = recorder().owners.lock().unwrap_or_else(|e| e.into_inner());
    owners.0 == day && owners.1.contains(&fingerprint(day, ip, user_agent))
}

/// Make the client at `ip` with `user_agent` the owner for the rest of
/// the day, on every site: its visitors on each site are re-decided as the
/// owner (an open visit is dropped, not counted).
#[doc(hidden)]
pub fn mark_owner(ip: IpAddr, user_agent: Option<&str>, now: u64) {
    let day = now / 86_400;
    let fp = fingerprint(day, ip, user_agent);
    {
        let mut owners = recorder().owners.lock().unwrap_or_else(|e| e.into_inner());
        if owners.0 != day {
            *owners = (day, std::collections::HashSet::new());
        }
        // Bounded: a day's owner clients are few; past 1024 something is off.
        if owners.1.len() >= 1024 || !owners.1.insert(fp) {
            return;
        }
    }
    let slots = recorder().hosts.read().unwrap_or_else(|e| e.into_inner()).len();
    for host in 0..slots.max(1) {
        let v = visitor(day, host as u8, ip, user_agent);
        let mut shard = recorder().shards[(v % SHARDS as u64) as usize].lock().unwrap_or_else(|e| e.into_inner());
        if shard.today != day {
            continue;
        }
        shard.pending.remove(&v);
        shard.open.remove(&v);
        shard.seen.insert(v, Group::Owner);
        if shard.cache.len() >= MAX_CACHE {
            shard.cache.clear();
        }
        shard.cache.insert(v, Cached { class: owner_class(), audience: None, app_version: None });
    }
    OWNER_GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// Extensions of files counted as downloads.
const DOWNLOAD_EXTENSIONS: &[&str] = &[
    ".dmg", ".pkg", ".zip", ".tar.gz", ".tgz", ".tar.xz", ".exe", ".msi", ".msix", ".deb", ".rpm", ".appimage", ".apk", ".aab", ".ipa", ".vsix", ".xpi",
    ".crx", ".flatpak", ".snap", ".7z",
];

/// A downloaded file: an installer, package or archive, or an install
/// script (`install.sh`, `install.ps1`), fetched in full.
fn is_download(hit: &Hit<'_>, path: &str) -> bool {
    if hit.status != 200 || hit.method != "GET" {
        return false;
    }
    let last = path.rsplit('/').next().unwrap_or("").as_bytes();
    // Pages, scripts, styles and fonts end in a byte no download ends in.
    if !matches!(last.last().map(u8::to_ascii_lowercase), Some(b'g' | b'e' | b'p' | b'z' | b'b' | b'i' | b'x' | b'k' | b'a' | b'm' | b'h' | b'1')) {
        return false;
    }
    let ends = |ext: &str| last.len() >= ext.len() && last[last.len() - ext.len()..].eq_ignore_ascii_case(ext.as_bytes());
    DOWNLOAD_EXTENSIONS.iter().any(|ext| ends(ext)) || last.eq_ignore_ascii_case(b"install.sh") || last.eq_ignore_ascii_case(b"install.ps1")
}

fn browser_without_assets() -> Class {
    Class { kind: Kind::Browser, name: "browser, no assets", operator: "", verified: false }
}

impl Shard {
    fn day(&mut self, day: u64) -> &mut Day {
        self.days.entry(day).or_default()
    }

    /// A visitor first seen today, in group `g`, on host slot `host`.
    fn new_visitor(&mut self, day: u64, host: u8, g: usize) {
        let name = host_name(host);
        let day = self.day(day);
        day.visitors[g] += 1;
        day.site_visitors.add_capped(&format!("{name}\t{g}"), 1, DIM_TOP);
    }

    /// A new day: forget today's visitors and cache; drop what's past retention.
    fn roll(&mut self, day: u64, retention_days: u64) {
        if self.today == day {
            return;
        }
        self.today = day;
        // Visitors are new each day, so no session continues past midnight.
        self.close_sessions(|_| true);
        self.seen.clear();
        self.cache.clear();
        self.verified.clear();
        self.visit_numbers.clear();
        self.app_reports.clear();
        let oldest_day = day.saturating_sub(retention_days.saturating_sub(1));
        self.days.retain(|d, _| *d >= oldest_day);
        self.hours.retain(|(h, _), _| *h / 24 >= oldest_day);
    }

    /// A person's page view as a database row.
    #[cfg(feature = "model-postgres")]
    fn store_view(&self, visitor: u64, at: u64, page: &str, referrer: Option<&str>) {
        let (host, path) = page.find('/').map_or((page, "/"), |i| (&page[..i], &page[i..]));
        let session = self.open.get(&visitor);
        let audience = self.cache.get(&visitor).and_then(|c| c.audience.as_ref());
        super::store::enqueue(super::store::Row::PageView(super::store::PageView {
            at,
            host: host.to_string(),
            path: path.to_string(),
            visitor,
            session_start: session.map_or(at, |s| s.start),
            referrer_host: referrer.map(str::to_string),
            channel: session.and_then(|s| s.channel),
            utm: session.map(|s| s.utm.clone()).unwrap_or_default(),
            country: audience.and_then(|a| a.country).map(|c| String::from_utf8_lossy(&c).into_owned()),
            browser: audience.map(|a| a.device.browser),
            os: audience.map(|a| a.device.os),
            device: audience.map(|a| a.device.class),
        }));
    }

    /// A person ran the script today: verified (counted once a day).
    fn mark_verified(&mut self, visitor: u64, day: u64, host: &str) {
        if self.verified.insert(visitor) {
            self.day(day).extra[SITE_TIERS].add_capped(&format!("{host}\tverified"), 1, DIM_TOP);
        }
    }

    /// A new app user: by app and platform, and by version when known.
    fn count_app_user(&mut self, day: u64, host: &str, app: &str, platform: &str, version: Option<&str>) {
        let day = self.day(day);
        day.extra[SITE_APPS].add_capped(&format!("{host}\t{app}\t{platform}"), 1, DIM_TOP);
        day.extra[APP_VERSIONS].add_capped(&format!("{host}\t{app}\t{}", version.unwrap_or("(unknown)")), 1, DIM_TOP);
    }

    /// A scanner's request: the path, and where it came from (country and
    /// network; the address too with `[traffic] probe_addresses`).
    fn count_probe(&mut self, day: u64, host: &str, path: &str, ip: IpAddr) {
        let country = super::geo::country(ip).map_or("(unknown)".to_string(), |c| String::from_utf8_lossy(&c).into_owned());
        let network = super::hosting::provider(ip).unwrap_or("-");
        let source = if super::config::probe_addresses() { format!("{host}\t{country}\t{network}\t{}", ip.to_canonical()) } else { format!("{host}\t{country}\t{network}") };
        let day = self.day(day);
        day.extra[SITE_PROBES].add_capped(&format!("{host}\t{}", path.chars().take(200).collect::<String>()), 1, DIM_TOP);
        day.extra[PROBE_SOURCES].add_capped(&source, 1, DIM_TOP);
    }

    /// A browser decided to be no person (it loaded no asset).
    fn count_page_only(&mut self, day: u64, host: u8) {
        let site = host_name(host);
        self.day(day).extra[SITE_TIERS].add_capped(&format!("{site}\tpage-only"), 1, DIM_TOP);
    }

    /// A new person's country, browser, OS and device class.
    fn count_audience(&mut self, day: u64, host: &str, audience: &Audience) {
        let day = self.day(day);
        let country = audience.country.map(|c| String::from_utf8_lossy(&c).into_owned());
        let values = [country.as_deref().unwrap_or("(unknown)"), audience.device.browser, audience.device.os, audience.device.class];
        for (i, value) in values.iter().enumerate() {
            day.dims[COUNTRIES + i].add_capped(&format!("{host}\t{value}"), 1, DIM_TOP);
        }
    }

    /// Close the open sessions `which` picks, adding them to the day they
    /// started.
    fn close_sessions(&mut self, which: impl Fn(&Session) -> bool) {
        let done: Vec<u64> = self.open.iter().filter(|(_, s)| which(s)).map(|(v, _)| *v).collect();
        for visitor in done {
            if let Some(session) = self.open.remove(&visitor) {
                self.close(session);
            }
        }
    }

    fn close(&mut self, mut session: Session) {
        session.verified |= self.verified.contains(&session.visitor);
        let day = self.day(session.start / 86_400);
        add_session(day, &session);
        if day.visits.len() >= MAX_VISITS {
            day.visits.remove(0);
        }
        day.visits.push(session.visit());
    }

    /// A person's page view at `at`: continues their session, or ends it and
    /// starts the next. True when it started one.
    fn session_page(&mut self, visitor: u64, at: u64, page: &str) -> bool {
        if let Some(session) = self.open.get_mut(&visitor) {
            if at.saturating_sub(session.last) < SESSION_GAP {
                session.last = session.last.max(at);
                session.pages += 1;
                session.exit = page.to_string();
                session.goals |= page_goals(page);
                if session.paths.len() < VISIT_PAGES {
                    session.paths.push(path_of(page).to_string());
                }
                return false;
            }
            let ended = self.open.remove(&visitor).expect("just found");
            self.close(ended);
        }
        if self.open.len() >= MAX_OPEN_SESSIONS {
            if let Some(quietest) = self.open.iter().min_by_key(|(_, s)| s.last).map(|(v, _)| *v) {
                let ended = self.open.remove(&quietest).expect("just found");
                self.close(ended);
            }
        }
        self.day(at / 86_400).entry_pages.add(page, 1);
        let audience = self.cache.get(&visitor).and_then(|c| c.audience.clone());
        let number = {
            let n = self.visit_numbers.entry(visitor).or_insert(0);
            *n += 1;
            *n
        };
        self.open.insert(
            visitor,
            Session {
                start: at,
                last: at,
                pages: 1,
                exit: page.to_string(),
                goals: 0,
                reports: 0,
                channel: None,
                utm: Default::default(),
                paths: vec![path_of(page).to_string()],
                visitor,
                audience,
                referrer: None,
                events: Vec::new(),
                number,
                verified: false,
            },
        );
        if let Some(session) = self.open.get_mut(&visitor) {
            session.goals |= page_goals(page);
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn count_person(&mut self, visitor: u64, at: u64, host: u8, page: Option<String>, referrer: Option<String>, not_found: Option<String>, arrival: Option<Arrival>) {
        let hour = at / 3_600;
        // Sources count once per session, from its first page view (as
        // Google Analytics attributes a session).
        let started = page.as_ref().is_some_and(|page| self.session_page(visitor, at, page));
        if let Some(page) = &page {
            let country = self.cache.get(&visitor).and_then(|c| c.audience.as_ref()).and_then(|a| a.country);
            while self.recent.front().is_some_and(|r| r.at + LIVE_WINDOW < at) || self.recent.len() >= MAX_RECENT {
                self.recent.pop_front();
            }
            self.recent.push_back(Recent { at, visitor, page: page.clone(), country });
        }
        let arrival = arrival.filter(|_| started);
        if let (Some(arrival), Some(session)) = (&arrival, self.open.get_mut(&visitor)) {
            session.channel = Some(arrival.channel);
            session.utm = arrival.utm.clone();
            session.referrer = referrer.clone();
        }
        #[cfg(feature = "model-postgres")]
        if let (true, Some(page)) = (super::store::enabled(), &page) {
            self.store_view(visitor, at, page, referrer.as_deref());
        }
        let counts = &mut self.hours.entry((hour, host)).or_default()[PEOPLE];
        counts.requests += 1;
        counts.pages += page.is_some() as u64;
        let day = self.day(hour / 24);
        if let (Some(arrival), Some(page)) = (arrival, &page) {
            // The page key starts with the host.
            let host = page.split('/').next().unwrap_or("");
            day.dims[CHANNELS].add_capped(&format!("{host}\t{}", arrival.channel), 1, DIM_TOP);
            day.extra[SITE_ENTRIES].add_capped(&format!("{page}\t{}", arrival.channel), 1, DIM_TOP);
            for (i, value) in arrival.utm.iter().enumerate() {
                if let Some(value) = value {
                    day.dims[UTM + i].add_capped(&format!("{host}\t{value}"), 1, DIM_TOP);
                }
            }
        }
        if let Some(page) = page {
            day.people_pages.add(&page, 1);
            if let Some(referrer) = referrer {
                let host = page.split('/').next().unwrap_or("");
                day.site_referrers.add_capped(&format!("{host}\t{referrer}"), 1, DIM_TOP);
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
        let name = host_name(host);
        let day = self.day(hour / 24);
        if (g == AI || g == BOTS) && !class.name.is_empty() && !matches!(class.kind, Kind::Hosting | Kind::Browser) {
            // What named crawlers fetch: pages, and the paths that 404 for them.
            if page.is_some() {
                day.extra[SITE_CRAWL].add_capped(&format!("{name}\t{}\tpages", class.name), 1, DIM_TOP);
            }
            if let Some(path) = &not_found {
                day.extra[SITE_CRAWL].add_capped(&format!("{name}\t{}\t404", class.name), 1, DIM_TOP);
                day.extra[CRAWL_404].add_capped(&format!("{path}\t{}", class.name), 1, DIM_TOP);
            }
        }
        if g == AI {
            day.site_ai_agents.add_capped(&format!("{name}\t{}\t{}\t{}\t{}", or_unknown(class.name), class.operator, class.kind.label(), class.verified), 1, DIM_TOP);
            if let Some(page) = page {
                day.ai_pages.add(&page, 1);
            }
        } else if g == OWNER {
            if let Some(page) = page {
                day.extra[OWNER_PAGES].add(&page, 1);
            }
            // The owner's 404s are still the site's.
        } else if g == BOTS {
            let kind = if class.kind == Kind::Browser { "browser-no-assets" } else { class.kind.label() };
            day.site_bots.add_capped(&format!("{name}\t{kind}\t{}", class.name), 1, DIM_TOP);
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
#[doc(hidden)]
pub fn settle(now: u64) {
    for shard in &recorder().shards {
        let mut shard = shard.lock().unwrap_or_else(|e| e.into_inner());
        shard.close_sessions(|s| now.saturating_sub(s.last) >= SESSION_GAP);
        let done: Vec<u64> = shard.pending.iter().filter(|(_, p)| now.saturating_sub(p.last) >= UNDECIDED.as_secs()).map(|(v, _)| *v).collect();
        for visitor in done {
            let Some(pending) = shard.pending.remove(&visitor) else { continue };
            let day = pending.requests.first().map_or(now / 86_400, |d| d.hour / 24);
            let host = pending.requests.first().map_or(0, |d| d.host);
            if shard.seen.insert(visitor, Group::Bot).is_none() {
                shard.new_visitor(day, host, BOTS);
                shard.count_page_only(day, host);
            }
            for d in pending.requests {
                shard.count(BOTS, d.hour, d.host, &browser_without_assets(), d.page, d.not_found, visitor);
            }
        }
    }
}

/// Everything counted, merged across shards: `(hour, host) → counts` and
/// `day → lists` (after settling quiet browsers as of `now`).
#[doc(hidden)]
pub fn snapshot(now: u64) -> (BTreeMap<(u64, u8), [Counts; GROUPS]>, BTreeMap<u64, Day>) {
    settle(now);
    drain_locals();
    let mut hours: BTreeMap<(u64, u8), [Counts; GROUPS]> = BTreeMap::new();
    let mut days: BTreeMap<u64, Day> = BTreeMap::new();
    for shard in &recorder().shards {
        let shard = shard.lock().unwrap_or_else(|e| e.into_inner());
        for (key, counts) in &shard.hours {
            let entry = hours.entry(*key).or_default();
            for g in 0..GROUPS {
                entry[g].requests += counts[g].requests;
                entry[g].pages += counts[g].pages;
            }
        }
        for (day, lists) in &shard.days {
            days.entry(*day).or_default().merge(lists);
        }
        // Open sessions count as if they ended now (they stay open).
        for session in shard.open.values() {
            let day = days.entry(session.start / 86_400).or_default();
            add_session(day, session);
            let mut visit = session.visit();
            visit.verified |= shard.verified.contains(&session.visitor);
            day.visits.push(visit);
        }
    }
    (hours, days)
}

/// A closed (or, in a snapshot, open) session's totals, exit page and goals
/// into its day.
fn add_session(day: &mut Day, session: &Session) {
    let host = session.exit.split('/').next().unwrap_or("").to_string();
    day.sessions.entry(host.clone()).or_default().add(&session.totals());
    day.exit_pages.add(&session.exit, 1);
    if session.goals != 0 {
        for (i, goal) in goals().iter().enumerate() {
            if session.goals & (1 << i) != 0 {
                day.behaviour[GOALS].add_capped(&format!("{host}\t{goal}"), 1, DIM_TOP);
            }
        }
    }
}

/// Whether any goal is a request goal: unknown (`u64::MAX`), 0 or 1.
static REQUEST_GOALS: AtomicU64 = AtomicU64::new(u64::MAX);

fn goals_lock() -> &'static RwLock<Vec<String>> {
    static GOALS_SET: OnceLock<RwLock<Vec<String>>> = OnceLock::new();
    GOALS_SET.get_or_init(|| RwLock::new(super::config::goals()))
}

/// The configured goals (read from the settings once).
fn goals() -> std::sync::RwLockReadGuard<'static, Vec<String>> {
    goals_lock().read().unwrap_or_else(|e| e.into_inner())
}

/// Replace the goals (tests).
#[cfg(test)]
#[doc(hidden)]
pub fn set_goals_for_tests(goals: &[&str]) {
    *goals_lock().write().unwrap_or_else(|e| e.into_inner()) = goals.iter().map(|g| g.to_string()).collect();
    REQUEST_GOALS.store(u64::MAX, Ordering::Relaxed);
}

/// The path of a page key (`host/path`).
fn path_of(page: &str) -> &str {
    page.find('/').map_or("/", |i| &page[i..])
}

/// The goals a page view (`host/path`) reaches.
fn page_goals(page: &str) -> u32 {
    let path = page.find('/').map_or("/", |i| &page[i..]);
    let mut reached = 0;
    for (i, goal) in goals().iter().enumerate() {
        let hit = match goal.strip_suffix('*') {
            _ if goal.starts_with("event:") || goal.starts_with("req:") => false,
            Some(prefix) => path.starts_with(prefix),
            None => path == goal,
        };
        if hit {
            reached |= 1 << i;
        }
    }
    reached
}

/// Whether any goal is a request (`req:/api/proxy`): then people's
/// requests other than pages are checked against them.
fn request_goals_configured() -> bool {
    match REQUEST_GOALS.load(Ordering::Relaxed) {
        u64::MAX => {
            let any = goals().iter().any(|g| g.starts_with("req:"));
            REQUEST_GOALS.store(any as u64, Ordering::Relaxed);
            any
        }
        n => n == 1,
    }
}

/// The request goals (`req:/path`, or `req:/prefix*`) a request path reaches.
fn request_goals(path: &str) -> u32 {
    let mut reached = 0;
    for (i, goal) in goals().iter().enumerate() {
        let Some(want) = goal.strip_prefix("req:") else { continue };
        let hit = match want.strip_suffix('*') {
            Some(prefix) => path.starts_with(prefix),
            None => path == want,
        };
        if hit {
            reached |= 1 << i;
        }
    }
    reached
}

/// The goals an event reaches.
fn event_goals(name: &str) -> u32 {
    let mut reached = 0;
    for (i, goal) in goals().iter().enumerate() {
        if goal.strip_prefix("event:") == Some(name) {
            reached |= 1 << i;
        }
    }
    reached
}

fn json_string(raw: &str) -> Option<String> {
    crate::mcp::json_rpc::extract_str(&format!("{{\"v\":{raw}}}"), "v")
}

fn width_bucket(width: u64) -> &'static str {
    match width {
        0 => "(unknown)",
        1..=599 => "phone",
        600..=1023 => "tablet",
        1024..=1439 => "laptop",
        _ => "wide",
    }
}

/// A report from the first-party script (`/_rws/t`): `{"t":"leave"|"view"|
/// "event", …}`. Counted only for a visitor already counted as a person
/// today — a client that never loaded the page's assets can't post reports
/// in.
#[doc(hidden)]
pub fn beacon(ip: IpAddr, raw_host: &str, user_agent: Option<&str>, body: &str, now: u64) {
    if !enabled() {
        return;
    }
    let Some(members) = crate::mcp::json_rpc::object_members(body) else { return };
    let get = |key: &str| members.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    let number = |key: &str| get(key).and_then(|v| v.trim().parse::<f64>().ok()).filter(|n| n.is_finite() && *n >= 0.0).map(|n| n as u64);
    let Some(kind) = get("t").and_then(json_string) else { return };
    let Some(path) = get("p").and_then(json_string).filter(|p| p.starts_with('/') && p.len() <= 1000) else { return };
    let host = host_index(raw_host);
    let day = now / 86_400;
    let visitor = visitor(day, host, ip, user_agent);
    let host_name = site_of(raw_host);

    if kind == "view" {
        // A single-page app's route change: a page view like any other,
        // arriving from the page before (same site, so not a new arrival).
        let from = get("r").and_then(json_string).filter(|r| r.starts_with('/')).unwrap_or_else(|| "/".to_string());
        let referer = format!("https://{host_name}{from}");
        let is_person = {
            let mut shard = recorder().shards[(visitor % SHARDS as u64) as usize].lock().unwrap_or_else(|e| e.into_inner());
            let person = shard.seen.get(&visitor) == Some(&Group::Person);
            if person {
                shard.mark_verified(visitor, day, &host_name);
            }
            person
        };
        if is_person {
            record(&Hit {
                ip,
                host: raw_host,
                user_agent,
                referer: Some(&referer),
                method: "GET",
                path: &path,
                status: 200,
                content_type: Some("text/html"),
                headers: &[],
                now,
            });
        }
        return;
    }

    let mut shard = recorder().shards[(visitor % SHARDS as u64) as usize].lock().unwrap_or_else(|e| e.into_inner());
    if shard.seen.get(&visitor) != Some(&Group::Person) {
        return;
    }
    shard.mark_verified(visitor, day, &host_name);
    if let Some(session) = shard.open.get_mut(&visitor) {
        session.reports += 1;
        if session.reports > MAX_REPORTS {
            return;
        }
    }
    let page = page_key(raw_host, &path);
    match kind.as_str() {
        "leave" => {
            // A report without a usable time isn't one.
            let Some(ms) = number("ms") else { return };
            let secs = ms.min(MAX_ENGAGED_SECS * 1000) / 1000;
            let depth = number("sd").unwrap_or(0).min(100);
            let width = number("w").unwrap_or(0);
            let bucket = match depth {
                0..=25 => "25",
                26..=50 => "50",
                51..=75 => "75",
                _ => "100",
            };
            #[cfg(feature = "model-postgres")]
            super::store::enqueue(super::store::Row::Report(super::store::Report {
                at: now,
                host: host_name.clone(),
                path: path.clone(),
                visitor,
                kind: "leave",
                secs: Some(secs),
                scroll: Some(depth),
                width: Some(width),
                ..Default::default()
            }));
            let d = shard.day(day);
            d.behaviour[ENGAGED_SECS].add(&page, secs);
            d.behaviour[ENGAGED_VIEWS].add(&page, 1);
            d.behaviour[SCROLL].add_capped(&format!("{host_name}\t{bucket}"), 1, DIM_TOP);
            d.behaviour[SCREENS].add_capped(&format!("{host_name}\t{}", width_bucket(width)), 1, DIM_TOP);
        }
        "event" => {
            let Some(name) = get("n").and_then(json_string).map(|n| n.chars().filter(|c| !c.is_control()).take(64).collect::<String>()).filter(|n| !n.is_empty()) else {
                return;
            };
            let props: Vec<(String, String)> = get("q")
                .and_then(crate::mcp::json_rpc::object_members)
                .unwrap_or_default()
                .into_iter()
                .take(4)
                .filter_map(|(k, v)| Some((k.chars().take(32).collect::<String>(), json_string(&v)?.chars().take(64).collect::<String>())))
                .collect();
            let reached = event_goals(&name);
            if let Some(session) = shard.open.get_mut(&visitor) {
                session.goals |= reached;
                if session.events.len() < 10 {
                    session.events.push(name.clone());
                }
            }
            #[cfg(feature = "model-postgres")]
            super::store::enqueue(super::store::Row::Report(super::store::Report {
                at: now,
                host: host_name.clone(),
                path: path.clone(),
                visitor,
                kind: "event",
                name: Some(name.clone()),
                props: (!props.is_empty()).then(|| props.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&")),
                ..Default::default()
            }));
            let d = shard.day(day);
            d.behaviour[EVENTS].add_capped(&format!("{host_name}\t{name}"), 1, DIM_TOP);
            for (k, v) in props {
                d.behaviour[EVENT_PROPS].add_capped(&format!("{host_name}\t{name}\t{k}={v}"), 1, DIM_TOP);
            }
        }
        _ => {}
    }
}

/// Platforms an app may report in an event (`p`); others are `other`.
const APP_PLATFORMS: &[&str] = &["ios", "ipados", "macos", "visionos", "watchos", "android", "windows", "linux", "chrome", "edge", "firefox", "safari", "web", "other"];

/// Why an app event was refused.
#[derive(Debug, PartialEq, Eq)]
#[doc(hidden)]
pub enum AppEventError {
    /// Not JSON, or without `app` and `n`.
    Malformed,
    /// `app` isn't one of `[traffic] apps`.
    UnknownApp,
}

/// An event from one of the site's apps (`POST /_rws/e`, 18.35):
/// `{"app":"HITOP","v":"2.3.1","p":"ios","n":"request_sent"}`. Native apps
/// and extensions don't load pages, so unlike the script's reports these
/// aren't tied to a person: the client counts as a user of `app` (by
/// `p` platform and `v` version) and the event under its name. Only
/// configured apps are taken, at most [`MAX_REPORTS`] events per client a
/// day; nothing is stored in the app.
#[doc(hidden)]
pub fn app_event(ip: IpAddr, raw_host: &str, user_agent: Option<&str>, body: &str, now: u64) -> Result<(), AppEventError> {
    if !enabled() {
        return Ok(());
    }
    let members = crate::mcp::json_rpc::object_members(body).ok_or(AppEventError::Malformed)?;
    let get = |key: &str| members.iter().find(|(k, _)| k == key).and_then(|(_, v)| json_string(v));
    let clean = |v: String, n: usize| v.chars().filter(|c| !c.is_control() && *c != '\t').take(n).collect::<String>();
    let app = get("app").ok_or(AppEventError::Malformed)?;
    let name = get("n").map(|n| clean(n, 64)).filter(|n| !n.is_empty()).ok_or(AppEventError::Malformed)?;
    let app = super::apps::configured(&app).ok_or(AppEventError::UnknownApp)?;
    let platform = get("p").map(|p| p.to_ascii_lowercase()).map_or("other", |p| APP_PLATFORMS.iter().copied().find(|k| *k == p).unwrap_or("other"));
    let version = get("v").map(|v| clean(v, 32)).filter(|v| !v.is_empty());
    let day = now / 86_400;
    let hour = now / 3_600;
    let host = host_index(raw_host);
    let visitor = visitor(day, host, ip, user_agent);
    let site = host_name(host);
    let recorder = recorder();
    let mut shard = recorder.shards[(visitor % SHARDS as u64) as usize].lock().unwrap_or_else(|e| e.into_inner());
    shard.roll(day, recorder.retention_days);
    let reports = shard.app_reports.entry(visitor).or_insert(0);
    *reports += 1;
    if *reports > MAX_REPORTS {
        return Ok(());
    }
    match shard.seen.get(&visitor).copied() {
        // The owner's own app use stays the owner's.
        Some(Group::Owner) => return Ok(()),
        Some(Group::App) => {}
        _ => {
            if shard.seen.insert(visitor, Group::App).is_none() {
                shard.new_visitor(day, host, APPS);
            }
            shard.count_app_user(day, &site, app, platform, version.as_deref());
            shard.hours.entry((hour, host)).or_default()[APPS].requests += 1;
        }
    }
    shard.day(day).extra[APP_EVENTS].add_capped(&format!("{site}\t{app}\t{name}"), 1, DIM_TOP);
    Ok(())
}

/// People's page views of the last [`LIVE_WINDOW`] before `now`, from every
/// shard (unordered). Visitor keys are the day's hashes: comparable within
/// the result, meaningless outside it.
#[doc(hidden)]
pub fn recent(now: u64) -> Vec<Recent> {
    settle(now);
    let mut out = Vec::new();
    for shard in &recorder().shards {
        let shard = shard.lock().unwrap_or_else(|e| e.into_inner());
        out.extend(shard.recent.iter().filter(|r| r.at + LIVE_WINDOW >= now && r.at <= now).cloned());
    }
    out
}

/// Put back what [`snapshot`] gave (loading a saved file): counts are added
/// to shard 0.
#[doc(hidden)]
pub fn restore(hosts: Vec<String>, hours: BTreeMap<(u64, u8), [Counts; GROUPS]>, days: BTreeMap<u64, Day>) {
    let recorder = recorder();
    let configured = recorder.configured.read().unwrap_or_else(|e| e.into_inner());
    let mut known = recorder.hosts.write().unwrap_or_else(|e| e.into_inner());
    // Configured sites first, so the hosts saved before them can't take
    // their slots.
    let mut order: Vec<usize> = (0..hosts.len()).collect();
    order.sort_by_key(|i| configured.binary_search(&hosts[*i]).is_err());
    let mut map = vec![0u8; hosts.len()];
    for i in order {
        map[i] = host_slot(&mut known, &configured, hosts[i].clone());
    }
    // Saved names that now count as another site (`www.`, unknown hosts):
    // their list keys move there too.
    let rename: HashMap<String, String> = hosts
        .iter()
        .enumerate()
        .filter_map(|(i, h)| known.get(map[i] as usize).filter(|n| *n != h).map(|n| (h.clone(), n.clone())))
        .collect();
    drop(known);
    drop(configured);
    HOSTS_GENERATION.fetch_add(1, Ordering::Relaxed);
    let mut shard = recorder.shards[0].lock().unwrap_or_else(|e| e.into_inner());
    for ((hour, host), counts) in hours {
        let host = map.get(host as usize).copied().unwrap_or(0);
        let entry = shard.hours.entry((hour, host)).or_default();
        for g in 0..GROUPS {
            entry[g].requests += counts[g].requests;
            entry[g].pages += counts[g].pages;
        }
    }
    for (day, mut lists) in days {
        if !rename.is_empty() {
            rename_hosts(&mut lists, &rename);
        }
        shard.days.entry(day).or_default().merge(&lists);
    }
}

/// Move a day's list keys (`host/…`, `host\t…`), sessions and visits from
/// the old host names in `rename` to the new ones, adding up what meets.
fn rename_hosts(day: &mut Day, rename: &HashMap<String, String>) {
    let rekey = |top: &mut Top| {
        if !top.0.keys().any(|k| rename.contains_key(&k[..k.find(['/', '\t']).unwrap_or(k.len())])) {
            return;
        }
        let old = std::mem::take(&mut top.0);
        for (key, n) in old {
            let at = key.find(['/', '\t']).unwrap_or(key.len());
            let key = match rename.get(&key[..at]) {
                Some(new) => format!("{new}{}", &key[at..]),
                None => key,
            };
            *top.0.entry(key).or_default() += n;
        }
    };
    for top in [
        &mut day.people_pages,
        &mut day.ai_pages,
        &mut day.site_visitors,
        &mut day.site_ai_agents,
        &mut day.site_bots,
        &mut day.site_referrers,
        &mut day.not_found,
        &mut day.entry_pages,
        &mut day.exit_pages,
    ] {
        rekey(top);
    }
    day.dims.iter_mut().for_each(rekey);
    day.behaviour.iter_mut().for_each(rekey);
    day.extra.iter_mut().for_each(rekey);
    for (old, new) in rename {
        if let Some(s) = day.sessions.remove(old) {
            day.sessions.entry(new.clone()).or_default().add(&s);
        }
    }
    for v in &mut day.visits {
        if let Some(new) = rename.get(&v.host) {
            v.host = new.clone();
        }
    }
}

#[cfg(test)]
#[doc(hidden)]
pub fn disable_for_tests() {
    ENABLED.store(false, Ordering::Relaxed);
}

#[cfg(test)]
#[doc(hidden)]
pub fn reset_for_tests() {
    let recorder = recorder();
    HOSTS_GENERATION.fetch_add(1, Ordering::Relaxed);
    for local in locals().lock().unwrap_or_else(|e| e.into_inner()).iter().filter_map(|w| w.upgrade()) {
        *local.lock().unwrap_or_else(|e| e.into_inner()) = Local::default();
    }
    for shard in &recorder.shards {
        *shard.lock().unwrap_or_else(|e| e.into_inner()) = Shard::default();
    }
    recorder.hosts.write().unwrap_or_else(|e| e.into_inner()).clear();
    recorder.configured.write().unwrap_or_else(|e| e.into_inner()).clear();
}
