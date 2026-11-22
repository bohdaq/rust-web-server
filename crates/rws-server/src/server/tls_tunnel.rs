//! TLS tunnels: a host name on the HTTPS listener whose connections are
//! not HTTP. rws completes the TLS handshake as usual (its certificate,
//! SNI, ACME), then hands the decrypted byte stream to a local TCP
//! address and copies both ways until either side closes. With
//! `tls_tunnels = ["turn.example.com=127.0.0.1:3478"]`
//! (`RWS_CONFIG_TLS_TUNNELS`, comma-separated `name=address`), a TURN
//! relay, an IMAP or any TCP service can sit behind port 443 beside the
//! web sites, one name each. Other names are served as before; without
//! the setting nothing changes and nothing is looked up.
//!
//! Targets must be loopback addresses (the config check refuses others):
//! a tunnel exposes a local port, never another machine. The list is read
//! once at start; a change needs a restart. Tunnelled connections have no
//! HTTP timeouts (they are long-lived by nature); they end with the process.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::OnceLock;

/// How long connecting to a tunnel's target may take.
pub const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Parses `name=address,name=address`: lower-case names, loopback socket
/// addresses. Entries that do not parse are skipped (the config check
/// reports them before start).
pub fn parse(value: &str) -> HashMap<String, SocketAddr> {
    value
        .split(',')
        .filter_map(|entry| {
            let (name, target) = entry.trim().split_once('=')?;
            let name = name.trim().trim_end_matches('.').to_ascii_lowercase();
            let target: SocketAddr = target.trim().parse().ok()?;
            (!name.is_empty() && target.ip().is_loopback()).then_some((name, target))
        })
        .collect()
}

/// Why an entry of `tls_tunnels` is not usable, for the config check.
pub fn problem(entry: &str) -> Option<String> {
    let Some((name, target)) = entry.trim().split_once('=') else {
        return Some(format!(
            "{entry:?}: expected name=address, e.g. turn.example.com=127.0.0.1:3478"
        ));
    };
    if name.trim().is_empty() {
        return Some(format!("{entry:?}: no host name"));
    }
    match target.trim().parse::<SocketAddr>() {
        Err(_) => Some(format!(
            "{entry:?}: {target:?} is not an address with a port"
        )),
        Ok(addr) if !addr.ip().is_loopback() => Some(format!(
            "{entry:?}: a tunnel goes to this machine only (127.0.0.1 or ::1)"
        )),
        Ok(_) => None,
    }
}

fn tunnels() -> &'static HashMap<String, SocketAddr> {
    static TUNNELS: OnceLock<HashMap<String, SocketAddr>> = OnceLock::new();
    TUNNELS.get_or_init(|| {
        std::env::var("RWS_CONFIG_TLS_TUNNELS")
            .map(|v| parse(&v))
            .unwrap_or_default()
    })
}

/// The local target for connections to `sni`, if that name is a tunnel.
pub fn target(sni: Option<&str>) -> Option<SocketAddr> {
    let tunnels = tunnels();
    if tunnels.is_empty() {
        return None;
    }
    tunnels
        .get(&sni?.trim_end_matches('.').to_ascii_lowercase())
        .copied()
}

/// Copies between the decrypted `stream` and `target` until either side
/// closes.
#[cfg(feature = "http2")]
pub async fn run<S>(mut stream: S, target: SocketAddr, peer: SocketAddr)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let backend =
        match tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::TcpStream::connect(target)).await {
            Ok(Ok(backend)) => backend,
            Ok(Err(e)) => return eprintln!("TLS tunnel to {target} for {peer}: {e}"),
            Err(_) => {
                return eprintln!(
                    "TLS tunnel to {target} for {peer}: no answer in {}s",
                    CONNECT_TIMEOUT.as_secs()
                )
            }
        };
    let _ = backend.set_nodelay(true);
    let mut backend = backend;
    // Ends when either side closes; at shutdown the runtime ends this task
    // with the other connections, so a tunnel never holds a drain open.
    let _ = tokio::io::copy_bidirectional(&mut stream, &mut backend).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_names_and_loopback_targets() {
        let t = parse(" Turn.Example.com.=127.0.0.1:3478, imap.example.com=[::1]:143 ,bad, x=10.0.0.5:22, =127.0.0.1:1");
        assert_eq!(2, t.len(), "{t:?}");
        assert_eq!(
            Some(&"127.0.0.1:3478".parse().unwrap()),
            t.get("turn.example.com")
        );
        assert_eq!(
            Some(&"[::1]:143".parse().unwrap()),
            t.get("imap.example.com")
        );
        assert!(problem("turn.example.com=127.0.0.1:3478").is_none());
        assert!(problem("turn.example.com")
            .unwrap()
            .contains("name=address"));
        assert!(problem("x=10.0.0.5:22")
            .unwrap()
            .contains("this machine only"));
        assert!(problem("x=localhost").unwrap().contains("not an address"));
        assert!(problem("=127.0.0.1:1").unwrap().contains("no host name"));
    }
}
