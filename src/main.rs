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
use std::{env, thread};
use std::collections::HashMap;
use std::fs::metadata;
use std::time::Duration;

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

impl Config {
    pub(crate) const RWS_CONFIG_IP: &'static str = "RWS_CONFIG_IP";
    pub(crate) const RWS_CONFIG_PORT: &'static str = "RWS_CONFIG_PORT";
    pub(crate) const RWS_CONFIG_THREAD_COUNT: &'static str = "RWS_CONFIG_THREAD_COUNT";
    pub(crate) const RWS_CONFIG_CORS_ALLOW_ALL: &'static str = "RWS_CONFIG_CORS_ALLOW_ALL";
    pub(crate) const RWS_CONFIG_CORS_ALLOW_ORIGINS: &'static str = "RWS_CONFIG_CORS_ALLOW_ORIGINS";
    pub(crate) const RWS_CONFIG_CORS_ALLOW_CREDENTIALS: &'static str = "RWS_CONFIG_CORS_ALLOW_CREDENTIALS";
    pub(crate) const RWS_CONFIG_CORS_ALLOW_HEADERS: &'static str = "RWS_CONFIG_CORS_ALLOW_HEADERS";
    pub(crate) const RWS_CONFIG_CORS_ALLOW_METHODS: &'static str = "RWS_CONFIG_CORS_ALLOW_METHODS";
    pub(crate) const RWS_CONFIG_CORS_EXPOSE_HEADERS: &'static str = "RWS_CONFIG_CORS_EXPOSE_HEADERS";
    pub(crate) const RWS_CONFIG_CORS_MAX_AGE: &'static str = "RWS_CONFIG_CORS_MAX_AGE";

    pub(crate) const RWS_DEFAULT_IP: &'static str = "127.0.0.1";
    pub(crate) const RWS_DEFAULT_PORT: &'static i32 = &7878;
    pub(crate) const RWS_DEFAULT_THREAD_COUNT: &'static i32 = &4;

}

fn main() {
    let is_test_mode = false;

    bootstrap(is_test_mode);
    let (ip, port, thread_count) = get_ip_port_thread_count();
    create_tcp_listener_with_thread_pool(ip.as_str(), port, thread_count);
}

fn bootstrap(is_test_mode: bool) {
    read_system_environment_variables();
    let is_config_provided = is_config_file_provided(is_test_mode);
    if is_config_provided {
        override_environment_variables_from_config(is_test_mode);
    }
    if !is_test_mode {
        override_environment_variables_from_command_line_args();
    }
}

fn read_system_environment_variables() {
    println!("Start Of System Environment Variables Section");

    let boxed_ip = env::var(Config::RWS_CONFIG_IP);
    if boxed_ip.is_ok() {
        println!("{}={}", Config::RWS_CONFIG_IP, boxed_ip.unwrap());
    }

    let boxed_port = env::var(Config::RWS_CONFIG_PORT);
    if boxed_port.is_ok() {
        println!("{}={}", Config::RWS_CONFIG_PORT, boxed_port.unwrap());
    }

    let boxed_thread_count = env::var(Config::RWS_CONFIG_THREAD_COUNT);
    if boxed_thread_count.is_ok() {
        println!("{}={}", Config::RWS_CONFIG_THREAD_COUNT, boxed_thread_count.unwrap());
    }

    let boxed_cors_allow_all = env::var(Config::RWS_CONFIG_CORS_ALLOW_ALL);
    if boxed_cors_allow_all.is_ok() {
        println!("{}={}", Config::RWS_CONFIG_CORS_ALLOW_ALL, boxed_cors_allow_all.unwrap());
    }

    let boxed_cors_allow_origins = env::var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS);
    if boxed_cors_allow_origins.is_ok() {
        println!("{}={}", Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, boxed_cors_allow_origins.unwrap());
    }

    let boxed_cors_allow_methods = env::var(Config::RWS_CONFIG_CORS_ALLOW_METHODS);
    if boxed_cors_allow_methods.is_ok() {
        println!("{}={}", Config::RWS_CONFIG_CORS_ALLOW_METHODS, boxed_cors_allow_methods.unwrap());
    }

    let boxed_cors_allow_headers = env::var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS);
    if boxed_cors_allow_headers.is_ok() {
        println!("{}={}", Config::RWS_CONFIG_CORS_ALLOW_HEADERS, boxed_cors_allow_headers.unwrap());
    }

    let boxed_cors_allow_credentials = env::var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS);
    if boxed_cors_allow_credentials.is_ok() {
        println!("{}={}", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, boxed_cors_allow_credentials.unwrap());
    }

    let boxed_cors_expose_headers = env::var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS);
    if boxed_cors_expose_headers.is_ok() {
        println!("{}={}", Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, boxed_cors_expose_headers.unwrap());
    }

    let boxed_cors_max_age = env::var(Config::RWS_CONFIG_CORS_MAX_AGE);
    if boxed_cors_max_age.is_ok() {
        println!("{}={}", Config::RWS_CONFIG_CORS_MAX_AGE, boxed_cors_max_age.unwrap());
    }

    println!("End of System Environment Variables Section");
}

