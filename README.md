# Welcome to rust-web-server!

Hi, rust-web-server (**rws**) is a simple web-server written in Rust. The **rws** server can serve static content inside the directory it is started.

## Features
1. [Cross-Origin Resource Sharing (CORS)](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS)
2. [HTTP Range Requests](https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests)
2. [HTTP Client Hints](https://developer.mozilla.org/en-US/docs/Web/HTTP/Client_hints)
3. [X-Content-Type-Options](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Content-Type-Options)
4. [X-Frame-Options](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Frame-Options)

## Download
Currently, you can [download binary](https://github.com/bohdaq/rust-web-server/releases) for x86_64-unknown-linux-gnu or x86_64-apple-darwin platforms. Also, you can clone the repository and build **rws** binary for [other platforms](https://doc.rust-lang.org/nightly/rustc/platform-support.html).

## Installation
Simply add downloaded **rws** binary to [$PATH](https://en.wikipedia.org/wiki/PATH_%28variable%29). To check installation execute the following code:

> $ rws --help
 
You will see similar output:

> rws rust-web-server 0.0.23
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
> -V, --version              Print version information

## Run
Simply run the following from command line:

> $ rws --ip=127.0.0.1 --port=8888 --threads=100

Make sure in root folder you provided index.html and 404.html files.

## Configuration

The rws will try to read configuration from [system environment variables](https://github.com/bohdaq/rust-web-server/blob/main/rws.variables) first, then it will override configuration by reading it from file named [rws.config.toml](https://github.com/bohdaq/rust-web-server/blob/main/rws.config.toml) placed in the same directory where you execute rws, at last it will apply config provided via [command-line arguments](https://github.com/bohdaq/rust-web-server/blob/main/rws.command_line).


## Build

If you want to build rust-web-server on your own, make sure you have [Rust installed](https://www.rust-lang.org/tools/install).

> $ cargo build --release
> 
> $ cd target/release
> 
> $ ./rws --ip=127.0.0.1 --port=8888 --threads=100

>You will see similar output:
>
>Port: 8888
>
>IP: 127.0.0.1
>
>Hello, rust-web-server is up and running: 127.0.0.1:8888


## Community
Rust Web Server has a [Discord](https://discord.gg/zaErjtr5Dm) where you can ask questions and share ideas. Follow the [Rust code of conduct](https://www.rust-lang.org/policies/code-of-conduct).

## Encryption

The rws is an [HTTP server](https://developer.mozilla.org/en-US/docs/Web/HTTP). This means if you are planning to use it somewhere else except the local machine you need to protect transferred data by using encryption.

The most common use cases are:
1. You need your webapp to be globally available via the internet. In such a case, the simplest solution is to use a reverse proxy and certificate provided by [Let's Encrypt](https://letsencrypt.org/). A proxy will redirect all HTTP traffic to HTTPS, decrypt it via certificate and forward the request to rws. Response from rws will be forwarded to a proxy, encrypted, and send to a client. As [reverse proxy](https://ssl-config.mozilla.org/) you may use Apache HTTP Server, lighttpd, etc.
2. You don't need your webapp to be globally available. In such case the solution may be to setup VPN.


## Donations
If you appreciate my work and want to support it, feel free to do it via [PayPal](https://www.paypal.com/donate/?hosted_button_id=7J69SYZWSP6HJ).

