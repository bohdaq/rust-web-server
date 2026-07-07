//! A minimal, purpose-built XML parser for SAML metadata and Response
//! documents — not a general-purpose XML library.
//!
//! This crate hand-rolls every text format it parses (JSON, HTTP,
//! form-urlencoded, `.well-known` discovery documents, ...) rather than
//! taking a dependency on a parsing convenience crate; the spec sketch for
//! this feature suggested `quick-xml`, but adding a general XML parser
//! would be the first departure from that pattern in this entire SSO
//! effort, for a format this module only ever needs a small, well-defined
//! subset of. A purpose-built parser is also easier to reason about
//! security-wise: it never processes `<!DOCTYPE>` (rejected outright, see
//! [`parse`]), so there is no DTD/external-entity resolution machinery to
//! misconfigure — XXE is eliminated by never implementing it, not by
//! disabling it.
//!
//! Every [`XmlNode`] tracks the exact byte range (`start`..`end`) it
//! occupies in the original document, which [`super::assertion`] relies on
//! to verify an XML-DSig signature against the literal bytes as
//! transmitted (see that module's docs for why, and what that trades off).

use std::collections::HashMap;

/// A parsed XML element. Attribute and tag names are stored with any
/// namespace prefix stripped (`ds:Signature` is stored as `Signature`) —
/// this parser matches purely on local name, which is adequate for the
/// fixed, well-known SAML/XML-DSig element set it looks for.
#[derive(Debug, Clone)]
pub(crate) struct XmlNode {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
    /// Concatenation of direct text/CDATA children, unescaped.
    pub text: String,
    /// Byte offset of this element's opening `<` in the original document.
    pub start: usize,
    /// Byte offset one past this element's closing `>` (or self-closing
    /// `/>`) in the original document.
    pub end: usize,
}

impl XmlNode {
    /// The unescaped value of an attribute, matched by local name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// The first direct child with this local name.
    pub fn child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Every descendant (depth-first, this element excluded) with this
    /// local name, at any depth.
    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a XmlNode>) {
        for child in &self.children {
            if child.name == name {
                out.push(child);
            }
            child.find_all(name, out);
        }
    }

    /// The first descendant (depth-first, this element excluded) with this
    /// local name, at any depth.
    pub fn find(&self, name: &str) -> Option<&XmlNode> {
        let mut out = Vec::new();
        self.find_all(name, &mut out);
        out.into_iter().next()
    }
}

/// Parse an XML document into a single root [`XmlNode`].
///
/// Rejects any document containing `<!DOCTYPE` outright — SAML documents
/// never legitimately need one, and refusing it eliminates the XXE attack
/// surface without needing to correctly parse (and therefore trust) a DTD.
pub(crate) fn parse(xml: &str) -> Result<XmlNode, String> {
    if xml.contains("<!DOCTYPE") {
        return Err("DOCTYPE declarations are not supported".to_string());
    }
    let bytes = xml.as_bytes();
    let mut pos = 0usize;
    skip_prolog(bytes, &mut pos);
    let (node, _) = parse_element(bytes, pos)?;
    Ok(node)
}

fn skip_prolog(bytes: &[u8], pos: &mut usize) {
    loop {
        skip_whitespace(bytes, pos);
        if starts_with(bytes, *pos, b"<?") {
            if let Some(end) = find(bytes, b"?>", *pos) {
                *pos = end + 2;
                continue;
            }
        }
        if starts_with(bytes, *pos, b"<!--") {
            if let Some(end) = find(bytes, b"-->", *pos) {
                *pos = end + 3;
                continue;
            }
        }
        break;
    }
}

