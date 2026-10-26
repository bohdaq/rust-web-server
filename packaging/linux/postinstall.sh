#!/bin/sh
# deb: "configure [<old-version>]"; rpm: 1 on install, 2 on upgrade.
set -e
if ! getent passwd rws >/dev/null; then
  nologin=/usr/sbin/nologin
  [ -x "$nologin" ] || nologin=/sbin/nologin
  useradd --system --user-group --no-create-home --home-dir /var/lib/rws --shell "$nologin" rws
fi
# Not booted with systemd (a container, a chroot): leave the service alone.
if [ -d /run/systemd/system ]; then
  systemctl daemon-reload
  if { [ "$1" = configure ] && [ -n "$2" ]; } || [ "$1" = 2 ]; then
    systemctl try-restart rws.service
  else
    systemctl enable --now rws.service
  fi
fi
