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
extern crate core;

use crate::entry_point::{bootstrap, get_ip_port_thread_count, set_default_values};
use crate::server::Server;
use crate::thread_pool::ThreadPool;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
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
    } else {
        let listener = boxed_listener.unwrap();
        let pool = ThreadPool::new(thread_count as usize);


        let server_url_thread_count = Log::server_url_thread_count("http", &bind_addr, thread_count);
        println!("{}", server_url_thread_count);


        for boxed_stream in listener.incoming() {
            if boxed_stream.is_err() {
                eprintln!("unable to get TCP stream: {}", boxed_stream.err().unwrap());
            } else {
                let stream = boxed_stream.unwrap();

                print!("Connection established, ");

                let boxed_local_addr = stream.local_addr();
                if boxed_local_addr.is_ok() {
                    print!("local addr: {}", boxed_local_addr.unwrap())
                } else {
                    eprintln!("\nunable to read local addr");
                }

                let boxed_peer_addr = stream.peer_addr();
                if boxed_peer_addr.is_ok() {
                    print!(", peer addr: {}\n", boxed_peer_addr.unwrap())
                } else {
                    eprintln!("\nunable to read peer addr");
                }

                pool.execute(move || {

                    let mut peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), 0);
                    let boxed_peer_addr = stream.peer_addr();
                    if boxed_peer_addr.is_ok() {
                        peer_addr = boxed_peer_addr.unwrap()
                    } else {
                        eprintln!("\nunable to read peer addr");
                    }

                    Server::process_request(stream, peer_addr);
                });
            }
        }
    }

}

