#!/usr/bin/env bash
# winget manifests for an rws release (portable rws.exe in the release zip):
#   packaging/winget/generate.sh <version> <out dir>
# writes <out>/manifests/b/bohdaq/rws/<version>/, the layout of
# microsoft/winget-pkgs. The zip's SHA-256 comes from the release's .sha256.
set -euo pipefail
version=$1 out=$2
zip="rws-$version-x86_64-pc-windows-msvc.zip"
url="https://github.com/bohdaq/rust-web-server/releases/download/$version/$zip"
sha=$(curl -fsSL "$url.sha256" | awk '{print toupper($1)}')
[ ${#sha} -eq 64 ] || { echo "no SHA-256 for $zip" >&2; exit 1; }

dir="$out/manifests/b/bohdaq/rws/$version"
mkdir -p "$dir"
schema=1.12.0

cat > "$dir/bohdaq.rws.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.$schema.schema.json
PackageIdentifier: bohdaq.rws
PackageVersion: $version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: $schema
YAML

cat > "$dir/bohdaq.rws.installer.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.$schema.schema.json
PackageIdentifier: bohdaq.rws
PackageVersion: $version
InstallerType: zip
NestedInstallerType: portable
NestedInstallerFiles:
  - RelativeFilePath: rws-$version-x86_64-pc-windows-msvc\\rws.exe
    PortableCommandAlias: rws
Dependencies:
  PackageDependencies:
    - PackageIdentifier: Microsoft.VCRedist.2015+.x64
ReleaseDate: $(gh release view "$version" --repo bohdaq/rust-web-server --json publishedAt -q '.publishedAt[:10]' 2>/dev/null || date -u +%F)
Installers:
  - Architecture: x64
    InstallerUrl: $url
    InstallerSha256: $sha
ManifestType: installer
ManifestVersion: $schema
YAML

cat > "$dir/bohdaq.rws.locale.en-US.yaml" <<YAML
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.$schema.schema.json
PackageIdentifier: bohdaq.rws
PackageVersion: $version
PackageLocale: en-US
Publisher: Bohdan Tsap
PublisherUrl: https://github.com/bohdaq
PackageName: rws
PackageUrl: https://rws8.tech
License: MIT
LicenseUrl: https://github.com/bohdaq/rust-web-server/blob/$version/LICENSE
ShortDescription: Web server and reverse proxy with automatic HTTPS, HTTP/3, load balancing and auth in one binary.
Description: |-
  rws serves static files and proxies to backends over HTTP/1.1, HTTP/2 and HTTP/3,
  with automatic HTTPS from Let's Encrypt, load balancing, auth and rate limiting,
  configured in one TOML file. Also runs as a Windows service.
Moniker: rws
Tags:
  - http3
  - https
  - reverse-proxy
  - web-server
ReleaseNotesUrl: https://github.com/bohdaq/rust-web-server/releases/tag/$version
Documentations:
  - DocumentLabel: Windows
    DocumentUrl: https://rws8.tech/deployment/windows/
ManifestType: defaultLocale
ManifestVersion: $schema
YAML
echo "$dir"
