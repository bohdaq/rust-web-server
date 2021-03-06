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

use std::env;
use std::fs::metadata;

use std::collections::HashMap;
use std::io::{self, Error, Read, Write};
use std::net::Shutdown;
use std::str::from_utf8;
use std::{thread, time};
use std::time::Instant;

use mio::event::{Event, Source};
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Registry, Token};

use crate::constant::{CONSTANTS, HTTP_VERSIONS, HTTPError, RESPONSE_STATUS_CODE_REASON_PHRASES, StatusCodeReasonPhrase};

use crate::request::Request;
use crate::response::Response;
use crate::server::Server;
use crate::thread_pool::ThreadPool;

use clap::Arg;
use clap::App as ClapApp;
use crate::app::App;
use serde::{Serialize, Deserialize};
use crate::cors::Cors;
use crate::mime_type::MimeType;
use crate::range::{ContentRange, Range};

const SERVER: Token = Token(0);

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    ip: String,
    port: i32,
    cors: Cors,
}

impl Config {
    pub(crate) const RWS_CONFIG_IP: &'static str = "RWS_CONFIG_IP";
    pub(crate) const RWS_CONFIG_PORT: &'static str = "RWS_CONFIG_PORT";
    pub(crate) const RWS_CONFIG_CORS_ALLOW_ALL: &'static str = "RWS_CONFIG_CORS_ALLOW_ALL";
    pub(crate) const RWS_CONFIG_CORS_ALLOW_ORIGINS: &'static str = "RWS_CONFIG_CORS_ALLOW_ORIGINS";
    pub(crate) const RWS_CONFIG_CORS_ALLOW_CREDENTIALS: &'static str = "RWS_CONFIG_CORS_ALLOW_CREDENTIALS";
    pub(crate) const RWS_CONFIG_CORS_ALLOW_HEADERS: &'static str = "RWS_CONFIG_CORS_ALLOW_HEADERS";
    pub(crate) const RWS_CONFIG_CORS_ALLOW_METHODS: &'static str = "RWS_CONFIG_CORS_ALLOW_METHODS";
    pub(crate) const RWS_CONFIG_CORS_EXPOSE_HEADERS: &'static str = "RWS_CONFIG_CORS_EXPOSE_HEADERS";
    pub(crate) const RWS_CONFIG_CORS_MAX_AGE: &'static str = "RWS_CONFIG_CORS_MAX_AGE";

    pub(crate) const RWS_DEFAULT_IP: &'static str = "127.0.0.1";
    pub(crate) const RWS_DEFAULT_PORT: &'static i32 = &7878;

}

fn main() {
    //test plan: test allowed request with config via system vars, config file or command line
    //           test misconfigured origin, header, presence allow credentials
    let is_test_mode = false;

    bootstrap(is_test_mode);
    let (ip, port) = get_ip_port();
    create_tcp_listener(ip.as_str(), port);
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
    let matches = ClapApp::new("rws rust-web-server")
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

fn get_ip_port() -> (String, i32) {
    let mut ip : String = Config::RWS_DEFAULT_IP.to_string();
    let mut port: i32 = *Config::RWS_DEFAULT_PORT;

    let boxed_ip = env::var(Config::RWS_CONFIG_IP);
    if boxed_ip.is_ok() {
        ip = boxed_ip.unwrap()
    }

    let boxed_port = env::var(Config::RWS_CONFIG_PORT);
    if boxed_port.is_ok() {
        port = boxed_port.unwrap().parse().unwrap()
    }

    (ip, port)
}

fn create_tcp_listener(ip: &str, port: i32) -> io::Result<()> {
    let bind_addr = [ip, ":", port.to_string().as_str()].join(CONSTANTS.EMPTY_STRING);

    // Create a poll instance.
    let mut poll = Poll::new()?;
    // Create storage for events.
    let mut events = Events::with_capacity(10000);

    // Setup the TCP server socket.
    let addr = bind_addr.parse().unwrap();
    let boxed_tcp_listener = TcpListener::bind(addr);
    if boxed_tcp_listener.is_err() {
        println!("Error: Unable to bind {}\nTo check what process is using port try to run 'lsof -i :PORT_NUMBER'\nTo get information about the running process try to run 'ps -p PID'", bind_addr);
    }

    let mut server = boxed_tcp_listener.unwrap();
    println!("Hello, rust-web-server is up and running: {}", bind_addr);


    // Register the server with poll we can receive events for it.
    poll.registry()
        .register(&mut server, SERVER, Interest::READABLE.add(Interest::WRITABLE))?;

    // Map of `Token` -> `TcpStream`.
    let mut connections = HashMap::new();
    // Unique token for each incoming connection.
    let mut unique_token = Token(SERVER.0 + 1);


    loop {
        poll.poll(&mut events, None)?;

        for event in events.iter() {
            let now = Instant::now();
            match event.token() {
                SERVER => loop {
                    // Received an event for the TCP server socket, which
                    // indicates we can accept an connection.
                    let (mut connection, address) = match server.accept() {
                        Ok((connection, address)) => (connection, address),
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // If we get a `WouldBlock` error we know our
                            // listener has no more incoming connections queued,
                            // so we can return to polling and wait for some
                            // more.
                            break;
                        }
                        Err(e) => {
                            // If it was any other kind of error, something went
                            // wrong and we terminate with an error.
                            return Err(e);
                        }
                    };

                    println!("\n\n\nAccepted connection from: {}", address);

                    let token = next(&mut unique_token);
                    poll.registry().register(
                        &mut connection,
                        token,
                        Interest::READABLE.add(Interest::WRITABLE),
                    )?;

                    connections.insert(token, connection);
                },
                token => {
                    // Maybe received an event for a TCP connection.
                    let done = if let Some(connection) = connections.get_mut(&token) {
                        handle_connection_event(poll.registry(), connection, event, now)?
                    } else {
                        // Sporadic events happen, we can safely ignore them.
                        false
                    };
                    if done {
                        if let Some(mut connection) = connections.remove(&token) {
                            poll.registry().deregister(&mut connection)?;
                        }
                    }
                }
            }
        }
    }

}


