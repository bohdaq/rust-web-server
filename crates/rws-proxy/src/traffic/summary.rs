//! The Traffic page's data: totals for a period and the one before it, a
//! series (hourly up to 48 hours, daily beyond), and the top lists.

use std::collections::BTreeMap;

use super::record::{snapshot, Day, Sessions, Top, DIMS};

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn pairs(top: &Top, n: usize, host: Option<&str>) -> String {
    let mut filtered = Top::default();
    for (key, count) in &top.0 {
        if host.is_none_or(|h| key.starts_with(h) || key == "(other)") {
            filtered.0.insert(key.clone(), *count);
        }
    }
    let rows: Vec<String> = filtered.sorted(n).iter().map(|(k, v)| format!("[{},{v}]", quote(k))).collect();
    format!("[{}]", rows.join(","))
}

/// An audience/source list (`host\tvalue` keys) as `[[value, n], …]`: one
/// host's, or all hosts' added up.
fn dim(top: &Top, n: usize, host: Option<&str>) -> String {
    let mut merged = Top::default();
    for (key, count) in &top.0 {
        let (h, value) = key.split_once('\t').unwrap_or(("", key.as_str()));
        if host.is_none_or(|wanted| h.eq_ignore_ascii_case(wanted)) || key == "(other)" {
            *merged.0.entry(value.to_string()).or_default() += count;
        }
    }
    let rows: Vec<String> = merged.sorted(n).iter().map(|(k, v)| format!("[{},{v}]", quote(k))).collect();
    format!("[{}]", rows.join(","))
}

/// The JSON for the last `hours` hours (1–720), optionally one host's.
pub fn json(hours: u64, host: Option<&str>, now: u64) -> String {
    let hours = hours.clamp(1, 24 * 90);
    let (buckets, days) = snapshot(now);
    let names = super::record::host_names();
    let host_index = host.and_then(|h| names.iter().position(|n| n.eq_ignore_ascii_case(h))).map(|i| i as u8);
    let wanted = |h: u8| host.is_none() || Some(h) == host_index;
    let this_hour = now / 3600;
    let start = this_hour + 1 - hours;
    let previous_start = start.saturating_sub(hours);

    let mut totals = [[0u64; 2]; 3];
    let mut previous = [[0u64; 2]; 3];
    let daily = hours > 48;
    let mut series: BTreeMap<u64, [u64; 3]> = BTreeMap::new();
    let mut first_hour = None;
    for ((hour, h), counts) in &buckets {
        if !wanted(*h) {
            continue;
        }
        first_hour = Some(first_hour.map_or(*hour, |f: u64| f.min(*hour)));
        let target = if *hour >= start && *hour <= this_hour {
            Some(&mut totals)
        } else if *hour >= previous_start && *hour < start {
            Some(&mut previous)
        } else {
            None
        };
        if let Some(t) = target {
            for g in 0..3 {
                t[g][0] += counts[g].requests;
                t[g][1] += counts[g].pages;
            }
        }
        if *hour >= start && *hour <= this_hour {
            let key = if daily { hour / 24 * 24 } else { *hour };
            let point = series.entry(key).or_default();
            for g in 0..3 {
                point[g] += counts[g].requests;
            }
        }
    }
    // Every bucket of the period, empty ones included.
    let step = if daily { 24 } else { 1 };
    let mut t = if daily { start / 24 * 24 } else { start };
    while t <= this_hour {
        series.entry(t).or_default();
        t += step;
    }

    let start_day = start / 24;
    let previous_day = previous_start / 24;
    let mut lists = Day::default();
    let mut visitors = [0u64; 3];
    let mut previous_visitors = [0u64; 3];
    let mut sessions = Sessions::default();
    let mut previous_sessions = Sessions::default();
    let host_sessions = |d: &Day, into: &mut Sessions| {
        for (h, s) in &d.sessions {
            if host.is_none_or(|wanted| h.eq_ignore_ascii_case(wanted)) {
                into.add(s);
            }
        }
    };
    for (day, d) in &days {
        if *day >= start_day {
            host_sessions(d, &mut sessions);
        } else if *day >= previous_day {
            host_sessions(d, &mut previous_sessions);
        }
        if *day >= start_day {
            lists.merge(d);
            for g in 0..3 {
                visitors[g] += d.visitors[g];
            }
        } else if *day >= previous_day {
            for g in 0..3 {
                previous_visitors[g] += d.visitors[g];
            }
        }
    }

    let group = |t: &[[u64; 2]; 3], v: &[u64; 3], g: usize| format!(r#"{{"requests":{},"pages":{},"visitors":{}}}"#, t[g][0], t[g][1], v[g]);
    let groups = |t: &[[u64; 2]; 3], v: &[u64; 3]| format!(r#"{{"people":{},"ai":{},"bots":{}}}"#, group(t, v, 0), group(t, v, 1), group(t, v, 2));
    let points: Vec<String> = series.iter().map(|(t, p)| format!(r#"{{"t":{},"people":{},"ai":{},"bots":{}}}"#, t * 3600, p[0], p[1], p[2])).collect();
    let agents: Vec<String> = lists
        .ai_agents
        .sorted(25)
        .iter()
        .map(|(key, n)| {
            let mut parts = key.split('\t');
            let (name, operator, kind, verified) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next() == Some("true"));
            format!(r#"{{"name":{},"operator":{},"kind":{},"verified":{verified},"requests":{n}}}"#, quote(name), quote(operator), quote(kind))
        })
        .collect();
    let bots: Vec<String> = lists
        .bots
        .sorted(25)
        .iter()
        .map(|(key, n)| {
            let (kind, name) = key.split_once('\t').unwrap_or((key, ""));
            format!(r#"{{"kind":{},"name":{},"requests":{n}}}"#, quote(kind), quote(name))
        })
        .collect();
    let dims: Vec<String> = DIMS.iter().enumerate().map(|(i, name)| format!("{}:{}", quote(name), dim(&lists.dims[i], if i == 0 { 50 } else { 25 }, host))).collect();
    let session_json = |s: &Sessions| format!(r#"{{"sessions":{},"bounces":{},"duration":{},"pages":{}}}"#, s.sessions, s.bounces, s.duration, s.pages);
    let geo = super::geo::table().map_or("null".to_string(), |t| quote(&t.source));
    format!(
        r#"{{"hours":{hours},"bucket":"{}","since":{},"hosts":[{}],"host":{},"totals":{},"previous":{},"series":[{}],"people_pages":{},"ai_pages":{},"ai_agents":[{}],"bots":[{}],"referrers":{},"not_found":{},{},"geo_source":{geo},"sessions":{},"previous_sessions":{},"entry_pages":{},"exit_pages":{}}}"#,
        if daily { "day" } else { "hour" },
        first_hour.map_or("null".to_string(), |h| (h * 3600).to_string()),
        names.iter().map(|n| quote(n)).collect::<Vec<_>>().join(","),
        host.map_or("null".to_string(), quote),
        groups(&totals, &visitors),
        groups(&previous, &previous_visitors),
        points.join(","),
        pairs(&lists.people_pages, 25, host),
        pairs(&lists.ai_pages, 25, host),
        agents.join(","),
        bots.join(","),
        pairs(&lists.referrers, 25, None),
        pairs(&lists.not_found, 25, host),
        dims.join(","),
        session_json(&sessions),
        session_json(&previous_sessions),
        pairs(&lists.entry_pages, 25, host),
        pairs(&lists.exit_pages, 25, host),
    )
}
