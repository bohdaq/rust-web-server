#!/bin/sh
set -e
if ! getent passwd rws >/dev/null; then
  nologin=/usr/sbin/nologin
  [ -x "$nologin" ] || nologin=/sbin/nologin
  useradd --system --user-group --no-create-home --home-dir /var/lib/rws --shell "$nologin" rws
fi
# Not booted with systemd (a container, a chroot): leave the service alone.
if [ -d /run/systemd/system ]; then
  systemctl daemon-reload
  if [ -e /run/rws-package-install ]; then
    systemctl enable --now rws.service
  else
    # An upgrade: restart if running; a service the admin disabled stays so.
    systemctl try-restart rws.service
  fi
fi
rm -f /run/rws-package-install
