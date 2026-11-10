//! Saving the counts so restarts and upgrades keep them: a small JSON file,
//! written to a temporary file and renamed, only when the disk has room
//! (`disk_guard`), read back at start.
//!
//! ```text
//! {"version":1,"hosts":["example.com"],
//!  "hours":[[hour, host, people req, people pages, ai req, ai pages, bot req, bot pages], …],
//!  "days":[{"day":N,"visitors":[p,a,b],"people_pages":[["example.com/",12],…],…,
//!           "countries":[["example.com\tDE",3],…],…}, …]}
//! ```
//!
//! The audience and source lists (`countries` … `utm_campaign`, 18.31) are
//! additive: older files load without them, older rws ignores them.
//!
//! The per-site lists (`site_visitors`, `site_ai_agents`, `site_bots`,
//! `site_referrers`, 18.35) carry the host as the key's first
//! tab-separated field; they replace `ai_agents`, `bots` and `referrers`,
//! which only days saved before them have (shown for all sites). New
//! names, so an older rws reading the file ignores them rather than
//! showing a host as a bot's name.
//!
//! Apps and the owner (18.35) are counted groups 4 and 5. Their hourly
//! counts go in `app_hours` (`[hour, host, apps req, apps pages, owner
//! req, owner pages]`) and their visitors in `more_visitors` (`[apps,
//! owner]`), so the `hours` rows keep their 8 numbers and an older rws
//! still reads them. The [`EXTRA`] lists and the recent `visits` are new
//! keys as well.
//!
//! Unique-visitor keys are never saved: a restart within a day can count a
//! returning visitor once more.

use std::collections::BTreeMap;
use std::path::Path;

use super::record::{Counts, Day, Top, Visit, BEHAVIOUR, DIMS, DIM_SAVED, EXTRA, GROUPS, MAX_VISITS, TOP_SAVED, VISIT_PAGES};
use crate::mcp::json_rpc::{extract_raw, extract_str, split_array_elements};

const LISTS: [&str; 12] = [
    "people_pages",
    "ai_pages",
    "ai_agents",
    "bots",
    "referrers",
    "not_found",
    "entry_pages",
    "exit_pages",
    "site_visitors",
    "site_ai_agents",
    "site_bots",
    "site_referrers",
];

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn list<'a>(day: &'a Day, name: &str) -> &'a Top {
    if let Some(i) = DIMS.iter().position(|d| *d == name) {
        return &day.dims[i];
    }
    if let Some(i) = BEHAVIOUR.iter().position(|d| *d == name) {
        return &day.behaviour[i];
    }
    if let Some(i) = EXTRA.iter().position(|d| *d == name) {
        return &day.extra[i];
    }
    match name {
        "people_pages" => &day.people_pages,
        "ai_pages" => &day.ai_pages,
        "ai_agents" => &day.ai_agents,
        "bots" => &day.bots,
        "referrers" => &day.referrers,
        "entry_pages" => &day.entry_pages,
        "exit_pages" => &day.exit_pages,
        "site_visitors" => &day.site_visitors,
        "site_ai_agents" => &day.site_ai_agents,
        "site_bots" => &day.site_bots,
        "site_referrers" => &day.site_referrers,
        _ => &day.not_found,
    }
}

fn list_mut<'a>(day: &'a mut Day, name: &str) -> &'a mut Top {
    if let Some(i) = DIMS.iter().position(|d| *d == name) {
        return &mut day.dims[i];
    }
    if let Some(i) = BEHAVIOUR.iter().position(|d| *d == name) {
        return &mut day.behaviour[i];
    }
    if let Some(i) = EXTRA.iter().position(|d| *d == name) {
        return &mut day.extra[i];
    }
    match name {
        "people_pages" => &mut day.people_pages,
        "ai_pages" => &mut day.ai_pages,
        "ai_agents" => &mut day.ai_agents,
        "bots" => &mut day.bots,
        "referrers" => &mut day.referrers,
        "entry_pages" => &mut day.entry_pages,
        "exit_pages" => &mut day.exit_pages,
        "site_visitors" => &mut day.site_visitors,
        "site_ai_agents" => &mut day.site_ai_agents,
        "site_bots" => &mut day.site_bots,
        "site_referrers" => &mut day.site_referrers,
        _ => &mut day.not_found,
    }
}

