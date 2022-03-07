mod header;
mod request;
mod response;
mod server;
mod test;
mod app;
mod thread_pool;
mod constant;
mod mime_type;
mod range;
mod cors;

extern crate core;

use std::net::TcpListener;
use crate::constant::CONSTANTS;

use crate::request::Request;
use crate::response::Response;
use crate::server::Server;
use crate::thread_pool::ThreadPool;

use clap::{Arg, App};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    ip: Option<String>,
    port: Option<i32>,
    thread_count: Option<i32>,
}


fn main() {
    let mut ip :String = "127.0.0.1".to_string();
    let mut port = 7878;
    let mut thread_count = 4;

    const VERSION: &str = env!("CARGO_PKG_VERSION");

    let content = std::fs::read_to_string("config.toml");
    if content.is_ok() {
        let config: Config = toml::from_str(content.unwrap().as_str()).unwrap();
        if config.ip.is_some() {
            ip = config.ip.unwrap();
        }

        if config.port.is_some() {
            port = config.port.unwrap();
        }

        if config.thread_count.is_some() {
            thread_count = config.thread_count.unwrap();
        }

    }

    let matches = App::new("rws rust-web-server")
        .version(VERSION)
        .author("Bohdan Tsap <bohdan.tsap@tutanota.com>")
        .about("Hi, rust-web-server (rws) is a simple web-server written in Rust. The rws server can serve static content inside the directory it is started.")
        .arg(Arg::new("port")
            .short('p')
            .long("port")
            .takes_value(true)
            .help("Port"))
        .arg(Arg::new("ip")
            .short('i')
            .long("ip")
            .takes_value(true)
            .help("IP or domain"))
        .arg(Arg::new("threads")
            .short('t')
            .long("threads")
            .takes_value(true)
            .help("Number of threads"))
        .get_matches();

    let port_match = matches.value_of("port");
    match port_match {
        None => println!("Port: {}", port),
        Some(s) => {
            match s.parse::<i32>() {
                Ok(n) => {
                    port = n;
                    println!("Port: {}", n)
                },
                Err(_) => println!("That's not a number! {}", s),
            }
        }
    }

    let ip_match = matches.value_of("ip");
    match ip_match {
        None => println!("IP: {}", ip),
        Some(s) => {
            ip = s.to_string();
            println!("IP: {}", ip)
        }
    }

    let threads_match = matches.value_of("threads");
    match threads_match {
        None => println!("Threads: {}", thread_count),
        Some(s) => {
            match s.parse::<i32>() {
                Ok(n) => {
                    thread_count = n;
                    println!("Threads: {}", n)
                },
                Err(_) => println!("That's not a number! {}", s),
            }
        }
    }


    create_tcp_listener_with_thread_pool(ip.as_str(), port, thread_count);
}

fn create_tcp_listener_with_thread_pool(ip: &str, port: i32, thread_count: i32) {
    let bind_addr = [ip, ":", port.to_string().as_str()].join(CONSTANTS.EMPTY_STRING);
    println!("Hello, rust-web-server is up and running: {}", bind_addr);

    let listener = TcpListener::bind(bind_addr).unwrap();
    let pool = ThreadPool::new(thread_count as usize);

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established!");

        pool.execute(move ||  {
            Server::process_request(stream);
        });
    }
}