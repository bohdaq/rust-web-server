#!/bin/sh
# deb: "install [<old-version>]" when rws isn't installed (perhaps removed
# before, its config kept) or "upgrade"; rpm: 1 on install, 2 on upgrade.
# Only here can a reinstall be told from an upgrade: postinstall sees an old
# version for both on deb.
set -e
if [ "$1" = install ] || [ "$1" = 1 ]; then
  if [ -d /run ]; then : > /run/rws-package-install; fi
fi
