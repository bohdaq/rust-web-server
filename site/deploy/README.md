# rws8.tech host files

What runs on the VM that serves rws8.tech (spec/WEBSITE_REBUILD.md, phase 6),
kept here so the setup can be rebuilt and reviewed.

| File | On the VM |
|---|---|
| `rws.config.toml` | `/etc/rws/rws.config.toml` |
| `rws.service` | `/etc/systemd/system/rws.service` |
| `rws-mail-cert.path`, `rws-mail-cert.service` | `/etc/systemd/system/`: reload postfix and dovecot when rws writes a new certificate |
| `rws-rollback.sh` | `/usr/local/sbin/rws-rollback`: stop rws, put Caddy back on 80/443 |
| `rws-site-update.sh` | `/usr/local/sbin/rws-site-update`: install `rws8-site-<tag>.tar.gz` from the newest release |
| `rws-site-update.service`, `.timer` | `/etc/systemd/system/`: run the update hourly |

The site lives in `/var/www/rws8`. Each release attaches
`rws8-site-<tag>.tar.gz` (the `site/` build, with the signed `/apt` and `/rpm`
when the release has the GPG key); the timer installs the newest one with
rsync, keeping older `/<major.minor>/` folders and, when the archive has no
repositories, the current `/apt` and `/rpm`. The installed tag is in
`/var/www/rws8/.release`; `sudo systemctl start rws-site-update` installs
without waiting. Check a config change with
`rws --check=/etc/rws/rws.config.toml` before `systemctl restart rws`.

`rws-rollback.sh` also puts postfix and dovecot back on the old certbot
certificate, which expired on 2026-01-20: after a rollback, mail TLS serves
an expired certificate again until certbot can renew (it needs port 80).