fn parse_element(bytes: &[u8], mut pos: usize) -> Result<(XmlNode, usize), String> {
    skip_whitespace(bytes, &mut pos);
    if bytes.get(pos) != Some(&b'<') {
        return Err("expected '<'".to_string());
    }
    let start = pos;
    pos += 1;

    let name_start = pos;
    while pos < bytes.len() && !is_name_end(bytes[pos]) {
        pos += 1;
    }
    let raw_name = std::str::from_utf8(&bytes[name_start..pos]).map_err(|_| "invalid UTF-8 in tag name")?;
    let name = local_name(raw_name);

    let mut attrs = Vec::new();
    loop {
        skip_whitespace(bytes, &mut pos);
        if starts_with(bytes, pos, b"/>") {
            pos += 2;
            return Ok((XmlNode { name, attrs, children: Vec::new(), text: String::new(), start, end: pos }, pos));
        }
        if bytes.get(pos) == Some(&b'>') {
            pos += 1;
            break;
        }
        if pos >= bytes.len() {
            return Err(format!("unterminated start tag <{name}>"));
        }

        let attr_name_start = pos;
        while pos < bytes.len() && bytes[pos] != b'=' && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'>' && bytes[pos] != b'/' {
            pos += 1;
        }
        let attr_name = local_name(std::str::from_utf8(&bytes[attr_name_start..pos]).unwrap_or(""));
        skip_whitespace(bytes, &mut pos);
        if bytes.get(pos) != Some(&b'=') {
            return Err(format!("expected '=' in attribute of <{name}>"));
        }
        pos += 1;
        skip_whitespace(bytes, &mut pos);
        let quote = *bytes.get(pos).ok_or("unexpected end of input in attribute value")?;
        if quote != b'"' && quote != b'\'' {
            return Err("expected quote to start attribute value".to_string());
        }
        pos += 1;
        let val_start = pos;
        while pos < bytes.len() && bytes[pos] != quote {
            pos += 1;
        }
        if pos >= bytes.len() {
            return Err("unterminated attribute value".to_string());
        }
        let raw_val = std::str::from_utf8(&bytes[val_start..pos]).unwrap_or("");
        attrs.push((attr_name, unescape(raw_val)));
        pos += 1; // consume closing quote
    }

    let mut children = Vec::new();
    let mut text = String::new();
    loop {
        let text_start = pos;
        while pos < bytes.len() && bytes[pos] != b'<' {
            pos += 1;
        }
        if pos > text_start {
            text.push_str(&unescape(std::str::from_utf8(&bytes[text_start..pos]).unwrap_or("")));
        }
        if pos >= bytes.len() {
            return Err(format!("unterminated element <{name}>"));
        }

        if starts_with(bytes, pos, b"<!--") {
            let end = find(bytes, b"-->", pos).ok_or("unterminated comment")?;
            pos = end + 3;
            continue;
        }
        if starts_with(bytes, pos, b"<![CDATA[") {
            let end = find(bytes, b"]]>", pos + 9).ok_or("unterminated CDATA section")?;
            text.push_str(std::str::from_utf8(&bytes[pos + 9..end]).unwrap_or(""));
            pos = end + 3;
            continue;
        }
        if starts_with(bytes, pos, b"</") {
            pos += 2;
            let cname_start = pos;
            while pos < bytes.len() && !is_name_end(bytes[pos]) {
                pos += 1;
            }
            let close_raw = std::str::from_utf8(&bytes[cname_start..pos]).unwrap_or("");
            skip_whitespace(bytes, &mut pos);
            if bytes.get(pos) != Some(&b'>') {
                return Err(format!("malformed closing tag for <{name}>"));
            }
            pos += 1;
            if local_name(close_raw) != name {
                return Err(format!("mismatched closing tag: expected </{name}>, found </{close_raw}>"));
            }
            return Ok((XmlNode { name, attrs, children, text, start, end: pos }, pos));
        }

        let (child, new_pos) = parse_element(bytes, pos)?;
        pos = new_pos;
        children.push(child);
    }
}

fn is_name_end(b: u8) -> bool {
    b.is_ascii_whitespace() || b == b'>' || b == b'/'
}

fn skip_whitespace(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && bytes[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
}

fn starts_with(bytes: &[u8], pos: usize, needle: &[u8]) -> bool {
    bytes.len() >= pos + needle.len() && &bytes[pos..pos + needle.len()] == needle
}

fn find(bytes: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from > bytes.len() {
        return None;
    }
    bytes[from..].windows(needle.len()).position(|w| w == needle).map(|i| i + from)
}

/// Strip a `prefix:` namespace prefix, if any.
fn local_name(qualified: &str) -> String {
    match qualified.split_once(':') {
        Some((_, local)) => local.to_string(),
        None => qualified.to_string(),
    }
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }
        let mut entity = String::new();
        let mut closed = false;
        for c2 in chars.by_ref() {
            if c2 == ';' {
                closed = true;
                break;
            }
            entity.push(c2);
            if entity.len() > 10 {
                break;
            }
        }
        if !closed {
            out.push('&');
            out.push_str(&entity);
            continue;
        }
        match entity.as_str() {
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "amp" => out.push('&'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                if let Ok(code) = u32::from_str_radix(&entity[2..], 16) {
                    if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                    }
                }
            }
            _ if entity.starts_with('#') => {
                if let Ok(code) = entity[1..].parse::<u32>() {
                    if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                    }
                }
            }
            _ => {
                // Unknown entity (e.g. a DTD-defined one) — since DOCTYPE
                // is rejected outright, this can only be a malformed or
                // unsupported reference; keep it literal rather than fail
                // the whole parse.
                out.push('&');
                out.push_str(&entity);
                out.push(';');
            }
        }
    }
    out
}

/// Collect every `AttributeValue`-bearing `Attribute` element under
/// `attribute_statement` into a name → first-value map. SAML attributes
/// can be multi-valued; this parser keeps only the first value per name,
/// documented in [`super::assertion`].
pub(crate) fn collect_attributes(attribute_statement: &XmlNode) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut attrs = Vec::new();
    attribute_statement.find_all("Attribute", &mut attrs);
    for attr in attrs {
        let Some(name) = attr.attr("Name") else { continue };
        if out.contains_key(name) {
            continue;
        }
        if let Some(value) = attr.child("AttributeValue") {
            out.insert(name.to_string(), value.text.clone());
        }
    }
    out
}
