# Welcome to rust-web-server!

Hi, rust-web-server (**rws**) is a simple web-server written in Rust. The **rws** server can serve static content inside the directory it is started.

## Download
Currently, you can download binary for x86_64-unknown-linux-gnu or x86_64-apple-darwin platforms. Also, you can clone the repository and build **rws** binary for [other platforms](https://doc.rust-lang.org/nightly/rustc/platform-support.html).

## Run
Once **rws** binary is added to [$PATH](https://en.wikipedia.org/wiki/PATH_%28variable%29) you can simply run the following from command line
> $ ./rws

or

> $ ./rws 7777 localhost  6

where *7777* is *port*, *localhost* is *domain* or *ip* and *6* is the *number of threads*

## Build

If you want to build rust-web-server on your own, make sure you have [Rust installed](https://www.rust-lang.org/tools/install).

> $ cargo build --release
> $ cd target/debug
> $ ./rws

You will see similar output:

>["rws"]
>Hello, rust-web-server!
>address: 127.0.0.1:7878, thread count: 4


