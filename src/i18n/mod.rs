//! Translations (18.16.0, #33): message catalogs, one per language, and the
//! language each request asks for.
//!
//! ```rust
//! use rust_web_server::i18n::Catalogs;
//!
//! let catalogs = Catalogs::new("en")
//!     .add("en", r#"
//!         greeting = "Hello, {name}!"
//!         items.one = "{count} item"
//!         items.other = "{count} items"
//!     "#).unwrap()
//!     .add("uk", r#"
//!         greeting = "Привіт, {name}!"
//!         items.one = "{count} річ"
//!         items.few = "{count} речі"
//!         items.many = "{count} речей"
//!     "#).unwrap();
//!
//! // In a handler: catalogs.for_request(&request) reads Accept-Language.
//! let t = catalogs.translator("uk-UA,uk;q=0.9,en;q=0.5");
//! assert_eq!("uk", t.locale());
//! assert_eq!("Привіт, Олена!", t.t_with("greeting", &[("name", "Олена")]));
//! assert_eq!("3 речі", t.plural("items", 3));
//! assert_eq!("5 речей", t.plural("items", 5));
//! ```
//!
//! Catalog files are a flat subset of TOML (`key = "value"`, `#` comments,
//! `\n \t \" \\ \u{…}` escapes), one per language: `Catalogs::from_dir`
//! reads `<dir>/<language>.toml`. A key missing in the chosen language falls
//! back to its base language (`en-GB` → `en`), then the default language,
//! then the key itself.

use std::collections::HashMap;

use crate::request::Request;
use crate::response::Response;

/// At most this many `Accept-Language` entries are considered (the header
/// is the client's to make arbitrarily long).
pub const MAX_LANGUAGE_RANGES: usize = 32;

/// A client's language preferences, from `Accept-Language`: tags in the
/// order to try them (highest `q` first, then header order), lower-cased.
/// Ranges with `q=0` are left out; `*` is kept as `"*"`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AcceptLanguage(pub Vec<String>);

impl AcceptLanguage {
    /// Parse an `Accept-Language` value. Never fails: malformed entries are
    /// skipped.
    pub fn parse(value: &str) -> AcceptLanguage {
        let mut ranges: Vec<(u16, usize, String)> = Vec::new();
        for (index, entry) in value.split(',').take(MAX_LANGUAGE_RANGES).enumerate() {
            let mut parts = entry.split(';');
            let tag = parts.next().unwrap_or("").trim();
            if tag.is_empty() || tag.len() > 35 || !(tag == "*" || tag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')) {
                continue;
            }
            let mut q = 1000u16;
            for parameter in parts {
                if let Some(value) = parameter.trim().strip_prefix("q=").or_else(|| parameter.trim().strip_prefix("Q=")) {
                    q = parse_q(value.trim()).unwrap_or(0);
                }
            }
            if q > 0 {
                ranges.push((q, index, tag.to_ascii_lowercase()));
            }
        }
        ranges.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        AcceptLanguage(ranges.into_iter().map(|(_, _, tag)| tag).collect())
    }

    /// The request's preferences (empty without the header).
    pub fn from_request(request: &Request) -> AcceptLanguage {
        request.header(crate::header::Header::_ACCEPT_LANGUAGE).map(|h| AcceptLanguage::parse(&h.value)).unwrap_or_default()
    }

    /// The best of `available` for these preferences (RFC 4647 lookup:
    /// each range as is, then with subtags removed from the end; then a
    /// range's language matching an available regional tag — `en` gets
    /// `en-GB`), or `None`. `*` matches nothing in particular, so the caller's
    /// default applies.
    pub fn negotiate<'a>(&self, available: &[&'a str]) -> Option<&'a str> {
        for range in &self.0 {
            let mut candidate = range.as_str();
            loop {
                if let Some(found) = available.iter().find(|a| a.eq_ignore_ascii_case(candidate)) {
                    return Some(found);
                }
                match candidate.rfind('-') {
                    Some(cut) => candidate = &candidate[..cut],
                    None => break,
                }
            }
        }
        for range in &self.0 {
            let language = range.split('-').next().unwrap_or("");
            if language == "*" {
                continue;
            }
            if let Some(found) = available.iter().find(|a| a.split('-').next().is_some_and(|l| l.eq_ignore_ascii_case(language))) {
                return Some(found);
            }
        }
        None
    }
}

impl crate::extract::FromRequest for AcceptLanguage {
    fn from_request(request: &Request) -> Result<Self, Response> {
        Ok(AcceptLanguage::from_request(request))
    }
}

/// `q` as thousandths (`0.8` → 800); `None` when it isn't a valid qvalue.
fn parse_q(value: &str) -> Option<u16> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if fraction.len() > 3 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = format!("{fraction:0<3}");
    match whole {
        "0" => digits.parse().ok(),
        "1" if digits == "000" => Some(1000),
        _ => None,
    }
}

