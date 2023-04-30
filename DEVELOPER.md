[Read Me](README.md) > Developer

# Developer Info

Note: there is a [quick setup tutorial](DEVELOPER/UBUNTU_LTS_DEVELOPER_SETUP.md) for Ubuntu 22.04 LTS

Make sure you have [Rust installed](https://www.rust-lang.org/tools/install).

Main branch shall work fine for most of the time, but from time to time it may have some issues, as a result of ongoing development. Usually they are resolved as soon as possible. So you may think to base your changes on latest [tag](https://github.com/bohdaq/rust-web-server/tags) instead of `main` branch.

Minimum rust version is 1.66, as I'm testing on this specific version. However, if needed you may try to build rws on your own using older version with the _--ignore-rust-version_ flag.

Depending on your setup you may need to run commands listed below as an administrator (open CMD as an administrator on Windows or use `sudo` on Linux and macOS).

I personally use [IDEA Community Edition](https://www.jetbrains.com/idea/download/) with [Rust plugin](https://www.jetbrains.com/rust/), it is free and works quite well with code inspections.

On macOS to get IDE work properly run it as an administrator:

> sudo /Applications/IntelliJ\ IDEA\ CE.app/Contents/MacOS/idea

or if you're using CLion

> sudo /Applications/CLion.app/Contents/MacOS/clion

UPDATE 30.04.2023: You may encounter problems on `macOS` even when you're running IDE as root. To fix it make sure you have clonned the gihub repo as root as well. To clone repository as `root` on `macOS`, add user public and private keys to `/var/root/.ssh` folder.

> sudo mkdir -p /var/root/.ssh
> 
> sudo cp ~/.ssh/id_rsa /var/root/.ssh/
> 
> sudo cp ~/.ssh/id_rsa.pub /var/root/.ssh/


and then clone the repo:

> mkdir -p ~/git

> cd ~/git

> sudo git clone git@github.com:bohdaq/rust-web-server.git

UPDATE 28.01.2023: I've got an [open source license](https://www.jetbrains.com/community/opensource/) from JetBrains on all products. They also have [academic licenses](https://www.jetbrains.com/community/education/) available.

However, I **run and test from terminal**. 

Before proceeding, it is a good idea to update rust toolchain:
> rustup update

## Run
> cargo run --ignore-rust-version

## Test
> cargo test --ignore-rust-version

To run specific test (replace client_hint::tests::client_hints_header with test you want to run)

> cargo test --package rws --bin rws client_hint::tests::client_hints_header -- --exact --ignore-rust-version

## Debug

[IDEA Community Edition](https://plugins.jetbrains.com/plugin/8182-rust/docs/rust-debugging.html) does not have support for debugging, even though it is [stated otherwise](https://www.jetbrains.com/idea/download/) on their website.

While running a test you may notice the fact that stdout does not show the `println!`. To workaround this problem I usually create a file named as the test I'm running and instead of using `println!` macros simply writing the output to the file.

It's not a fancy debugger, but you may print to a file all info debugger shows - variable, it's value and any additional information.

Tests may be executed in parallel so use unique file name for each test to eliminate concurrency issues.

Debugger support is present in [CLion](https://www.jetbrains.com/clion/) and works quite well.


## Build
> cargo build --ignore-rust-version

## Release
Open [RELEASE](RELEASE.md) for details.