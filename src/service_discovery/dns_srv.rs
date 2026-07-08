//! DNS SRV record discovery — a hand-rolled RFC 1035/2782 query/response
//! codec over UDP, no third-party DNS crate. This is the mechanism headless
//! Kubernetes Services publish per-port endpoint info through (an A record
//! alone has no port), and the natural home for "weighted DNS": SRV records
//! carry a `weight` field A records don't have.

#[cfg(test)]
mod tests;

use std::net::UdpSocket;
use std::time::Duration;

const SRV_QTYPE: u16 = 33;
const IN_QCLASS: u16 = 1;
/// Each SRV target is repeated up to this many times in the returned backend
/// list so a plain round-robin consumer sees roughly proportional selection
/// frequency, without letting a single very-high-weight record blow up the
/// list size.
const MAX_WEIGHT_COPIES: u16 = 20;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SrvRecord {
    pub(crate) priority: u16,
    pub(crate) weight: u16,
    pub(crate) port: u16,
    pub(crate) target: String,
}

/// Resolves `record` (e.g. `_http._tcp.example.com`) to a `"host:port"` list,
/// weight-expanded within the lowest-priority tier. Returns an empty list on
/// any failure (no resolver configured, network error, NXDOMAIN, etc.) —
/// logged, not propagated, matching every other `DiscoverySource`.
pub(super) fn resolve(record: &str) -> Vec<String> {
    let resolver = match system_resolver() {
        Some(r) => r,
        None => {
            eprintln!("service_discovery: no DNS resolver found (checked /etc/resolv.conf)");
            return Vec::new();
        }
    };

    match query(record, resolver, Duration::from_secs(5)) {
        Ok(records) => expand_by_weight(records),
        Err(e) => {
            eprintln!("service_discovery: SRV query for {} failed: {}", record, e);
            Vec::new()
        }
    }
}

/// Reads the first `nameserver` line from `/etc/resolv.conf`. Unix-only
/// resolver discovery — there's no config-file equivalent to parse on
/// Windows; pass a resolver explicitly via [`query`] in that case.
fn system_resolver() -> Option<std::net::SocketAddr> {
    let contents = std::fs::read_to_string("/etc/resolv.conf").ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("nameserver") {
            let ip = rest.trim();
            if let Ok(addr) = format!("{}:53", ip).parse() {
                return Some(addr);
            }
        }
    }
    None
}

/// Sends one SRV query for `record` to `resolver` over UDP and parses the
/// response. Exposed at `pub(crate)` (not just `pub(super)`) so tests can
/// point it at a mock UDP server instead of the system resolver.
pub(crate) fn query(record: &str, resolver: std::net::SocketAddr, timeout: Duration) -> Result<Vec<SrvRecord>, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("bind failed: {}", e))?;
    socket.connect(resolver).map_err(|e| format!("connect to {} failed: {}", resolver, e))?;
    socket.set_read_timeout(Some(timeout)).map_err(|e| format!("set_read_timeout failed: {}", e))?;

    let id = query_id();
    let packet = build_query(id, record);
    socket.send(&packet).map_err(|e| format!("send failed: {}", e))?;

    let mut buf = [0u8; 4096];
    let n = socket.recv(&mut buf).map_err(|e| format!("recv failed: {}", e))?;

    parse_response(&buf[..n], id)
}

/// Not cryptographically random — this only needs to distinguish our own
/// in-flight query from an unrelated stray UDP packet, the same bar
/// `request_id::generate_request_id` sets for its own counter-based ID.
fn query_id() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    (nanos & 0xFFFF) as u16
}

fn encode_name(name: &str, buf: &mut Vec<u8>) {
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() {
            continue;
        }
        buf.push(label.len() as u8);
        buf.extend_from_slice(label.as_bytes());
    }
    buf.push(0);
}

fn build_query(id: u16, name: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(32 + name.len());
    buf.extend_from_slice(&id.to_be_bytes());
    buf.extend_from_slice(&0x0100u16.to_be_bytes()); // flags: standard query, recursion desired
    buf.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    buf.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT
    buf.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
    buf.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT

    encode_name(name, &mut buf);
    buf.extend_from_slice(&SRV_QTYPE.to_be_bytes());
    buf.extend_from_slice(&IN_QCLASS.to_be_bytes());
    buf
}

/// Reads a (possibly compressed, RFC 1035 §4.1.4) name starting at `pos`.
/// Returns the decoded name and the position immediately after it in the
/// *original* buffer (i.e. after following any first compression pointer,
/// not after any pointer target — that's the wire-format rule: a pointer is
/// always the last thing in a name).
fn read_name(buf: &[u8], mut pos: usize) -> Result<(String, usize), String> {
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

fn parse_response(buf: &[u8], expected_id: u16) -> Result<Vec<SrvRecord>, String> {
    if buf.len() < 12 {
        return Err("response shorter than DNS header".to_string());
    }
    let id = u16::from_be_bytes([buf[0], buf[1]]);
    if id != expected_id {
        return Err("response ID does not match query".to_string());
    }
    let flags = u16::from_be_bytes([buf[2], buf[3]]);
    let rcode = flags & 0x000F;
    let qdcount = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    let ancount = u16::from_be_bytes([buf[6], buf[7]]) as usize;

    let mut pos = 12usize;
    for _ in 0..qdcount {
        let (_name, next) = read_name(buf, pos)?;
        pos = next + 4; // QTYPE + QCLASS
    }

    if rcode != 0 {
        // NXDOMAIN (3) and friends: no records, not a transport error.
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for _ in 0..ancount {
        let (_name, next) = read_name(buf, pos)?;
        pos = next;
        let rtype = u16::from_be_bytes([*buf.get(pos).ok_or("truncated RR type")?, *buf.get(pos + 1).ok_or("truncated RR type")?]);
        pos += 2 + 2 + 4; // TYPE + CLASS + TTL
        let rdlength = u16::from_be_bytes([*buf.get(pos).ok_or("truncated RDLENGTH")?, *buf.get(pos + 1).ok_or("truncated RDLENGTH")?]) as usize;
        pos += 2;
        let rdata_start = pos;

        if rtype == SRV_QTYPE {
            if rdlength < 6 {
                return Err("SRV RDATA shorter than 6 bytes".to_string());
            }
            let priority = u16::from_be_bytes([buf[rdata_start], buf[rdata_start + 1]]);
            let weight = u16::from_be_bytes([buf[rdata_start + 2], buf[rdata_start + 3]]);
            let port = u16::from_be_bytes([buf[rdata_start + 4], buf[rdata_start + 5]]);
            let (target, _) = read_name(buf, rdata_start + 6)?;
            records.push(SrvRecord { priority, weight, port, target });
        }

        pos = rdata_start + rdlength;
    }

    Ok(records)
}

/// Keeps only the lowest-priority tier (RFC 2782: clients try that tier
/// first), then repeats each `target:port` `weight.clamp(1, MAX_WEIGHT_COPIES)`
/// times so a flat round-robin `Vec<String>` consumer still favors
/// higher-weight targets proportionally.
fn expand_by_weight(mut records: Vec<SrvRecord>) -> Vec<String> {
    let Some(min_priority) = records.iter().map(|r| r.priority).min() else {
        return Vec::new();
    };
    records.retain(|r| r.priority == min_priority);

    let mut backends = Vec::new();
    for r in &records {
        let copies = r.weight.clamp(1, MAX_WEIGHT_COPIES);
        let target = r.target.trim_end_matches('.');
        for _ in 0..copies {
            backends.push(format!("{}:{}", target, r.port));
        }
    }
    backends
}
