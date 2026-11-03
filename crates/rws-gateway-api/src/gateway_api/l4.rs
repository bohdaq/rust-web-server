//! TLS, TCP and UDP listeners: TLSRoutes (passthrough by SNI, or
//! terminated with the listener's certificates), TCPRoutes and UDPRoutes.
//! A connection (or a UDP client's datagrams) goes to one route — the one
//! whose hostname best matches the SNI, else the oldest — and to a backend
//! of its first rule by weight; bytes are relayed both ways.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpStream, UdpSocket};

use super::router::{host_score, pick_listener, PortListeners};
use super::routes::CompiledHttpRoute;
use super::translate::ListenerConfig;

/// The most a ClientHello may take before passthrough gives up on it.
const MAX_CLIENT_HELLO: usize = 16 * 1024;
/// How long a UDP client's session lives without traffic.
pub const UDP_IDLE: Duration = Duration::from_secs(60);
/// Connect timeout for backends.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The route a connection goes to: the best hostname match for `sni`
/// (a route without hostnames matches anything, below any hostname match),
/// ties to the oldest route, then `namespace/name`.
pub fn pick_route<'a>(routes: &'a [CompiledHttpRoute], sni: Option<&str>) -> Option<&'a CompiledHttpRoute> {
    let sni = sni.unwrap_or("").to_ascii_lowercase();
    routes
        .iter()
        .filter_map(|route| {
            let score = if route.hostnames.is_empty() { Some((0, 0)) } else { route.hostnames.iter().filter_map(|h| host_score(h, &sni)).max() };
            score.map(|s| (s, route))
        })
        .max_by(|(a, ra), (b, rb)| a.cmp(b).then_with(|| (rb.created.as_str(), &rb.key).cmp(&(ra.created.as_str(), &ra.key))))
        .map(|(_, route)| route)
}

/// An endpoint of the route's first rule with backends, by weight; `None`
/// when there is nothing to send to.
pub fn pick_endpoint(route: &CompiledHttpRoute, counter: &AtomicUsize) -> Option<String> {
    let rule = route.rules.iter().find(|r| !r.backends.is_empty())?;
    let total: u32 = rule.backends.iter().map(|b| b.weight).sum();
    if total == 0 {
        return None;
    }
    let n = counter.fetch_add(1, Ordering::Relaxed);
    let mut pick = ((n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) as u32 % total;
    let backend = rule.backends.iter().find(|b| {
        if pick < b.weight {
            true
        } else {
            pick -= b.weight;
            false
        }
    })?;
    let endpoints = backend.endpoints.as_ref().ok().filter(|e| !e.is_empty())?;
    Some(endpoints[n % endpoints.len()].clone())
}

/// The listener and endpoint for a connection with this SNI.
fn target(listeners: &PortListeners, sni: Option<&str>, counter: &AtomicUsize) -> Option<String> {
    let listeners: Arc<Vec<ListenerConfig>> = listeners.read().unwrap_or_else(|e| e.into_inner()).clone();
    let listener = pick_listener(&listeners, &sni.unwrap_or("").to_ascii_lowercase())?;
    pick_endpoint(pick_route(&listener.http_routes, sni)?, counter)
}

async fn connect(endpoint: &str) -> Option<TcpStream> {
    let address: SocketAddr = endpoint.parse().ok()?;
    tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address)).await.ok()?.ok()
}

/// A TCPRoute connection: straight to a backend.
pub async fn serve_tcp(mut client: TcpStream, listeners: PortListeners, counter: Arc<AtomicUsize>) {
    let Some(endpoint) = target(&listeners, None, &counter) else { return };
    let Some(mut backend) = connect(&endpoint).await else { return };
    let _ = tokio::io::copy_bidirectional(&mut client, &mut backend).await;
}

/// What the start of a TLS connection says.
#[derive(Debug, PartialEq)]
pub enum Hello {
    /// Not a whole ClientHello yet.
    NeedMore,
    /// A ClientHello, with its SNI if it has one.
    Sni(Option<String>),
    /// Not a ClientHello rustls accepts.
    NotTls,
}

/// Read the ClientHello at the start of `bytes`.
pub fn client_hello(bytes: &[u8]) -> Hello {
    let mut acceptor = rustls::server::Acceptor::default();
    let mut input = bytes;
    while !input.is_empty() {
        if acceptor.read_tls(&mut input).is_err() {
            return Hello::NotTls;
        }
    }
    match acceptor.accept() {
        Ok(Some(accepted)) => Hello::Sni(accepted.client_hello().server_name().map(str::to_string)),
        Ok(None) => Hello::NeedMore,
        Err(_) => Hello::NotTls,
    }
}

