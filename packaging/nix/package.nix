# The nixpkgs expression for rws (pkgs/by-name/ru/rust-web-server/package.nix
# there). At a release: set version, then the two hashes from the errors of
# `nix-build packaging/nix/release.nix` (fake hashes report the real ones).
{
  lib,
  rustPlatform,
  fetchFromGitHub,
  cmake,
  perl,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "rust-web-server";
  version = "18.2.0";

  src = fetchFromGitHub {
    owner = "bohdaq";
    repo = "rust-web-server";
    tag = finalAttrs.version;
    hash = lib.fakeHash;
  };

  cargoHash = lib.fakeHash;

  # aws-lc-sys (TLS) builds with cmake and perl.
  nativeBuildInputs = [
    cmake
    perl
  ];

  cargoBuildFlags = [ "--bin" "rws" ];
  buildFeatures = [
    "acme"
    "auth"
  ];

  # The library's unit tests; the rest bind sockets or need the network.
  cargoTestFlags = [ "--lib" ];

  meta = {
    description = "Web server and reverse proxy with automatic HTTPS, HTTP/3, load balancing and auth";
    homepage = "https://rws8.tech";
    changelog = "https://github.com/bohdaq/rust-web-server/blob/${finalAttrs.version}/CHANGELOG.md";
    license = lib.licenses.mit;
    mainProgram = "rws";
    maintainers = [ ];
  };
})
