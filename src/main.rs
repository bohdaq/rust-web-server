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
extern crate core;

use crate::entry_point::{bootstrap, get_ip_port_thread_count, set_default_values};
use crate::server::Server;
use crate::thread_pool::ThreadPool;
use std::net::TcpListener;
use crate::entry_point::command_line_args::CommandLineArgument;
use crate::symbol::SYMBOL;

fn main() {
    start();
}

pub fn start() {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
    const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
    const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
    const RUST_VERSION: &str = env!("CARGO_PKG_RUST_VERSION");
    const LICENSE: &str = env!("CARGO_PKG_LICENSE");

    println!("Rust Web Server");
    println!("Version:       {}", VERSION);
    println!("Authors:       {}", AUTHORS);
    println!("Repository:    {}", REPOSITORY);
    println!("Desciption:    {}", DESCRIPTION);
    println!("Rust Version:  {}", RUST_VERSION);
    println!("License:       {}\n\n", LICENSE);

    println!("Usage:\n");
    let command_line_arg_list = CommandLineArgument::get_command_line_arg_list();
    for arg in command_line_arg_list {
        println!("  {} environment variable\n  -{} or --{} as command line line argument\n  {}\n\n", arg.environment_variable, arg.short_form, arg.long_form, arg._hint.unwrap())
    }
    println!("End of usage section\n\n");


    println!("RWS Configuration Start: \n");
    set_default_values();
    bootstrap();
    let (ip, port, thread_count) = get_ip_port_thread_count();
    create_tcp_listener_with_thread_pool(ip.as_str(), port, thread_count);
}

pub fn create_tcp_listener_with_thread_pool(ip: &str, port: i32, thread_count: i32) {
    let bind_addr = [ip, SYMBOL.colon, port.to_string().as_str()].join(SYMBOL.empty_string);
    println!("Setting up {}...", bind_addr);

    let boxed_listener = TcpListener::bind(&bind_addr);
    if boxed_listener.is_err() {
        eprintln!("unable to set up TCP listener: {}", boxed_listener.err().unwrap());
    } else {
        let listener = boxed_listener.unwrap();
        let pool = ThreadPool::new(thread_count as usize);

        println!("Hello, rust-web-server is up and running: http://{}", &bind_addr);

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

                let boxed_peer_addr = stream.local_addr();
                if boxed_peer_addr.is_ok() {
                    print!(", peer addr: {}\n", boxed_peer_addr.unwrap())
                } else {
                    eprintln!("\nunable to read peer addr");
                }

                pool.execute(move || {
                    Server::process_request(stream);
                });
            }
        }
    }

}
