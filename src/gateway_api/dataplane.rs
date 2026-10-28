//! The data plane: one accept loop (or UDP socket) per container port of a
//! served listener, started, updated and stopped as the controller's
//! configuration changes. HTTP and HTTPS connections go through rws's own
//! `serve_plain_connection` / `serve_tls_connection` (HTTP/1.1, h2c, TLS with
//! HTTP/2, PROXY protocol, timeouts, metrics) with a [`PortRouter`] as the
//! application; TLS, TCP and UDP listeners through [`super::l4`].

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;

use super::router::{host_score, PortListeners, PortRouter};
use super::translate::{ClientCa, GatewayConfig, ListenerConfig};

/// How a port's connections are served.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// HTTP or HTTPS: rws's HTTP server with a [`PortRouter`].
    Http,
    /// TLS, terminated with the listeners' certificates (TLSRoute).
    TlsTerminate,
    /// TLS passed through by SNI (TLSRoute).
    TlsPassthrough,
    /// TCPRoute.
    Tcp,
    /// UDPRoute.
    Udp,
}

impl Kind {
    fn of(listener: &ListenerConfig) -> Option<Kind> {
        Some(match listener.protocol.as_str() {
            "HTTP" | "HTTPS" => Kind::Http,
            "TLS" if listener.tls.is_some() => Kind::TlsTerminate,
            "TLS" => Kind::TlsPassthrough,
            "TCP" => Kind::Tcp,
            "UDP" => Kind::Udp,
            _ => return None,
        })
    }
}

/// A port being served.
struct Port {
    kind: Kind,
    listeners: PortListeners,
    tls: Arc<RwLock<Option<tokio_rustls::TlsAcceptor>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Port {
    fn drop(&mut self) {
        self.task.abort(); // closes the listening socket
    }
}

/// The ports being served, by container port and transport (a TCP and a
/// UDP listener can share a number).
#[derive(Default)]
pub struct DataPlane {
    ports: Mutex<BTreeMap<(u16, bool), Port>>,
    /// The address to bind (`0.0.0.0` in a pod).
    bind_ip: String,
}

impl DataPlane {
    pub fn new(bind_ip: &str) -> DataPlane {
        DataPlane { ports: Mutex::new(BTreeMap::new()), bind_ip: bind_ip.to_string() }
    }

    /// The container ports currently served.
    pub fn ports(&self) -> Vec<u16> {
        self.ports.lock().unwrap_or_else(|e| e.into_inner()).keys().map(|(port, _)| *port).collect()
    }

    /// Serve `config`: bind new ports, update the routes and certificates of
    /// running ones, stop the rest. Must run inside a tokio runtime.
    pub fn apply(&self, config: &GatewayConfig) {
        let mut wanted: BTreeMap<(u16, bool), (Kind, Vec<ListenerConfig>)> = BTreeMap::new();
        for listener in &config.listeners {
            let Some(kind) = Kind::of(listener) else { continue };
            let entry = wanted.entry((listener.container_port, kind == Kind::Udp)).or_insert((kind, Vec::new()));
            // One port, one way of serving: a terminating TLS listener wins.
            entry.0 = entry.0.min(kind);
            entry.1.push(listener.clone());
        }
        let mut ports = self.ports.lock().unwrap_or_else(|e| e.into_inner());
        ports.retain(|key, port| wanted.get(key).is_some_and(|(kind, _)| *kind == port.kind));
        for (key, (kind, listeners)) in wanted {
            let acceptor = match kind {
                Kind::Http => tls_acceptor(&listeners, true),
                Kind::TlsTerminate => tls_acceptor(&listeners, false),
                _ => None,
            };
            if let Some(running) = ports.get(&key) {
                *running.listeners.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(listeners);
                *running.tls.write().unwrap_or_else(|e| e.into_inner()) = acceptor;
                continue;
            }
            match self.serve(key.0, kind, listeners, acceptor) {
                Ok(running) => {
                    ports.insert(key, running);
                }
                Err(e) => eprintln!("[gateway-api] can't serve port {}: {e}", key.0),
            }
        }
    }

