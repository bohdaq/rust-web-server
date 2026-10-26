#!/bin/sh
# deb: "remove" or "upgrade"; rpm: 0 on erase, 1 on upgrade.
set -e
if { [ "$1" = remove ] || [ "$1" = 0 ]; } && [ -d /run/systemd/system ]; then
  systemctl disable --now rws.service || true
fi
