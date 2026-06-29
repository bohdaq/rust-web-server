use crate::app::App;
use crate::core::New;
use crate::server::Server;

pub mod app;
pub mod client_hint;
pub mod cors;
pub mod entry_point;
pub mod ext;
pub mod header;
pub mod http;
pub mod language;
pub mod mime_type;
pub mod range;
pub mod request;
pub mod response;
pub mod server;
pub mod symbol;
pub mod thread_pool;
pub mod log;
pub mod body;
pub mod json;
pub mod null;
pub mod url;
pub mod core;
pub mod application;
pub mod controller;

#[cfg(feature = "http2")]
pub mod tls;
#[cfg(feature = "http2")]
pub mod h2_handler;

#[cfg(not(feature = "http2"))]
fn main() {
    let new_server = Server::setup();
    if new_server.is_err() {
        eprintln!("{}", new_server.as_ref().err().unwrap());
        return;
    }

    let (listener, pool) = new_server.unwrap();
    let app = App::new();

    Server::run(listener, pool, app);
}

#[cfg(feature = "http2")]
#[tokio::main]
async fn main() {
    let new_server = Server::setup();
    if new_server.is_err() {
        eprintln!("{}", new_server.as_ref().err().unwrap());
        return;
    }

    let (listener, pool) = new_server.unwrap();
    let app = App::new();

    Server::run_tls(listener, pool, app).await;
}