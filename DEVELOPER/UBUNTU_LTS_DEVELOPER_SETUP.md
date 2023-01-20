## Developer environment setup on Ubuntu 22.04.1 LTS

Assumption is you have fresh installation of Ubuntu 22.04.1 LTS.

### 1. Install Rust
> sudo apt install curl

> curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Press Enter to proceed with default installation

> source ~/.cargo/env

> rustc -V

### 2. Setup git

> cd ~

> mkdir git

> cd git

> sudo apt install git

### 3. Clone repository

> git clone https://github.com/bohdaq/rust-web-server.git

> cd rust-web-server

### 4. Install required build tools

> sudo apt install build-essential

### 5. Run tests

> cargo test

If you see failed test, rerun previous command

### 6. Start server

> cargo run

At this point, server is started on loopback device (ip 127.0.0.1) 
and is not accessible from the network.

Try to open url in the browser
Press Ctrl + C (CMD + C) to stop the server

### 7. Allow requests from network

> sudo ufw disable

This will disable firewall and enable requests to the server from your network

### 8. Start server on network connected device

> sudo apt install net-tools

> ifconfig

Find your ip and restart the server

> cargo run -- --ip=IP_FROM_IFCONFIG

Check again url in the browser
