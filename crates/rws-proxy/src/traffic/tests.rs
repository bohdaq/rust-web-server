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
        Kind::SocialPreview, Kind::Feed, Kind::Archiver, Kind::Scanner, Kind::Spoofed, Kind::Tool, Kind::NoUserAgent, Kind::Local] {
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