/// A passthrough TLSRoute connection: read the ClientHello for its SNI,
/// then replay it to the backend and relay the encrypted stream.
pub async fn serve_tls_passthrough(mut client: TcpStream, listeners: PortListeners, counter: Arc<AtomicUsize>) {
    use tokio::io::AsyncReadExt;
    let mut hello = Vec::new();
    let sni = loop {
        let mut buffer = [0u8; 4096];
        let n = match tokio::time::timeout(Duration::from_secs(10), client.read(&mut buffer)).await {
            Ok(Ok(n)) if n > 0 => n,
            _ => return,
        };
        hello.extend_from_slice(&buffer[..n]);
        match client_hello(&hello) {
            Hello::Sni(sni) => break sni,
            Hello::NeedMore if hello.len() < MAX_CLIENT_HELLO => continue,
            _ => return,
        }
    };
    let Some(endpoint) = target(&listeners, sni.as_deref(), &counter) else { return };
    let Some(mut backend) = connect(&endpoint).await else { return };
    if backend.write_all(&hello).await.is_err() {
        return;
    }
    let _ = tokio::io::copy_bidirectional(&mut client, &mut backend).await;
}

/// A terminated TLSRoute connection: handshake with the listener's
/// certificates, then relay the plaintext.
pub async fn serve_tls_terminate(client: TcpStream, acceptor: tokio_rustls::TlsAcceptor, listeners: PortListeners, counter: Arc<AtomicUsize>) {
    let Ok(Ok(mut tls)) = tokio::time::timeout(Duration::from_secs(10), acceptor.accept(client)).await else { return };
    let sni = tls.get_ref().1.server_name().map(str::to_string);
    let Some(endpoint) = target(&listeners, sni.as_deref(), &counter) else { return };
    let Some(mut backend) = connect(&endpoint).await else { return };
    let _ = tokio::io::copy_bidirectional(&mut tls, &mut backend).await;
}

/// A UDP client's session: its socket to the backend and when it was last used.
struct Session {
    backend: Arc<UdpSocket>,
    seen: Arc<Mutex<Instant>>,
}

/// A UDPRoute listener: each client address gets its own socket to a
/// backend (picked once), replies go back from the listener's socket;
/// sessions end after [`UDP_IDLE`] without traffic — or with the port:
/// they're in a `JoinSet` this task owns, so dropping it (the port given up)
/// aborts them and releases the socket at once.
pub async fn serve_udp(socket: Arc<UdpSocket>, listeners: PortListeners) {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut session_tasks = tokio::task::JoinSet::new();
    let sessions: Arc<Mutex<HashMap<SocketAddr, Session>>> = Arc::default();
    let mut buffer = vec![0u8; 65_535];
    loop {
        // Reap finished sessions so the set doesn't grow.
        while session_tasks.try_join_next().is_some() {}
        let Ok((n, client)) = socket.recv_from(&mut buffer).await else {
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        };
        let existing = sessions.lock().unwrap_or_else(|e| e.into_inner()).get(&client).map(|s| (Arc::clone(&s.backend), Arc::clone(&s.seen)));
        let (backend, seen) = match existing {
            Some(session) => session,
            None => {
                let Some(endpoint) = target(&listeners, None, &counter) else { continue };
                let Some(backend) = udp_connect(&endpoint).await else { continue };
                let backend = Arc::new(backend);
                let seen = Arc::new(Mutex::new(Instant::now()));
                sessions.lock().unwrap_or_else(|e| e.into_inner()).insert(client, Session { backend: Arc::clone(&backend), seen: Arc::clone(&seen) });
                session_tasks.spawn(udp_replies(Arc::clone(&socket), Arc::clone(&backend), client, Arc::clone(&seen), Arc::clone(&sessions)));
                (backend, seen)
            }
        };
        *seen.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        let _ = backend.send(&buffer[..n]).await;
    }
}

async fn udp_connect(endpoint: &str) -> Option<UdpSocket> {
    let address: SocketAddr = endpoint.parse().ok()?;
    let local = if address.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
    let socket = UdpSocket::bind(local).await.ok()?;
    socket.connect(address).await.ok()?;
    Some(socket)
}

/// Backend → client for one session, until it's idle for [`UDP_IDLE`].
async fn udp_replies(listener: Arc<UdpSocket>, backend: Arc<UdpSocket>, client: SocketAddr, seen: Arc<Mutex<Instant>>, sessions: Arc<Mutex<HashMap<SocketAddr, Session>>>) {
    let mut buffer = vec![0u8; 65_535];
    loop {
        let idle = seen.lock().unwrap_or_else(|e| e.into_inner()).elapsed();
        if idle >= UDP_IDLE {
            break;
        }
        match tokio::time::timeout(UDP_IDLE - idle, backend.recv(&mut buffer)).await {
            Ok(Ok(n)) => {
                *seen.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
                let _ = listener.send_to(&buffer[..n], client).await;
            }
            Ok(Err(_)) => break,
            Err(_) => continue, // re-check the idle time
        }
    }
    sessions.lock().unwrap_or_else(|e| e.into_inner()).remove(&client);
}

#[cfg(test)]
mod tests;
