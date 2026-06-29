[Read Me](README.md) > Configuration

# Configuration Info
The rws can be started without any configuration. The following is the default config - the server will bind to IP 127.0.0.1 and port 7878, will spawn 200 threads, CORS requests are allowed.

The rws will try to read configuration from [system environment variables](https://github.com/bohdaq/rust-web-server/blob/main/rws.variables) first, then it will override configuration
by reading it from file named [rws.config.toml](https://github.com/bohdaq/rust-web-server/blob/main/rws.config.toml) placed in the same directory where you execute rws, at last it will
apply config provided via [command-line arguments](https://github.com/bohdaq/rust-web-server/blob/main/rws.command_line).

I personally prefer to use system environment variables, as once it is set correctly, they are hard to break accidentally by overwriting config, or each time providing command line arguments
during restarts.

There may be a use case when you need to run more than one instance, in such a case config file per instance or command line configuration is an option. 

## HTTPS and HTTP/2

The `http2` build of rws has built-in TLS support using [rustls](https://github.com/rustls/rustls) (aws-lc-rs crypto backend — no OpenSSL required). Providing a certificate and key enables HTTPS on the configured port. HTTP/2 is negotiated automatically via ALPN alongside HTTP/1.1 — no separate port or extra config needed.

To obtain a free certificate for a public domain use [Let's Encrypt](https://letsencrypt.org/).

For local development, generate a self-signed certificate:
```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
  -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"
```

### TLS configuration

| Environment variable | Config file key | Command-line arg | Description |
|---|---|---|---|
| `RWS_CONFIG_TLS_CERT_FILE` | `tls_cert_file` | `--tls-cert-file` / `-s` | Path to PEM certificate file |
| `RWS_CONFIG_TLS_KEY_FILE` | `tls_key_file` | `--tls-key-file` / `-k` | Path to PEM private key file |

Example — environment variables:
```bash
export RWS_CONFIG_TLS_CERT_FILE="/path/to/cert.pem"
export RWS_CONFIG_TLS_KEY_FILE="/path/to/key.pem"
```

Example — `rws.config.toml`:
```toml
tls_cert_file = '/path/to/cert.pem'
tls_key_file  = '/path/to/key.pem'
```

Example — command line:
```bash
rws --tls-cert-file=/path/to/cert.pem --tls-key-file=/path/to/key.pem
```

The server must be built with `--features http2` for TLS to take effect.

## Memory
As any other application, rws will allocate memory required to serve the request.
For example if the client will make an HTTP GET for resource which has size more
than free available memory on the running instance, rws will throw Out Of Memory error.

In such case valid options are:
1. Use range requests on the client for big resources to get a portion at a time.
2. Balance the overall load on instance in case you have heavy load by spinning up
   more rws instances and share traffic between them.
