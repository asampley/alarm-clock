{
  autoPatchelfHook,
  makeWrapper,
  rustPlatform,
  stdenvNoCC,
  src,
  zlib,
}:
stdenvNoCC.mkDerivation {
  pname = "xtensa-rust-src";
  version = "x";

  inherit src;

  cargoDeps = rustPlatform.importCargoLock {
    lockFile = "${src}/rust-src/lib/rustlib/src/rust/library/Cargo.lock";
  };

  nativeBuildInputs = [
    autoPatchelfHook
    makeWrapper
    rustPlatform.cargoSetupHook
    zlib
  ];

  patchPhase = ''
    patchShebangs install.sh
  '';

  installPhase = ''
    ./install.sh --prefix=$out
  '';

  dontStrip = true;
}
