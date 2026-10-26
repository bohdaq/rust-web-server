---
title: Windows
description: Run rws on Windows, in a terminal or as a Windows service started at boot and stopped gracefully by the Service Control Manager.
---

The `rws.exe` in each release's `x86_64-pc-windows-msvc` zip on the
[GitHub release page](https://github.com/bohdaq/rust-web-server/releases)
runs as it does elsewhere: it serves the current directory on port 7878.

```powershell
Expand-Archive rws-18.2.0-x86_64-pc-windows-msvc.zip C:\rws
cd C:\www
C:\rws\rws-18.2.0-x86_64-pc-windows-msvc\rws.exe
```

## As a Windows service

`rws.exe --windows-service=<directory>` runs under the Service Control
Manager (since 18.2.0): it serves `<directory>` and reads
`<directory>\rws.config.toml` if there is one. Stopping the service — or
shutting Windows down — stops rws gracefully, finishing requests in flight.

In an administrator PowerShell:

```powershell
New-Service -Name rws -DisplayName "rws web server" -StartupType Automatic `
  -BinaryPathName '"C:\rws\rws.exe" --windows-service="C:\www"'
Start-Service rws
curl.exe http://localhost:7878/healthz
```

| Task | Command |
|---|---|
| Stop / start | `Stop-Service rws`, `Start-Service rws` |
| Apply a config change | `Restart-Service rws` — there is no SIGHUP on Windows. With `watch = true` rws reloads settings and routes by itself ([Hot reload](/features/hot-reload/)); a new certificate still needs a restart |
| Remove | `Stop-Service rws; sc.exe delete rws` |

The directory is required: a service starts in `C:\Windows\System32`, and
rws serves its working directory. Keep only what should be public in it
(rws never serves `rws.config.toml`, `*.pem` or `*.key`).

A service has no console, so its access log goes nowhere unless you write it
to a file — in `<directory>\rws.config.toml`:

```toml
access_log = 'C:\rws\logs\access.log'
access_log_max_size = 104857600   # rotate at 100 MiB
```

The service runs as LocalSystem by default; to run it as a less privileged
account, set one with `sc.exe config rws obj= "NT AUTHORITY\LocalService"`
and give that account read access to the directory and write access to the
log directory.