fn is_config_file_provided(is_test_mode: bool) -> bool {
    println!("Start of Config Section");
    println!("Is Test Mode: {}", is_test_mode);

    let mut filepath = "/rws.config.toml";
    if is_test_mode {
        filepath = "/src/test/rws.config.toml"
    }
    let static_filepath = Server::get_static_filepath(filepath);
    let mut is_config_provided = metadata(&static_filepath).is_ok();

    if !is_config_provided {
        println!("rws.config.toml is not provided");
        println!("End of Config Section");

    } else {
        let md = metadata(&static_filepath).unwrap();
        is_config_provided = md.is_file();
    }
    is_config_provided
}

fn override_environment_variables_from_config(is_test_mode: bool) {
    let mut config: Config = Config {
        ip: "".to_string(),
        port: 0,
        thread_count: 0,
        cors: Cors {
            allow_all: false,
            allow_origins: vec![],
            allow_methods: vec![],
            allow_headers: vec![],
            allow_credentials: false,
            expose_headers: vec![],
            max_age: "".to_string()
        }
    };

    let mut filepath = "/rws.config.toml";
    if is_test_mode {
        filepath = "/src/test/rws.config.toml"
    }
    let static_filepath = Server::get_static_filepath(filepath);
    let content = std::fs::read_to_string(static_filepath);

    if content.is_err() {
        println!("Unable to parse rws.config.toml\n{}", content.err().unwrap());
    } else {
        config = toml::from_str(content.unwrap().as_str()).unwrap();
    }

    env::set_var(Config::RWS_CONFIG_IP, config.ip.to_string());
    println!("Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_IP, config.ip.to_string());

    env::set_var(Config::RWS_CONFIG_PORT, config.port.to_string());
    println!("Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_PORT, config.port.to_string());

    env::set_var(Config::RWS_CONFIG_THREAD_COUNT, config.thread_count.to_string());
    println!("Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_THREAD_COUNT, config.thread_count.to_string());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ALL, config.cors.allow_all.to_string());
    println!("Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_ALL, config.cors.allow_all.to_string());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, config.cors.allow_origins.join(","));
    println!("Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, config.cors.allow_origins.join(","));

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, config.cors.allow_credentials.to_string());
    println!("Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, config.cors.allow_credentials.to_string());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS, config.cors.allow_headers.join(","));
    println!("Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_HEADERS, config.cors.allow_headers.join(",").to_lowercase());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_METHODS, config.cors.allow_methods.join(","));
    println!("Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_METHODS, config.cors.allow_methods.join(","));

    env::set_var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, config.cors.expose_headers.join(","));
    println!("Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, config.cors.expose_headers.join(",").to_lowercase());

    env::set_var(Config::RWS_CONFIG_CORS_MAX_AGE, &config.cors.max_age);
    println!("Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_MAX_AGE, config.cors.max_age);

    println!("End of Config Section");
}

fn override_environment_variables_from_command_line_args() {
    println!("Start of Reading Command Line Arguments");

    const VERSION: &str = env!("CARGO_PKG_VERSION");
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
        .arg(Arg::new("cors-allow-all")
            .short('a')
            .long("cors-allow-all")
            .takes_value(true)
            .help("If set to true, will allow all CORS requests, other CORS properties will be ignored"))
        .arg(Arg::new("cors-allow-origins")
            .short('o')
            .long("cors-allow-origins")
            .takes_value(true)
            .help("Comma separated list of allowed origins, example: https://foo.example,https://bar.example"))
        .arg(Arg::new("cors-allow-methods")
            .short('m')
            .long("cors-allow_methods")
            .takes_value(true)
            .help("Comma separated list of allowed methods, example: POST,PUT"))
        .arg(Arg::new("cors-allow-headers")
            .short('h')
            .long("cors-allow-headers")
            .takes_value(true)
            .help("Comma separated list of allowed request headers, in lowercase, example: content-type,x-custom-header"))
        .arg(Arg::new("cors-allow-credentials")
            .short('c')
            .long("cors-allow-credentials")
            .takes_value(true)
            .help("If set to true, will allow to transmit credentials via CORS requests"))
        .arg(Arg::new("cors-expose-headers")
            .short('e')
            .long("cors-expose-headers")
            .takes_value(true)
            .help("Comma separated list of allowed response headers, in lowercase, example: content-type,x-custom-header"))
        .arg(Arg::new("cors-max-age")
            .short('g')
            .long("cors-max-age")
            .takes_value(true)
            .help("In seconds, time to cache in browser CORS information, example: 86400"))
        .get_matches();

    let port_match = matches.value_of("port");
    match port_match {
        None => print!(""),
        Some(s) => {
            match s.parse::<i32>() {
                Ok(port) => {
                    env::set_var(Config::RWS_CONFIG_PORT, port.to_string());
                    println!("Set env variable '{}' to value '{}' from command line argument", Config::RWS_CONFIG_PORT, port.to_string());
                },
                Err(_) => println!("That's not a number! {}", s),
            }
        }
    }

    let ip_match = matches.value_of("ip");
    match ip_match {
        None => print!(""),
        Some(ip) => {
            env::set_var(Config::RWS_CONFIG_IP, ip.to_string());
            println!("Set env variable '{}' to value '{}' from command line argument", Config::RWS_CONFIG_IP, ip.to_string());
        }
    }

    let threads_match = matches.value_of("threads");
    match threads_match {
        None => print!(""),
        Some(s) => {
            match s.parse::<i32>() {
                Ok(thread_count) => {
                    env::set_var(Config::RWS_CONFIG_THREAD_COUNT, thread_count.to_string());
                    println!("Set env variable '{}' to value '{}' from command line argument", Config::RWS_CONFIG_THREAD_COUNT, thread_count.to_string());
                },
                Err(_) => println!("That's not a number! {}", s),
            }
        }
    }

    let cors_allow_all = matches.value_of("cors-allow-all");
    match cors_allow_all {
        None => print!(""),
        Some(allow_all) => {
            let is_allow_all: bool = allow_all.parse().unwrap();
            env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ALL, is_allow_all.to_string());
            println!("Set env variable '{}' to value '{}' from command line argument", Config::RWS_CONFIG_CORS_ALLOW_ALL, is_allow_all.to_string());
        }
    }

    let cors_allow_origins = matches.value_of("cors-allow-origins");
    match cors_allow_origins {
        None => print!(""),
        Some(allow_origins) => {
            env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, allow_origins.to_string());
            println!("Set env variable '{}' to value '{}' from command line argument", Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, allow_origins.to_string());
        }
    }

    let cors_allow_methods = matches.value_of("cors-allow-methods");
    match cors_allow_methods {
        None => print!(""),
        Some(allow_origins) => {
            env::set_var(Config::RWS_CONFIG_CORS_ALLOW_METHODS, allow_origins.to_string());
            println!("Set env variable '{}' to value '{}' from command line argument", Config::RWS_CONFIG_CORS_ALLOW_METHODS, allow_origins.to_string());
        }
    }

    let cors_allow_headers = matches.value_of("cors-allow-headers");
    match cors_allow_headers {
        None => print!(""),
        Some(allow_headers) => {
            env::set_var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS, allow_headers.to_string());
            println!("Set env variable '{}' to value '{}' from command line argument", Config::RWS_CONFIG_CORS_ALLOW_HEADERS, allow_headers.to_lowercase());
        }
    }

    let cors_allow_credentials = matches.value_of("cors-allow-credentials");
    match cors_allow_credentials {
        None => print!(""),
        Some(allow_credentials) => {
            let is_allow_credentials: bool = allow_credentials.parse().unwrap();
            env::set_var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, is_allow_credentials.to_string());
            println!("Set env variable '{}' to value '{}' from command line argument", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, is_allow_credentials.to_string());
        }
    }

    let cors_expose_headers = matches.value_of("cors-expose-headers");
    match cors_expose_headers {
        None => print!(""),
        Some(expose_headers) => {
            env::set_var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, expose_headers.to_string());
            println!("Set env variable '{}' to value '{}' from command line argument", Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, expose_headers.to_lowercase());
        }
    }

    let cors_max_age = matches.value_of("cors-max-age");
    match cors_max_age {
        None => print!(""),
        Some(max_age) => {
            env::set_var(Config::RWS_CONFIG_CORS_MAX_AGE, max_age.to_string());
            println!("Set env variable '{}' to value '{}' from command line argument", Config::RWS_CONFIG_CORS_MAX_AGE, max_age.to_string());
        }
    }

    println!("End of Reading Command Line Arguments");
}

fn get_ip_port_thread_count() -> (String, i32, i32) {
    let mut ip : String = Config::RWS_DEFAULT_IP.to_string();
    let mut port: i32 = *Config::RWS_DEFAULT_PORT;
    let mut thread_count: i32 = *Config::RWS_DEFAULT_THREAD_COUNT;

    let boxed_ip = env::var(Config::RWS_CONFIG_IP);
    if boxed_ip.is_ok() {
        ip = boxed_ip.unwrap()
    }

    let boxed_port = env::var(Config::RWS_CONFIG_PORT);
    if boxed_port.is_ok() {
        port = boxed_port.unwrap().parse().unwrap()
    }

    let boxed_thread_count = env::var(Config::RWS_CONFIG_THREAD_COUNT);
    if boxed_thread_count.is_ok() {
        thread_count = boxed_thread_count.unwrap().parse().unwrap()
    }

    (ip, port, thread_count)
}

fn create_tcp_listener_with_thread_pool(ip: &str, port: i32, thread_count: i32) {
    let bind_addr = [ip, ":", port.to_string().as_str()].join(CONSTANTS.EMPTY_STRING);
    println!("Hello, rust-web-server is up and running: {}", bind_addr);

    let listener = TcpListener::bind(bind_addr).unwrap();
    let pool = ThreadPool::new(thread_count as usize);


    for boxed_stream in listener.incoming() {
        let stream = boxed_stream.unwrap();
        println!("Connection established, local addr: {}, peer addr: {}", stream.local_addr().unwrap(), stream.peer_addr().unwrap());

        pool.execute(move ||  {
            Server::process_request(stream);
        });
    }
}