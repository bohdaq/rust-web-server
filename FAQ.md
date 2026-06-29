[Read Me](README.md) > FAQ

# Frequently Asked Questions

## Problem 1
Getting error:
> unable to set up TCP listener: Permission denied (os error 13)

### Solution
Run `rws` with administrator privileges (`sudo` on Linux/macOS, run as Administrator on Windows).

## Problem 2
Getting error:
> unable to set up TCP listener: Address already in use (os error 48)

### Solution
Another process is already using that port. Find and stop it:
```bash
sudo lsof -i :7878     # macOS and Linux
sudo fuser 7878/tcp    # Linux only
```

## Problem 3
Started `rws` on `127.0.0.1` but cannot reach it from other devices on the local network.

### Solution
`127.0.0.1` is the loopback address — it only accepts connections from the same machine. Start `rws` bound to the machine's network IP or `0.0.0.0`:
```bash
rws --ip=0.0.0.0
```

## Problem 4
Getting error in the console:
> unable to parse request: invalid utf-8 sequence of _n_ bytes from index _m_

### Solution
The server received a request that is not valid UTF-8. This can happen when a client sends binary data or when a browser connects via HTTP to an HTTPS-only server. Safe to ignore.

## Problem 5
Getting error in the console:
> unable to parse request: Unable to parse method, request uri and http version

### Solution
The server received a malformed or unexpected request (e.g. a port scanner or protocol mismatch). Safe to ignore.

## Problem 6
Getting error on Linux:
> unable to set up TCP listener: Cannot assign requested address (os error 99)

### Solution
The IP address you specified is not assigned to any local interface. Use `ip addr` to list available addresses.

## Problem 7
How do I start the server on IPv6?

### Solution
```bash
rws --ip=::
```

## Problem 8
Started with `--ip=::` but cannot connect via `fe80::...` link-local address.

### Solution
Use the interface's `inet` (IPv4) address instead. It will be mapped internally to `[::ffff:192.168.x.y]`.

## Problem 9
Trying to open a directory URL gets a _404 Not Found_ error.

### Solution
Directory listing is intentionally not supported (a security decision). Create an `index.html` file in the directory, or link to the files explicitly in an HTML page.

## Problem 10
Cannot connect to server:
> Failed to connect to 192.168.x.y port N: Connection refused

### Solution
A firewall is likely blocking the port. Temporarily disable the firewall and retry.

## Problem 11
Cannot start server as root:
> Command not found

### Solution
`/usr/local/bin` or `~/.cargo/bin` is not in root's `$PATH`. Use the full path:
```bash
sudo /usr/local/bin/rws
# or
sudo ~/.cargo/bin/rws
```

## Problem 12
Build fails with:
> linker 'cc' not found

### Solution
Install the C development toolchain:
```bash
sudo apt-get install build-essential    # Debian/Ubuntu
sudo dnf group install "Development Tools"  # Fedora/RHEL
sudo pacman -S base-devel               # Arch Linux
```

## Problem 13
Build fails in IDE with:
> error[E0514]: found crate `NAME` compiled by an incompatible version of rustc

### Solution
```bash
cargo clean
cargo build
```

## Problem 14
Build fails with registry lock error:
> failed open - 'PATH.git/FETCH_HEAD' is locked: Permission denied

### Solution
```bash
sudo cargo clean
sudo cargo build
```

## Problem 15
Getting error when pulling sources:
> The following untracked working tree files would be overwritten by merge: Cargo.lock

### Solution
```bash
rm Cargo.lock
```

It will be regenerated on the next build.

## Problem 16
Why do some methods start with `_`?

### Solution
Rust warns about unused items. The leading underscore suppresses that warning for intentionally kept-but-unused public API methods.

## Problem 17
How do I view server logs over HTTP?

### Solution
Redirect stdout to a file and serve it:
```bash
rws &> out.txt &
```
Then open `http://hostname:port/out.txt`. The file grows over time; watch its size.

## Problem 18
Started `rws` but HTTPS / HTTP/2 / HTTP/3 is not working.

### Solution
No TLS certificate is configured. Provide certificate and key paths:
```bash
rws --tls-cert-file=/path/to/cert.pem --tls-key-file=/path/to/key.pem
```
Without a certificate the server falls back to plain HTTP/1.1 automatically. See [CONFIGURE](CONFIGURE.md) for all configuration options.

## Problem 19
Getting a TLS error on startup:
> TLS setup failed: failed to read cert file ...

### Solution
The certificate or key file path is wrong or the process does not have read permission. Verify the paths and permissions. Both files must be in PEM format.

## Problem 20
Browser shows a security warning when connecting to the local HTTPS server.

### Solution
Self-signed certificates are not trusted by browsers by default. For local development, add the certificate to your system trust store. For production, use a certificate from [Let's Encrypt](https://letsencrypt.org/).

## Problem 21
Kubernetes liveness or readiness probe is failing with _Connection refused_.

### Solution
The default bind IP is `0.0.0.0` since v17.5.0, so new deployments should work out of the box. If you are running an older version or have overridden the IP to `127.0.0.1`, the kubelet cannot reach the pod IP. Set the bind address explicitly:
```bash
rws --ip=0.0.0.0
```
Or in `rws.config.toml`:
```toml
ip = '0.0.0.0'
```

## Problem 22
Kubernetes readiness probe returns _503 Service Unavailable_ immediately after the pod starts.

### Solution
This is expected — `GET /readyz` returns `503` until the server finishes startup, then switches to `200 OK`. If it stays at `503`, the server failed to start (check logs). Add `initialDelaySeconds` to give the process time to initialize:
```yaml
readinessProbe:
  httpGet:
    path: /readyz
    port: 7878
  initialDelaySeconds: 5
  periodSeconds: 5
```

## Problem 23
Logs are in JSON format but I want the classic Combined Log Format.

### Solution
Set `RWS_CONFIG_LOG_FORMAT=combined` (or `log_format = 'combined'` in `rws.config.toml`). JSON is the default since v17.5.0.

## Problem 24
How do I scrape Prometheus metrics from `rws`?

### Solution
`GET /metrics` returns counters and a gauge in Prometheus text format. Point your scrape config at the server:
```yaml
scrape_configs:
  - job_name: rws
    static_configs:
      - targets: ['hostname:7878']
```
Available metrics: `rws_requests_total`, `rws_errors_total`, `rws_active_connections`.

## Problem 25
The server does not shut down cleanly when Kubernetes sends SIGTERM — in-flight requests are dropped.

### Solution
The `http2` and `http3` builds handle SIGTERM via `tokio::signal` and stop accepting new connections while finishing in-flight requests. The plain `http1` build exits on the next accept-loop tick after the signal. To minimize dropped requests, add a `preStop` hook to delay container termination until the load balancer has drained:
```yaml
lifecycle:
  preStop:
    exec:
      command: ["/bin/sh", "-c", "sleep 5"]
```