/// The counts as the saved file's text.
#[doc(hidden)]
pub fn to_json(hosts: &[String], hours: &BTreeMap<(u64, u8), [Counts; GROUPS]>, days: &BTreeMap<u64, Day>) -> String {
    let mut out = String::from("{\"version\":1,\"hosts\":[");
    out.push_str(&hosts.iter().map(|h| quote(h)).collect::<Vec<_>>().join(","));
    out.push_str("],\"hours\":[");
    let rows: Vec<String> = hours
        .iter()
        .map(|((hour, host), c)| format!("[{hour},{host},{},{},{},{},{},{}]", c[0].requests, c[0].pages, c[1].requests, c[1].pages, c[2].requests, c[2].pages))
        .collect();
    out.push_str(&rows.join(","));
    out.push_str("],\"app_hours\":[");
    let rows: Vec<String> = hours
        .iter()
        .filter(|(_, c)| c[3].requests + c[3].pages + c[4].requests + c[4].pages > 0)
        .map(|((hour, host), c)| format!("[{hour},{host},{},{},{},{}]", c[3].requests, c[3].pages, c[4].requests, c[4].pages))
        .collect();
    out.push_str(&rows.join(","));
    out.push_str("],\"days\":[");
    // The newest visits, at most MAX_VISITS across all days.
    let mut keep = MAX_VISITS;
    let mut visits_kept: BTreeMap<u64, usize> = BTreeMap::new();
    for (n, day) in days.iter().rev() {
        let k = day.visits.len().min(keep);
        visits_kept.insert(*n, k);
        keep -= k;
    }
    let days: Vec<String> = days
        .iter()
        .map(|(n, day)| {
            let lists: Vec<String> = LISTS
                .iter()
                .chain(DIMS.iter())
                .chain(BEHAVIOUR.iter())
                .chain(EXTRA.iter())
                .map(|name| {
                    let keep = if DIMS.contains(name) || BEHAVIOUR.contains(name) || EXTRA.contains(name) || name.starts_with("site_") { DIM_SAVED } else { TOP_SAVED };
                    let entries: Vec<String> = list(day, name).sorted(keep).iter().map(|(k, v)| format!("[{},{v}]", quote(k))).collect();
                    format!("{}:[{}]", quote(name), entries.join(","))
                })
                .collect();
            let sessions: Vec<String> =
                day.sessions.iter().map(|(host, t)| format!("[{},{},{},{},{}]", quote(host), t.sessions, t.bounces, t.duration, t.pages)).collect();
            let mut visits = day.visits.clone();
            visits.sort_by_key(|v| v.end);
            let skip = visits.len() - visits_kept.get(n).copied().unwrap_or(0);
            let visits: Vec<String> = visits[skip..].iter().map(visit_json).collect();
            format!(
                "{{\"day\":{n},\"visitors\":[{},{},{}],\"more_visitors\":[{},{}],\"sessions\":[{}],{},\"visits\":[{}]}}",
                day.visitors[0],
                day.visitors[1],
                day.visitors[2],
                day.visitors[3],
                day.visitors[4],
                sessions.join(","),
                lists.join(","),
                visits.join(",")
            )
        })
        .collect();
    out.push_str(&days.join(","));
    out.push_str("]}\n");
    out
}

fn strings(list: &[String]) -> String {
    format!("[{}]", list.iter().map(|s| quote(s)).collect::<Vec<_>>().join(","))
}

/// A visit as saved (and as the API reports it).
#[doc(hidden)]
pub fn visit_json(v: &Visit) -> String {
    format!(
        r#"{{"start":{},"end":{},"host":{},"views":{},"pages":{},"country":{},"browser":{},"os":{},"device":{},"channel":{},"referrer":{},"campaign":{},"verified":{},"number":{},"events":{}}}"#,
        v.start,
        v.end,
        quote(&v.host),
        v.views,
        strings(&v.pages),
        quote(&v.country),
        quote(&v.browser),
        quote(&v.os),
        quote(&v.device),
        quote(&v.channel),
        quote(&v.referrer),
        quote(&v.campaign),
        v.verified,
        v.number,
        strings(&v.events),
    )
}

fn visit_from_json(object: &str) -> Option<Visit> {
    let text = |key: &str| extract_str(object, key).unwrap_or_default().chars().take(300).collect::<String>();
    let number = |key: &str| extract_raw(object, key).and_then(|v| v.trim().parse::<u64>().ok());
    let list = |key: &str, n: usize| -> Vec<String> {
        split_array_elements(&extract_raw(object, key).unwrap_or_default())
            .iter()
            .filter_map(|v| extract_str(&format!("{{\"v\":{v}}}"), "v"))
            .take(n)
            .collect()
    };
    Some(Visit {
        start: number("start")?,
        end: number("end")?,
        host: text("host"),
        views: number("views").unwrap_or(0),
        pages: list("pages", VISIT_PAGES),
        country: text("country"),
        browser: text("browser"),
        os: text("os"),
        device: text("device"),
        channel: text("channel"),
        referrer: text("referrer"),
        campaign: text("campaign"),
        verified: extract_raw(object, "verified").is_some_and(|v| v.trim() == "true"),
        number: number("number").unwrap_or(1) as u32,
        events: list("events", 10),
    })
}

fn numbers(json: &str) -> Vec<u64> {
    split_array_elements(json).iter().filter_map(|n| n.trim().parse().ok()).collect()
}

