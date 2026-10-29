# rws8.tech host files

What runs on the VM that serves rws8.tech (spec/WEBSITE_REBUILD.md, phase 6),
kept here so the setup can be rebuilt and reviewed.

| File | On the VM |
|---|---|
| `rws.config.toml` | `/etc/rws/rws.config.toml` |
| `rws.service` | `/etc/systemd/system/rws.service` |
| `rws-mail-cert.path`, `rws-mail-cert.service` | `/etc/systemd/system/`: reload postfix and dovecot when rws writes a new certificate |
| `rws-rollback.sh` | `/usr/local/sbin/rws-rollback`: stop rws, put Caddy back on 80/443 |

The site itself is `site/dist` (without `.vite/`) in `/var/www/rws8`, plus
`/apt`, `/rpm` and the per-release `llms*.txt` directories the release
workflow writes. Check a config change with
`rws --check=/etc/rws/rws.config.toml` before `systemctl restart rws`.
