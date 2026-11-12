//! The mesh sidecar (`RWS_CONFIG_GATEWAY_API_MODE=mesh-sidecar`): rws in
//! each meshed pod, receiving the pod's outbound TCP connections, which the
//! `rws-mesh-init` container's iptables rules redirect to
//! [`OUTBOUND_PORT`] (all but the sidecar's own, by its UID
//! [`SIDECAR_UID`], and loopback).
//!
//! For each connection the original destination (`SO_ORIGINAL_DST`) is
//! looked up in the [`MeshTable`](super::MeshTable) of the pod's namespace:
//! a Service port with routes whose client speaks HTTP/1.1 or h2c is served
//! by the gateway router; everything else goes to the original destination
//! byte for byte. Until the controller's first snapshot arrives, everything
//! goes through unchanged.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::net::TcpStream;

use super::super::router::PortRouter;
use super::config::{PATH, VERSION_HEADER};
use super::MeshSnapshot;

/// The port the iptables rules redirect outbound TCP to.
pub const OUTBOUND_PORT: u16 = 15001;
/// The UID the sidecar runs as; its own connections aren't redirected.
pub const SIDECAR_UID: u32 = 1337;
/// How long the start of a routed connection may take to show whether it's
/// HTTP; a client that waits for the server to speak first gets relayed.
const SNIFF_TIMEOUT: Duration = Duration::from_secs(5);

/// Where the sidecar gets its routes, and for which namespace.
#[derive(Clone, Debug)]
pub struct SidecarSettings {
    /// The controller's endpoint, e.g. `http://rws-mesh.rws-system:15010`.
    pub config_url: String,
    /// The pod's namespace: consumer routes there apply to it.
    pub namespace: String,
    pub outbound_port: u16,
}

/// A router per destination with routes.
type Routers = Arc<RwLock<Arc<HashMap<(String, u16), PortRouter>>>>;

/// Routers for a snapshot, for clients in `namespace`.
pub fn build_routers(snapshot: &MeshSnapshot, namespace: &str) -> HashMap<(String, u16), PortRouter> {
    super::table(snapshot, namespace).into_iter().map(|(key, listeners)| (key, PortRouter::new(Arc::new(RwLock::new(listeners))))).collect()
}

/// Run the sidecar: follow the controller's snapshots and serve redirected
/// connections. Returns only when the outbound port can't be bound.
pub async fn run(settings: SidecarSettings) {
    let routers: Routers = Arc::default();
    tokio::spawn(follow(settings.clone(), Arc::clone(&routers)));
    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", settings.outbound_port)).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("[gateway-api] mesh sidecar: can't listen on {}: {e}", settings.outbound_port);
            return;
        }
    };
    println!("[gateway-api] mesh sidecar for namespace {} on 127.0.0.1:{}", settings.namespace, settings.outbound_port);
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        };
        let routers = routers.read().unwrap_or_else(|e| e.into_inner()).clone();
        tokio::spawn(handle(stream, peer, routers, settings.outbound_port));
    }
}

/// One redirected connection: routed, or relayed to where it was going.
async fn handle(stream: TcpStream, peer: SocketAddr, routers: Arc<HashMap<(String, u16), PortRouter>>, outbound_port: u16) {
    let Some(destination) = original_destination(&stream) else { return };
    // A connection made to the sidecar itself, not redirected to it: refuse
    // it rather than loop.
    if destination.ip().is_loopback() && destination.port() == outbound_port {
        return;
    }
    if let Some(router) = routers.get(&(destination.ip().to_string(), destination.port())) {
        if speaks_http(&stream).await {
            crate::server::serve_plain_connection(stream, peer, router.clone()).await;
            return;
        }
    }
    relay(stream, destination).await;
}

/// Bytes both ways between the client and its original destination.
async fn relay(mut client: TcpStream, destination: SocketAddr) {
    let Ok(Ok(mut upstream)) = tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(destination)).await else { return };
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
}