/// One language's messages.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    messages: HashMap<String, String>,
}

impl Catalog {
    /// Parse catalog text (`key = "value"` lines). Errors name the line.
    pub fn parse(text: &str) -> Result<Catalog, String> {
        let mut messages = HashMap::new();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let at = |what: &str| format!("line {}: {what}", number + 1);
            let (key, value) = line.split_once('=').ok_or_else(|| at("expected key = \"value\""))?;
            let key = key.trim().trim_matches('"').trim();
            if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')) {
                return Err(at(&format!("{key:?} isn't a key (letters, digits, _ - .)")));
            }
            let value = parse_string(value.trim()).map_err(|e| at(&e))?;
            messages.insert(key.to_string(), value);
        }
        Ok(Catalog { messages })
    }

    /// The message for `key`, as written.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.messages.get(key).map(String::as_str)
    }

    /// How many messages it has.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

/// A quoted TOML basic string (`"…"`) or literal string (`'…'`), with an
/// optional `# comment` after it.
fn parse_string(raw: &str) -> Result<String, String> {
    let mut chars = raw.chars();
    let quote = chars.next().filter(|c| *c == '"' || *c == '\'').ok_or("the value must be quoted")?;
    let mut out = String::new();
    loop {
        let c = chars.next().ok_or("the value has no closing quote")?;
        if c == quote {
            break;
        }
        if c != '\\' || quote == '\'' {
            out.push(c);
            continue;
        }
        match chars.next().ok_or("the value ends in a backslash")? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                let code = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).ok_or("bad \\u escape")?;
                out.push(code);
            }
            other => return Err(format!("unknown escape \\{other}")),
        }
    }
    let rest = chars.as_str().trim();
    if !rest.is_empty() && !rest.starts_with('#') {
        return Err(format!("unexpected {rest:?} after the value"));
    }
    Ok(out)
}

/// Every language's catalog and the default language.
#[derive(Debug, Clone)]
pub struct Catalogs {
    default: String,
    catalogs: Vec<(String, Catalog)>,
}

impl Catalogs {
    /// No catalogs yet; `default` is the language used when a request asks
    /// for none of them (and for keys a language lacks).
    pub fn new(default: &str) -> Catalogs {
        Catalogs { default: default.to_string(), catalogs: Vec::new() }
    }

    /// Add (or replace) `language`'s catalog from its text.
    pub fn add(mut self, language: &str, text: &str) -> Result<Catalogs, String> {
        let catalog = Catalog::parse(text).map_err(|e| format!("{language}: {e}"))?;
        self.catalogs.retain(|(l, _)| !l.eq_ignore_ascii_case(language));
        self.catalogs.push((language.to_string(), catalog));
        Ok(self)
    }

    /// Every `<language>.toml` in `dir`; `default` as in [`Catalogs::new`].
    pub fn from_dir(dir: impl AsRef<std::path::Path>, default: &str) -> Result<Catalogs, String> {
        let dir = dir.as_ref();
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("can't read {}: {e}", dir.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        entries.sort();
        let mut catalogs = Catalogs::new(default);
        for path in entries {
            let Some(language) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            let text = std::fs::read_to_string(&path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
            catalogs = catalogs.add(language, &text).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        if catalogs.catalogs.is_empty() {
            return Err(format!("no <language>.toml catalogs in {}", dir.display()));
        }
        Ok(catalogs)
    }

    /// The languages with a catalog.
    pub fn languages(&self) -> Vec<&str> {
        self.catalogs.iter().map(|(l, _)| l.as_str()).collect()
    }

    /// The language to answer in for these preferences.
    pub fn negotiate(&self, preferences: &AcceptLanguage) -> &str {
        preferences.negotiate(&self.languages()).unwrap_or(&self.default)
    }

    /// A translator for the request's `Accept-Language`.
    pub fn for_request(&self, request: &Request) -> Translator<'_> {
        self.for_preferences(&AcceptLanguage::from_request(request))
    }

    /// A translator for an `Accept-Language` value.
    pub fn translator(&self, accept_language: &str) -> Translator<'_> {
        self.for_preferences(&AcceptLanguage::parse(accept_language))
    }

    /// A translator for parsed preferences.
    pub fn for_preferences(&self, preferences: &AcceptLanguage) -> Translator<'_> {
        Translator { catalogs: self, locale: self.negotiate(preferences).to_string() }
    }

    /// A translator for one language (a user's saved choice, say).
    pub fn for_language(&self, language: &str) -> Translator<'_> {
        Translator { catalogs: self, locale: language.to_string() }
    }

    fn catalog(&self, language: &str) -> Option<&Catalog> {
        self.catalogs.iter().find(|(l, _)| l.eq_ignore_ascii_case(language)).map(|(_, c)| c)
    }

    /// `key` in `language`, then its base language, then the default.
    fn lookup(&self, language: &str, key: &str) -> Option<&str> {
        let base = language.split('-').next().unwrap_or(language);
        [language, base, self.default.as_str()].into_iter().find_map(|l| self.catalog(l)?.get(key))
    }
}

