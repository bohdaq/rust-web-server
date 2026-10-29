#!/bin/sh
# Install the rws binary from a GitHub release:
#
#   curl -fsSL https://rws8.tech/install.sh | sh
#   curl -fsSL https://rws8.tech/install.sh | sh -s -- --version 18.8.3 --dir ~/bin
#
# Linux (x86_64, aarch64; static musl builds, any distribution) and macOS
# (Apple Silicon, Intel). Downloads rws-<version>-<target>.tar.gz and its
# .sha256 from https://github.com/bohdaq/rust-web-server/releases, checks
# the checksum, and installs `rws` into /usr/local/bin when that is writable
# (or with sudo, asked first), else ~/.local/bin. Windows: use the .zip from
# the releases page, `cargo binstall rust-web-server` or winget.
#
# Environment: RWS_VERSION, RWS_INSTALL_DIR (same as the flags).
set -eu

repo=bohdaq/rust-web-server
version=${RWS_VERSION:-}
dir=${RWS_INSTALL_DIR:-}

say() { printf '%s\n' "$*"; }
fail() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --version) version=${2:?--version needs a value}; shift 2 ;;
    --version=*) version=${1#*=}; shift ;;
    --dir) dir=${2:?--dir needs a value}; shift 2 ;;
    --dir=*) dir=${1#*=}; shift ;;
    -h|--help) sed -n '2,15p' "$0" 2>/dev/null || true; exit 0 ;;
    *) fail "unknown option $1 (try --help)" ;;
  esac
done

need() { command -v "$1" >/dev/null 2>&1 || fail "needs $1"; }
need uname
need tar
need mktemp
if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL --proto '=https' --tlsv1.2 -o "$2" "$1"; }
  fetch_text() { curl -fsSL --proto '=https' --tlsv1.2 "$1"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -q -O "$2" "$1"; }
  fetch_text() { wget -q -O - "$1"; }
else
  fail "needs curl or wget"
fi
if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d ' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | cut -d ' ' -f1; }
else
  fail "needs sha256sum or shasum to check the download"
fi

os=$(uname -s)
arch=$(uname -m)
case "$arch" in
  x86_64|amd64) arch=x86_64 ;;
  aarch64|arm64) arch=aarch64 ;;
  *) fail "no prebuilt rws for $arch; build it with: cargo install rust-web-server" ;;
esac
case "$os" in
  Linux) target="$arch-unknown-linux-musl" ;;
  Darwin) target="$arch-apple-darwin" ;;
  *) fail "no prebuilt rws for $os here; see https://rws8.tech/docs/getting-started/installation/" ;;
esac

if [ -z "$version" ]; then
  version=$(fetch_text "https://api.github.com/repos/$repo/releases/latest" 2>/dev/null \
    | sed -n 's/^ *"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1 || true)
  case "$version" in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *) fail "could not find the latest release; pass --version, e.g. --version 18.8.3" ;;
  esac
fi
version=${version#v}

name="rws-$version-$target"
base="https://github.com/$repo/releases/download/$version"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

say "Downloading rws $version for $target"
fetch "$base/$name.tar.gz" "$work/$name.tar.gz" || fail "no $name.tar.gz in release $version"
fetch "$base/$name.tar.gz.sha256" "$work/$name.tar.gz.sha256" || fail "no checksum for $name.tar.gz"
expected=$(cut -d ' ' -f1 "$work/$name.tar.gz.sha256")
actual=$(sha256 "$work/$name.tar.gz")
[ -n "$expected" ] && [ "$expected" = "$actual" ] || fail "checksum mismatch for $name.tar.gz (expected $expected, got $actual)"

tar -xzf "$work/$name.tar.gz" -C "$work"
bin=$(find "$work" -type f -name rws | head -n 1)
[ -n "$bin" ] || fail "the archive has no rws binary"
chmod 0755 "$bin"

if [ -z "$dir" ]; then
  if [ -w /usr/local/bin ] || [ "$(id -u)" = 0 ]; then
    dir=/usr/local/bin
  elif command -v sudo >/dev/null 2>&1 && (: </dev/tty) 2>/dev/null; then
    # Piped into sh, stdin is the script: sudo asks on the terminal.
    dir=/usr/local/bin
    use_sudo=1
  else
    dir="$HOME/.local/bin"
  fi
fi

if [ "${use_sudo:-}" = 1 ]; then
  say "Installing to $dir (sudo)"
  sudo mkdir -p "$dir" </dev/tty
  sudo install -m 0755 "$bin" "$dir/rws" </dev/tty
else
  mkdir -p "$dir"
  cp "$bin" "$dir/rws.tmp" && chmod 0755 "$dir/rws.tmp" && mv -f "$dir/rws.tmp" "$dir/rws"
fi

say "Installed $("$dir/rws" --version 2>/dev/null || echo "rws $version") to $dir/rws"
case ":$PATH:" in
  *":$dir:"*) ;;
  *) say "Add $dir to your PATH, e.g.: export PATH=\"$dir:\$PATH\"" ;;
esac
say "Serve a folder: cd my-site && rws"
