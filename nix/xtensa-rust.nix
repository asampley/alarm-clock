{
  autoPatchelfHook,
  gcc,
  makeWrapper,
  stdenv,
  xtensa-rust-archive-bin,
  xtensa-rust-src,
  zlib,
}:
stdenv.mkDerivation {
  pname = "xtensa-rust";
  version = "x";

  src = xtensa-rust-archive-bin;

  nativeBuildInputs = [
    autoPatchelfHook
    gcc.cc.lib
    makeWrapper
    zlib
  ];

  patchPhase = ''
    patchShebangs ./install.sh
  '';

  installPhase = ''
    ./install.sh --prefix="$out"

    cp -r "${xtensa-rust-src}/"* "$out"
  '';

  dontStrip = true;
}
