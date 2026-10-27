//! DNS-01 (RFC 8555 §8.4): the CA looks up `_acme-challenge.<domain>` TXT
//! and expects base64url(SHA-256(key authorization)). rws publishes the
//! record through the configured provider, waits until it can be seen, and
//! removes it after validation.
//!
//! `rfc2136` sends DNS UPDATEs (RFC 2136) over TCP to the zone's primary
//! server, signed with TSIG (RFC 8945, HMAC-SHA256/512) when a key is
//! configured; the reply's RCODE and TSIG error are checked (its MAC is not
//! verified: the update's effect is checked by looking the record up). The
//! zone is configured or found from the SOA the server returns for the name.
//! `exec` runs a program — any DNS API, scripted.

use std::time::Duration;

use super::{crypto, DnsConfig, DnsProvider, Tsig};
use crate::dns;

/// How long one exchange with a DNS server may take.
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(10);
/// TTL of the challenge record.
const RECORD_TTL: u32 = 60;
/// TSIG fudge: the time window (seconds) the server accepts.
const TSIG_FUDGE: u16 = 300;

/// `_acme-challenge.<domain>` — for a wildcard, the authorization's base name.
pub(crate) fn record_name(domain: &str) -> String {
    format!("_acme-challenge.{}", domain.trim_end_matches('.'))
}

/// The TXT value: base64url(SHA-256(key authorization)), unpadded.
pub(crate) fn record_value(key_authorization: &str) -> String {
    let digest = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, key_authorization.as_bytes());
    crypto::base64url(digest.as_ref())
}

/// Publish `value` at `name`, replacing whatever TXT was there (a record
/// from a failed earlier run mustn't satisfy the lookup that follows).
pub(crate) async fn present(config: &DnsConfig, name: &str, value: &str) -> Result<(), String> {
    match &config.provider {
        DnsProvider::Rfc2136 { server, tsig } => {
            let zone = zone_of(config, server, name).await?;
            let rdata = dns::txt_rdata(value.as_bytes());
            update(server, tsig.as_ref(), &zone, name, &[Change::DeleteAll, Change::Add(&rdata)]).await
        }
        DnsProvider::Exec { program, .. } => run_hook(program, "present", name, value).await,
    }
}

/// Remove the record published by [`present`].
pub(crate) async fn cleanup(config: &DnsConfig, name: &str, value: &str) -> Result<(), String> {
    match &config.provider {
        DnsProvider::Rfc2136 { server, tsig } => {
            let zone = zone_of(config, server, name).await?;
            let rdata = dns::txt_rdata(value.as_bytes());
            update(server, tsig.as_ref(), &zone, name, &[Change::Delete(&rdata)]).await
        }
        DnsProvider::Exec { program, .. } => run_hook(program, "cleanup", name, value).await,
    }
}

