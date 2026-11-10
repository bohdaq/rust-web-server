use super::*;
use std::net::IpAddr;

const CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
const FIREFOX: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:144.0) Gecko/20100101 Firefox/144.0";
const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
const GPTBOT: &str = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.3; +https://openai.com/gptbot)";
const CHATGPT_USER: &str = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; ChatGPT-User/1.0; +https://openai.com/bot";
const CLAUDEBOT: &str = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; ClaudeBot/1.0; +claudebot@anthropic.com)";
const GOOGLEBOT: &str = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; Googlebot/2.1; +http://www.google.com/bot.html) Chrome/141.0.0.0 Safari/537.36";

fn ip(text: &str) -> IpAddr {
    text.parse().unwrap()
}

fn class(addr: &str, ua: &str, path: &str) -> Class {
    classify(ip(addr), (!ua.is_empty()).then_some(ua), path)
}

#[test]
fn browsers_are_candidates_for_people() {
    for ua in [CHROME, FIREFOX, IPHONE] {
        let c = class("203.0.113.9", ua, "/docs/");
        assert_eq!((c.kind, c.group(), c.name), (Kind::Browser, Group::Person, ""), "{ua}");
    }
}

#[test]
fn published_networks_verify_crawlers_and_expose_impostors() {
    let gpt = class("132.196.86.7", GPTBOT, "/");
    assert_eq!((gpt.kind, gpt.name, gpt.operator, gpt.verified), (Kind::AiTraining, "GPTBot", "OpenAI", true));
    let user = class("104.208.184.200", CHATGPT_USER, "/docs/");
    assert_eq!((user.kind, user.group(), user.verified), (Kind::AiAssistant, Group::Ai, true));
    let google = class("192.178.4.10", GOOGLEBOT, "/");
    assert_eq!((google.kind, google.name, google.verified), (Kind::SearchEngine, "Googlebot", true));
    let google6 = class("2001:4860:4801:10::5", GOOGLEBOT, "/");
    assert!(google6.verified, "IPv6 prefixes too");
    // The same User-Agents from somewhere else are lying.
    let fake = class("198.51.100.3", GOOGLEBOT, "/");
    assert_eq!((fake.kind, fake.group(), fake.name, fake.verified), (Kind::Spoofed, Group::Bot, "Googlebot", false));
    assert_eq!(class("198.51.100.3", GPTBOT, "/").kind, Kind::Spoofed);
    // A crawler's network without a crawler User-Agent is still that crawler.
    assert_eq!(class("192.178.4.10", CHROME, "/").name, "Googlebot");
}

