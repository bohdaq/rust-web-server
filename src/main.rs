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


use std::net::TcpListener;
use crate::entry_point::{bootstrap, get_ip_port_thread_count, get_request_allocation_size, set_default_values};
use crate::server::{Address, ConnectionInfo, Server};
use crate::thread_pool::ThreadPool;
use crate::app::App;
use crate::core::New;

fn main() {
    start();
}

pub fn start() {
    let new_server = Server::setup();
    if new_server.is_err() {
        eprintln!("{}", new_server.as_ref().err().unwrap());
    }
    let (listener, pool) = new_server.unwrap();
    run(listener, pool);
}

pub fn run(listener : TcpListener, pool: ThreadPool) {
    for boxed_stream in listener.incoming() {
        if boxed_stream.is_err() {
            eprintln!("unable to get TCP stream: {}", boxed_stream.err().unwrap());
            return;
        }

        let stream = boxed_stream.unwrap();

        print!("Connection established, ");

        let boxed_local_addr = stream.local_addr();
        if boxed_local_addr.is_ok() {
            print!("local addr: {}", boxed_local_addr.unwrap())
        } else {
            eprintln!("\nunable to read local addr");
            return;
        }

        let boxed_peer_addr = stream.peer_addr();
        if boxed_peer_addr.is_err() {
            eprintln!("\nunable to read peer addr");
            return;
        }
        let peer_addr = boxed_peer_addr.unwrap();
        print!(", peer addr: {}\n", peer_addr.to_string());

        let (server_ip, server_port, _thread_count) = get_ip_port_thread_count();
        let client_ip = peer_addr.ip().to_string();
        let client_port = peer_addr.port() as i32;
        let request_allocation_size = get_request_allocation_size();

        let connection = ConnectionInfo {
            client: Address {
                ip: client_ip.to_string(),
                port: client_port
            },
            server: Address {
                ip: server_ip,
                port: server_port
            },
            request_size: request_allocation_size,
        };



        pool.execute(move || {
            let app = App::new();

            let boxed_process = Server::process(stream, connection, app);
            if boxed_process.is_err() {
                let message = boxed_process.err().unwrap();
                eprintln!("{}", message);
            }
        });

    }


}

