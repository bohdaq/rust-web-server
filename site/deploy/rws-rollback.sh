#!/bin/sh
# Back to Caddy on 80/443 (the state before the swap). Mail configs are
# restored from their backups too.
set -x
systemctl stop rws rws-mail-cert.path
systemctl disable rws rws-mail-cert.path 2>/dev/null
if [ -f /etc/postfix/main.cf.bak-rws ]; then cp /etc/postfix/main.cf.bak-rws /etc/postfix/main.cf; fi
if [ -f /etc/dovecot/conf.d/10-ssl.conf.bak-rws ]; then cp /etc/dovecot/conf.d/10-ssl.conf.bak-rws /etc/dovecot/conf.d/10-ssl.conf; fi
systemctl reload postfix dovecot
systemctl enable --now caddy
systemctl start caddy
sleep 2
ss -tlnp | grep -E ':(80|443) '
