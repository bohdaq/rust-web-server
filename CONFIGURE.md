[Read Me](README.md) > Configuration

# Configuration Info
The rws can be started without any configuration. The following is the default config - the server will bind to IP 127.0.0.1 and port 7878, will spawn 200 threads, CORS requests are allowed.

The rws will try to read configuration from [system environment variables](https://github.com/bohdaq/rust-web-server/blob/main/rws.variables) first, then it will override configuration
by reading it from file named [rws.config.toml](https://github.com/bohdaq/rust-web-server/blob/main/rws.config.toml) placed in the same directory where you execute rws, at last it will
apply config provided via [command-line arguments](https://github.com/bohdaq/rust-web-server/blob/main/rws.command_line).

I personally prefer to use system environment variables, as once it is set correctly, they are hard to break accidentally by overwriting config, or each time providing command line arguments
during restarts.

There may be a use case when you need to run more than one instance, in such a case config file per instance or command line configuration is an option. 

## Encryption

The rws is an [HTTP server](https://developer.mozilla.org/en-US/docs/Web/HTTP). This means if you are planning to use it somewhere else except the local machine you need to protect transferred data by using encryption.

There is a [Rust TLS Server](https://github.com/bohdaq/rust-tls-server) for handling HTTPS over TLS.

Alternative solutions to Rust TLS Server are:
1. You need your webapp to be globally available via the internet. In such a case, the simplest solution is to use a reverse proxy and certificate provided by [Let's Encrypt](https://letsencrypt.org/). A proxy will redirect all HTTP traffic to HTTPS, decrypt it via certificate and forward the request to rws. Response from rws will be forwarded to a proxy, encrypted, and sent to a client. As [reverse proxy](https://ssl-config.mozilla.org/) you may use Apache HTTP Server, lighttpd, etc.
2. You don't need your webapp to be globally available. In such case the solution may be to setup VPN.

## Memory
As any other application, rws will allocate memory required to serve the request.
For example if the client will make an HTTP GET for resource which has size more
than free available memory on the running instance, rws will throw Out Of Memory error.

In such case valid options are:
1. Use range requests on the client for big resources to get a portion at a time.
2. Balance the overall load on instance in case you have heavy load by spinning up
   more rws instances and share traffic between them.