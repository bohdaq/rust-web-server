#[cfg(test)]
mod tests;
pub mod command_line_args;


use std::net::TcpListener;
use std::{env};

use crate::server::Server;
use crate::thread_pool::ThreadPool;

use serde::{Serialize, Deserialize};
use crate::cors::Cors;
use crate::entry_point::command_line_args::CommandLineArgument;
use crate::ext::file_ext::FileExt;
use crate::symbol::SYMBOL;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    ip: String,
    port: i32,
    thread_count: i32,
    cors: Cors,
}

impl Config {
    pub const RWS_CONFIG_IP: &'static str = "RWS_CONFIG_IP";
    pub const RWS_CONFIG_PORT: &'static str = "RWS_CONFIG_PORT";
    pub const RWS_CONFIG_THREAD_COUNT: &'static str = "RWS_CONFIG_THREAD_COUNT";

    pub const RWS_CONFIG_CORS_ALLOW_ALL: &'static str = "RWS_CONFIG_CORS_ALLOW_ALL";
    pub const RWS_CONFIG_CORS_ALLOW_ORIGINS: &'static str = "RWS_CONFIG_CORS_ALLOW_ORIGINS";
    pub const RWS_CONFIG_CORS_ALLOW_CREDENTIALS: &'static str = "RWS_CONFIG_CORS_ALLOW_CREDENTIALS";
    pub const RWS_CONFIG_CORS_ALLOW_HEADERS: &'static str = "RWS_CONFIG_CORS_ALLOW_HEADERS";
    pub const RWS_CONFIG_CORS_ALLOW_METHODS: &'static str = "RWS_CONFIG_CORS_ALLOW_METHODS";
    pub const RWS_CONFIG_CORS_EXPOSE_HEADERS: &'static str = "RWS_CONFIG_CORS_EXPOSE_HEADERS";
    pub const RWS_CONFIG_CORS_MAX_AGE: &'static str = "RWS_CONFIG_CORS_MAX_AGE";

    pub const RWS_DEFAULT_IP: &'static str = "127.0.0.1";
    pub const RWS_DEFAULT_PORT: &'static i32 = &7878;
    pub const RWS_DEFAULT_THREAD_COUNT: &'static i32 = &4;

}

pub fn start() {
    bootstrap();
    let (ip, port, thread_count) = get_ip_port_thread_count();
    create_tcp_listener_with_thread_pool(ip.as_str(), port, thread_count);
}

pub fn bootstrap() {
    read_system_environment_variables();
    override_environment_variables_from_config(None);
    override_environment_variables_from_command_line_args();
}

pub fn read_system_environment_variables() {
    println!("  Start Of System Environment Variables Section");

    let boxed_ip = env::var(Config::RWS_CONFIG_IP);
    if boxed_ip.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_IP,
                 boxed_ip.unwrap());
    }

    let boxed_port = env::var(Config::RWS_CONFIG_PORT);
    if boxed_port.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_PORT,
                 boxed_port.unwrap());
    }

    let boxed_thread_count = env::var(Config::RWS_CONFIG_THREAD_COUNT);
    if boxed_thread_count.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_THREAD_COUNT,
                 boxed_thread_count.unwrap());
    }

    let boxed_cors_allow_all = env::var(Config::RWS_CONFIG_CORS_ALLOW_ALL);
    if boxed_cors_allow_all.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_ALLOW_ALL,
                 boxed_cors_allow_all.unwrap());
    }

    let boxed_cors_allow_origins = env::var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS);
    if boxed_cors_allow_origins.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_ALLOW_ORIGINS,
                 boxed_cors_allow_origins.unwrap());
    }

    let boxed_cors_allow_methods = env::var(Config::RWS_CONFIG_CORS_ALLOW_METHODS);
    if boxed_cors_allow_methods.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_ALLOW_METHODS,
                 boxed_cors_allow_methods.unwrap());
    }

    let boxed_cors_allow_headers = env::var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS);
    if boxed_cors_allow_headers.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_ALLOW_HEADERS,
                 boxed_cors_allow_headers.unwrap());
    }

    let boxed_cors_allow_credentials = env::var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS);
    if boxed_cors_allow_credentials.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS,
                 boxed_cors_allow_credentials.unwrap());
    }

    let boxed_cors_expose_headers = env::var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS);
    if boxed_cors_expose_headers.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_EXPOSE_HEADERS,
                 boxed_cors_expose_headers.unwrap());
    }

    let boxed_cors_max_age = env::var(Config::RWS_CONFIG_CORS_MAX_AGE);
    if boxed_cors_max_age.is_ok() {
        println!("    Set env variable '{}' to value '{}' environment variable",
                 Config::RWS_CONFIG_CORS_MAX_AGE,
                 boxed_cors_max_age.unwrap());
    }

    println!("  End of System Environment Variables Section");
}

