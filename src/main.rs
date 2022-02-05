mod header;
mod request;
mod response;
mod server;
mod test;

extern crate core;

use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::{env, fs};

use crate::request::Request;
use crate::response::Response;
use crate::server::Server;


fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);

    let mut port = 7878;
    if args.len() >= 2 {
        port = (&args[1]).parse().unwrap();
    }

    let mut ip_addr = "127.0.0.1";
    if args.len() >= 3 {
        ip_addr = args[2].as_str();
    }

    let bind_addr = [ip_addr, ":", &port.to_string()].join("");

    println!("Hello, rust-web-server!");

    let listener = TcpListener::bind(bind_addr).unwrap();

    let server = Server {
        bind_addr: String::from(ip_addr),
        port
    };

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        println!("Connection established!");
        Server::handle_connection(stream);
    }
}