/// Read a saved file's text; `None` when it isn't one.
#[allow(clippy::type_complexity)]
#[doc(hidden)]
pub fn from_json(text: &str) -> Option<(Vec<String>, BTreeMap<(u64, u8), [Counts; GROUPS]>, BTreeMap<u64, Day>)> {
    if extract_raw(text, "version")?.trim() != "1" {
        return None;
    }
    let hosts: Vec<String> = split_array_elements(&extract_raw(text, "hosts")?)
        .iter()
        .filter_map(|h| extract_str(&format!("{{\"v\":{h}}}"), "v"))
        .collect();
    let mut hours = BTreeMap::new();
    for row in split_array_elements(&extract_raw(text, "hours")?) {
        let n = numbers(&row);
        if n.len() == 8 && (n[1] as usize) < hosts.len() {
            let entry: &mut [Counts; GROUPS] = hours.entry((n[0], n[1] as u8)).or_default();
            entry[0] = Counts { requests: n[2], pages: n[3] };
            entry[1] = Counts { requests: n[4], pages: n[5] };
            entry[2] = Counts { requests: n[6], pages: n[7] };
        }
    }
    for row in split_array_elements(&extract_raw(text, "app_hours").unwrap_or_default()) {
        let n = numbers(&row);
        if n.len() == 6 && (n[1] as usize) < hosts.len() {
            let entry: &mut [Counts; GROUPS] = hours.entry((n[0], n[1] as u8)).or_default();
            entry[3] = Counts { requests: n[2], pages: n[3] };
            entry[4] = Counts { requests: n[4], pages: n[5] };
        }
    }
    let mut days = BTreeMap::new();
    for object in split_array_elements(&extract_raw(text, "days")?) {
        let Some(n) = extract_raw(&object, "day").and_then(|d| d.trim().parse::<u64>().ok()) else { continue };
        let mut day = Day::default();
        let visitors = numbers(&extract_raw(&object, "visitors").unwrap_or_default());
        for (g, v) in visitors.iter().take(3).enumerate() {
            day.visitors[g] = *v;
        }
        let more = numbers(&extract_raw(&object, "more_visitors").unwrap_or_default());
        for (g, v) in more.iter().take(2).enumerate() {
            day.visitors[3 + g] = *v;
        }
        // Last in the object: its members' keys (`events`, `pages`) are
        // found after the day's lists of the same names.
        for v in split_array_elements(&extract_raw(&object, "visits").unwrap_or_default()).iter().take(MAX_VISITS) {
            if let Some(visit) = visit_from_json(v) {
                day.visits.push(visit);
            }
        }
        for row in split_array_elements(&extract_raw(&object, "sessions").unwrap_or_default()) {
            let parts = split_array_elements(&row);
            let n: Vec<u64> = parts.iter().skip(1).filter_map(|v| v.trim().parse().ok()).collect();
            if let (Some(host), 4) = (parts.first().and_then(|h| extract_str(&format!("{{\"v\":{h}}}"), "v")), n.len()) {
                day.sessions.insert(host, super::record::Sessions { sessions: n[0], bounces: n[1], duration: n[2], pages: n[3] });
            }
        }
        for name in LISTS.iter().chain(DIMS.iter()).chain(BEHAVIOUR.iter()).chain(EXTRA.iter()) {
            let raw = extract_raw(&object, name).unwrap_or_default();
            for pair in split_array_elements(&raw) {
                let parts = split_array_elements(&pair);
                if parts.len() != 2 {
                    continue;
                }
                let (Some(key), Ok(count)) = (extract_str(&format!("{{\"v\":{}}}", parts[0]), "v"), parts[1].trim().parse::<u64>()) else { continue };
                list_mut(&mut day, name).0.insert(key, count);
            }
        }
        days.insert(n, day);
    }
    Some((hosts, hours, days))
}

/// Write the current counts to `path` (temporary file + rename); skipped
/// when the disk is low.
#[doc(hidden)]
pub fn save(path: &Path) -> std::io::Result<()> {
    if !crate::disk_guard::has_room(path) {
        return Ok(());
    }
    let (hours, days) = super::record::snapshot(super::record::now());
    let text = to_json(&super::record::host_names(), &hours, &days);
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    write_private(&tmp, text.as_bytes())?;
    std::fs::rename(&tmp, path)
}

/// Write a new file only its owner can read (0600 on Unix): the counts
/// name pages and referrers, which other local users needn't see.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(bytes)
}

/// Load `path` into the counts; a missing file is fine, an unreadable one
/// is reported and ignored.
#[doc(hidden)]
pub fn load(path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    match from_json(&text) {
        Some((hosts, hours, days)) => super::record::restore(hosts, hours, days),
        None => eprintln!("traffic: {} isn't a traffic file this rws can read; starting empty", path.display()),
    }
}
