# Developer Info
Make sure you have [Rust installed](https://www.rust-lang.org/tools/install).

Minimum rust version is 1.65, as I'm testing on this specific version. However, if needed you may try to build rws on your own using older version with the _--ignore-rust-version_ flag.

## Run
> cargo run

## Test
> cargo test

## Build

> cargo build --release
>
> cd target/release
>
> ./rws --ip=127.0.0.1 --port=8888 --threads=100


# Release
Here is the list of supported architectures:
1. x86_64-apple-darwin
2. x86_64-unknown-linux-gnu
3. aarch64_unknown_linux_gnu
4. x86_64-pc-windows-msvc

Build binary on specific platform to prepare release. 

## Build Templates
There are templates for 
[Homebrew](https://brew.sh/), 
[Debian](https://www.debian.org/) and 
[RPM](https://rpm.org/).
1. [Homebrew](https://github.com/bohdaq/homebrew-rust-web-server)
2. rws-x86_64-create-deb TODO
3. rws-arm-create-deb TODO
4. rws-x86_64-rpm-builder TODO
