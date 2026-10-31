use std::net::IpAddr;
use std::sync::OnceLock;

use super::data::{AI_AGENTS, CRAWLERS, RANGES};

/// The three groups the Traffic page shows (and this server's own calls).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    /// Browsers that behave like a person's.
    Person,
    /// AI crawlers, AI search and AI assistants fetching for someone.
    Ai,
    /// Everything else automated.
    Bot,
    /// This machine (health checks, updaters, the admin UI's own calls).
    Internal,
}

/// What a client is, more precisely.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// A browser User-Agent; a person if it loads page assets.
    Browser,
    AiTraining,
    AiSearch,
    /// An assistant fetching a page because someone asked it to.
    AiAssistant,
    AiAgent,
    SearchEngine,
    Seo,
    Monitoring,
    SocialPreview,
    Feed,
    Archiver,
    /// Probing for secrets and admin pages (`/.env`, `/wp-login.php`).
    Scanner,
    /// Claims to be a crawler whose published network it isn't from.
    Spoofed,
    /// A library or tool (`curl`, `python-requests`) or an unknown bot.
    Tool,
    NoUserAgent,
    Local,
}

impl Kind {
    pub fn group(self) -> Group {
        match self {
            Kind::Browser => Group::Person,
            Kind::AiTraining | Kind::AiSearch | Kind::AiAssistant | Kind::AiAgent => Group::Ai,
            Kind::Local => Group::Internal,
            _ => Group::Bot,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Browser => "browser",
            Kind::AiTraining => "ai-training",
            Kind::AiSearch => "ai-search",
            Kind::AiAssistant => "ai-assistant",
            Kind::AiAgent => "ai-agent",
            Kind::SearchEngine => "search-engine",
            Kind::Seo => "seo",
            Kind::Monitoring => "monitoring",
            Kind::SocialPreview => "social-preview",
            Kind::Feed => "feed",
            Kind::Archiver => "archiver",
            Kind::Scanner => "scanner",
            Kind::Spoofed => "spoofed",
            Kind::Tool => "tool",
            Kind::NoUserAgent => "no-user-agent",
            Kind::Local => "local",
        }
    }

    fn from_label(label: &str) -> Kind {
        match label {
            "ai-training" => Kind::AiTraining,
            "ai-search" => Kind::AiSearch,
            "ai-assistant" => Kind::AiAssistant,
            "ai-agent" => Kind::AiAgent,
            "search-engine" | "fetcher" => Kind::SearchEngine,
            "seo" => Kind::Seo,
            "monitoring" => Kind::Monitoring,
            "social-preview" => Kind::SocialPreview,
            "feed" => Kind::Feed,
            "archiver" => Kind::Archiver,
            "scanner" => Kind::Scanner,
            _ => Kind::Tool,
        }
    }
}

/// A classified client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Class {
    pub kind: Kind,
    /// The bot's or agent's name (`GPTBot`, `Googlebot`), empty for browsers.
    pub name: &'static str,
    /// Who runs it, when known (`OpenAI`).
    pub operator: &'static str,
    /// The client's address is in the network its operator publishes.
    pub verified: bool,
}

impl Class {
    pub fn group(&self) -> Group {
        self.kind.group()
    }

    fn of(kind: Kind, name: &'static str, operator: &'static str, verified: bool) -> Class {
        Class { kind, name, operator, verified }
    }
}

/// One published crawler network, parsed.
struct Network {
    name: &'static str,
    kind: Kind,
    /// The User-Agent token it verifies (`googlebot`), may be empty.
    token: &'static str,
    v4: Vec<(u32, u32)>,
    v6: Vec<(u128, u128)>,
}

impl Network {
    fn contains(&self, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let ip = u32::from(v4);
                self.v4.iter().any(|(net, mask)| ip & mask == *net)
            }
            IpAddr::V6(v6) => {
                if let Some(v4) = v6.to_ipv4_mapped() {
                    return self.contains(IpAddr::V4(v4));
                }
                let ip = u128::from(v6);
                self.v6.iter().any(|(net, mask)| ip & mask == *net)
            }
        }
    }
}

