# Welcome to rust-web-server!

Hi, rust-web-server (**rws**) is a simple web-server written in Rust. The **rws** server can serve static content inside the directory it is started.

## Features
1. [Cross-Origin Resource Sharing (CORS)](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS)
2. [HTTP range requests](https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests)

## Download
Currently, you can download binary for [x86_64-unknown-linux-gnu](https://cv.bohdaq.name/rust-web-server/0.0.6/x86_64-unknown-linux-gnu/rws) or [x86_64-apple-darwin](https://cv.bohdaq.name/rust-web-server/0.0.6/x86_64-apple-darwin/rws) platforms. Also, you can clone the repository and build **rws** binary for [other platforms](https://doc.rust-lang.org/nightly/rustc/platform-support.html).

## Installation
Simply add downloaded **rws** binary to [$PATH](https://en.wikipedia.org/wiki/PATH_%28variable%29). To check installation execute the following code:

> $ rws --help
 
You will see similar output:

> rws rust-web-server 0.0.6
> 
> Bohdan Tsap <bohdan.tsap@tutanota.com>
> 
> Hi, rust-web-server (rws) is a simple web-server written in Rust. The rws server can serve static
> content inside the directory it is started.
>
> USAGE:
> 
> rws [OPTIONS]
> 
>
> OPTIONS:
> 
> -h, --help                 Print help information
> 
> -i, --ip <ip>              IP or domain
> 
> -p, --port <port>          Port
> 
> -t, --threads <threads>    Number of threads
> 
> -V, --version              Print version information

## Run
Simply run the following from command line:

> $ rws --ip=127.0.0.1 --port=8888 --threads=5

Make sure in root folder you provided index.html and 404.html files.

## Configuration

The rws will try to read configuration from [system environment variables](https://github.com/bohdaq/rust-web-server/blob/main/rws.variables) first, then it will override configuration by reading it from file named [rws.config.toml](https://github.com/bohdaq/rust-web-server/blob/main/rws.config.toml) placed in the same directory where you execute rws, at last it will apply config provided via [command-line arguments](https://github.com/bohdaq/rust-web-server/blob/main/rws.command_line).


## Build

If you want to build rust-web-server on your own, make sure you have [Rust installed](https://www.rust-lang.org/tools/install).

> $ cargo build --release
> 
> $ cd target/release
> 
> $ ./rws --ip=127.0.0.1 --port=8888 --threads=5

>You will see similar output:
>
>Port: 8888
>
>IP: 127.0.0.1
>
>Thread count: 5
>
>Hello, rust-web-server is up and running: 127.0.0.1:8888


## Community
Rust Web Server has a [Discord](https://discord.gg/zaErjtr5Dm) where you can ask questions and share ideas.


