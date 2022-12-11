# rust-web-server

rust-web-server (**rws**) is a static content web-server written in Rust.


## Features
1. [Cross-Origin Resource Sharing (CORS)](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS)
1. [HTTP Range Requests](https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests)
1. [HTTP Client Hints](https://developer.mozilla.org/en-US/docs/Web/HTTP/Client_hints)
1. [X-Content-Type-Options](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Content-Type-Options)
1. [X-Frame-Options](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Frame-Options)
1. No third party dependencies

## Development
Open [DEVELOPER](INSTALL.md) for details.

## Installation
Open [INSTALL](INSTALL.md) for details.

## Configuration

The rws can be started without any configuration. The following is the default config - the server will bind to IP 127.0.0.1 and port 7887, will spawn 200 threads, CORS requests are allowed.

The rws will try to read configuration from [system environment variables](https://github.com/bohdaq/rust-web-server/blob/main/rws.variables) first, then it will override configuration 
by reading it from file named [rws.config.toml](https://github.com/bohdaq/rust-web-server/blob/main/rws.config.toml) placed in the same directory where you execute rws, at last it will 
apply config provided via [command-line arguments](https://github.com/bohdaq/rust-web-server/blob/main/rws.command_line). 

I personally prefer to use system environment variables, as once it is set correctly, they are hard to break accidentally by overwriting config, or each time providing command line arguments 
during restarts.

There may be a use case when you need to run more than one instance, in such a case config file per instance or command line configuration is an option. 



## Community
Rust Web Server has a [Discord](https://discord.gg/zaErjtr5Dm) where you can ask questions and share ideas. Follow the [Rust code of conduct](https://www.rust-lang.org/policies/code-of-conduct).

## Encryption

The rws is an [HTTP server](https://developer.mozilla.org/en-US/docs/Web/HTTP). This means if you are planning to use it somewhere else except the local machine you need to protect transferred data by using encryption.

There is a [Rust TLS Server](https://github.com/bohdaq/rust-tls-server) for handling HTTPS over TLS.

Alternative solutions to Rust TLS Server are:
1. You need your webapp to be globally available via the internet. In such a case, the simplest solution is to use a reverse proxy and certificate provided by [Let's Encrypt](https://letsencrypt.org/). A proxy will redirect all HTTP traffic to HTTPS, decrypt it via certificate and forward the request to rws. Response from rws will be forwarded to a proxy, encrypted, and send to a client. As [reverse proxy](https://ssl-config.mozilla.org/) you may use Apache HTTP Server, lighttpd, etc.
2. You don't need your webapp to be globally available. In such case the solution may be to setup VPN.

## Memory
As any other application, rws will allocate memory required to serve the request. 
For example if the client will make an HTTP GET for resource which has size more 
than free available memory on the running instance, rws will throw Out Of Memory error.

In such case valid options are:
1. Use range requests on the client for big resources to get a portion at a time.
2. Balance the overall load on instance in case you have heavy load by spinning up 
more rws instances and share traffic between them.

## Donations
If you appreciate my work and want to support it, feel free to do it via [PayPal](https://www.paypal.com/donate/?hosted_button_id=7J69SYZWSP6HJ).

