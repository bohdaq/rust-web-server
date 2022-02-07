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

struct Config<'a> {
    static_dirs: &'a str,
    port: usize,
    ip: &'a str,
    thread_count: usize,
}

fn main() {
    // to run execute following:
    // cargo run 7777 localhost /static,/assets 6
    // where
    // 7777 --> port
    // localhost --> ip
    // /static,/assets --> list of directories in root with static assets
    // 6 --> thread count

    // alternatively you can run built executable 12
    // rws 8888 127.0.0.1 /images,/assets
    // where
    // 8888 --> port
    // 127.0.0.1 --> ip
    // /images,/assets --> list of directories in root with static assets
    // 12 --> thread count

    const CONFIG: Config<'static> = Config {
        static_dirs: "/static,/assets",
        port: 7878,
        ip: "127.0.0.1",
        thread_count: 4
    };

    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);

    if args.len() >= 2 {
        CONFIG.port = (&args[1]).parse().unwrap();
    }

    if args.len() >= 3 {
        CONFIG.ip = &args[2];
    }

    if args.len() >= 4 {
        CONFIG.static_dirs = &args[3];
    }

    if args.len() >= 5 {
        CONFIG.thread_count = (&args[4]).parse().unwrap();
    }

    let bind_addr = [CONFIG.ip, ":", CONFIG.port.to_string().as_str()].join("");
    println!("Hello, rust-web-server! {}", bind_addr);
    let listener = TcpListener::bind(bind_addr).unwrap();
    let pool = ThreadPool::new(CONFIG.thread_count);

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established!");

        pool.execute(move ||  {
            Server::handle_connection(stream, CONFIG.static_dirs);
        });
    }
}