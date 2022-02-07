mod header;
mod request;
mod response;
mod server;
mod test;
mod app;
mod thread_pool;

extern crate core;

use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::{env, fs};

use crate::request::Request;
use crate::response::Response;
use crate::server::Server;
use crate::thread_pool::ThreadPool;


fn main() {
    // to run execute following:
    // cargo run 7777 localhost /static,/assets
    // where
    // 7777 --> port
    // localhost --> ip
    // /static,/assets --> list of directories in root with static assets

    // alternatively you can run built executable
    // rws 8888 127.0.0.1 /images,/assets
    // where
    // 8888 --> port
    // 127.0.0.1 --> ip
    // /images,/assets --> list of directories in root with static assets

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
    let bind_addr = [ip, ":", &port.to_string()].join("");

    let mut static_directories_args = "/static,/assets";
    if args.len() >= 4 {
        //static_directories_args = &args[3];
    }

    let mut threads_count = 4;
    if args.len() >= 5 {
        threads_count = (&args[4]).parse().unwrap();
    }


    println!("Hello, rust-web-server! {}", bind_addr);
    let listener = TcpListener::bind(bind_addr).unwrap();
    let pool = ThreadPool::new(threads_count);

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established!");

        pool.execute(move || {
            Server::handle_connection(stream, static_directories_args);
        });
    }
}