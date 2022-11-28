# rust-web-server

rust-web-server (**rws**) is a static content web-server written in Rust. 


## Features
1. [Cross-Origin Resource Sharing (CORS)](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS)
1. [HTTP Range Requests](https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests)
1. [HTTP Client Hints](https://developer.mozilla.org/en-US/docs/Web/HTTP/Client_hints)
1. [X-Content-Type-Options](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Content-Type-Options)
1. [X-Frame-Options](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Frame-Options)
1. No third party dependencies

## Download
Currently, you can [download binary](https://github.com/bohdaq/rust-web-server/releases) from releases page. Also, you can clone the repository and build **rws** binary for [other platforms](https://doc.rust-lang.org/nightly/rustc/platform-support.html). There is a mirror for downloads on [Google Drive](https://drive.google.com/drive/folders/13iSR3VxmfFvZgOZ0LddP_EJp7GJ-lQd8?usp=sharing).

## Installation
rws is a binary so there are a couple of ways to install it

### Platform Independent
Simply add downloaded **rws** binary to [$PATH](https://en.wikipedia.org/wiki/PATH_%28variable%29). As an example copy it to bin folder and make it executable:
> $ sudo cp rws /usr/local/bin
> 
> $ sudo chmod +x /usr/local/bin/rws
 
### Homebrew on macOS

> $ brew tap bohdaq/rust-web-server
> 
> $ brew install rws

### Linux

#### Debian based distros

- [.deb package for x86_64](https://github.com/bohdaq/rws-deb-package/raw/x86_64/rws.deb) (Ubuntu, Linux Mint, etc.)
- [.deb package for arm_64](https://github.com/bohdaq/rws-deb-package/raw/arm_64/rws.deb) (Raspberry Pi, Ubuntu ARM, etc.)

and execute
> $ sudo dpkg -i rws.deb
#### RPM based distros (CentOS, openSUSE, Oracle Linux, etc)
- [.rpm package for x86_64](https://github.com/bohdaq/rws-rpm-package/raw/x86_64/rws-0.0.28-0.x86_64.rpm)

and execute
> $ sudo rpm -i rws-0.0.28-0.x86_64.rpm

 

### Testing installation
To check installation execute the following code:

> $ rws

You will see similar output:

> Rust Web Server
> 
> Version:       0.0.28
> 
> Authors:       Bohdan Tsap <bohdan.tsap@tutanota.com>
> 
> Repository:    https://github.com/bohdaq/rust-web-server
> 
> Desciption:    rust-web-server (rws) is a simple web-server written in Rust. The rws http server can serve static content inside the directory it is started.
> 
> Rust Version:  1.64


## Run
Simply run the following from command line:

> $ rws --ip=127.0.0.1 --port=8888 --threads=100

Make sure in the folder where you execute rws there are index.html and 404.html files.

## Configuration

The rws can be started without any configuration. The following is the default config - the server will bind to IP 127.0.0.1 and port 7887, will spawn 200 threads, CORS requests are allowed.

The rws will try to read configuration from [system environment variables](https://github.com/bohdaq/rust-web-server/blob/main/rws.variables) first, then it will override configuration by reading it from file named [rws.config.toml](https://github.com/bohdaq/rust-web-server/blob/main/rws.config.toml) placed in the same directory where you execute rws, at last it will apply config provided via [command-line arguments](https://github.com/bohdaq/rust-web-server/blob/main/rws.command_line). 

I personally prefer to use system environment variables, as once it is set correctly, they are hard to break accidentally by overwriting config, or each time providing command line arguments during restarts.

There may be a use case when you need to run more than one instance, in such a case config file per instance or command line configuration is an option. 


## Build

If you want to build rust-web-server on your own, make sure you have [Rust installed](https://www.rust-lang.org/tools/install).

Minimum rust version is 1.64, as I'm testing on this specific version. However, if needed you may try to build rws on your own using older version with the _--ignore-rust-version_ flag.

> $ cargo build --release
> 
> $ cd target/release
> 
> $ ./rws --ip=127.0.0.1 --port=8888 --threads=100



## Community
Rust Web Server has a [Discord](https://discord.gg/zaErjtr5Dm) where you can ask questions and share ideas. Follow the [Rust code of conduct](https://www.rust-lang.org/policies/code-of-conduct).

## Encryption

The rws is an [HTTP server](https://developer.mozilla.org/en-US/docs/Web/HTTP). This means if you are planning to use it somewhere else except the local machine you need to protect transferred data by using encryption.

The most common use cases are:
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

