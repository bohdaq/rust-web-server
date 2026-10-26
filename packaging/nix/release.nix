# package.nix as nixpkgs would build it, from the release tag on GitHub. With
# the fake hashes, the build fails and prints the real ones.
{ pkgs ? import <nixpkgs> { } }:

pkgs.callPackage ./package.nix { }
