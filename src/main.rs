mod header;
mod request;
mod response;
mod server;
mod test;
mod app;
mod thread_pool;
mod constant;

extern crate core;

use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::{env, fs};
use crate::constant::CONSTANTS;

use crate::request::Request;
use crate::response::Response;
use crate::server::Server;
use crate::thread_pool::ThreadPool;

use clap::{Arg, App};

fn main() {
    let mut ip = "127.0.0.1";
    let mut port = 7878;
    let mut thread_count = 4;

    let matches = App::new("rws rust-web-server")
        .version("0.0.1")
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
        None => println!("Port is not provided from command line. Using default value: {}", port),
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
        None => println!("IP is not provided from command line. . Using default value: {}", ip),
        Some(s) => {
            ip = s;
            println!("IP: {}", s)
        }
    }

    let threads_match = matches.value_of("threads");
    match threads_match {
        None => println!("Thread count is not provided from command line. Using default value: {}", thread_count),
        Some(s) => {
            match s.parse::<i32>() {
                Ok(n) => {
                    thread_count = n;
                    println!("Thread count: {}", n)
                },
                Err(_) => println!("That's not a number! {}", s),
            }
        }
    }


    let bind_addr = [ip, ":", port.to_string().as_str()].join(CONSTANTS.EMPTY_STRING);
    println!("Hello, rust-web-server is up and running: {}", bind_addr);

    let listener = TcpListener::bind(bind_addr).unwrap();
    let pool = ThreadPool::new(thread_count as usize);

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established!");

        pool.execute(move ||  {
            Server::handle_connection(stream);
        });
    }
}