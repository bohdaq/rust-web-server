//! DNS wire format (RFC 1035): names, queries and the records of a response,
//! for the SRV service discovery and ACME's DNS-01 (TXT records, SOA lookups,
//! RFC 2136 updates). Responses are untrusted input: parsing loops (no
//! recursion), follows at most 128 compression pointers per name, and checks
//! every length against the buffer.

// Without `acme` only the SRV discovery uses this module.
#![cfg_attr(not(feature = "acme"), allow(dead_code))]

#[cfg(test)]
mod tests;

/// Record types used here.
pub(crate) const TYPE_SOA: u16 = 6;
pub(crate) const TYPE_TXT: u16 = 16;
pub(crate) const TYPE_SRV: u16 = 33;
pub(crate) const TYPE_TSIG: u16 = 250;
/// Classes.
pub(crate) const CLASS_IN: u16 = 1;
pub(crate) const CLASS_NONE: u16 = 254;
pub(crate) const CLASS_ANY: u16 = 255;

/// `name` in wire format, appended to `buf`: lower-cased labels (canonical,
/// as TSIG needs), no compression. Labels over 63 bytes or a name over 255
/// are refused.
pub(crate) fn encode_name(name: &str, buf: &mut Vec<u8>) -> Result<(), String> {
    let start = buf.len();
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() {
            continue;
        }
        if label.len() > 63 {
            return Err(format!("DNS label longer than 63 bytes in '{name}'"));
        }
        buf.push(label.len() as u8);
        buf.extend(label.bytes().map(|b| b.to_ascii_lowercase()));
    }
    buf.push(0);
    if buf.len() - start > 255 {
        return Err(format!("DNS name longer than 255 bytes: '{name}'"));
    }
    Ok(())
}

/// Reads a (possibly compressed, RFC 1035 §4.1.4) name starting at `pos`.
/// Returns the dotted name (no trailing dot) and the position just after it
/// in the *original* buffer (after a first compression pointer, not after its
/// target — a pointer ends the name in place).
pub(crate) fn read_name(buf: &[u8], mut pos: usize) -> Result<(String, usize), String> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut return_pos = pos;
    let mut hops = 0u32;

    loop {
        hops += 1;
        if hops > 128 {
            return Err("DNS name compression pointer loop".to_string());
        }
        let len = *buf.get(pos).ok_or("truncated name")?;
        if len == 0 {
            if !jumped {
                return_pos = pos + 1;
            }
            break;
        }
        if len & 0xC0 == 0xC0 {
            let b2 = *buf.get(pos + 1).ok_or("truncated compression pointer")?;
            let offset = (((len as usize) & 0x3F) << 8) | b2 as usize;
            if !jumped {
                return_pos = pos + 2;
            }
            jumped = true;
            pos = offset;
            continue;
        }
        let start = pos + 1;
        let end = start + len as usize;
        let label = buf.get(start..end).ok_or("truncated label")?;
        labels.push(String::from_utf8_lossy(label).to_string());
        pos = end;
    }

    Ok((labels.join("."), return_pos))
}

/// A query for `name`/`qtype` (class IN), recursion desired or not.
pub(crate) fn build_query(id: u16, name: &str, qtype: u16, recursion_desired: bool) -> Result<Vec<u8>, String> {
    let mut buf = Vec::with_capacity(32 + name.len());
    buf.extend_from_slice(&id.to_be_bytes());
    buf.extend_from_slice(&(if recursion_desired { 0x0100u16 } else { 0 }).to_be_bytes());
    buf.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]); // QDCOUNT 1, the rest 0
    encode_name(name, &mut buf)?;
    buf.extend_from_slice(&qtype.to_be_bytes());
    buf.extend_from_slice(&CLASS_IN.to_be_bytes());
    Ok(buf)
}

