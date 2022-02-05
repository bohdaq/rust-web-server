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
use crate::server::{HandleConnection, Server};


fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);

    let mut port = 7878;
    if args.len() >= 2 {
        port = (&args[1]).parse().unwrap();
    }

    let mut ip = "127.0.0.1";
    if args.len() >= 3 {
        ip = &args[2];
    }

    let ip_addr = ip.to_string();
    let bind_addr = [ip, ":", &port.to_string()].join("");

    println!("Hello, rust-web-server! {}", bind_addr);
    let listener = TcpListener::bind(bind_addr).unwrap();

    let server = Server {
        ip_addr,
        port
    };

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established!");

        server.handle_connection(stream);
    }
}