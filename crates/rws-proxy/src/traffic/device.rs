//! Browser, operating system and device class of a person's browser, from
//! its User-Agent — and from Chromium's default client hints
//! (`Sec-CH-UA-Platform`, `Sec-CH-UA-Mobile`), which win when present
//! because Chromium freezes those parts of its User-Agent. Families only;
//! versions aren't kept.
//!
//! Known limit: iPad Safari sends a macOS desktop User-Agent, so it counts
//! as a desktop Mac.

/// What a browser runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Device {
    pub browser: &'static str,
    pub os: &'static str,
    pub class: &'static str,
}

fn has(ua: &str, token: &str) -> bool {
    ua.contains(token)
}

/// The browser family. Order matters: most browsers also say "Chrome" and
/// "Safari", so the more specific tokens come first.
fn browser(ua: &str) -> &'static str {
    const BROWSERS: &[(&str, &str)] = &[
        ("Edg/", "Edge"),
        ("EdgA/", "Edge"),
        ("EdgiOS/", "Edge"),
        ("OPR/", "Opera"),
        ("OPiOS/", "Opera"),
        ("Opera", "Opera"),
        ("YaBrowser/", "Yandex"),
        ("SamsungBrowser/", "Samsung Internet"),
        ("UCBrowser/", "UC Browser"),
        ("Vivaldi/", "Vivaldi"),
        ("Brave", "Brave"),
        ("DuckDuckGo/", "DuckDuckGo"),
        ("FxiOS/", "Firefox"),
        ("Firefox/", "Firefox"),
        ("CriOS/", "Chrome"),
        ("Chrome/", "Chrome"),
        ("Chromium/", "Chrome"),
        ("Safari/", "Safari"),
    ];
    BROWSERS.iter().find(|(token, _)| has(ua, token)).map_or("Other", |(_, name)| name)
}

fn os(ua: &str) -> &'static str {
    if has(ua, "Windows") {
        "Windows"
    } else if has(ua, "iPhone") || has(ua, "iPad") || has(ua, "iPod") {
        "iOS"
    } else if has(ua, "Android") {
        "Android"
    } else if has(ua, "CrOS") {
        "ChromeOS"
    } else if has(ua, "Mac OS X") || has(ua, "Macintosh") {
        "macOS"
    } else if has(ua, "Linux") || has(ua, "X11") {
        "Linux"
    } else {
        "Other"
    }
}

fn class(ua: &str, os: &str) -> &'static str {
    if has(ua, "iPad") || has(ua, "Tablet") || (os == "Android" && !has(ua, "Mobile")) {
        "tablet"
    } else if has(ua, "Mobi") || has(ua, "iPhone") || has(ua, "iPod") || os == "Android" {
        "mobile"
    } else {
        "desktop"
    }
}

/// `Sec-CH-UA-Platform` (a quoted string) as an OS family.
fn os_from_hint(platform: &str) -> Option<&'static str> {
    Some(match platform.trim().trim_matches('"') {
        "Windows" => "Windows",
        "macOS" => "macOS",
        "Android" => "Android",
        "iOS" => "iOS",
        "Chrome OS" | "ChromeOS" => "ChromeOS",
        "Linux" => "Linux",
        "" => return None,
        _ => "Other",
    })
}

/// The device of a browser: `user_agent` and, when sent, the
/// `Sec-CH-UA-Platform` and `Sec-CH-UA-Mobile` hints.
pub fn parse(user_agent: &str, platform_hint: Option<&str>, mobile_hint: Option<&str>) -> Device {
    let os = platform_hint.and_then(os_from_hint).unwrap_or_else(|| os(user_agent));
    let mut class = class(user_agent, os);
    // `?1`: a phone; `?0`: not one (a desktop, or a tablet in desktop mode).
    match mobile_hint.map(str::trim) {
        Some("?1") => class = "mobile",
        Some("?0") if class == "mobile" => class = "desktop",
        _ => {}
    }
    Device { browser: browser(user_agent), os, class }
}
