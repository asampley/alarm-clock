{
  lib,
  cargo,
  coreutils,
  rustPlatform,
  stdenv,
  stdenvNoCC,
  symlinkJoin,
  xtensa-gcc,
  build-std ? stdenvNoCC.hostPlatform.isNone,
}:
let
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
stdenv.mkDerivation {
  pname = manifest.package.name;
  version = manifest.package.version;

  nativeBuildInputs = [
    cargo
    cargoDeps
    coreutils
    xtensa-gcc
  ];

  inherit src;

  buildPhase = ''
    ln -s "${cargoDeps}" cargo-vendor-dir
    cp -r "${src}/" src
    chmod u+w src

    cd src

    chmod u+w .cargo/config.toml
    cat >> .cargo/config.toml <<- 'EOF'
      [source.crates-io]
      replace-with = "vendored-sources"
      [source.vendored-sources]
      directory = "../cargo-vendor-dir"
    EOF

    cargo build --frozen --release ${
      if build-std then "-Zbuild-std" else ""
    } --target "${stdenvNoCC.hostPlatform.rust.rustcTarget}" --target-dir ../build
  '';

  installPhase = ''
    mkdir -p "$out/bin"

    cp '../build/${stdenvNoCC.hostPlatform.rust.rustcTarget}/release/${manifest.package.name}' "$out/bin"
  '';
}
