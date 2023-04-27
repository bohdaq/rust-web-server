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
pub mod core;
pub mod application;


fn main() {
    let new_server = Server::setup();
    if new_server.is_err() {
        eprintln!("{}", new_server.as_ref().err().unwrap());
    }


    let (listener, pool) = new_server.unwrap();
    let app = App::new();


    Server::run(listener, pool, app);
}