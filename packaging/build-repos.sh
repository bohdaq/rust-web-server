#!/usr/bin/env bash
# Signed apt and dnf repositories from a directory of rws .deb/.rpm files:
#   packaging/build-repos.sh <packages dir> <out dir> <base URL>
# writes <out>/apt (suite "stable", component "main") and <out>/rpm, each
# with the public key (apt/rws.gpg for signed-by=, rpm/rws.asc) and
# rpm/rws.repo pointing at <base URL>/rpm. Needs apt-ftparchive (apt-utils),
# createrepo_c and gpg holding the secret key GPG_KEY_ID (passphrase in
# GPG_PASSPHRASE, if any). The rpm files themselves are signed by
# build-packages.sh with the same key.
set -euo pipefail
packages=$1 out=$2 url=${3%/}

sign() {
  local args=(--batch --yes --pinentry-mode loopback --local-user "$GPG_KEY_ID" --digest-algo SHA256)
  if [ -n "${GPG_PASSPHRASE:-}" ]; then args+=(--passphrase "$GPG_PASSPHRASE"); fi
  gpg "${args[@]}" "$@"
}

# apt
apt=$out/apt
mkdir -p "$apt/pool/main"
cp "$packages"/*.deb "$apt/pool/main/"
(
  cd "$apt"
  for arch in amd64 arm64; do
    mkdir -p "dists/stable/main/binary-$arch"
    apt-ftparchive --arch "$arch" packages pool > "dists/stable/main/binary-$arch/Packages"
    gzip -9kfn "dists/stable/main/binary-$arch/Packages"
  done
  apt-ftparchive \
    -o APT::FTPArchive::Release::Origin=rws -o APT::FTPArchive::Release::Label=rws \
    -o APT::FTPArchive::Release::Suite=stable -o APT::FTPArchive::Release::Codename=stable \
    -o APT::FTPArchive::Release::Architectures="amd64 arm64" -o APT::FTPArchive::Release::Components=main \
    release dists/stable > Release.tmp
  mv Release.tmp dists/stable/Release
)
sign --clearsign -o "$apt/dists/stable/InRelease" "$apt/dists/stable/Release"
sign --detach-sign --armor -o "$apt/dists/stable/Release.gpg" "$apt/dists/stable/Release"
gpg --batch --yes --export "$GPG_KEY_ID" > "$apt/rws.gpg"
gpg --batch --yes --armor --export "$GPG_KEY_ID" > "$apt/rws.asc"

# dnf / yum
rpm=$out/rpm
mkdir -p "$rpm/packages"
cp "$packages"/*.rpm "$rpm/packages/"
createrepo_c --quiet "$rpm"
sign --detach-sign --armor -o "$rpm/repodata/repomd.xml.asc" "$rpm/repodata/repomd.xml"
gpg --batch --yes --armor --export "$GPG_KEY_ID" > "$rpm/rws.asc"
cat > "$rpm/rws.repo" <<REPO
[rws]
name=rws
baseurl=$url/rpm
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=$url/rpm/rws.asc
REPO