fn networks() -> &'static [Network] {
    static NETWORKS: OnceLock<Vec<Network>> = OnceLock::new();
    NETWORKS.get_or_init(|| {
        RANGES
            .iter()
            .map(|(name, _group, kind, token, prefixes)| {
                let mut network = Network { name, kind: Kind::from_label(kind), token, v4: Vec::new(), v6: Vec::new() };
                for prefix in *prefixes {
                    let Some((address, bits)) = prefix.split_once('/') else { continue };
                    let Ok(bits) = bits.parse::<u32>() else { continue };
                    match address.parse::<IpAddr>() {
                        Ok(IpAddr::V4(a)) if bits <= 32 => {
                            let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits) };
                            network.v4.push((u32::from(a) & mask, mask));
                        }
                        Ok(IpAddr::V6(a)) if bits <= 128 => {
                            let mask = if bits == 0 { 0 } else { u128::MAX << (128 - bits) };
                            network.v6.push((u128::from(a) & mask, mask));
                        }
                        _ => {}
                    }
                }
                network
            })
            .collect()
    })
}

/// Requests only scanners make: secrets, VCS metadata, other servers'
/// admin pages. Checked on the lowercase path.
const SCANNER_PATHS: &[&str] = &[
    "/.env", "/.git/", "/.aws/", "/.ssh/", "/.svn/", "/.hg/", "/.ds_store", "/wp-login.php", "/wp-admin", "/wp-content/",
    "/wp-includes/", "/xmlrpc.php", "/phpmyadmin", "/pma/", "/cgi-bin/", "/boaform", "/hnap1", "/actuator/", "/server-status",
    "/vendor/phpunit", "/.well-known/acme-challenge/../", "/config.json", "/config.php", "/owa/", "/ecp/", "/solr/", "/druid/",
    "/telescope/", "/_ignition/", "/debug/default/view", "/console/", "/manager/html", "/geoserver/", "/autodiscover/",
];

fn scanner_path(path: &str) -> bool {
    let path = path.split('?').next().unwrap_or(path).to_ascii_lowercase();
    SCANNER_PATHS.iter().any(|p| path.starts_with(p) || path.contains(p))
        || path.ends_with(".sql")
        || path.ends_with(".bak")
}

fn browser(ua: &str) -> bool {
    ua.starts_with("mozilla/") && ["chrome/", "safari/", "firefox/", "edg/", "opr/"].iter().any(|t| ua.contains(t))
}

/// What the client at `ip`, sending `user_agent`, asking for `path`, is.
pub fn classify(ip: IpAddr, user_agent: Option<&str>, path: &str) -> Class {
    if ip.is_loopback() || ip.to_canonical().is_loopback() {
        return Class::of(Kind::Local, "", "", true);
    }
    let ua = user_agent.map(|u| u.trim().to_ascii_lowercase()).unwrap_or_default();
    // A published crawler network: verified, whatever the User-Agent says.
    let network = networks().iter().find(|n| n.contains(ip));
    if let Some(network) = network {
        return Class::of(network.kind, network.name, operator_of(network.name), true);
    }
    // Claims a crawler that publishes its network, from outside it.
    if let Some(claimed) = networks().iter().find(|n| !n.token.is_empty() && ua.contains(n.token)) {
        return Class::of(Kind::Spoofed, claimed.name, operator_of(claimed.name), false);
    }
    if scanner_path(path) {
        return Class::of(Kind::Scanner, "", "", false);
    }
    if ua.is_empty() {
        return Class::of(Kind::NoUserAgent, "", "", false);
    }
    if let Some((_, name, operator, kind)) = AI_AGENTS.iter().find(|(token, ..)| ua.contains(token)) {
        return Class::of(Kind::from_label(kind), name, operator, false);
    }
    if let Some((_, _, name, kind)) = CRAWLERS.iter().find(|(anchored, literal, ..)| if *anchored { ua.starts_with(literal) } else { ua.contains(literal) }) {
        return Class::of(Kind::from_label(kind), name, "", false);
    }
    if browser(&ua) {
        return Class::of(Kind::Browser, "", "", false);
    }
    Class::of(Kind::Tool, "", "", false)
}

fn operator_of(network: &str) -> &'static str {
    match network {
        "GPTBot" | "OAI-SearchBot" | "ChatGPT-User" => "OpenAI",
        "PerplexityBot" | "Perplexity-User" => "Perplexity",
        "CCBot" => "Common Crawl",
        "Bingbot" => "Microsoft",
        "Applebot" => "Apple",
        "DuckDuckBot" => "DuckDuckGo",
        _ => "Google",
    }
}
