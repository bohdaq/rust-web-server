#!/usr/bin/env bash
# deb and rpm of one rws binary, with nfpm (packaging/linux/nfpm.yaml):
#   packaging/build-packages.sh <version> <amd64|arm64> <binary> <out dir>
# With RPM_SIGNING_KEY_FILE (an armored private key; passphrase in
# NFPM_RPM_PASSPHRASE, if any) the rpm is signed. Run from the repo root.
set -euo pipefail
version=$1 arch=$2 binary=$3 out=$4

mkdir -p dist "$out"
cp "$binary" dist/rws
config=packaging/linux/nfpm.yaml
if [ -n "${RPM_SIGNING_KEY_FILE:-}" ]; then
  config=dist/nfpm-signed.yaml
  cat packaging/linux/nfpm.yaml - > "$config" <<YAML
rpm:
  signature:
    key_file: $RPM_SIGNING_KEY_FILE
YAML
fi
VERSION=$version ARCH=$arch nfpm package -f "$config" -p deb -t "$out/"
VERSION=$version ARCH=$arch nfpm package -f "$config" -p rpm -t "$out/"
