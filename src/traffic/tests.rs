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
    super::persist::save(&file).unwrap();
    let text = std::fs::read_to_string(&*file).unwrap();
    assert!(text.starts_with(r#"{"version":1,"hosts":["example.com"],"hours":[["#), "{text}");
    assert!(!text.contains("203.0.113") && !text.contains("Chrome"), "no addresses or user agents saved");
    reset_for_tests();
    assert_eq!(totals(&summary(24, None, T0 + 60), "people").0, 0);
    super::persist::load(&file);
    assert_eq!(summary(24, None, T0 + 60), before);
    std::fs::write(&*file, "not json").unwrap();
    super::persist::load(&file); // reported, ignored
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