/// A resource record of a parsed message; its RDATA stays in the buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Record {
    /// Owner name, dotted, without the trailing dot, as sent (case kept).
    pub(crate) name: String,
    pub(crate) rtype: u16,
    pub(crate) class: u16,
    pub(crate) ttl: u32,
    /// Where the RDATA is in the message.
    pub(crate) rdata: std::ops::Range<usize>,
}

/// A parsed DNS message: header fields and the three record sections.
#[derive(Debug)]
pub(crate) struct Message {
    pub(crate) id: u16,
    pub(crate) flags: u16,
    pub(crate) answers: Vec<Record>,
    pub(crate) authority: Vec<Record>,
    pub(crate) additional: Vec<Record>,
}

impl Message {
    /// RCODE (the low four bits of the flags).
    pub(crate) fn rcode(&self) -> u16 {
        self.flags & 0x000F
    }
}

fn u16_at(buf: &[u8], pos: usize) -> Result<u16, String> {
    Ok(u16::from_be_bytes([*buf.get(pos).ok_or("truncated DNS message")?, *buf.get(pos + 1).ok_or("truncated DNS message")?]))
}

/// Parse a whole message. Questions are skipped; every record is checked to
/// fit, RDATA included.
pub(crate) fn parse(buf: &[u8]) -> Result<Message, String> {
    if buf.len() < 12 {
        return Err("response shorter than DNS header".to_string());
    }
    let id = u16_at(buf, 0)?;
    let flags = u16_at(buf, 2)?;
    let counts = [u16_at(buf, 4)?, u16_at(buf, 6)?, u16_at(buf, 8)?, u16_at(buf, 10)?];
    let mut pos = 12usize;
    for _ in 0..counts[0] {
        let (_, next) = read_name(buf, pos)?;
        pos = next + 4; // QTYPE + QCLASS
    }
    let mut sections: [Vec<Record>; 3] = Default::default();
    for (section, &count) in sections.iter_mut().zip(&counts[1..]) {
        for _ in 0..count {
            let (name, next) = read_name(buf, pos)?;
            pos = next;
            let rtype = u16_at(buf, pos)?;
            let class = u16_at(buf, pos + 2)?;
            let ttl = (u16_at(buf, pos + 4)? as u32) << 16 | u16_at(buf, pos + 6)? as u32;
            let rdlength = u16_at(buf, pos + 8)? as usize;
            let start = pos + 10;
            let end = start + rdlength;
            if end > buf.len() {
                return Err("truncated RDATA".to_string());
            }
            section.push(Record { name, rtype, class, ttl, rdata: start..end });
            pos = end;
        }
    }
    let [answers, authority, additional] = sections;
    Ok(Message { id, flags, answers, authority, additional })
}

/// The character-strings of TXT RDATA, joined (a long value is split into
/// 255-byte strings).
pub(crate) fn txt_value(rdata: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rdata.len());
    let mut pos = 0;
    while let Some(&len) = rdata.get(pos) {
        let Some(chunk) = rdata.get(pos + 1..pos + 1 + len as usize) else { break };
        out.extend_from_slice(chunk);
        pos += 1 + len as usize;
    }
    out
}

/// TXT RDATA holding `value` (in 255-byte character-strings).
pub(crate) fn txt_rdata(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + value.len() / 255 + 1);
    for chunk in value.chunks(255) {
        out.push(chunk.len() as u8);
        out.extend_from_slice(chunk);
    }
    if value.is_empty() {
        out.push(0);
    }
    out
}

/// The first `nameserver` in `/etc/resolv.conf`, port 53.
pub(crate) fn system_resolver() -> Option<std::net::SocketAddr> {
    let contents = std::fs::read_to_string("/etc/resolv.conf").ok()?;
    contents.lines().find_map(|line| {
        let ip: std::net::IpAddr = line.trim().strip_prefix("nameserver")?.trim().parse().ok()?;
        Some(std::net::SocketAddr::new(ip, 53))
    })
}

/// Not cryptographically random — only to match our reply to our query.
pub(crate) fn query_id() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    (nanos & 0xFFFF) as u16
}