pub fn override_environment_variables_from_config(filepath: Option<&str>) {
    println!("\n  Start of Config Section");

    let config: Config;

    let path: &str;
    if filepath.is_none() {
        path = "/rws.config.toml";
    } else {
        path = filepath.unwrap();
    }
    let static_filepath = FileExt::get_static_filepath(path);
    let content = std::fs::read_to_string(static_filepath);

    if content.is_err() {
        eprintln!("    Unable to parse rws.config.toml\n{}", content.err().unwrap());
        println!("  End of Config Section");
        return;
    } else {
        config = toml::from_str(content.unwrap().as_str()).unwrap();
    }

    env::set_var(Config::RWS_CONFIG_IP, config.ip.to_string());
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_IP, config.ip.to_string());

    env::set_var(Config::RWS_CONFIG_PORT, config.port.to_string());
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_PORT, config.port.to_string());

    env::set_var(Config::RWS_CONFIG_THREAD_COUNT, config.thread_count.to_string());
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_THREAD_COUNT, config.thread_count.to_string());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ALL, config.cors.allow_all.to_string());
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_ALL, config.cors.allow_all.to_string());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, config.cors.allow_origins.join(","));
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_ORIGINS, config.cors.allow_origins.join(","));

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, config.cors.allow_credentials.to_string());
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_CREDENTIALS, config.cors.allow_credentials.to_string());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_HEADERS, config.cors.allow_headers.join(","));
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_HEADERS, config.cors.allow_headers.join(",").to_lowercase());

    env::set_var(Config::RWS_CONFIG_CORS_ALLOW_METHODS, config.cors.allow_methods.join(","));
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_ALLOW_METHODS, config.cors.allow_methods.join(","));

    env::set_var(Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, config.cors.expose_headers.join(","));
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_EXPOSE_HEADERS, config.cors.expose_headers.join(",").to_lowercase());

    env::set_var(Config::RWS_CONFIG_CORS_MAX_AGE, &config.cors.max_age);
    println!("    Set env variable '{}' to value '{}' from rws.config.toml", Config::RWS_CONFIG_CORS_MAX_AGE, config.cors.max_age);

    println!("  End of Config Section");
}

pub fn override_environment_variables_from_command_line_args() {
    println!("\n  Start of Reading Command Line Arguments Section");

    let args = env::args().collect();
    let params = CommandLineArgument::get_command_line_arg_list();
    CommandLineArgument::_parse(args, params);

    println!("  End of Reading Command Line Arguments\n");
    println!("RWS Configuration End\n\n");
}

pub fn get_ip_port_thread_count() -> (String, i32, i32) {
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

pub fn create_tcp_listener_with_thread_pool(ip: &str, port: i32, thread_count: i32) {
    let bind_addr = [ip, ":", port.to_string().as_str()].join(SYMBOL.empty_string);

    let listener = TcpListener::bind(&bind_addr).unwrap();
    let pool = ThreadPool::new(thread_count as usize);

    println!("Hello, rust-web-server is up and running: {}", &bind_addr);

    for boxed_stream in listener.incoming() {
        let stream = boxed_stream.unwrap();
        println!("Connection established, local addr: {}, peer addr: {}", stream.local_addr().unwrap(), stream.peer_addr().unwrap());

        pool.execute(move ||  {
            Server::process_request(stream);
        });
    }
}