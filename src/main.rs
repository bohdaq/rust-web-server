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
use crate::symbol::SYMBOL;
use crate::log::Log;

fn main() {
    start();
}

pub fn start() {
    let info = Log::info("Rust Web Server");
    println!("{}", info);

    let usage_info = Log::usage_information();
    println!("{}", usage_info);


    println!("RWS Configuration Start: \n");

    set_default_values();
    bootstrap();

    println!("\nRWS Configuration End\n\n");


    let (ip, port, thread_count) = get_ip_port_thread_count();
    create_tcp_listener_with_thread_pool(ip.as_str(), port, thread_count);
}

pub fn create_tcp_listener_with_thread_pool(ip: &str, port: i32, thread_count: i32) {
    let mut ip_readable = ip.to_string();

    if ip.contains(":") {
        ip_readable = [SYMBOL.opening_square_bracket, ip, SYMBOL.closing_square_bracket].join("");
    }

    let bind_addr = [ip_readable, SYMBOL.colon.to_string(), port.to_string()].join(SYMBOL.empty_string);
    println!("Setting up http://{}...", &bind_addr);

    let boxed_listener = TcpListener::bind(&bind_addr);
    if boxed_listener.is_err() {
        eprintln!("unable to set up TCP listener: {}", boxed_listener.err().unwrap());
        return;
    }

    let listener = boxed_listener.unwrap();
    let pool = ThreadPool::new(thread_count as usize);


    let server_url_thread_count = Log::server_url_thread_count("http", &bind_addr, thread_count);
    println!("{}", server_url_thread_count);


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

