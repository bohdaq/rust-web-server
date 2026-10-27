//! The data plane: one accept loop per container port of a served listener,
//! started, updated and stopped as the controller's configuration changes.
//! Connections go through rws's own `serve_plain_connection` /
//! `serve_tls_connection` (HTTP/1.1, h2c, TLS with HTTP/2, PROXY protocol,
//! timeouts, metrics), with a [`PortRouter`] as the application.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;

use super::router::{host_score, PortListeners, PortRouter};
use super::translate::{GatewayConfig, ListenerConfig};

/// A port being served.
struct Port {
    listeners: PortListeners,
    tls: Arc<RwLock<Option<tokio_rustls::TlsAcceptor>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Port {
    fn drop(&mut self) {
        self.task.abort(); // closes the listening socket
    }
}

/// The ports being served, by container port.
#[derive(Default)]
pub struct DataPlane {
    ports: Mutex<BTreeMap<u16, Port>>,
    /// The address to bind (`0.0.0.0` in a pod).
    bind_ip: String,
}

impl DataPlane {
    pub fn new(bind_ip: &str) -> DataPlane {
        DataPlane { ports: Mutex::new(BTreeMap::new()), bind_ip: bind_ip.to_string() }
    }

    /// The container ports currently served.
    pub fn ports(&self) -> Vec<u16> {
        self.ports.lock().unwrap_or_else(|e| e.into_inner()).keys().copied().collect()
    }

    /// Serve `config`: bind new ports, update the routes and certificates of
    /// running ones, stop the rest. Must run inside a tokio runtime.
    pub fn apply(&self, config: &GatewayConfig) {
        let mut wanted: BTreeMap<u16, Vec<ListenerConfig>> = BTreeMap::new();
        for listener in &config.listeners {
            if matches!(listener.protocol.as_str(), "HTTP" | "HTTPS") {
                wanted.entry(listener.container_port).or_default().push(listener.clone());
            }
        }
        let mut ports = self.ports.lock().unwrap_or_else(|e| e.into_inner());
        ports.retain(|port, _| wanted.contains_key(port));
        for (port, listeners) in wanted {
            let acceptor = tls_acceptor(&listeners);
            if let Some(running) = ports.get(&port) {
                *running.listeners.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(listeners);
                *running.tls.write().unwrap_or_else(|e| e.into_inner()) = acceptor;
                continue;
            }
            match self.serve(port, listeners, acceptor) {
                Ok(running) => {
                    ports.insert(port, running);
                }
                Err(e) => eprintln!("[gateway-api] can't serve port {port}: {e}"),
            }
        }
    }

    fn serve(&self, port: u16, listeners: Vec<ListenerConfig>, acceptor: Option<tokio_rustls::TlsAcceptor>) -> Result<Port, String> {
        let socket = std::net::TcpListener::bind((self.bind_ip.as_str(), port)).map_err(|e| e.to_string())?;
        socket.set_nonblocking(true).map_err(|e| e.to_string())?;
        let socket = tokio::net::TcpListener::from_std(socket).map_err(|e| e.to_string())?;
        let listeners: PortListeners = Arc::new(RwLock::new(Arc::new(listeners)));
        let tls = Arc::new(RwLock::new(acceptor));
        let router = PortRouter::new(Arc::clone(&listeners));
        let tls_for_loop = Arc::clone(&tls);
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, peer)) = socket.accept().await else {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    continue;
                };
                let acceptor = tls_for_loop.read().unwrap_or_else(|e| e.into_inner()).clone();
                let router = router.clone();
                tokio::spawn(async move {
                    match acceptor {
                        Some(acceptor) => crate::server::serve_tls_connection(stream, peer, acceptor, router).await,
                        None => crate::server::serve_plain_connection(stream, peer, router).await,
                    }
                });
            }
        });
        Ok(Port { listeners, tls, task })
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

/// A TLS acceptor for the port's HTTPS listeners; `None` for plain HTTP.
fn tls_acceptor(listeners: &[ListenerConfig]) -> Option<tokio_rustls::TlsAcceptor> {
    let certs: Vec<(String, Arc<CertifiedKey>)> = listeners
        .iter()
        .filter(|l| l.protocol == "HTTPS")
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
    let mut config = rustls::ServerConfig::builder().with_no_client_auth().with_cert_resolver(Arc::new(ListenerCerts { certs }));
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Some(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
}

#[cfg(test)]
mod tests;