/// Whether the client's first bytes are an HTTP/1.x request line or the
/// HTTP/2 preface (read with `peek`, so nothing is consumed).
async fn speaks_http(stream: &TcpStream) -> bool {
    let mut buffer = [0u8; 16];
    let deadline = tokio::time::Instant::now() + SNIFF_TIMEOUT;
    loop {
        let n = match tokio::time::timeout_at(deadline, stream.peek(&mut buffer)).await {
            Ok(Ok(n)) if n > 0 => n,
            _ => return false,
        };
        match looks_like_http(&buffer[..n]) {
            Some(answer) => return answer,
            None if n < buffer.len() => tokio::time::sleep(Duration::from_millis(2)).await,
            None => return false,
        }
    }
}

/// `Some(true)`: HTTP; `Some(false)`: not; `None`: too short to tell.
pub fn looks_like_http(start: &[u8]) -> Option<bool> {
    const PREFACE: &[u8] = b"PRI * HTTP/2.0";
    if PREFACE.starts_with(start) {
        return if start.len() >= 4 { Some(true) } else { None };
    }
    if start.starts_with(PREFACE) {
        return Some(true);
    }
    // An HTTP/1.x method: upper-case letters, then a space.
    for (i, byte) in start.iter().enumerate() {
        match byte {
            b'A'..=b'Z' => {}
            b' ' => return Some(i > 0),
            _ => return Some(false),
        }
    }
    None
}

/// The address a redirected connection was made to.
#[cfg(target_os = "linux")]
pub fn original_destination(stream: &TcpStream) -> Option<SocketAddr> {
    use std::os::fd::AsRawFd;
    // linux/netfilter_ipv4.h SO_ORIGINAL_DST, linux/netfilter_ipv6/ip6_tables.h IP6T_SO_ORIGINAL_DST.
    const SO_ORIGINAL_DST: libc::c_int = 80;
    let fd = stream.as_raw_fd();
    if stream.local_addr().ok()?.is_ipv4() {
        // SAFETY: getsockopt writes at most `len` bytes into `addr`.
        let mut addr: libc::sockaddr_in = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
        let rc = unsafe { libc::getsockopt(fd, libc::SOL_IP, SO_ORIGINAL_DST, &mut addr as *mut _ as *mut libc::c_void, &mut len) };
        (rc == 0).then(|| SocketAddr::from((std::net::Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr)), u16::from_be(addr.sin_port))))
    } else {
        // SAFETY: as above.
        let mut addr: libc::sockaddr_in6 = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t;
        let rc = unsafe { libc::getsockopt(fd, libc::SOL_IPV6, SO_ORIGINAL_DST, &mut addr as *mut _ as *mut libc::c_void, &mut len) };
        (rc == 0).then(|| SocketAddr::from((std::net::Ipv6Addr::from(addr.sin6_addr.s6_addr), u16::from_be(addr.sin6_port))))
    }
}

/// Only Linux has `SO_ORIGINAL_DST`.
#[cfg(not(target_os = "linux"))]
pub fn original_destination(_stream: &TcpStream) -> Option<SocketAddr> {
    None
}

/// Long-poll the controller for snapshots and swap the routers in.
async fn follow(settings: SidecarSettings, routers: Routers) {
    let client = crate::http_client::AsyncClient::new().timeout_ms((super::config::LONG_POLL.as_millis() + 15_000) as u64);
    let mut version = 0u64;
    loop {
        let url = format!("{}{PATH}?version={version}", settings.config_url.trim_end_matches('/'));
        match client.get(&url).send().await {
            Ok(response) if response.status() == 200 => {
                let next = response.header(VERSION_HEADER).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
                match serde_json::from_slice::<MeshSnapshot>(response.bytes()) {
                    Ok(snapshot) if next != version => {
                        let built = build_routers(&snapshot, &settings.namespace);
                        println!("[gateway-api] mesh sidecar: snapshot {next}, {} routed destinations", built.len());
                        *routers.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(built);
                        version = next;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("[gateway-api] mesh sidecar: bad snapshot: {e}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
            _ => tokio::time::sleep(Duration::from_secs(1)).await,
        }
    }
}
