//! Native apps and browser extensions (18.35): their requests are the
//! site's users, not bots, but they never load a page's stylesheets, so the
//! browser test can't find them. They're recognised here instead:
//!
//! - `[traffic] apps` (`name=pattern`, comma-separated): a pattern with
//!   `://` matches the start of the request's `Origin` (an extension:
//!   `chrome-extension://<id>`), any other pattern is looked for in the
//!   `User-Agent`, ignoring case (`HITOP/` in `HITOP/2.3 CFNetwork/…`);
//!   after a pattern ending in `/` the version is read;
//! - without a configured match, by platform: an extension `Origin`
//!   (`chrome-extension://`, `moz-extension://`, `safari-web-extension://`),
//!   Apple's networking stack (`CFNetwork/… Darwin/…`, `NetworkingExtension/`),
//!   Android's (`Dalvik/`) and Electron (`Electron/`) — counted under a
//!   name for the platform ("iOS/macOS app"), since anything in a request
//!   can be made up and only configured names are kept as names.
//!
//! An app's own events go to `/_rws/e` ([`super::record::app_event`]).

use std::sync::{OnceLock, RwLock};

/// The most `[traffic] apps` entries read.
pub const MAX_APPS: usize = 64;

/// A recognised app client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    /// The configured name, or the platform's (`"iOS/macOS app"`).
    pub name: &'static str,
    /// `ios`, `macos`, `apple`, `android`, `windows`, `linux`, `chrome`,
    /// `edge`, `firefox`, `safari`, or `other`.
    pub platform: &'static str,
    /// Read from the `User-Agent` after a configured pattern ending in `/`.
    pub version: Option<String>,
}

/// One `[traffic] apps` entry.
#[derive(Clone, Debug)]
struct Pattern {
    name: &'static str,
    /// Lowercase.
    pattern: String,
    origin: bool,
}

fn patterns() -> &'static RwLock<Vec<Pattern>> {
    static PATTERNS: OnceLock<RwLock<Vec<Pattern>>> = OnceLock::new();
    PATTERNS.get_or_init(|| RwLock::new(parse(&super::config::apps())))
}

/// Configured names are kept for the process's life: there are at most
/// [`MAX_APPS`], set from the configuration, so leaking them is bounded.
fn intern(name: &str) -> &'static str {
    static NAMES: OnceLock<std::sync::Mutex<Vec<&'static str>>> = OnceLock::new();
    let mut names = NAMES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(n) = names.iter().find(|n| **n == name) {
        return n;
    }
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    names.push(leaked);
    leaked
}

fn parse(entries: &[String]) -> Vec<Pattern> {
    entries
        .iter()
        .filter_map(|entry| {
            let (name, pattern) = entry.split_once('=')?;
            let (name, pattern) = (name.trim(), pattern.trim());
            if name.is_empty() || pattern.is_empty() || name.len() > 64 {
                return None;
            }
            let name: String = name.chars().filter(|c| !c.is_control() && *c != '\t').collect();
            Some(Pattern { name: intern(&name), pattern: pattern.to_ascii_lowercase(), origin: pattern.contains("://") })
        })
        .take(MAX_APPS)
        .collect()
}

/// Replace the configured apps (a configuration reload, tests).
pub fn set_apps(entries: &[String]) {
    *patterns().write().unwrap_or_else(|e| e.into_inner()) = parse(entries);
}

/// Whether `name` is a configured app (an app's event must name one).
pub fn configured(name: &str) -> Option<&'static str> {
    patterns().read().unwrap_or_else(|e| e.into_inner()).iter().find(|p| p.name.eq_ignore_ascii_case(name)).map(|p| p.name)
}

/// Whether any app is configured.
pub fn any_configured() -> bool {
    !patterns().read().unwrap_or_else(|e| e.into_inner()).is_empty()
}

fn extension_platform(origin: &str, ua: &str) -> Option<&'static str> {
    if origin.starts_with("chrome-extension://") {
        Some(if ua.contains(" edg/") { "edge" } else { "chrome" })
    } else if origin.starts_with("moz-extension://") {
        Some("firefox")
    } else if origin.starts_with("safari-web-extension://") {
        Some("safari")
    } else {
        None
    }
}

fn os_of(ua: &str) -> &'static str {
    if ua.contains("windows") {
        "windows"
    } else if ua.contains("iphone") || ua.contains("ipad") || ua.contains(" ios/") {
        "ios"
    } else if ua.contains("mac os x") || ua.contains("macintosh") || ua.contains("macos/") {
        "macos"
    } else if ua.contains("android") || ua.contains("dalvik/") {
        "android"
    } else if ua.contains("linux") {
        "linux"
    } else if ua.contains("cfnetwork/") || ua.contains("darwin/") {
        "apple"
    } else {
        "other"
    }
}

/// The version after `pattern` (ending in `/`) in `ua` (both lowercase).
fn version_after(ua_original: &str, ua: &str, pattern: &str) -> Option<String> {
    if !pattern.ends_with('/') {
        return None;
    }
    let at = ua.find(pattern)? + pattern.len();
    let version: String = ua_original.get(at..)?.chars().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+')).take(32).collect();
    (!version.is_empty()).then_some(version)
}

/// The app a request comes from, if any.
pub fn detect(user_agent: Option<&str>, origin: Option<&str>) -> Option<App> {
    let original = user_agent.unwrap_or("").trim();
    let ua = original.to_ascii_lowercase();
    let origin = origin.unwrap_or("").trim().to_ascii_lowercase();
    {
        let patterns = patterns().read().unwrap_or_else(|e| e.into_inner());
        for p in patterns.iter() {
            if p.origin {
                if !origin.is_empty() && origin.starts_with(&p.pattern) {
                    let platform = extension_platform(&origin, &ua).unwrap_or_else(|| os_of(&ua));
                    return Some(App { name: p.name, platform, version: None });
                }
            } else if ua.contains(&p.pattern) {
                return Some(App { name: p.name, platform: os_of(&ua), version: version_after(original, &ua, &p.pattern) });
            }
        }
    }
    if let Some(platform) = extension_platform(&origin, &ua) {
        let name = match platform {
            "edge" => "Edge extension",
            "firefox" => "Firefox extension",
            "safari" => "Safari extension",
            _ => "Chrome extension",
        };
        return Some(App { name, platform, version: None });
    }
    if (ua.contains("cfnetwork/") && ua.contains("darwin/")) || ua.starts_with("networkingextension/") {
        return Some(App { name: "iOS/macOS app", platform: os_of(&ua), version: None });
    }
    if ua.starts_with("dalvik/") {
        return Some(App { name: "Android app", platform: "android", version: None });
    }
    if ua.contains(" electron/") {
        return Some(App { name: "Desktop app (Electron)", platform: os_of(&ua), version: None });
    }
    None
}
