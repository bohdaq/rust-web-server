#!/bin/sh
# Install rws8.tech from the newest rust-web-server release: its
# rws8-site-<tag>.tar.gz, checked against the .sha256 next to it, synced into
# /var/www/rws8. Run hourly by rws-site-update.timer; does nothing when the
# installed tag (/var/www/rws8/.release) is current or the release has no
# site archive. The archive carries /apt and /rpm only when the release
# signed them; otherwise the ones already here are kept.
set -eu
repo=bohdaq/rust-web-server
root=/var/www/rws8
api="https://api.github.com/repos/$repo/releases/latest"

tag=$(curl -fsSL "$api" | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"])')
case "$tag" in *[!0-9.]*|"") echo "unexpected tag '$tag'" >&2; exit 1;; esac
[ "$(cat "$root/.release" 2>/dev/null)" = "$tag" ] && exit 0

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
base="https://github.com/$repo/releases/download/$tag"
if ! curl -fsSL -o "$work/site.tar.gz" "$base/rws8-site-$tag.tar.gz"; then
  echo "$tag has no site archive; keeping $(cat "$root/.release" 2>/dev/null || echo the current site)"
  exit 0
fi
curl -fsSL -o "$work/site.tar.gz.sha256" "$base/rws8-site-$tag.tar.gz.sha256"
expected=$(cut -d' ' -f1 "$work/site.tar.gz.sha256")
actual=$(sha256sum "$work/site.tar.gz" | cut -d' ' -f1)
[ "$expected" = "$actual" ] || { echo "checksum mismatch for $tag" >&2; exit 1; }

mkdir "$work/site"
tar -xzf "$work/site.tar.gz" -C "$work/site"
[ -f "$work/site/index.html" ] && [ -d "$work/site/docs" ] || { echo "$tag archive has no site" >&2; exit 1; }
# Old /<major.minor>/ llms folders stay; so do the repositories when this
# archive brings none.
set -- "--filter=P_/[0-9]*.[0-9]*/"
[ -d "$work/site/apt" ] || set -- "$@" "--filter=P_/apt/" "--filter=P_/rpm/"
rsync -a --delete-after "$@" "$work/site/" "$root/"
chown -R root:root "$root"
chmod -R a+rX "$root"
echo "$tag" > "$root/.release"
echo "installed rws8.tech from $tag"

# The docs MCP server: its binary from the same release when it has one,
# and a restart either way, since it loads the new index only at start.
mcp="rws-docs-mcp-$tag-x86_64-unknown-linux-gnu.tar.gz"
if curl -fsSL -o "$work/mcp.tar.gz" "$base/$mcp" && curl -fsSL -o "$work/mcp.tar.gz.sha256" "$base/$mcp.sha256"; then
  [ "$(cut -d' ' -f1 "$work/mcp.tar.gz.sha256")" = "$(sha256sum "$work/mcp.tar.gz" | cut -d' ' -f1)" ] || { echo "checksum mismatch for $mcp" >&2; exit 1; }
  tar -xzf "$work/mcp.tar.gz" -C "$work" rws-docs-mcp
  install -m 0755 "$work/rws-docs-mcp" /usr/local/bin/rws-docs-mcp
  echo "installed rws-docs-mcp from $tag"
fi
systemctl try-restart rws-docs-mcp.service