/// Messages in one language.
#[derive(Debug, Clone)]
pub struct Translator<'a> {
    catalogs: &'a Catalogs,
    locale: String,
}

impl Translator<'_> {
    /// The language chosen (for `Content-Language`, `<html lang>`).
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// The message for `key`; the key itself when no catalog has it.
    pub fn t(&self, key: &str) -> String {
        self.t_with(key, &[])
    }

    /// The message for `key` with `{name}` placeholders filled from `args`
    /// (`{{` and `}}` are literal braces; unknown names stay as written).
    pub fn t_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        match self.catalogs.lookup(&self.locale, key) {
            Some(message) => format_message(message, args),
            None => key.to_string(),
        }
    }

    /// `key.<category>` for `count` in this language (`one`, `few`, `many`,
    /// `other`; see [`plural_category`]), falling back to `key.other`, with
    /// `{count}` and `args` filled in.
    pub fn plural(&self, key: &str, count: u64) -> String {
        self.plural_with(key, count, &[])
    }

    /// [`Translator::plural`] with more placeholders.
    pub fn plural_with(&self, key: &str, count: u64, args: &[(&str, &str)]) -> String {
        let count_text = count.to_string();
        let mut all: Vec<(&str, &str)> = vec![("count", &count_text)];
        all.extend_from_slice(args);
        let category = plural_category(&self.locale, count);
        let chosen = format!("{key}.{category}");
        match self.catalogs.lookup(&self.locale, &chosen).or_else(|| self.catalogs.lookup(&self.locale, &format!("{key}.other"))) {
            Some(message) => format_message(message, &all),
            None => chosen,
        }
    }

    /// Mark `response` as being in this language: `Content-Language` and
    /// `Vary: Accept-Language` (added to an existing `Vary`).
    pub fn localize(&self, mut response: Response) -> Response {
        use crate::header::Header;
        response.headers.retain(|h| !h.name.eq_ignore_ascii_case(Header::_CONTENT_LANGUAGE));
        response.headers.push(Header::new(Header::_CONTENT_LANGUAGE, self.locale.clone()));
        match response.headers.iter_mut().find(|h| h.name.eq_ignore_ascii_case(Header::_VARY)) {
            Some(vary) if vary.value.split(',').any(|v| v.trim().eq_ignore_ascii_case("accept-language")) => {}
            Some(vary) => vary.value = format!("{}, Accept-Language", vary.value).into(),
            None => response.headers.push(Header::new(Header::_VARY, "Accept-Language".to_string())),
        }
        response
    }
}

/// Fill `{name}` placeholders.
fn format_message(message: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
        } else if let Some(end) = tail.strip_prefix('{').and_then(|t| t.find('}')) {
            let name = &tail[1..1 + end];
            match args.iter().find(|(n, _)| *n == name) {
                Some((_, value)) => out.push_str(value),
                None => out.push_str(&tail[..end + 2]),
            }
            rest = &tail[end + 2..];
        } else {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// The CLDR plural category of a whole `count` in `language`: `"one"`,
/// `"few"`, `"many"` or `"other"`. Covered: East Slavic and Polish
/// (`uk`, `ru`, `be`, `pl`: one/few/many), Czech and Slovak (one/few),
/// French and Brazilian Portuguese (0 and 1 are `one`), languages without
/// plural forms (`ja`, `zh`, `ko`, `vi`, `th`, `id`, `ms`, `tr`… : other);
/// every other language uses English's one/other.
pub fn plural_category(language: &str, count: u64) -> &'static str {
    let lower = language.to_ascii_lowercase();
    let base = lower.split('-').next().unwrap_or("");
    let (last, last_two) = (count % 10, count % 100);
    match base {
        "uk" | "ru" | "be" => {
            if last == 1 && last_two != 11 {
                "one"
            } else if (2..=4).contains(&last) && !(12..=14).contains(&last_two) {
                "few"
            } else {
                "many"
            }
        }
        "pl" => {
            if count == 1 {
                "one"
            } else if (2..=4).contains(&last) && !(12..=14).contains(&last_two) {
                "few"
            } else {
                "many"
            }
        }
        "cs" | "sk" => match count {
            1 => "one",
            2..=4 => "few",
            _ => "other",
        },
        "fr" => {
            if count <= 1 {
                "one"
            } else {
                "other"
            }
        }
        "pt" if lower != "pt-pt" => {
            if count <= 1 {
                "one"
            } else {
                "other"
            }
        }
        "ja" | "zh" | "ko" | "vi" | "th" | "id" | "ms" | "lo" | "my" | "km" | "tr" => "other",
        _ => {
            if count == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

#[cfg(test)]
mod tests;