fn next(current: &mut Token) -> Token {
    let next = current.0;
    current.0 += 1;
    Token(next)
}

/// Returns `true` if the connection is done.
fn handle_connection_event(
    registry: &Registry,
    connection: &mut TcpStream,
    event: &Event,
    now: Instant,
) -> io::Result<bool> {

    let mut connection_closed = false;


    let mut received_data = vec![0; 4096];
    let mut bytes_read = 0;
    // We can (maybe) read from the connection.
    loop {
        match connection.read(&mut received_data[bytes_read..]) {
            Ok(0) => {
                // Reading 0 bytes means the other side has closed the
                // connection or is done writing, then so are we.
                connection_closed = true;
                break;
            }
            Ok(n) => {
                bytes_read += n;
                if bytes_read == received_data.len() {
                    received_data.resize(received_data.len() + 1024, 0);
                }
            }
            // Would block "errors" are the OS's way of saying that the
            // connection is not actually ready to perform this I/O operation.
            Err(ref err) if would_block(err) => break,
            Err(ref err) if interrupted(err) => continue,
            // Other errors we'll consider fatal.
            Err(err) => return Err(err),
        }
    }

    let mut raw_response : Vec<u8> = vec![];

    if bytes_read != 0 {
        let received_data = &received_data[..bytes_read];
        println!("Read {} bytes", received_data.len());

        let boxed_request = Request::parse_request(received_data.as_ref());
        if boxed_request.is_ok() {
            let request = boxed_request.unwrap();
            let (response, request) = App::handle_request(request);
            raw_response = Response::generate_response(response, request);
        } else {
            let error = boxed_request.err().unwrap();
            let content_range = ContentRange {
                unit: CONSTANTS.BYTES.to_string(),
                range: Range {
                    start: 0,
                    end: error.len() as u64
                },
                size: error.len().to_string(),
                body: error.into_bytes(),
                content_type: MimeType::TEXT_PLAIN.to_string()
            };
            let bad_request_error_response = Response {
                http_version: HTTP_VERSIONS.HTTP_VERSION_1_1.to_string(),
                status_code: RESPONSE_STATUS_CODE_REASON_PHRASES.N400_BAD_REQUEST.STATUS_CODE.to_string(),
                reason_phrase: RESPONSE_STATUS_CODE_REASON_PHRASES.N400_BAD_REQUEST.REASON_PHRASE.to_string(),
                headers: vec![],
                content_range_list: vec![content_range]
            };
            raw_response = Response::generate_response(bad_request_error_response, Request {
                method: "".to_string(),
                request_uri: "".to_string(),
                http_version: "".to_string(),
                headers: vec![]
            });

        }

    }


    // We can (maybe) write to the connection.

    match connection.write(raw_response.as_ref()) {
        // We want to write the entire `DATA` buffer in a single go. If we
        // write less we'll return a short write error (same as
        // `io::Write::write_all` does).
        Ok(n) if n < raw_response.len() => return Err(io::ErrorKind::WriteZero.into()),
        Ok(_) => {
            // After we've written something we'll reregister the connection
            // to only respond to readable events.
            registry.reregister(connection, event.token(), Interest::READABLE)?;

            println!("Written {} bytes", raw_response.len());
        }
        // Would block "errors" are the OS's way of saying that the
        // connection is not actually ready to perform this I/O operation.
        Err(ref err) if would_block(err) => {}
        // Got interrupted (how rude!), we'll try again.
        Err(ref err) if interrupted(err) => {
            return handle_connection_event(registry, connection, event, now)
        }
        // Other errors we'll consider fatal.
        Err(err) => return Err(err),
    }

    if connection_closed {
        println!("Connection closed. Processing took {} milliseconds", now.elapsed().as_millis());
        return Ok(true);
    }

    Ok(false)
}

fn would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}

fn interrupted(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Interrupted
}
