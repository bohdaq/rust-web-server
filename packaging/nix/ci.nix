# package.nix built from this checkout instead of a release tag (CI: nix.yml).
{ pkgs ? import <nixpkgs> { } }:

(pkgs.callPackage ./package.nix { }).overrideAttrs (_: {
  version = "dev";
  src = pkgs.lib.cleanSourceWith {
    src = ../..;
    filter = path: _: !(builtins.elem (baseNameOf path) [ "target" "node_modules" ".git" ]);
  };
  cargoDeps = pkgs.rustPlatform.importCargoLock { lockFile = ../../Cargo.lock; };
})