#[test]
fn ai_agents_and_bots_come_from_the_lists() {
    let claude = class("216.73.216.10", CLAUDEBOT, "/docs/");
    assert_eq!((claude.kind, claude.name, claude.operator, claude.verified), (Kind::AiTraining, "ClaudeBot", "Anthropic", false));
    assert_eq!(class("1.2.3.4", "Mozilla/5.0 (compatible; Bytespider; spider-feedback@bytedance.com)", "/").group(), Group::Ai);
    assert_eq!(class("1.2.3.4", "Mozilla/5.0 (compatible; SemrushBot/7~bl; +http://www.semrush.com/bot.html)", "/").kind, Kind::Seo);
    assert_eq!(class("1.2.3.4", "Mozilla/5.0 (compatible; AhrefsBot/7.0; +http://ahrefs.com/robot/)", "/").kind, Kind::Seo);
    assert_eq!(class("1.2.3.4", "facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)", "/").kind, Kind::SocialPreview);
    assert_eq!(class("1.2.3.4", "Mozilla/5.0+(compatible; UptimeRobot/2.0; http://www.uptimerobot.com/)", "/").kind, Kind::Monitoring);
    assert_eq!(class("1.2.3.4", "curl/8.5.0", "/").kind, Kind::Tool);
    assert_eq!(class("1.2.3.4", "python-requests/2.32.3", "/").group(), Group::Bot);
    assert_eq!(class("1.2.3.4", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/141.0.0.0 Safari/537.36", "/").group(), Group::Bot);
    assert_eq!(class("1.2.3.4", "SomethingNew/1.0", "/").kind, Kind::Tool);
}

#[test]
fn scanners_missing_user_agents_and_this_machine() {
    for path in ["/.env", "/.git/config", "/wp-login.php", "/admin/.env.backup", "/backup.sql", "/.aws/credentials", "/phpmyadmin/index.php"] {
        assert_eq!(class("1.2.3.4", CHROME, path).kind, Kind::Scanner, "{path}");
    }
    assert_eq!(class("1.2.3.4", CHROME, "/index.php?page=about").kind, Kind::Browser, "a PHP site's pages aren't scans");
    assert_eq!(class("1.2.3.4", "", "/").kind, Kind::NoUserAgent);
    let local = class("127.0.0.1", "curl/8", "/healthz");
    assert_eq!((local.kind, local.group()), (Kind::Local, Group::Internal));
    assert_eq!(class("::1", CHROME, "/").group(), Group::Internal);
    assert_eq!(class("::ffff:127.0.0.1", CHROME, "/").group(), Group::Internal);
}

#[test]
fn every_kind_has_a_label_and_a_group() {
    for kind in [Kind::Browser, Kind::AiTraining, Kind::AiSearch, Kind::AiAssistant, Kind::AiAgent, Kind::SearchEngine, Kind::Seo, Kind::Monitoring,
        Kind::SocialPreview, Kind::Feed, Kind::Archiver, Kind::Scanner, Kind::Spoofed, Kind::Hosting, Kind::Tool, Kind::NoUserAgent, Kind::Local] {
        assert!(!kind.label().is_empty());
        let _ = kind.group();
    }
}

// ── counting (record, summary, persist) ───────────────────────────────────────

use super::record::{record, reset_for_tests, Hit, TOP, UNDECIDED};

const T0: u64 = 1_793_000_000 / 3600 * 3600; // an hour boundary

fn hit<'a>(ip: &str, ua: &'a str, path: &'a str, now: u64) -> Hit<'a> {
    Hit {
        ip: ip.parse().unwrap(),
        host: "example.com",
        user_agent: (!ua.is_empty()).then_some(ua),
        referer: None,
        method: "GET",
        path,
        status: 200,
        content_type: if path.ends_with('/') { Some("text/html; charset=utf-8") } else { None },
        headers: &[],
        now,
    }
}

fn fresh() -> std::sync::MutexGuard<'static, ()> {
    let guard = crate::test_env::lock();
    reset_for_tests();
    super::enable();
    guard
}

fn summary(hours: u64, host: Option<&str>, now: u64) -> String {
    super::summary::json(hours, host, now)
}

fn totals(json: &str, group: &str) -> (u64, u64, u64) {
    let totals = crate::mcp::json_rpc::extract_raw(json, "totals").unwrap();
    let g = crate::mcp::json_rpc::extract_raw(&totals, group).unwrap();
    let n = |k: &str| crate::mcp::json_rpc::extract_raw(&g, k).unwrap().trim().parse::<u64>().unwrap();
    (n("requests"), n("pages"), n("visitors"))
}

#[test]
fn people_are_browsers_that_load_assets_and_the_rest_waits() {
    let _g = fresh();
    // A person: a page, then its stylesheet a moment later.
    record(&hit("203.0.113.1", CHROME, "/docs/", T0));
    record(&hit("203.0.113.1", CHROME, "/assets/main.css", T0 + 1));
    record(&hit("203.0.113.1", CHROME, "/why/", T0 + 30));
    // A browser User-Agent that never loads assets.
    record(&hit("203.0.113.2", CHROME, "/docs/", T0 + 2));
    record(&hit("203.0.113.2", CHROME, "/benchmarks/", T0 + 3));
    // AI and bots.
    record(&hit("132.196.86.7", GPTBOT, "/docs/", T0 + 4));
    record(&hit("216.73.216.10", CLAUDEBOT, "/llms.txt", T0 + 5));
    record(&hit("198.51.100.9", "curl/8.5.0", "/install.sh", T0 + 6));
    record(&hit("198.51.100.10", CHROME, "/.env", T0 + 7));
    record(&hit("127.0.0.1", "curl/8", "/docs/", T0 + 8));
    record(&hit("203.0.113.3", CHROME, "/healthz", T0 + 9));

    // Within the waiting window the no-assets browser isn't counted yet.
    let early = summary(24, None, T0 + 60);
    assert_eq!(totals(&early, "people"), (3, 2, 1), "{early}");
    assert_eq!(totals(&early, "ai"), (2, 1, 2));
    assert_eq!(totals(&early, "bots"), (2, 0, 2), "curl + the scanner; loopback and /healthz not counted");

    let later = summary(24, None, T0 + 10 + UNDECIDED.as_secs());
    assert_eq!(totals(&later, "people"), (3, 2, 1));
    assert_eq!(totals(&later, "bots"), (4, 2, 3), "the no-assets browser joined the bots: {later}");
    assert!(later.contains(r#""kind":"browser-no-assets","name":"browser, no assets","requests":2"#), "{later}");
    assert!(later.contains(r#""name":"GPTBot","operator":"OpenAI","kind":"ai-training","verified":true,"requests":1"#), "{later}");
    assert!(later.contains(r#""name":"ClaudeBot","operator":"Anthropic","kind":"ai-training","verified":false"#));
    assert!(later.contains(r#""kind":"scanner""#));
    assert!(later.contains(r#""people_pages":[["example.com/docs/",1],["example.com/why/",1]]"#), "{later}");
    assert!(later.contains(r#""ai_pages":[["example.com/docs/",1]]"#));

    // 32 requests without an asset: decided at once, no waiting.
    for i in 0..32 {
        let path = format!("/p{i}/");
        record(&hit("203.0.113.40", CHROME, &path, T0 + 200));
    }
    let now = summary(24, None, T0 + 201);
    assert_eq!(totals(&now, "bots").0, 4 + 32, "{now}");
    assert!(later.contains(r#""bucket":"hour""#));
}

#[test]
fn the_summary_has_every_bucket_a_previous_period_and_a_host_filter() {
    let _g = fresh();
    record(&hit("132.196.86.7", GPTBOT, "/a/", T0 - 30 * 3600)); // the previous day
    record(&hit("132.196.86.7", GPTBOT, "/b/", T0));
    let mut other = hit("132.196.86.7", GPTBOT, "/c/", T0);
    other.host = "Other.Example:443";
    record(&other);
    let json = summary(24, None, T0 + 60);
    assert_eq!(crate::mcp::json_rpc::split_array_elements(&crate::mcp::json_rpc::extract_raw(&json, "series").unwrap()).len(), 24);
    assert_eq!(totals(&json, "ai").0, 2);
    let previous = crate::mcp::json_rpc::extract_raw(&json, "previous").unwrap();
    assert!(previous.contains(r#""ai":{"requests":1"#), "{previous}");
    assert!(json.contains(r#""hosts":["example.com","other.example"]"#), "{json}");
    let one = summary(24, Some("other.example"), T0 + 60);
    assert_eq!(totals(&one, "ai").0, 1);
    assert!(one.contains(r#""ai_pages":[["other.example/c/",1]]"#), "{one}");
    let week = summary(24 * 7, None, T0 + 60);
    assert!(week.contains(r#""bucket":"day""#));
    assert!(crate::mcp::json_rpc::split_array_elements(&crate::mcp::json_rpc::extract_raw(&week, "series").unwrap()).len() >= 7);
}

#[test]
fn lists_and_hosts_stay_bounded() {
    let _g = fresh();
    for i in 0..(TOP + 50) {
        let path = format!("/page{i}/");
        record(&hit("132.196.86.7", GPTBOT, &path, T0));
    }
    let pages = summary(24, None, T0 + 60);
    assert!(pages.contains(r#"["(other)",50]"#), "the overflow counts as (other)");
    for i in 0..40 {
        let host = format!("h{i}.example");
        let mut h = hit("132.196.86.7", GPTBOT, "/", T0);
        h.host = &host;
        record(&h);
    }
    let json = summary(24, None, T0 + 60);
    let hosts = crate::mcp::json_rpc::split_array_elements(&crate::mcp::json_rpc::extract_raw(&json, "hosts").unwrap());
    assert!(hosts.len() <= super::record::MAX_HOSTS + 1 && hosts.iter().any(|h| h == "\"other\""), "{hosts:?}");
}

#[test]
fn configured_sites_get_slots_first_and_are_listed_first() {
    let _g = fresh();
    // Junk Host headers arrive first and would take every slot.
    for i in 0..40 {
        let host = format!("10.0.0.{i}");
        let mut h = hit("132.196.86.7", GPTBOT, "/", T0);
        h.host = &host;
        record(&h);
    }
    super::record::set_configured_hosts(["Site.Example:443".to_string(), "quiet.example".to_string(), "*.wild.example".to_string()]);
    let mut site = hit("132.196.86.7", GPTBOT, "/docs/", T0);
    site.host = "site.example";
    record(&site);
    let json = summary(24, None, T0 + 60);
    let hosts = crate::mcp::json_rpc::split_array_elements(&crate::mcp::json_rpc::extract_raw(&json, "hosts").unwrap());
    assert_eq!(hosts[..2], ["\"site.example\"", "\"quiet.example\""], "configured first, busiest first, even without traffic: {hosts:?}");
    assert_eq!(hosts.last().map(String::as_str), Some("\"other\""));
    assert!(json.contains(r#""configured":["quiet.example","site.example"]"#), "{json}");
    let one = summary(24, Some("site.example"), T0 + 60);
    assert_eq!(totals(&one, "ai").0, 1, "{one}");
    assert_eq!(totals(&summary(24, Some("quiet.example"), T0 + 60), "ai").0, 0);

    // A restart: saved junk hosts don't take a configured site's slot.
    let names = super::record::host_names();
    let (hours, days) = super::record::snapshot(T0 + 60);
    reset_for_tests();
    super::record::set_configured_hosts(["site.example".to_string(), "late.example".to_string()]);
    super::record::restore(names, hours, days);
    let mut late = hit("132.196.86.7", GPTBOT, "/", T0 + 61);
    late.host = "late.example";
    record(&late);
    assert_eq!(totals(&summary(24, Some("late.example"), T0 + 120), "ai").0, 1);
    assert_eq!(totals(&summary(24, Some("site.example"), T0 + 120), "ai").0, 1);
}

#[test]
fn a_site_shows_only_its_own_visitors_agents_bots_and_referrers() {
    let _g = fresh();
    for (host, ip, referer) in [("a.example", "203.0.113.1", "https://news.example/x"), ("a.example.org", "203.0.113.2", "https://other.example/")] {
        let mut page = hit(ip, CHROME, "/docs/", T0);
        page.host = host;
        page.referer = Some(referer);
        record(&page);
        let mut asset = hit(ip, CHROME, "/main.js", T0 + 1);
        asset.host = host;
        record(&asset);
        let mut bot = hit("1.2.3.4", "curl/8.5.0", "/", T0 + 2);
        bot.host = host;
        record(&bot);
    }
    let mut gpt = hit("132.196.86.7", GPTBOT, "/x/", T0);
    gpt.host = "a.example.org";
    record(&gpt);
    let a = summary(24, Some("a.example"), T0 + 60);
    assert_eq!(totals(&a, "people").2, 1, "{a}");
    assert_eq!(totals(&a, "bots").2, 1, "{a}");
    assert!(a.contains(r#""people_pages":[["a.example/docs/",1]]"#), "no a.example.org pages: {a}");
    assert!(a.contains(r#""referrers":[["news.example",1]]"#), "{a}");
    assert!(a.contains(r#""ai_agents":[]"#), "{a}");
    let org = summary(24, Some("a.example.org"), T0 + 60);
    assert!(org.contains(r#""name":"GPTBot""#), "{org}");
    assert!(org.contains(r#""referrers":[["other.example",1]]"#), "{org}");
    let all = summary(24, None, T0 + 60);
    assert_eq!(totals(&all, "people").2, 2);
    assert!(all.contains(r#""bots":[{"kind":"tool","name":"curl","requests":2}]"#), "added up across sites: {all}");

    // Saved and loaded, the per-site lists stay per site.
    let text = super::persist::to_json(&super::record::host_names(), &super::record::snapshot(T0 + 60).0, &super::record::snapshot(T0 + 60).1);
    assert!(text.contains("\"site_visitors\":[") && !text.contains("\"bots\":[[\"a.example"), "{text}");
}

#[test]
#[cfg(feature = "geo")]
fn hosting_networks_are_named_and_vpn_and_isp_networks_are_not() {
    // Addresses from rws8.tech's log of 2026-11-08, with what they were.
    for (addr, expected) in [
        ("54.80.28.156", Some("AWS")),         // an "iPhone" that ran the script: a link scanner
        ("35.188.140.12", Some("Google Cloud")),
        ("144.217.86.46", Some("OVHcloud")),   // downloaded the CV
        ("147.182.208.223", Some("DigitalOcean")),
        ("88.99.48.186", Some("Hetzner")),
        ("199.165.159.153", Some("other hosting")), // Shadowserver
        ("45.138.12.180", Some("other hosting")),   // TC Datacenter, probing for secrets
        ("37.54.23.252", None),     // Ukrtelecom: a phone using the "ask" box
        ("109.138.199.255", None),  // Proximus
        ("88.255.41.28", None),     // Türk Telekom
        ("92.71.99.158", None),     // Datacamp: a consumer VPN
        ("104.16.0.1", None),       // Cloudflare (WARP, Private Relay)
        ("66.249.64.110", None),    // Googlebot: verified by name instead
        ("2001:db8::1", None),
    ] {
        assert_eq!(super::hosting::provider(ip(addr)), expected, "{addr}");
    }
    assert_eq!(super::hosting::table().map(|t| t.len().0 > 10_000), Some(true));
}

#[test]
#[cfg(feature = "geo")]
fn a_browser_from_a_hosting_network_is_a_bot_even_when_it_loads_assets() {
    let _g = fresh();
    record(&hit("54.80.28.156", IPHONE, "/", T0));
    record(&hit("54.80.28.156", IPHONE, "/app.js", T0 + 1));
    record(&hit("37.54.23.252", CHROME, "/", T0));
    record(&hit("37.54.23.252", CHROME, "/app.js", T0 + 1));
    let json = summary(24, None, T0 + 60);
    assert_eq!(totals(&json, "people").2, 1, "{json}");
    assert_eq!(totals(&json, "bots").2, 1, "{json}");
    assert!(json.contains(r#"{"kind":"hosting","name":"AWS","requests":1}"#), "{json}");
    assert!(json.contains(r#""countries":[["UA",1]]"#), "no audience for hosting networks: {json}");
}

#[test]
fn counts_survive_a_save_and_load() {
    let _g = fresh();
    record(&hit("203.0.113.1", CHROME, "/docs/", T0));
    record(&hit("203.0.113.1", CHROME, "/main.js", T0 + 1));
    record(&hit("132.196.86.7", GPTBOT, "/docs/", T0 + 2));
    let before = summary(24, None, T0 + 60);
    let file = crate::test_env::TempPath::new("traffic_counts.json");
    std::fs::write(&*file, "older, world-readable").unwrap();
    super::persist::save(&file).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&*file).unwrap().permissions().mode() & 0o777, 0o600, "only the owner reads the counts");
    }
    let text = std::fs::read_to_string(&*file).unwrap();
    assert!(text.starts_with(r#"{"version":1,"hosts":["example.com"],"hours":[["#), "{text}");
    assert!(!text.contains("203.0.113") && !text.contains(CHROME), "no addresses or user agents saved (browser families are)");
    reset_for_tests();
    assert_eq!(totals(&summary(24, None, T0 + 60), "people").0, 0);
    super::persist::load(&file);
    assert_eq!(summary(24, None, T0 + 60), before);
    std::fs::write(&*file, "not json").unwrap();
    super::persist::load(&file); // reported, ignored
}

// ── audience and sources (18.31) ──────────────────────────────────────────────

#[cfg(feature = "geo")]
#[test]
fn the_built_in_table_knows_countries() {
    let table = super::geo::table().expect("built in");
    assert!(table.source.starts_with("dbip-country-lite-"), "{}", table.source);
    let c = |ip: &str| table.country(ip.parse().unwrap()).map(|c| String::from_utf8(c.to_vec()).unwrap());
    assert_eq!(c("8.8.8.8").as_deref(), Some("US"));
    assert_eq!(c("1.1.1.1").as_deref(), Some("AU"));
    assert_eq!(c("5.172.33.9").as_deref(), Some("PL"));
    assert_eq!(c("193.0.6.139").as_deref(), Some("NL"));
    assert_eq!(c("2a00:1450:4001:80b::200e").as_deref(), Some("DE"));
    assert_eq!(c("::ffff:8.8.8.8").as_deref(), Some("US"), "IPv4-mapped looks up the IPv4 table");
    for private in ["10.1.2.3", "127.0.0.1", "0.0.0.0", "255.255.255.255", "::1", "fd00::1"] {
        assert_eq!(c(private), None, "{private}");
    }
}

#[test]
fn a_damaged_geo_table_is_refused_not_trusted() {
    use super::geo::Table;
    assert!(Table::parse(b"").is_err());
    assert!(Table::parse(b"RWSGEO1 x").is_err(), "no header end");
    // one country, one range each: a valid minimal table…
    let ok = b"RWSGEO1 t\n\x01DE\x01\x01\x00\x00\x00\x00".to_vec();
    let table = Table::parse(&ok).unwrap();
    assert_eq!(table.country("9.9.9.9".parse().unwrap()), Some(*b"DE"));
    assert_eq!(table.len(), (1, 1));
    // …and broken ones.
    let mut bad_index = ok.clone();
    *bad_index.last_mut().unwrap() = 7;
    assert!(Table::parse(&bad_index).is_err(), "country index past the list");
    assert!(Table::parse(&ok[..ok.len() - 1]).is_err(), "truncated");
    let mut trailing = ok.clone();
    trailing.push(0);
    assert!(Table::parse(&trailing).is_err());
    assert!(Table::parse(b"RWSGEO1 t\n\x01DE\x01\x01\x05\x00\x00\x00").is_err(), "first range must start at 0");
    assert!(Table::parse(b"RWSGEO1 t\n\x01DE\xff\xff\xff\xff\x0f\x01").is_err(), "absurd range count");
    assert!(Table::parse(b"RWSGEO1 t\n\x01DE\x02\x01\x00\x00\x00\x00\x00").is_err(), "repeated start");
    assert!(Table::decode(b"not deflated").is_err());
    // fuzz/seeds/traffic_geo/minimal.raw: two IPv4 ranges (DE, then none from 1.0.0.0).
    let seed = Table::parse(b"RWSGEO1 seed\n\x02DE--\x02\x01\x00\x80\x80\x80\x08\x00\x00\x01\x00").unwrap();
    assert_eq!(seed.country("0.255.0.1".parse().unwrap()), Some(*b"DE"));
    assert_eq!(seed.country("1.0.0.0".parse().unwrap()), None);
    assert_eq!(seed.country("::1".parse().unwrap()), Some(*b"DE"));
}

#[test]
fn devices_from_user_agents_and_client_hints() {
    use super::device::parse;
    let d = |ua: &str| {
        let d = parse(ua, None, None);
        (d.browser, d.os, d.class)
    };
    assert_eq!(d(CHROME), ("Chrome", "macOS", "desktop"));
    assert_eq!(d("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0"), ("Edge", "Windows", "desktop"));
    assert_eq!(d("Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:144.0) Gecko/20100101 Firefox/144.0"), ("Firefox", "macOS", "desktop"));
    assert_eq!(d("Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1"), ("Safari", "iOS", "mobile"));
    assert_eq!(d("Mozilla/5.0 (iPad; CPU OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/141.0 Mobile/15E148 Safari/604.1"), ("Chrome", "iOS", "tablet"));
    assert_eq!(d("Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/28.0 Chrome/130.0.0.0 Mobile Safari/537.36"), ("Samsung Internet", "Android", "mobile"));
    assert_eq!(d("Mozilla/5.0 (Linux; Android 14; SM-X710) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36"), ("Chrome", "Android", "tablet"));
    assert_eq!(d("Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 OPR/120.0"), ("Opera", "ChromeOS", "desktop"));
    assert_eq!(d(""), ("Other", "Other", "desktop"));
    // Chromium's frozen UA says "Android 10; K"; the hints say what it is.
    let hinted = parse(CHROME, Some("\"Windows\""), Some("?0"));
    assert_eq!((hinted.os, hinted.class), ("Windows", "desktop"));
    let phone = parse(CHROME, Some("\"Android\""), Some("?1"));
    assert_eq!((phone.os, phone.class), ("Android", "mobile"));
    assert_eq!(parse(CHROME, Some("\"\""), None).os, "macOS", "an empty hint falls back to the UA");
}

#[test]
fn utm_values_and_channels() {
    use super::record::{channel_of, utm};
    assert_eq!(utm("/p/?utm_source=news%20letter&utm_medium=email&utm_campaign=fall+sale&x=1"), [Some("news letter".into()), Some("email".into()), Some("fall sale".into())]);
    assert_eq!(utm("/p/?utm_source=&utm_medium=%zz"), [None, Some("%zz".into()), None], "empty is none; bad escapes kept");
    assert_eq!(utm("/p/#utm_source=x"), [None, None, None]);
    assert_eq!(utm("/?utm_source=a&utm_source=b"), [Some("a".into()), None, None], "the first wins");
    let long = format!("/?utm_campaign={}", "x".repeat(5000));
    assert_eq!(utm(&long)[2].as_ref().unwrap().len(), 100);
    assert_eq!(channel_of("www.google.com"), "search");
    assert_eq!(channel_of("google.co.uk"), "search");
    assert_eq!(channel_of("duckduckgo.com"), "search");
    assert_eq!(channel_of("t.co"), "social");
    assert_eq!(channel_of("old.reddit.com"), "social");
    assert_eq!(channel_of("chatgpt.com"), "ai");
    assert_eq!(channel_of("www.perplexity.ai"), "ai");
    assert_eq!(channel_of("blog.example.org"), "referral");
    assert_eq!(channel_of("notgoogle.com"), "referral");
}

fn dim(json: &str, name: &str) -> String {
    crate::mcp::json_rpc::extract_raw(json, name).unwrap()
}

#[test]
fn people_get_an_audience_and_sources_bots_dont() {
    let _g = fresh();
    let iphone = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1";
    // A person from a search engine…
    let mut first = hit("8.8.8.8", iphone, "/docs/", T0);
    first.referer = Some("https://www.google.com/");
    record(&first);
    record(&hit("8.8.8.8", iphone, "/site.css", T0 + 1));
    // …who then navigates within the site (not a new arrival).
    let mut inside = hit("8.8.8.8", iphone, "/why/", T0 + 20);
    inside.referer = Some("https://example.com/docs/");
    record(&inside);
    // A person from a campaign, on another host.
    let mut campaign = hit("193.0.6.139", CHROME, "/?utm_source=newsletter&utm_medium=email&utm_campaign=launch", T0 + 2);
    campaign.host = "shop.example";
    campaign.content_type = Some("text/html");
    record(&campaign);
    let mut asset = hit("193.0.6.139", CHROME, "/app.js", T0 + 3);
    asset.host = "shop.example";
    record(&asset);
    // A browser that never loads assets: no audience, no sources.
    record(&hit("1.1.1.1", CHROME, "/docs/", T0 + 4));

    let json = summary(24, None, T0 + 10 + UNDECIDED.as_secs());
    assert_eq!(totals(&json, "people").2, 2, "{json}");
    #[cfg(feature = "geo")]
    assert_eq!(dim(&json, "countries"), r#"[["NL",1],["US",1]]"#, "the no-assets browser (AU) isn't a person: {json}");
    #[cfg(not(feature = "geo"))]
    assert_eq!(dim(&json, "countries"), r#"[["(unknown)",2]]"#, "no table: {json}");
    assert_eq!(dim(&json, "browsers"), r#"[["Chrome",1],["Safari",1]]"#);
    assert_eq!(dim(&json, "os"), r#"[["iOS",1],["macOS",1]]"#);
    assert_eq!(dim(&json, "devices"), r#"[["desktop",1],["mobile",1]]"#);
    assert_eq!(dim(&json, "channels"), r#"[["campaign",1],["search",1]]"#, "in-site navigation isn't an arrival: {json}");
    assert_eq!(dim(&json, "utm_source"), r#"[["newsletter",1]]"#);
    assert_eq!(dim(&json, "utm_medium"), r#"[["email",1]]"#);
    assert_eq!(dim(&json, "utm_campaign"), r#"[["launch",1]]"#);
    #[cfg(feature = "geo")]
    assert!(json.contains(r#""geo_source":"dbip-country-lite-"#), "{json}");

    let shop = summary(24, Some("shop.example"), T0 + 10 + UNDECIDED.as_secs());
    #[cfg(feature = "geo")]
    assert_eq!(dim(&shop, "countries"), r#"[["NL",1]]"#, "{shop}");
    assert_eq!(dim(&shop, "channels"), r#"[["campaign",1]]"#);

    // Saved and loaded with the rest.
    let file = crate::test_env::TempPath::new("traffic_audience.json");
    super::persist::save(&file).unwrap();
    let before = summary(24, None, T0 + 10 + UNDECIDED.as_secs());
    reset_for_tests();
    super::persist::load(&file);
    assert_eq!(summary(24, None, T0 + 10 + UNDECIDED.as_secs()), before);
}

#[test]
fn a_direct_visit_has_no_referer() {
    let _g = fresh();
    record(&hit("8.8.8.8", CHROME, "/", T0));
    record(&hit("8.8.8.8", CHROME, "/a.css", T0 + 1));
    let json = summary(24, None, T0 + 5);
    assert_eq!(dim(&json, "channels"), r#"[["direct",1]]"#, "{json}");
}

// ── sessions (18.31, tranche 2) ───────────────────────────────────────────────

fn sessions(json: &str, key: &str) -> (u64, u64, u64, u64) {
    let s = crate::mcp::json_rpc::extract_raw(json, key).unwrap();
    let n = |k: &str| crate::mcp::json_rpc::extract_raw(&s, k).unwrap().trim().parse::<u64>().unwrap();
    (n("sessions"), n("bounces"), n("duration"), n("pages"))
}

#[test]
fn sessions_bounces_duration_entry_and_exit() {
    let _g = fresh();
    use super::record::SESSION_GAP;
    // Person A: three pages over 10 minutes (one session), the first two
    // waiting until the stylesheet decides them.
    record(&hit("8.8.8.8", CHROME, "/docs/", T0));
    record(&hit("8.8.8.8", CHROME, "/why/", T0 + 60));
    record(&hit("8.8.8.8", CHROME, "/a.css", T0 + 61));
    record(&hit("8.8.8.8", CHROME, "/pricing/", T0 + 600));
    // …then back after more than 30 minutes: a second, one-page session.
    record(&hit("8.8.8.8", CHROME, "/blog/", T0 + 600 + SESSION_GAP + 1));
    // Person B: one page, a bounce.
    record(&hit("1.1.1.1", CHROME, "/", T0 + 5));
    record(&hit("1.1.1.1", CHROME, "/a.css", T0 + 6));
    // A bot's pages make no sessions.
    record(&hit("132.196.86.7", GPTBOT, "/docs/", T0 + 7));

    // While the last sessions are still open they count as if ending now.
    let open = summary(24, None, T0 + 600 + SESSION_GAP + 2);
    assert_eq!(sessions(&open, "sessions"), (3, 2, 600, 5), "{open}");
    // Closed by settling, the same numbers.
    let closed = summary(24, None, T0 + 4 * 3600);
    assert_eq!(sessions(&closed, "sessions"), (3, 2, 600, 5), "{closed}");
    assert!(closed.contains(r#""channels":[["direct",3]]"#), "one arrival per session: {closed}");
    assert!(closed.contains(r#""entry_pages":[["example.com/",1],["example.com/blog/",1],["example.com/docs/",1]]"#), "{closed}");
    assert!(closed.contains(r#""exit_pages":[["example.com/",1],["example.com/blog/",1],["example.com/pricing/",1]]"#), "{closed}");
    assert_eq!(sessions(&closed, "previous_sessions"), (0, 0, 0, 0));

    // Saved and loaded.
    let file = crate::test_env::TempPath::new("traffic_sessions.json");
    super::persist::save(&file).unwrap();
    reset_for_tests();
    super::persist::load(&file);
    assert_eq!(summary(24, None, T0 + 4 * 3600), closed);

    // Another host's filter sees none of them.
    assert_eq!(sessions(&summary(24, Some("other.example"), T0 + 4 * 3600), "sessions"), (0, 0, 0, 0));
}

#[test]
fn open_sessions_stay_bounded() {
    let _g = fresh();
    use super::record::MAX_OPEN_SESSIONS;
    // More people at once than one shard keeps open (spread over 16 shards).
    let n = MAX_OPEN_SESSIONS * 16 + 500;
    for i in 0..n {
        let ip = format!("2001:db8::{:x}:{:x}", i >> 16, i & 0xffff);
        record(&hit(&ip, CHROME, "/a.css", T0));
        record(&hit(&ip, CHROME, "/", T0 + 1));
    }
    let json = summary(24, None, T0 + 2);
    // Every session is counted once, closed early or not.
    assert_eq!(sessions(&json, "sessions").0, totals(&json, "people").2, "{}", &json[..300]);
}

// ── live view (18.31, tranche 3) ──────────────────────────────────────────────

#[test]
fn the_live_view_shows_who_is_here_now() {
    let _g = fresh();
    let now = T0 + 3600;
    // Two people now (one on another host), one 20 minutes ago, one an hour ago.
    for (ip, page, at) in [("8.8.8.8", "/docs/", now - 30), ("8.8.8.8", "/why/", now - 10), ("193.0.6.139", "/", now - 20 * 60), ("1.1.1.1", "/old/", now - 3600)] {
        record(&hit(ip, CHROME, page, at));
        record(&hit(ip, CHROME, "/a.css", at));
    }
    let mut shop = hit("5.172.33.9", CHROME, "/cart/", now - 60);
    shop.host = "shop.example";
    record(&shop);
    let mut css = hit("5.172.33.9", CHROME, "/a.css", now - 60);
    css.host = "shop.example";
    record(&css);
    // A bot isn't a visitor.
    record(&hit("132.196.86.7", GPTBOT, "/docs/", now - 5));

    let live = super::summary::live_json(None, now);
    assert!(live.contains(r#""visitors":2,"views_5m":3"#), "{live}");
    assert!(live.contains(r#""pages":[["example.com/docs/",1],["example.com/why/",1],["shop.example/cart/",1]]"#), "{live}");
    #[cfg(feature = "geo")]
    assert!(live.contains(r#""countries":[["PL",1],["US",1]]"#), "{live}");
    let series = crate::mcp::json_rpc::split_array_elements(&crate::mcp::json_rpc::extract_raw(&live, "series").unwrap());
    assert_eq!(series.len(), 30);
    let views: u64 = series.iter().map(|p| crate::mcp::json_rpc::extract_raw(p, "views").unwrap().trim().parse::<u64>().unwrap()).sum();
    assert_eq!(views, 4, "the last 30 minutes, not the hour-old one: {live}");
    // `now` is on a minute boundary: the last minute's three views sit just before it.
    assert!(series.last().unwrap().contains(r#""views":0"#) && series[28].contains(r#""views":3"#), "{live}");

    let one = super::summary::live_json(Some("shop.example"), now);
    assert!(one.contains(r#""visitors":1,"views_5m":1"#), "{one}");
}

// ── first-party script (18.31, tranche 4) ─────────────────────────────────────

struct Inner;
impl crate::application::Application for Inner {
    fn execute(&self, _r: &crate::request::Request, _c: &crate::server::ConnectionInfo) -> Result<crate::response::Response, String> {
        Ok(crate::response::Response::text(crate::response::STATUS_CODE_REASON_PHRASE.n200_ok, "inner"))
    }
}

fn through(layer: &super::TrafficScriptLayer, raw: &str) -> crate::response::Response {
    use crate::middleware::Middleware;
    let request = crate::request::Request::parse(raw.as_bytes()).unwrap();
    let mut connection = crate::server::ConnectionInfo::default();
    connection.client.ip = "8.8.8.8".to_string();
    layer.handle(&request, &connection, &Inner).unwrap()
}

fn body(r: &crate::response::Response) -> String {
    r.content_range_list.iter().map(|c| String::from_utf8_lossy(&c.body).into_owned()).collect()
}

#[test]
fn the_script_layer_answers_only_its_paths() {
    use crate::middleware::Middleware;
    let off = super::TrafficScriptLayer::new(false);
    assert_eq!(body(&through(&off, "GET /_rws/t.js HTTP/1.1\r\nHost: example.com\r\n\r\n")), "inner", "off: passed through");
    let on = super::TrafficScriptLayer::new(true);
    let request = |raw: &str| crate::request::Request::parse(raw.as_bytes()).unwrap();
    let conn = crate::server::ConnectionInfo::default();
    assert!(on.is_passthrough(&request("GET /docs/ HTTP/1.1\r\n\r\n"), &conn), "other paths keep the async forwarder");
    assert!(!on.is_passthrough(&request("POST /_rws/t HTTP/1.1\r\n\r\n"), &conn));
    assert!(off.is_passthrough(&request("POST /_rws/t HTTP/1.1\r\n\r\n"), &conn));

    let script = through(&on, "GET /_rws/t.js?v=1 HTTP/1.1\r\nHost: example.com\r\n\r\n");
    assert_eq!(script.status_code, 200);
    assert!(body(&script).contains("sendBeacon"));
    assert!(script.content_range_list[0].content_type.starts_with("text/javascript"));
    let etag = script.headers.iter().find(|h| h.name.eq_ignore_ascii_case("etag")).unwrap().value.to_string();
    let again = through(&on, &format!("GET /_rws/t.js HTTP/1.1\r\nHost: example.com\r\nIf-None-Match: {etag}\r\n\r\n"));
    assert_eq!(again.status_code, 304);
    assert_eq!(through(&on, "POST /_rws/t.js HTTP/1.1\r\nHost: example.com\r\nContent-Length: 0\r\n\r\n").status_code, 405);
    assert_eq!(through(&on, "GET /_rws/t HTTP/1.1\r\nHost: example.com\r\n\r\n").status_code, 405);
    let big = "x".repeat(super::script::MAX_BEACON + 1);
    assert_eq!(through(&on, &format!("POST /_rws/t HTTP/1.1\r\nHost: example.com\r\nContent-Length: {}\r\n\r\n{big}", big.len())).status_code, 413);
    assert_eq!(through(&on, "POST /_rws/t HTTP/1.1\r\nHost: example.com\r\nOrigin: https://evil.example\r\nContent-Length: 2\r\n\r\n{}").status_code, 403);
    assert_eq!(through(&on, "POST /_rws/t HTTP/1.1\r\nHost: example.com\r\nOrigin: https://example.com\r\nContent-Length: 2\r\n\r\n{}").status_code, 204);
    assert_eq!(body(&through(&on, "GET /docs/ HTTP/1.1\r\nHost: example.com\r\n\r\n")), "inner");
}

#[test]
fn script_reports_add_time_scroll_screens_events_and_goals() {
    let _g = fresh();
    use super::record::beacon;
    super::record::set_goals_for_tests(&["/thanks/", "event:signup", "/docs/*"]);
    // A person: a page and its stylesheet.
    record(&hit("8.8.8.8", CHROME, "/docs/intro/", T0));
    record(&hit("8.8.8.8", CHROME, "/a.css", T0 + 1));
    // The script reports leaving it, a route change, an event, leaving again.
    beacon("8.8.8.8".parse().unwrap(), "example.com", Some(CHROME), r#"{"t":"leave","p":"/docs/intro/","ms":42500,"sd":80,"w":1280}"#, T0 + 43);
    beacon("8.8.8.8".parse().unwrap(), "example.com", Some(CHROME), r#"{"t":"view","p":"/pricing/","r":"/docs/intro/"}"#, T0 + 43);
    beacon("8.8.8.8".parse().unwrap(), "example.com", Some(CHROME), r#"{"t":"event","p":"/pricing/","n":"signup","q":{"plan":"pro","p":"/x"}}"#, T0 + 50);
    beacon("8.8.8.8".parse().unwrap(), "example.com", Some(CHROME), r#"{"t":"leave","p":"/pricing/","ms":10000,"sd":20,"w":390}"#, T0 + 60);
    // Not counted: a bot, a browser never decided, garbage.
    beacon("132.196.86.7".parse().unwrap(), "example.com", Some(GPTBOT), r#"{"t":"event","p":"/","n":"signup"}"#, T0 + 61);
    beacon("1.1.1.1".parse().unwrap(), "example.com", Some(CHROME), r#"{"t":"leave","p":"/","ms":1000,"sd":1,"w":1}"#, T0 + 62);
    for junk in ["", "[]", "{", r#"{"t":"leave"}"#, r#"{"t":"leave","p":"no-slash"}"#, r#"{"t":"event","p":"/","n":""}"#, r#"{"t":"leave","p":"/","ms":-5,"sd":1e400}"#] {
        beacon("8.8.8.8".parse().unwrap(), "example.com", Some(CHROME), junk, T0 + 63);
    }

    let json = summary(24, None, T0 + 4 * 3600);
    super::record::set_goals_for_tests(&[]);
    assert_eq!(totals(&json, "people").1, 2, "the route change is a page view: {json}");
    assert_eq!(sessions(&json, "sessions"), (1, 0, 43, 2), "{json}");
    assert!(json.contains(r#""time_on_page":[["example.com/docs/intro/",42,1],["example.com/pricing/",10,1]]"#), "{json}");
    assert_eq!(dim(&json, "scroll"), r#"[["100",1],["25",1]]"#, "{json}");
    assert_eq!(dim(&json, "screens"), r#"[["laptop",1],["phone",1]]"#);
    assert_eq!(dim(&json, "events"), r#"[["signup",1]]"#);
    assert_eq!(dim(&json, "event_props"), r#"[["signup\u0009p=/x",1],["signup\u0009plan=pro",1]]"#);
    assert_eq!(dim(&json, "goals"), r#"[["/docs/*",1],["event:signup",1]]"#, "one session reached two goals: {json}");
    assert!(json.contains(r#""channels":[["direct",1]]"#), "the route change isn't an arrival: {json}");

    // Saved and loaded.
    let file = crate::test_env::TempPath::new("traffic_behaviour.json");
    super::persist::save(&file).unwrap();
    reset_for_tests();
    super::persist::load(&file);
    assert_eq!(summary(24, None, T0 + 4 * 3600), json);
}

// ── PostgreSQL store (18.31, tranche 5) ───────────────────────────────────────

#[cfg(feature = "model-postgres")]
#[test]
fn store_sql_is_parameterized() {
    use super::store::{insert_sql, parse_day, Query};
    assert_eq!(parse_day("1970-01-01"), Some(0));
    assert_eq!(parse_day("2026-11-05"), Some(20762));
    assert_eq!(parse_day("2024-02-29"), Some(19782));
    for bad in ["", "2026-13-01", "2026-1", "x-y-z", "1969-12-31", "2026-11-05T00"] {
        assert_eq!(parse_day(bad), None, "{bad}");
    }
    assert_eq!(insert_sql("t", &["at", "a", "b"], 2, &[0], &[2]), "INSERT INTO t (at, a, b) VALUES (to_timestamp($1), $2, $3::bigint), (to_timestamp($4), $5, $6::bigint)");
    let q = Query { from: 10, to: 20, host: Some("Example.COM".into()), path: Some("/docs/100%_*".into()), country: Some("de".into()), channel: Some("".into()), ..Default::default() };
    let (sql, params) = q.filter();
    assert_eq!(sql, "at >= to_timestamp($1) AND at < to_timestamp($2) AND host = $3 AND path LIKE $4 AND country = $5");
    use crate::model::Value;
    assert_eq!(params, vec![Value::Int(10), Value::Int(20), Value::Text("example.com".into()), Value::Text("/docs/100\\%\\_%".into()), Value::Text("DE".into())]);
    let (sql, _) = Query { path: Some("/exact".into()), ..Default::default() }.filter();
    assert!(sql.ends_with("path = $3"), "{sql}");
}

/// Against a real server: `RWS_TEST_POSTGRES_URL` (CI's model-databases
/// job); skipped without it unless `RWS_TEST_REQUIRE_BACKENDS` names postgres.
#[cfg(feature = "model-postgres")]
#[test]
fn store_writes_rows_and_answers_queries() {
    let Ok(url) = std::env::var("RWS_TEST_POSTGRES_URL") else {
        assert!(!std::env::var("RWS_TEST_REQUIRE_BACKENDS").unwrap_or_default().contains("postgres"), "RWS_TEST_POSTGRES_URL is required");
        return;
    };
    let _g = fresh();
    use crate::model::DbPool;
    // A schema of this test's own, dropped at the end.
    let schema = format!("rws_traffic_test_{}", std::process::id());
    let admin = rws_server::async_bridge::block_on_shared({
        let url = url.clone();
        async move { DbPool::connect(&url).await.unwrap() }
    });
    let run = |sql: String| {
        let admin = admin.clone();
        rws_server::async_bridge::block_on_shared(async move { admin.execute(&sql, &[]).await.unwrap() })
    };
    run(format!("DROP SCHEMA IF EXISTS {schema} CASCADE"));
    run(format!("CREATE SCHEMA {schema}"));
    let sep = if url.contains('?') { '&' } else { '?' };
    super::store::start(&format!("{url}{sep}options=-c%20search_path%3D{schema}")).unwrap();
    let (written_before, _, _) = super::store::stats();

    // Two people, one from a campaign on another host, and a script report.
    super::record::set_goals_for_tests(&[]);
    record(&hit("8.8.8.8", CHROME, "/docs/", T0));
    record(&hit("8.8.8.8", CHROME, "/a.css", T0 + 1));
    record(&hit("8.8.8.8", CHROME, "/docs/intro/", T0 + 60));
    let mut shop = hit("193.0.6.139", CHROME, "/?utm_source=newsletter&utm_campaign=launch", T0 + 5);
    shop.host = "shop.example";
    shop.content_type = Some("text/html");
    record(&shop);
    let mut css = hit("193.0.6.139", CHROME, "/a.css", T0 + 6);
    css.host = "shop.example";
    record(&css);
    super::record::beacon("8.8.8.8".parse().unwrap(), "example.com", Some(CHROME), r#"{"t":"event","p":"/docs/intro/","n":"signup","q":{"plan":"pro"}}"#, T0 + 70);
    super::store::stop();
    assert!(super::store::wait_written(written_before + 4, std::time::Duration::from_secs(15)), "rows written: {:?}", super::store::stats());

    let q = |extra: super::store::Query| {
        super::store::query_json(&super::store::Query { from: T0 / 86_400 * 86_400, to: (T0 / 86_400 + 1) * 86_400, ..extra }).unwrap()
    };
    let all = q(Default::default());
    assert!(all.contains(r#""views":3,"visitors":2,"sessions":2,"bounces":1"#), "{all}");
    assert!(all.contains(r#""pages":[["example.com/docs/",1],["example.com/docs/intro/",1],["shop.example/",1]]"#), "{all}");
    assert!(all.contains(r#""utm_campaign":[["launch",1]]"#), "{all}");
    #[cfg(feature = "geo")]
    assert!(all.contains(r#""countries":[["US",2],["NL",1]]"#), "{all}");
    assert!(all.contains(r#""channels":[["direct",2],["campaign",1]]"#), "each page view carries its session's channel: {all}");
    let docs = q(super::store::Query { path: Some("/docs/*".into()), host: Some("example.com".into()), ..Default::default() });
    assert!(docs.contains(r#""views":2,"visitors":1,"sessions":1,"bounces":0"#), "{docs}");
    let campaign = q(super::store::Query { channel: Some("campaign".into()), ..Default::default() });
    assert!(campaign.contains(r#""views":1,"visitors":1"#), "the campaign session only: {campaign}");
    let none = q(super::store::Query { country: Some("FR".into()), ..Default::default() });
    assert!(none.contains(r#""views":0,"visitors":0,"sessions":0"#), "{none}");

    let rows = {
        let admin = admin.clone();
        let schema = schema.clone();
        rws_server::async_bridge::block_on_shared(async move {
            admin.query_rows(&format!("SELECT kind, name, props FROM {schema}.rws_traffic_reports"), &[]).await.unwrap()
        })
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String>("props").unwrap(), "plan=pro");
    let addresses = {
        let admin = admin.clone();
        let schema = schema.clone();
        rws_server::async_bridge::block_on_shared(async move {
            admin.query_rows(&format!("SELECT count(*)::bigint AS n FROM {schema}.rws_traffic_pageviews WHERE path LIKE '%8.8.8.8%' OR host LIKE '%Mozilla%'"), &[]).await.unwrap()
        })
    };
    assert_eq!(addresses[0].get::<i64>("n").unwrap(), 0, "no addresses or user agents in the rows");
    run(format!("DROP SCHEMA {schema} CASCADE"));
}

// ── Traffic page redesign (18.32) ─────────────────────────────────────────────

#[test]
fn day_ranges_and_the_comparison_series() {
    use super::summary::day_range;
    let now = T0 + 5 * 3600 + 30; // 5:00 into a day (T0 is an hour boundary)
    let today = now / 86_400;
    let date = |d: u64| {
        // back to YYYY-MM-DD (the inverse of parse_day) for the test
        let z = d as i64 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let dd = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + (m <= 2) as i64;
        format!("{y:04}-{m:02}-{dd:02}")
    };
    assert_eq!(super::summary::parse_day(&date(today)), Some(today));
    // Today only: from midnight to now.
    let (hours, end) = day_range(&date(today), "", now).unwrap();
    assert_eq!((hours, end), ((now / 3600) - today * 24 + 1, now));
    // Two past days: 48 hours ending at the second's last second.
    let (hours, end) = day_range(&date(today - 3), &date(today - 2), now).unwrap();
    assert_eq!((hours, end), (48, (today - 1) * 86_400 - 1));
    assert!(day_range(&date(today), &date(today - 1), now).is_err());
    assert!(day_range("nope", "", now).is_err());

    let _g = fresh();
    // A page view now and one at the same time a week ago (a week view
    // compares day by day).
    record(&hit("8.8.8.8", CHROME, "/a/", now - 7 * 86_400));
    record(&hit("8.8.8.8", CHROME, "/x.css", now - 7 * 86_400));
    record(&hit("1.1.1.1", CHROME, "/b/", now));
    record(&hit("1.1.1.1", CHROME, "/x.css", now));
    let json = summary(168, None, now + 1);
    let last = |key: &str| crate::mcp::json_rpc::split_array_elements(&crate::mcp::json_rpc::extract_raw(&json, key).unwrap()).last().unwrap().clone();
    assert!(last("series").contains(r#""pages":1"#), "{json}");
    assert!(last("previous_series").contains(r#""pages":1"#), "the same day last week: {json}");
    assert_eq!(last("series").split(',').next(), last("previous_series").split(',').next(), "same bucket times");
    assert!(last("days").contains(r#""visitors":1"#) && last("days").contains(r#""previous_visitors":1"#), "{json}");
}

#[test]
#[ignore]
fn zz_cost_per_request() {
    let _g = fresh();
    let request = crate::request::Request::parse(b"GET /f.txt HTTP/1.1\r\nHost: example.com\r\nUser-Agent: Mozilla/5.0 (X11) Chrome/141.0 Safari/537.36\r\nAccept: */*\r\n\r\n").unwrap();
    let response = crate::response::Response::text(crate::response::STATUS_CODE_REASON_PHRASE.n200_ok, "x");
    let ip: std::net::IpAddr = "203.0.113.5".parse().unwrap();
    let css = crate::request::Request::parse(b"GET /main.css HTTP/1.1\r\nHost: example.com\r\nUser-Agent: Mozilla/5.0 (X11) Chrome/141.0 Safari/537.36\r\n\r\n").unwrap();
    let mut best = u128::MAX;
    let started = std::time::Instant::now();
    for _ in 0..100_000 {
        super::observe(&request, &response, ip);
    }
    eprintln!("COST undecided browser: {} ns per request", started.elapsed().as_nanos() / 100_000);
    super::observe(&css, &response, ip); // loads an asset: a person from here on
    for _ in 0..1000 {
        super::observe(&request, &response, ip);
    }
    for _ in 0..5 {
        let started = std::time::Instant::now();
        for _ in 0..1_000_000 {
            super::observe(&request, &response, ip);
        }
        best = best.min(started.elapsed().as_nanos() / 1_000_000);
    }
    eprintln!("COST decided visitor: {best} ns per request (best of 5 × 1M)");
}

#[test]
#[ignore]
fn zz_cost_parts() {
    let _g = fresh();
    let time = |label: &str, f: &mut dyn FnMut()| {
        let mut best = u128::MAX;
        for _ in 0..5 {
            let started = std::time::Instant::now();
            for _ in 0..1_000_000 {
                f();
            }
            best = best.min(started.elapsed().as_nanos() / 1_000_000);
        }
        eprintln!("COST {label}: {best} ns");
    };
    let request = crate::request::Request::parse(b"GET /f.txt HTTP/1.1\r\nHost: example.com\r\nUser-Agent: Mozilla/5.0 (X11) Chrome/141.0 Safari/537.36\r\nAccept: */*\r\n\r\n").unwrap();
    let ip: std::net::IpAddr = "203.0.113.5".parse().unwrap();
    time("now", &mut || { std::hint::black_box(super::record::now()); });
    time("3 headers", &mut || { std::hint::black_box((request.header("host"), request.header("user-agent"), request.header("referer"))); });
    let mut h = std::collections::hash_map::RandomState::new();
    time("siphash ip+ua", &mut || {
        use std::hash::{BuildHasher, Hash, Hasher};
        let mut s = h.build_hasher(); 1u64.hash(&mut s); ip.hash(&mut s); "Mozilla/5.0 (X11) Chrome/141.0 Safari/537.36".hash(&mut s); std::hint::black_box(s.finish());
    });
    let _ = &mut h;
}
