//! The docs index (one JSON file per docs version, written by
//! site/scripts/mcp-index.js) and a BM25 search over its sections.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Section {
    pub id: String,
    pub heading: String,
    #[allow(dead_code)] // in the index for clients that nest sections
    pub depth: u8,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Page {
    pub path: String,
    pub group: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub markdown: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Setting {
    pub env: String,
    pub key: String,
    pub default: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiItem {
    pub name: String,
    pub module: String,
    pub description: String,
    pub group: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Example {
    pub name: String,
    pub file: String,
    pub description: String,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDocs {
    pub version: String,
    pub tag: String,
    #[allow(dead_code)]
    pub latest: bool,
    pub pages: Vec<Page>,
    pub settings: Vec<Setting>,
    pub api: Vec<ApiItem>,
    pub examples: Vec<Example>,
    /// agents/skills/rws/SKILL.md at this version ("" before it existed).
    #[serde(default)]
    pub skill: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub version: String,
    pub title: String,
    pub markdown: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    latest: String,
    versions: Vec<ManifestVersion>,
    changelog: Vec<Release>,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestVersion {
    version: String,
}

/// Every docs version, with a search index per version.
pub struct Docs {
    pub latest: String,
    /// Newest first, as the manifest lists them.
    pub order: Vec<String>,
    pub versions: HashMap<String, (VersionDocs, Search)>,
    pub changelog: Vec<Release>,
}

impl Docs {
    pub fn load(dir: &Path) -> Result<Docs, String> {
        let read = |name: &str| {
            std::fs::read_to_string(dir.join(name)).map_err(|e| format!("{}: {e}", dir.join(name).display()))
        };
        let manifest: Manifest = serde_json::from_str(&read("index.json")?).map_err(|e| format!("index.json: {e}"))?;
        let mut versions = HashMap::new();
        for v in &manifest.versions {
            let docs: VersionDocs =
                serde_json::from_str(&read(&format!("{}.json", v.version))?).map_err(|e| format!("{}.json: {e}", v.version))?;
            let search = Search::new(&docs);
            versions.insert(v.version.clone(), (docs, search));
        }
        Ok(Docs {
            latest: manifest.latest,
            order: manifest.versions.into_iter().map(|v| v.version).collect(),
            versions,
            changelog: manifest.changelog,
        })
    }

    /// `None`/"latest" → the newest; "18.7" or "18.7.2" → 18.7.
    pub fn version(&self, wanted: Option<&str>) -> Result<&(VersionDocs, Search), String> {
        let key = match wanted.map(str::trim).filter(|w| !w.is_empty() && *w != "latest") {
            None => self.latest.clone(),
            Some(w) => {
                let w = w.trim_start_matches('v');
                let mut parts = w.split('.');
                match (parts.next(), parts.next()) {
                    (Some(major), Some(minor)) => format!("{major}.{minor}"),
                    _ => w.to_string(),
                }
            }
        };
        self.versions
            .get(&key)
            .ok_or_else(|| format!("no docs for {key}; available: {} (latest {})", self.order.join(", "), self.latest))
    }
}

const STOP: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "can", "do", "does", "for", "from", "how", "i", "in", "into", "is", "it",
    "my", "of", "on", "or", "the", "to", "use", "what", "when", "with", "add", "set", "get", "make", "you", "your",
];

/// Lower-cased words, plurals folded ("parameters" = "parameter",
/// "proxies" = "proxy"), common words dropped.
fn tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| t.len() > 1)
        .map(|t| t.to_lowercase())
        .filter(|t| !STOP.contains(&t.as_str()))
        .map(|t| {
            if let Some(stem) = t.strip_suffix("ies").filter(|s| s.len() > 2) {
                format!("{stem}y")
            } else if t.len() > 3 && t.ends_with('s') && !t.ends_with("ss") && !t.ends_with("us") {
                t[..t.len() - 1].to_string()
            } else {
                t
            }
        })
}

/// BM25 over sections; a section's heading and its page title count three
/// times, so a query that names a feature finds the section about it.
pub struct Search {
    /// (page index, section index) per document.
    docs: Vec<(usize, usize)>,
    lengths: Vec<f64>,
    average: f64,
    postings: HashMap<String, Vec<(u32, u32)>>,
}

pub struct Hit {
    pub page: usize,
    pub section: usize,
    pub score: f64,
}

impl Search {
    pub fn new(v: &VersionDocs) -> Search {
        let mut docs = Vec::new();
        let mut lengths = Vec::new();
        let mut postings: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
        for (p, page) in v.pages.iter().enumerate() {
            for (s, section) in page.sections.iter().enumerate() {
                let id = docs.len() as u32;
                let mut counts: HashMap<String, u32> = HashMap::new();
                let mut length = 0u32;
                for (text, weight) in [(section.heading.as_str(), 3), (page.title.as_str(), 3), (section.text.as_str(), 1)] {
                    for t in tokens(text) {
                        *counts.entry(t).or_default() += weight;
                        length += weight;
                    }
                }
                for (t, n) in counts {
                    postings.entry(t).or_default().push((id, n));
                }
                docs.push((p, s));
                lengths.push(f64::from(length));
            }
        }
        let average = if lengths.is_empty() { 1.0 } else { lengths.iter().sum::<f64>() / lengths.len() as f64 };
        Search { docs, lengths, average, postings }
    }

    pub fn query(&self, query: &str, limit: usize) -> Vec<Hit> {
        const K1: f64 = 1.2;
        const B: f64 = 0.75;
        let n = self.docs.len() as f64;
        let mut scores: HashMap<u32, f64> = HashMap::new();
        let mut seen = std::collections::HashSet::new();
        for term in tokens(query) {
            if !seen.insert(term.clone()) {
                continue;
            }
            let Some(list) = self.postings.get(&term) else { continue };
            let df = list.len() as f64;
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
            for &(doc, tf) in list {
                let tf = f64::from(tf);
                let norm = 1.0 - B + B * self.lengths[doc as usize] / self.average;
                *scores.entry(doc).or_default() += idf * tf * (K1 + 1.0) / (tf + K1 * norm);
            }
        }
        let mut hits: Vec<Hit> = scores
            .into_iter()
            .map(|(doc, score)| {
                let (page, section) = self.docs[doc as usize];
                Hit { page, section, score }
            })
            .collect();
        hits.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.page.cmp(&b.page)).then(a.section.cmp(&b.section)));
        hits.truncate(limit);
        hits
    }
}

/// The first `max` characters of a section, cut at a word.
pub fn excerpt(text: &str, max: usize) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let cut: String = flat.chars().take(max).collect();
    match cut.rfind(' ') {
        Some(i) => format!("{}…", &cut[..i]),
        None => format!("{cut}…"),
    }
}
