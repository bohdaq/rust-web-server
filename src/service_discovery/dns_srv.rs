//! DNS SRV record discovery — a hand-rolled RFC 1035/2782 query/response
//! codec over UDP, no third-party DNS crate. This is the mechanism headless
//! Kubernetes Services publish per-port endpoint info through (an A record
//! alone has no port), and the natural home for "weighted DNS": SRV records
//! carry a `weight` field A records don't have.

#[cfg(test)]
mod tests;

use std::net::UdpSocket;
use std::time::Duration;

use crate::dns::read_name;

const SRV_QTYPE: u16 = crate::dns::TYPE_SRV;
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
    let resolver = match crate::dns::system_resolver() {
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


/// Sends one SRV query for `record` to `resolver` over UDP and parses the
/// response. Exposed at `pub(crate)` (not just `pub(super)`) so tests can
/// point it at a mock UDP server instead of the system resolver.
pub(crate) fn query(record: &str, resolver: std::net::SocketAddr, timeout: Duration) -> Result<Vec<SrvRecord>, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("bind failed: {}", e))?;
    socket.connect(resolver).map_err(|e| format!("connect to {} failed: {}", resolver, e))?;
    socket.set_read_timeout(Some(timeout)).map_err(|e| format!("set_read_timeout failed: {}", e))?;

    let id = crate::dns::query_id();
    let packet = crate::dns::build_query(id, record, SRV_QTYPE, true)?;
    socket.send(&packet).map_err(|e| format!("send failed: {}", e))?;

    let mut buf = [0u8; 4096];
    let n = socket.recv(&mut buf).map_err(|e| format!("recv failed: {}", e))?;

    parse_response(&buf[..n], id)
}





pub(crate) fn parse_response(buf: &[u8], expected_id: u16) -> Result<Vec<SrvRecord>, String> {
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
            // RDLENGTH is the sender's claim; the bytes may not be there.
            let fixed = buf.get(rdata_start..rdata_start + 6).ok_or("truncated SRV RDATA")?;
            let priority = u16::from_be_bytes([fixed[0], fixed[1]]);
            let weight = u16::from_be_bytes([fixed[2], fixed[3]]);
            let port = u16::from_be_bytes([fixed[4], fixed[5]]);
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
