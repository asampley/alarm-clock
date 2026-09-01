{
  lib,
  cargo,
  rustc,
  rustPlatform,
  stdenv,
  symlinkJoin,
  build-std ? stdenv.hostPlatform.isNone,
}:
let
  manifest = lib.importTOML ../alarm-clock/Cargo.toml;
  cargoDeps = symlinkJoin {
    name = "cargo-vendor-dir";
    paths = [
      # must be first to get Cargo.lock to be correct in setup phase hook
      (rustPlatform.importCargoLock { lockFile = ../Cargo.lock; })
    ]
    ++ (
      if !build-std then
        [ ]
      else
        [
          (rustPlatform.importCargoLock {
            lockFile = "${cargo}/lib/rustlib/src/rust/library/Cargo.lock";
          })
        ]
    );
  };
in
#stdenv.mkDerivation {
#  inherit cargoDeps;
#
#  pname = manifest.package.name;
#  version = manifest.package.version;
#
#  src =
#    with lib.fileset;
#    toSource {
#      root = ../.;
#      fileset = difference ../. (unions [
#        ../flake.nix
#        ../flake.lock
#        ../nix
#      ]);
#    };
#
#  nativeBuildInputs = [
#    rustPlatform.cargoSetupHook
#    rustPlatform.cargoInstallHook
#    cargo
#    rustc
#  ];
#
#  buildPhase = ''
#    cargo build --offline --locked --release --target "${stdenv.hostPlatform.rust.rustcTarget}" ${if build-std then "-Zbuild-std" else ""}
#  '';
#}
rustPlatform.buildRustPackage {
  pname = manifest.package.name;
  version = manifest.package.version;

  #cargoLock.lockFile = ../Cargo.lock;
  inherit cargoDeps;

  cargoBuildFlags = lib.optionals build-std [ "-Zbuild-std" ];

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
