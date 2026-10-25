# Configuration

Configuration is documented on the docs site:

- [Configuration overview](https://rws8.tech/configuration/overview/) — layering (defaults → environment → `rws.config.toml` → command line), hot reload, memory
- [Environment variables](https://rws8.tech/configuration/env-vars/) — every `RWS_CONFIG_*` setting with its default
- [Config file](https://rws8.tech/configuration/config-file/) — `rws.config.toml`, including proxy routes and upstreams
- [Command-line arguments](https://rws8.tech/configuration/cli-args/) — flags, `--check`, `--version`
- [HTTPS / TLS](https://rws8.tech/features/https-tls/), [virtual hosting](https://rws8.tech/features/virtual-hosting/), [ACME](https://rws8.tech/features/acme/)

The same pages are in this repository under
[`docs/src/content/docs/configuration/`](docs/src/content/docs/configuration/).
`rws` runs with no configuration at all: `0.0.0.0:7878`, CORS open to all origins.
