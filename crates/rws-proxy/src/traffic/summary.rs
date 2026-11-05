//! The Traffic page's data: totals for a period and the one before it, a
//! series (hourly up to 48 hours, daily beyond), and the top lists.

use std::collections::BTreeMap;

use super::record::{snapshot, Day, Sessions, Top, BEHAVIOUR, DIMS};

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
    let daily = hours > 48;
    // Daily views cover whole UTC days (today and the days before it), so
    // the per-day counts — visitors, sessions, lists — line up with them.
    let hours = if daily { hours.div_ceil(24) * 24 } else { hours };
    let start = if daily { (this_hour / 24 + 1).saturating_sub(hours / 24) * 24 } else { this_hour + 1 - hours };
    let previous_start = start.saturating_sub(hours);

    let mut totals = [[0u64; 2]; 3];
    let mut previous = [[0u64; 2]; 3];
    // Per bucket: requests per group, then people's page views.
    let mut series: BTreeMap<u64, [u64; 4]> = BTreeMap::new();
    // The period before, by the same bucket offsets (for comparing).
    let mut previous_series: BTreeMap<u64, [u64; 4]> = BTreeMap::new();
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
        let into = if *hour >= start && *hour <= this_hour {
            Some((&mut series, *hour))
        } else if *hour >= previous_start && *hour < start {
            Some((&mut previous_series, *hour + hours))
        } else {
            None
        };
        if let Some((map, at)) = into {
            let key = if daily { at / 24 * 24 } else { at };
            let point = map.entry(key).or_default();
            for g in 0..3 {
                point[g] += counts[g].requests;
            }
            point[3] += counts[0].pages;
        }
    }
    // Every bucket of the period, empty ones included.
    let step = if daily { 24 } else { 1 };
    let mut t = if daily { start / 24 * 24 } else { start };
    while t <= this_hour {
        series.entry(t).or_default();
        previous_series.entry(t).or_default();
        t += step;
    }
    previous_series.retain(|k, _| series.contains_key(k));

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
    // People's visitors and sessions per day (current and previous period,
    // the previous keyed by the matching current day).
    let mut per_day: BTreeMap<u64, [u64; 6]> = BTreeMap::new();
    let period_days = hours.div_ceil(24).max(1);
    for day in start_day..=this_hour / 24 {
        per_day.entry(day).or_default();
    }
    for (day, d) in &days {
        let mut s = Sessions::default();
        host_sessions(d, &mut s);
        let (key, offset) = if *day >= start_day { (*day, 0) } else { (*day + period_days, 3) };
        if let Some(row) = per_day.get_mut(&key) {
            row[offset] += d.visitors[0];
            row[offset + 1] += s.sessions;
            row[offset + 2] += s.bounces;
        }
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
    let point = |(t, p): (&u64, &[u64; 4])| format!(r#"{{"t":{},"people":{},"ai":{},"bots":{},"pages":{}}}"#, t * 3600, p[0], p[1], p[2], p[3]);
    let points: Vec<String> = series.iter().map(point).collect();
    let previous_points: Vec<String> = previous_series.iter().map(point).collect();
    let day_points: Vec<String> = per_day
        .iter()
        .map(|(d, r)| format!(r#"{{"t":{},"visitors":{},"sessions":{},"bounces":{},"previous_visitors":{},"previous_sessions":{},"previous_bounces":{}}}"#, d * 86_400, r[0], r[1], r[2], r[3], r[4], r[5]))
        .collect();
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
    // Time on page: seconds and reported views per page, as [page, avg, views].
    let mut on_page: Vec<(String, u64, u64)> = lists.behaviour[1]
        .0
        .iter()
        .filter(|(k, _)| host.is_none_or(|h| k.starts_with(h)) && k.as_str() != "(other)")
        .map(|(k, views)| (k.clone(), lists.behaviour[0].0.get(k).copied().unwrap_or(0) / (*views).max(1), *views))
        .collect();
    on_page.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    on_page.truncate(25);
    let on_page: Vec<String> = on_page.iter().map(|(k, avg, views)| format!("[{},{avg},{views}]", quote(k))).collect();
    let behaviour: Vec<String> = BEHAVIOUR
        .iter()
        .enumerate()
        .skip(2)
        .map(|(i, name)| format!("{}:{}", quote(name), dim(&lists.behaviour[i], 25, host)))
        .collect();
    let session_json = |s: &Sessions| format!(r#"{{"sessions":{},"bounces":{},"duration":{},"pages":{}}}"#, s.sessions, s.bounces, s.duration, s.pages);
    let geo = super::geo::table().map_or("null".to_string(), |t| quote(&t.source));
    format!(
        r#"{{"hours":{hours},"bucket":"{}","since":{},"hosts":[{}],"host":{},"totals":{},"previous":{},"series":[{}],"people_pages":{},"ai_pages":{},"ai_agents":[{}],"bots":[{}],"referrers":{},"not_found":{},{},"geo_source":{geo},"sessions":{},"previous_sessions":{},"entry_pages":{},"exit_pages":{},"previous_series":[{}],"days":[{}],"script":{},"store":{},"time_on_page":[{}],{}}}"#,
        if daily { "day" } else { "hour" },
        first_hour.map_or("null".to_string(), |h| (h * 3600).to_string()),
        // Only sites with counted traffic (this machine's own calls, which
        // aren't counted, still register their Host).
        names
            .iter()
            .enumerate()
            .filter(|(i, _)| buckets.iter().any(|((_, h), c)| *h as usize == *i && c.iter().any(|g| g.requests > 0)))
            .map(|(_, n)| quote(n))
            .collect::<Vec<_>>()
            .join(","),
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
        previous_points.join(","),
        day_points.join(","),
        super::config::script(),
        store_on(),
        on_page.join(","),
        behaviour.join(","),
    )
}

/// The live view: people on the site in the last 5 minutes, page views per
/// minute for the last 30, and what the active ones read and where they
/// are; optionally one host's.
pub fn live_json(host: Option<&str>, now: u64) -> String {
    use std::collections::{HashMap, HashSet};
    let prefix = host.map(|h| format!("{}/", h.to_ascii_lowercase()));
    let views: Vec<_> = super::record::recent(now).into_iter().filter(|r| prefix.as_ref().is_none_or(|p| r.page.starts_with(p.as_str()))).collect();
    let this_minute = now / 60;
    let mut minutes = [0u64; 30];
    let active_since = now.saturating_sub(5 * 60);
    let mut active: HashSet<u64> = HashSet::new();
    let mut pages = Top::default();
    let mut countries: HashMap<u64, String> = HashMap::new();
    for r in &views {
        let ago = this_minute.saturating_sub(r.at / 60) as usize;
        if ago < 30 {
            minutes[29 - ago] += 1;
        }
        if r.at >= active_since {
            active.insert(r.visitor);
            *pages.0.entry(r.page.clone()).or_default() += 1;
            let country = r.country.map_or("(unknown)".to_string(), |c| String::from_utf8_lossy(&c).into_owned());
            countries.insert(r.visitor, country);
        }
    }
    let mut by_country = Top::default();
    for c in countries.values() {
        *by_country.0.entry(c.clone()).or_default() += 1;
    }
    let series: Vec<String> = minutes.iter().enumerate().map(|(i, n)| format!(r#"{{"t":{},"views":{n}}}"#, (this_minute + 1 + i as u64 - 30) * 60)).collect();
    format!(
        r#"{{"now":{now},"host":{},"visitors":{},"views_5m":{},"series":[{}],"pages":{},"countries":{}}}"#,
        host.map_or("null".to_string(), quote),
        active.len(),
        views.iter().filter(|r| r.at >= active_since).count(),
        series.join(","),
        pairs(&pages, 10, None),
        pairs(&by_country, 10, None),
    )
}

fn store_on() -> bool {
    #[cfg(feature = "model-postgres")]
    return super::store::enabled();
    #[cfg(not(feature = "model-postgres"))]
    false
}

/// Days since the epoch of `YYYY-MM-DD`.
#[doc(hidden)]
pub fn parse_day(text: &str) -> Option<u64> {
    let mut parts = text.trim().splitn(3, '-');
    let (y, m, d): (i64, i64, i64) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    if !(1970..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    (days >= 0).then_some(days as u64)
}

/// The `hours` and `now` for the summary of days `from`..=`to` (UTC, as
/// `YYYY-MM-DD`): the period ends with `to`, or now if `to` is today.
pub fn day_range(from: &str, to: &str, now: u64) -> Result<(u64, u64), String> {
    let today = now / 86_400;
    let to = if to.is_empty() { today } else { parse_day(to).ok_or(format!("to: expected YYYY-MM-DD, got {to:?}"))? };
    let from = parse_day(from).ok_or(format!("from: expected YYYY-MM-DD, got {from:?}"))?;
    if from > to {
        return Err("from must be on or before to".to_string());
    }
    let end = if to >= today { now } else { (to + 1) * 86_400 - 1 };
    let hours = (end / 3600 + 1).saturating_sub(from * 24).clamp(1, 24 * 90);
    Ok((hours, end))
}
