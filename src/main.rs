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
    SERVER_READY.store(true, Ordering::SeqCst);

    tokio::join!(
        Server::run_tls(listener, pool, app),
        Server::run_quic(app),
        Server::run_redirect(),
    );
}