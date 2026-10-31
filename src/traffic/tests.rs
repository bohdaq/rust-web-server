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