/// Wait (up to `propagation_secs`) until a lookup of `name` returns `value`:
/// from `dns_server` (`rfc2136`'s primary, or the one given for `exec`),
/// else from the system resolver. `false` when it never did — the CA may
/// still see it.
pub(crate) async fn wait_visible(config: &DnsConfig, name: &str, value: &str) -> bool {
    let (server, recursive) = match &config.provider {
        DnsProvider::Rfc2136 { server, .. } | DnsProvider::Exec { server: Some(server), .. } => (Some(server_addr(server)), false),
        DnsProvider::Exec { server: None, .. } => (dns::system_resolver().map(|a| a.to_string()), true),
    };
    let Some(server) = server else { return false };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(config.propagation_secs);
    loop {
        if let Ok(values) = lookup_txt(&server, name, recursive).await {
            if values.iter().any(|v| v == value.as_bytes()) {
                return true;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// `host[:port]` → `host:port` (port 53), brackets kept for IPv6.
fn server_addr(server: &str) -> String {
    let server = server.trim();
    if server.starts_with('[') {
        return if server.contains("]:") { server.to_string() } else { format!("{server}:53") };
    }
    match server.matches(':').count() {
        0 => format!("{server}:53"),
        1 => server.to_string(),
        _ => format!("[{server}]:53"), // a bare IPv6 address
    }
}

/// One message to `server` over TCP (two-byte length prefix), one reply.
async fn exchange(server: &str, message: &[u8]) -> Result<Vec<u8>, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let work = async {
        let mut stream = tokio::net::TcpStream::connect(server).await.map_err(|e| format!("DNS server {server}: {e}"))?;
        let mut framed = (message.len() as u16).to_be_bytes().to_vec();
        framed.extend_from_slice(message);
        stream.write_all(&framed).await.map_err(|e| format!("DNS server {server}: {e}"))?;
        let mut len = [0u8; 2];
        stream.read_exact(&mut len).await.map_err(|e| format!("DNS server {server}: {e}"))?;
        let mut reply = vec![0u8; u16::from_be_bytes(len) as usize];
        stream.read_exact(&mut reply).await.map_err(|e| format!("DNS server {server}: {e}"))?;
        Ok::<_, String>(reply)
    };
    tokio::time::timeout(EXCHANGE_TIMEOUT, work).await.map_err(|_| format!("DNS server {server}: no answer in {}s", EXCHANGE_TIMEOUT.as_secs()))?
}

async fn lookup_txt(server: &str, name: &str, recursive: bool) -> Result<Vec<Vec<u8>>, String> {
    let id = dns::query_id();
    let reply = exchange(server, &dns::build_query(id, name, dns::TYPE_TXT, recursive)?).await?;
    let message = dns::parse(&reply)?;
    if message.id != id {
        return Err("DNS reply to another query".to_string());
    }
    Ok(message
        .answers
        .iter()
        .filter(|r| r.rtype == dns::TYPE_TXT && r.name.eq_ignore_ascii_case(name.trim_end_matches('.')))
        .map(|r| dns::txt_value(&reply[r.rdata.clone()]))
        .collect())
}

/// The configured zone, or the owner of the SOA record the server returns
/// for `name` (in the answer, or the authority section of a negative reply).
async fn zone_of(config: &DnsConfig, server: &str, name: &str) -> Result<String, String> {
    if let Some(zone) = &config.zone {
        return Ok(zone.trim_end_matches('.').to_string());
    }
    let server = server_addr(server);
    let id = dns::query_id();
    let reply = exchange(&server, &dns::build_query(id, name, dns::TYPE_SOA, false)?).await?;
    let message = dns::parse(&reply)?;
    message
        .answers
        .iter()
        .chain(&message.authority)
        .find(|r| r.rtype == dns::TYPE_SOA)
        .map(|r| r.name.clone())
        .filter(|zone| !zone.is_empty())
        .ok_or_else(|| format!("couldn't find the zone of {name} at {server} (set dns_zone)"))
}

/// One change to the RRset of the challenge name.
pub(crate) enum Change<'a> {
    /// Delete every TXT record at the name.
    DeleteAll,
    /// Add a TXT record with this RDATA.
    Add(&'a [u8]),
    /// Delete the TXT record with this RDATA.
    Delete(&'a [u8]),
}

async fn update(server: &str, tsig: Option<&Tsig>, zone: &str, name: &str, changes: &[Change<'_>]) -> Result<(), String> {
    let id = dns::query_id();
    let mut message = update_message(id, zone, name, changes)?;
    if let Some(tsig) = tsig {
        sign(&mut message, tsig, unix_now())?;
    }
    let reply = exchange(&server_addr(server), &message).await?;
    check_update_reply(&reply, id).map_err(|e| format!("DNS UPDATE of {name} in {zone} at {server}: {e}"))
}

/// An RFC 2136 UPDATE: zone section (zone SOA IN), no prerequisites, the
/// changes as update records.
pub(crate) fn update_message(id: u16, zone: &str, name: &str, changes: &[Change<'_>]) -> Result<Vec<u8>, String> {
    let mut message = Vec::with_capacity(128);
    message.extend_from_slice(&id.to_be_bytes());
    message.extend_from_slice(&(5u16 << 11).to_be_bytes()); // opcode UPDATE
    message.extend_from_slice(&1u16.to_be_bytes()); // ZOCOUNT
    message.extend_from_slice(&0u16.to_be_bytes()); // PRCOUNT
    message.extend_from_slice(&(changes.len() as u16).to_be_bytes()); // UPCOUNT
    message.extend_from_slice(&0u16.to_be_bytes()); // ADCOUNT
    dns::encode_name(zone, &mut message)?;
    message.extend_from_slice(&dns::TYPE_SOA.to_be_bytes());
    message.extend_from_slice(&dns::CLASS_IN.to_be_bytes());
    for change in changes {
        dns::encode_name(name, &mut message)?;
        message.extend_from_slice(&dns::TYPE_TXT.to_be_bytes());
        let (class, ttl, rdata): (u16, u32, &[u8]) = match change {
            Change::DeleteAll => (dns::CLASS_ANY, 0, &[]),
            Change::Add(rdata) => (dns::CLASS_IN, RECORD_TTL, rdata),
            Change::Delete(rdata) => (dns::CLASS_NONE, 0, rdata),
        };
        message.extend_from_slice(&class.to_be_bytes());
        message.extend_from_slice(&ttl.to_be_bytes());
        message.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
        message.extend_from_slice(rdata);
    }
    Ok(message)
}

/// Append a TSIG record (RFC 8945 §4) signing `message` as it is: the MAC
/// covers the message (its ARCOUNT not yet counting the TSIG record), then
/// the TSIG variables — key name and algorithm in canonical form, class ANY,
/// TTL 0, time signed, fudge, error 0, no other data.
pub(crate) fn sign(message: &mut Vec<u8>, tsig: &Tsig, time_signed: u64) -> Result<(), String> {
    if message.len() < 12 {
        return Err("DNS message shorter than its header".to_string());
    }
    let mut key_name = Vec::new();
    dns::encode_name(&tsig.key_name, &mut key_name)?;
    let mut algorithm = Vec::new();
    dns::encode_name(&tsig.algorithm, &mut algorithm)?;
    let time = &time_signed.to_be_bytes()[2..]; // 48 bits

    let mut signed = message.clone();
    signed.extend_from_slice(&key_name);
    signed.extend_from_slice(&dns::CLASS_ANY.to_be_bytes());
    signed.extend_from_slice(&0u32.to_be_bytes());
    signed.extend_from_slice(&algorithm);
    signed.extend_from_slice(time);
    signed.extend_from_slice(&TSIG_FUDGE.to_be_bytes());
    signed.extend_from_slice(&[0, 0, 0, 0]); // error, other len
    let hmac_algorithm = match tsig.algorithm.trim_end_matches('.').to_ascii_lowercase().as_str() {
        "hmac-sha256" => aws_lc_rs::hmac::HMAC_SHA256,
        "hmac-sha512" => aws_lc_rs::hmac::HMAC_SHA512,
        other => return Err(format!("unsupported TSIG algorithm {other}")),
    };
    let mac = aws_lc_rs::hmac::sign(&aws_lc_rs::hmac::Key::new(hmac_algorithm, &tsig.secret), &signed);
    let mac = mac.as_ref();

    let original_id = [message[0], message[1]];
    let mut rdata = algorithm;
    rdata.extend_from_slice(time);
    rdata.extend_from_slice(&TSIG_FUDGE.to_be_bytes());
    rdata.extend_from_slice(&(mac.len() as u16).to_be_bytes());
    rdata.extend_from_slice(mac);
    rdata.extend_from_slice(&original_id);
    rdata.extend_from_slice(&[0, 0, 0, 0]); // error, other len

    message.extend_from_slice(&key_name);
    message.extend_from_slice(&dns::TYPE_TSIG.to_be_bytes());
    message.extend_from_slice(&dns::CLASS_ANY.to_be_bytes());
    message.extend_from_slice(&0u32.to_be_bytes());
    message.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
    message.extend_from_slice(&rdata);
    let arcount = u16::from_be_bytes([message[10], message[11]]) + 1;
    message[10..12].copy_from_slice(&arcount.to_be_bytes());
    Ok(())
}

/// The update's reply: our id, RCODE NOERROR; a refusal names the RCODE and
/// the TSIG error (BADSIG, BADKEY, BADTIME) when the server sent one.
pub(crate) fn check_update_reply(reply: &[u8], id: u16) -> Result<(), String> {
    let message = dns::parse(reply)?;
    if message.id != id {
        return Err("reply to another message".to_string());
    }
    let rcode = message.rcode();
    if rcode == 0 {
        return Ok(());
    }
    let tsig_error = message.additional.iter().find(|r| r.rtype == dns::TYPE_TSIG).and_then(|r| tsig_error(&reply[r.rdata.clone()]));
    let rcode_name = match rcode {
        1 => "FORMERR",
        2 => "SERVFAIL",
        3 => "NXDOMAIN",
        4 => "NOTIMP",
        5 => "REFUSED (does the key's update policy cover this name?)",
        8 => "NXRRSET",
        9 => "NOTAUTH (the server isn't the primary for the zone, or the TSIG key was rejected)",
        10 => "NOTZONE (the name isn't in the zone)",
        _ => "an error",
    };
    Err(match tsig_error {
        Some(16) => format!("rcode {rcode} {rcode_name}; TSIG BADSIG: wrong secret"),
        Some(17) => format!("rcode {rcode} {rcode_name}; TSIG BADKEY: the server doesn't know the key or algorithm"),
        Some(18) => format!("rcode {rcode} {rcode_name}; TSIG BADTIME: the clocks differ by more than {TSIG_FUDGE}s"),
        Some(code) if code != 0 => format!("rcode {rcode} {rcode_name}; TSIG error {code}"),
        _ => format!("rcode {rcode} {rcode_name}"),
    })
}

/// The error field of TSIG RDATA.
fn tsig_error(rdata: &[u8]) -> Option<u16> {
    let (_, after_algorithm) = dns::read_name(rdata, 0).ok()?;
    // time (6), fudge (2), MAC size (2), MAC, original id (2), error (2)
    let mac_size = u16::from_be_bytes([*rdata.get(after_algorithm + 8)?, *rdata.get(after_algorithm + 9)?]) as usize;
    let error_at = after_algorithm + 10 + mac_size + 2;
    Some(u16::from_be_bytes([*rdata.get(error_at)?, *rdata.get(error_at + 1)?]))
}

/// Run the exec provider: `<program> <action> <fqdn.> <value>`, no shell.
async fn run_hook(program: &str, action: &str, name: &str, value: &str) -> Result<(), String> {
    let (program, action, fqdn, value) = (program.to_string(), action.to_string(), format!("{}.", name.trim_end_matches('.')), value.to_string());
    let run = tokio::task::spawn_blocking(move || std::process::Command::new(&program).args([&action, &fqdn, &value]).output().map(|o| (program, action, o)));
    let (program, action, output) = run.await.map_err(|e| e.to_string())?.map_err(|e| format!("DNS-01 hook: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        eprintln!("[ACME] {program} {action}: {}", stderr.trim());
    }
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("DNS-01 hook {program} {action} exited with {}", output.status))
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests;
