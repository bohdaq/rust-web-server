---
title: systemd
description: Run rws as a hardened systemd service — from the deb/rpm packages or by hand; unprivileged user, read-only system, ports below 1024, reload and log rotation by signal.
---

On a Linux host without containers, run `rws` as a systemd service. The unit
below runs it as an unprivileged user that can bind ports 80 and 443, sees
the system read-only except for its own state and log directories, and
reloads its configuration on `systemctl reload`.

## Install from a package

Since 18.2.0 rws has `.deb` and `.rpm` packages for x86_64 and arm64, from
signed repositories (so `apt upgrade` / `dnf upgrade` bring new releases) or
from each [GitHub release page](https://github.com/bohdaq/rust-web-server/releases).
They install the binary as `/usr/bin/rws`, the unit below, a configuration
in `/etc/rws/rws.config.toml` and a placeholder page in `/var/www/rws`,
create the `rws` user, and start the service on port 7878.

Debian, Ubuntu:

```bash
curl -fsSL https://rws8.tech/apt/rws.gpg | sudo tee /usr/share/keyrings/rws.gpg >/dev/null
echo "deb [signed-by=/usr/share/keyrings/rws.gpg] https://rws8.tech/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/rws.list
sudo apt update && sudo apt install rws
curl localhost:7878/                           # rws is running
```

Fedora, RHEL, Rocky, Alma (packages and repository metadata are signed):

```bash
sudo curl -fsSL -o /etc/yum.repos.d/rws.repo https://rws8.tech/rpm/rws.repo
sudo dnf install rws
```

A downloaded package installs the same way: `sudo apt install ./rws_18.2.0_amd64.deb`,
`sudo dnf install ./rws-18.2.0-1.x86_64.rpm`. The repositories hold the
newest release only; older ones stay on the release pages.

Put your site in `/var/www/rws`, change settings in
`/etc/rws/rws.config.toml` (an upgrade keeps your edits) and apply them with
`sudo systemctl reload rws`. To serve on port 80, set `port = 80` — the unit
already grants the capability.

## Install by hand

```bash
sudo install -m 0755 rws /usr/bin/rws
sudo useradd --system --user-group --no-create-home --home-dir /var/lib/rws --shell /usr/sbin/nologin rws
sudo mkdir -p /etc/rws /var/www/rws
sudo cp rws.config.toml /etc/rws/
sudoedit /etc/systemd/system/rws.service      # the unit below
sudo systemctl daemon-reload
sudo systemctl enable --now rws
```

## The unit

The packages install this file as `/usr/lib/systemd/system/rws.service`
([`packaging/linux/rws.service`](https://github.com/bohdaq/rust-web-server/blob/main/packaging/linux/rws.service)):

```ini
[Unit]
Description=rws web server
Documentation=https://rws8.tech
After=network-online.target
Wants=network-online.target

[Service]
# rws says when it serves (after listeners and certificates), around
# reloads and at shutdown, and pings the watchdog while it runs.
Type=notify
NotifyAccess=main
WatchdogSec=30
User=rws
Group=rws
WorkingDirectory=/var/www/rws
Environment=RWS_CONFIG_FILE=/etc/rws/rws.config.toml
# ACME certificates and account key in /var/lib/rws (StateDirectory below).
Environment=RWS_CONFIG_ACME_CERT_PATH=/var/lib/rws/cert.pem
Environment=RWS_CONFIG_ACME_KEY_PATH=/var/lib/rws/key.pem
Environment=RWS_CONFIG_ACME_ACCOUNT_KEY_PATH=/var/lib/rws/acme_account.key
ExecStart=/usr/bin/rws
# Re-read the config and certificates without dropping connections.
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=2s
LimitNOFILE=65536

# Ports 80/443 without root.
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
NoNewPrivileges=yes

# Writable: /var/lib/rws (state) and /var/log/rws (access log) only.
StateDirectory=rws
LogsDirectory=rws
ProtectSystem=strict
ReadOnlyPaths=/var/www/rws /etc/rws
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
ProtectProc=invisible
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
SystemCallArchitectures=native
SystemCallFilter=@system-service
SystemCallFilter=~@privileged @resources
UMask=0027

[Install]
WantedBy=multi-user.target
```

`systemd-analyze security rws.service` rates it "OK" or better; check it on
your host after installing.

## Access log

Write the access log under `/var/log/rws` (the unit's `LogsDirectory`) in
`/etc/rws/rws.config.toml`:

```toml
access_log = "/var/log/rws/access.log"
```

Rotate it with logrotate, which tells rws to reopen the file with
`systemctl kill -s USR1 rws.service` — see
[Writing to a file](/deployment/observability/#writing-to-a-file) for the
logrotate snippet, or set `access_log_max_size` and let rws rotate it.
Without `access_log`, lines go to stdout and land in the journal
(`journalctl -u rws`).

## Readiness and the watchdog

With `Type=notify` (since 18.20.0) systemd counts rws as started only once
it serves: listeners bound, certificates loaded (ACME included). `systemctl
start` and `restart` wait for that and fail if rws doesn't get there.
`systemctl reload` waits for the reload to finish (`RELOADING=1`, then
`READY=1`), and `systemctl status` shows rws's status line. With
`WatchdogSec=` rws pings systemd from its event loop at half the interval;
a hung process stops pinging and is restarted. An older unit with
`Type=simple` keeps working: rws only notifies when systemd asks for it
(`NOTIFY_SOCKET`).

## Socket activation

The packages also install `rws.socket`. Enabled, systemd opens the ports
and hands them to rws, so a restart refuses nothing (connections wait in
the socket's queue) and rws doesn't need `CAP_NET_BIND_SERVICE`:

```sh
sudo systemctl enable --now rws.socket
sudo systemctl restart rws        # no refused connections
```

```ini
[Socket]
ListenStream=443
FileDescriptorName=https
ListenStream=80
FileDescriptorName=redirect
ListenDatagram=443
FileDescriptorName=quic
Service=rws.service
```

rws takes each listener from systemd when one matches by name (`https` or
`http` for the main port, `redirect`, `quic`, `admin`) or by port, and
binds the rest itself. With per-core runtimes every runtime accepts on its
own copy of the inherited socket. Startup prints which sockets systemd
passed.

## Day to day

| Task | Command |
|---|---|
| Check a config change before applying it | `rws --check /etc/rws/rws.config.toml` |
| Apply it (config, routes, certificates) | `sudo systemctl reload rws` |
| Logs and errors | `journalctl -u rws -f` |
| Reopen the access log | `sudo systemctl kill -s USR1 rws` |
| Restart | `sudo systemctl restart rws` |
