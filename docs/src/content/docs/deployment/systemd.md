---
title: systemd
description: Run rws as a hardened systemd service — unprivileged user, read-only system, ports below 1024, access log in /var/log, reload and log rotation by signal.
---

On a Linux host without containers, run `rws` as a systemd service. The unit
below runs it as an unprivileged user that can bind ports 80 and 443, sees
the system read-only except for its own state and log directories, and
reloads its configuration on `systemctl reload`.

## Install

```bash
sudo install -m 0755 rws /usr/local/bin/rws
sudo useradd --system --no-create-home --shell /usr/sbin/nologin rws
sudo mkdir -p /etc/rws /srv/www
sudo cp rws.config.toml /etc/rws/
sudoedit /etc/systemd/system/rws.service      # the unit below
sudo systemctl daemon-reload
sudo systemctl enable --now rws
```

## The unit

```ini
[Unit]
Description=rws web server
Documentation=https://rws8.tech
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=rws
Group=rws
WorkingDirectory=/srv/www
Environment=RWS_CONFIG_FILE=/etc/rws/rws.config.toml
# ACME certificates and account key in /var/lib/rws (StateDirectory below).
Environment=RWS_CONFIG_ACME_CERT_PATH=/var/lib/rws/cert.pem
Environment=RWS_CONFIG_ACME_KEY_PATH=/var/lib/rws/key.pem
Environment=RWS_CONFIG_ACME_ACCOUNT_KEY_PATH=/var/lib/rws/acme_account.key
ExecStart=/usr/local/bin/rws
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
ReadOnlyPaths=/srv/www /etc/rws
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

## Day to day

| Task | Command |
|---|---|
| Check a config change before applying it | `rws --check /etc/rws/rws.config.toml` |
| Apply it (config, routes, certificates) | `sudo systemctl reload rws` |
| Logs and errors | `journalctl -u rws -f` |
| Reopen the access log | `sudo systemctl kill -s USR1 rws` |
| Restart | `sudo systemctl restart rws` |
