{
  lib,
  rustPlatform,
}:
let
  manifest = lib.importTOML ../alarm-clock/Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = manifest.package.name;
  version = manifest.package.version;

  cargoLock.lockFile = ../Cargo.lock;

  src =
    with lib.fileset;
    toSource {
      root = ../.;
      fileset = difference ../. (unions [
        ../flake.nix
        ../flake.lock
        ../nix
      ]);
    };
}
