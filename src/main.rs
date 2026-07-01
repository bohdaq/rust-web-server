use rust_web_server::app::App;
use rust_web_server::core::New;
use rust_web_server::metrics::SERVER_READY;
use rust_web_server::server::Server;
use std::sync::atomic::Ordering;

#[cfg(not(feature = "http2"))]
fn main() {
    let new_server = Server::setup();
    if new_server.is_err() {
        eprintln!("{}", new_server.as_ref().err().unwrap());
        return;
    }

    let (listener, pool) = new_server.unwrap();
    let app = App::new();
    SERVER_READY.store(true, Ordering::SeqCst);
    Server::run(listener, pool, app);
}

#[cfg(all(feature = "http2", not(feature = "http3")))]
#[tokio::main]
async fn main() {
    let new_server = Server::setup();
    if new_server.is_err() {
        eprintln!("{}", new_server.as_ref().err().unwrap());
        return;
    }

    let (listener, pool) = new_server.unwrap();
    let app = App::new();

    #[cfg(feature = "acme")]
    {
        use rust_web_server::acme::{AcmeConfig, AcmeManager};
        if let Some(cfg) = AcmeConfig::from_env() {
            let mgr = AcmeManager::new(cfg);
            if let Err(e) = mgr.provision_if_needed().await {
                eprintln!("[ACME] Startup provisioning failed: {e}");
            }
            tokio::spawn(mgr.run_renewal_loop());
        }
    }

    SERVER_READY.store(true, Ordering::SeqCst);

    tokio::join!(
        Server::run_tls(listener, pool, app),
        Server::run_redirect(),
    );
}

#[cfg(feature = "http3")]
#[tokio::main]
async fn main() {
    let new_server = Server::setup();
    if new_server.is_err() {
        eprintln!("{}", new_server.as_ref().err().unwrap());
        return;
    }

    let (listener, pool) = new_server.unwrap();
    let app = App::new();

    #[cfg(feature = "acme")]
    {
        use rust_web_server::acme::{AcmeConfig, AcmeManager};
        if let Some(cfg) = AcmeConfig::from_env() {
            let mgr = AcmeManager::new(cfg);
            if let Err(e) = mgr.provision_if_needed().await {
                eprintln!("[ACME] Startup provisioning failed: {e}");
            }
            tokio::spawn(mgr.run_renewal_loop());
        }
    }

    SERVER_READY.store(true, Ordering::SeqCst);

    tokio::join!(
        Server::run_tls(listener, pool, app),
        Server::run_quic(app),
        Server::run_redirect(),
    );
}