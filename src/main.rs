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
use std::env;

use crate::constant::CONSTANTS;

use crate::request::Request;
use crate::response::Response;
use crate::server::Server;
use crate::thread_pool::ThreadPool;

use clap::{Arg, App};
use serde::{Serialize, Deserialize};
use crate::cors::Cors;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    ip: String,
    port: i32,
    thread_count: i32,
    cors: Cors,
}

fn main() {
    const VERSION: &str = env!("CARGO_PKG_VERSION");

    let mut config: Config = read_config(false);

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
        None => println!("Port: {}", config.port),
        Some(s) => {
            match s.parse::<i32>() {
                Ok(port) => {
                    config.port = port;
                    println!("Port: {}", port)
                },
                Err(_) => println!("That's not a number! {}", s),
            }
        }
    }

    let ip_match = matches.value_of("ip");
    match ip_match {
        None => println!("IP: {}", config.ip),
        Some(s) => {
            config.ip = s.to_string();
            println!("IP: {}", config.ip)
        }
    }

    let threads_match = matches.value_of("threads");
    match threads_match {
        None => println!("Threads: {}", config.thread_count),
        Some(s) => {
            match s.parse::<i32>() {
                Ok(thread_count) => {
                    config.thread_count = thread_count;
                    println!("Threads: {}", thread_count)
                },
                Err(_) => println!("That's not a number! {}", s),
            }
        }
    }

    setup_environment_variables(config);

    let ip : String = env::var("rws.config.ip").unwrap();
    let port : i32 = env::var("rws.config.port").unwrap().parse().unwrap();
    let thread_count : i32 = env::var("rws.config.thread_count").unwrap().parse().unwrap();

    create_tcp_listener_with_thread_pool(ip.as_str(), port, thread_count);
}

fn read_config(is_test_mode: bool) -> Config {
    let mut config: Config = Config {
        ip: "".to_string(),
        port: 0,
        thread_count: 0,
        cors: Cors {
            allow_origins: vec![],
            allow_methods: vec![],
            allow_headers: vec![],
            allow_credentials: false,
            expose_headers: vec![],
            max_age: "".to_string()
        }
    };
    let mut filepath = "config.toml";
    if is_test_mode {
        filepath = "src/test/config.toml"
    }

    let content = std::fs::read_to_string(filepath);
    if content.is_ok() {
        config = toml::from_str(content.unwrap().as_str()).unwrap();
    }
    config
}

fn setup_environment_variables(config: Config) {
    env::set_var("rws.config.ip", config.ip.to_string());
    env::set_var("rws.config.port", config.port.to_string());
    env::set_var("rws.config.thread_count", config.thread_count.to_string());
    env::set_var("rws.config.cors.allow_origins", config.cors.allow_origins.join(", "));
    env::set_var("rws.config.cors.allow_credentials", config.cors.allow_credentials.to_string());
    env::set_var("rws.config.cors.allow_headers", config.cors.allow_headers.join(", "));
    env::set_var("rws.config.cors.allow_methods", config.cors.allow_methods.join(", "));
    env::set_var("rws.config.cors.expose_headers", config.cors.expose_headers.join(", "));
    env::set_var("rws.config.cors.max_age", config.cors.max_age);
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