    fn serve(&self, port: u16, kind: Kind, listeners: Vec<ListenerConfig>, acceptor: Option<tokio_rustls::TlsAcceptor>) -> Result<Port, String> {
        let listeners: PortListeners = Arc::new(RwLock::new(Arc::new(listeners)));
        let tls = Arc::new(RwLock::new(acceptor));
        if kind == Kind::Udp {
            let socket = std::net::UdpSocket::bind((self.bind_ip.as_str(), port)).map_err(|e| e.to_string())?;
            socket.set_nonblocking(true).map_err(|e| e.to_string())?;
            let socket = Arc::new(tokio::net::UdpSocket::from_std(socket).map_err(|e| e.to_string())?);
            let task = tokio::spawn(super::l4::serve_udp(socket, Arc::clone(&listeners)));
            return Ok(Port { kind, listeners, tls, task });
        }
        let socket = std::net::TcpListener::bind((self.bind_ip.as_str(), port)).map_err(|e| e.to_string())?;
        socket.set_nonblocking(true).map_err(|e| e.to_string())?;
        let socket = tokio::net::TcpListener::from_std(socket).map_err(|e| e.to_string())?;
        let router = PortRouter::new(Arc::clone(&listeners));
        let (listeners_for_loop, tls_for_loop) = (Arc::clone(&listeners), Arc::clone(&tls));
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, peer)) = socket.accept().await else {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    continue;
                };
                let acceptor = tls_for_loop.read().unwrap_or_else(|e| e.into_inner()).clone();
                let (router, listeners, counter) = (router.clone(), Arc::clone(&listeners_for_loop), Arc::clone(&counter));
                tokio::spawn(async move {
                    match (kind, acceptor) {
                        (Kind::Http, Some(acceptor)) => crate::server::serve_tls_connection(stream, peer, acceptor, router).await,
                        (Kind::Http, None) => crate::server::serve_plain_connection(stream, peer, router).await,
                        (Kind::TlsTerminate, Some(acceptor)) => super::l4::serve_tls_terminate(stream, acceptor, listeners, counter).await,
                        (Kind::TlsPassthrough, _) => super::l4::serve_tls_passthrough(stream, listeners, counter).await,
                        (Kind::Tcp, _) => super::l4::serve_tcp(stream, listeners, counter).await,
                        _ => {}
                    }
                });
            }
        });
        Ok(Port { kind, listeners, tls, task })
    }
}

/// Certificates by listener hostname, for SNI.
#[derive(Debug)]
struct ListenerCerts {
    /// (hostname or empty for any, certificate).
    certs: Vec<(String, Arc<CertifiedKey>)>,
}

impl ResolvesServerCert for ListenerCerts {
    fn resolve(&self, hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        let name = hello.server_name().map(str::to_ascii_lowercase).unwrap_or_default();
        self.certs
            .iter()
            .filter_map(|(pattern, cert)| if pattern.is_empty() { Some(((0, 0), cert)) } else { host_score(pattern, &name).map(|s| (s, cert)) })
            .max_by_key(|(score, _)| *score)
            .map(|(_, cert)| Arc::clone(cert))
    }
}

/// A TLS acceptor for the port's listeners that terminate TLS (HTTPS, or
/// TLS in `Terminate` mode); `None` when none do. `http` offers h2 and
/// http/1.1 by ALPN. With a client CA (per port, so the same for all of
/// them) client certificates are asked for: required and verified, or under
/// `AllowInsecureFallback` asked for and let through either way.
fn tls_acceptor(listeners: &[ListenerConfig], http: bool) -> Option<tokio_rustls::TlsAcceptor> {
    let certs: Vec<(String, Arc<CertifiedKey>)> = listeners
        .iter()
        .filter(|l| matches!(l.protocol.as_str(), "HTTPS" | "TLS"))
        .filter_map(|l| {
            let (cert, key) = l.tls.as_ref()?;
            match crate::tls::certified_key_from_pem(cert, key) {
                Ok(certified) => Some((l.hostname.clone().unwrap_or_default(), Arc::new(certified))),
                Err(e) => {
                    eprintln!("[gateway-api] listener {}: {e}", l.name);
                    None
                }
            }
        })
        .collect();
    if certs.is_empty() {
        return None;
    }
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let builder = rustls::ServerConfig::builder();
    let builder = match listeners.iter().find_map(|l| l.client_ca.as_ref()).map(client_verifier) {
        Some(Ok(verifier)) => builder.with_client_cert_verifier(verifier),
        Some(Err(e)) => {
            eprintln!("[gateway-api] client CA: {e}");
            return None; // fail closed: no connection without the asked-for validation
        }
        None => builder.with_no_client_auth(),
    };
    let mut config = builder.with_cert_resolver(Arc::new(ListenerCerts { certs }));
    if http {
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    }
    Some(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
}

fn client_verifier(ca: &ClientCa) -> Result<Arc<dyn rustls::server::danger::ClientCertVerifier>, String> {
    use rustls::pki_types::pem::PemObject;
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls::pki_types::CertificateDer::pem_slice_iter(&ca.pem) {
        roots.add(cert.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    }
    let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(roots)).build().map_err(|e| e.to_string())?;
    Ok(if ca.insecure_fallback { Arc::new(InsecureFallback(verifier)) } else { verifier })
}

/// `AllowInsecureFallback`: asks for a client certificate (naming the CAs)
/// but lets the connection through without one or with one that doesn't
/// verify; handshake signatures are still checked.
#[derive(Debug)]
struct InsecureFallback(Arc<dyn rustls::server::danger::ClientCertVerifier>);

impl rustls::server::danger::ClientCertVerifier for InsecureFallback {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        self.0.root_hint_subjects()
    }

    fn client_auth_mandatory(&self) -> bool {
        false
    }

    fn verify_client_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
        Ok(rustls::server::danger::ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.supported_verify_schemes()
    }
}

#[cfg(test)]
mod tests;
