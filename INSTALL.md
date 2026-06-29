[Read Me](README.md) > Install

## Install via cargo

```bash
cargo install rust-web-server
```

This installs the `rws` binary to `~/.cargo/bin/`. Make sure that directory is in your `$PATH` (the Rust installer adds it automatically).

To update to a newer version:
```bash
cargo install rust-web-server --force
```

## Build from source

```bash
git clone https://github.com/bohdaq/rust-web-server.git
cd rust-web-server
cargo build --release
sudo cp target/release/rws /usr/local/bin/   # macOS
sudo cp target/release/rws /usr/bin/         # Linux
```

See [DEVELOPER](DEVELOPER.md) for full build instructions.

## Verify installation

```bash
rws
```

You should see output similar to:

> Rust Web Server
>
> Version:       17.0.0
>
> ...
>
> Server is up and running at: http://127.0.0.1:7878

Open `http://127.0.0.1:7878` in a browser. Press `Ctrl+C` to stop.
