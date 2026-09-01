{
  autoPatchelfHook,
  gcc,
  lib,
  stdenvNoCC,
  xtensa-gcc-x86_64-linux-bin,
  ...
}:
stdenvNoCC.mkDerivation {
  pname = "esp-xtensa-gcc";
  version = "x";

  src = xtensa-gcc-x86_64-linux-bin;

  nativeBuildInputs = [
    autoPatchelfHook
    gcc.cc.lib
  ];

  installPhase = ''
    runHook preInstall
    mkdir -p $out
    cp -r ./* $out/
    find $out/
    runHook postInstall
  '';

  passthru = {
    isGNU = true;
    isClang = false;
    nativeLibc = false;
    targetPrefix = "xtensa-esp32s3-elf-";
    bintools = {
      isLLVM = false;
    };
  };

  dontStrip = true;

  meta = with lib; {
    description = "ESP32 Xtensa GCC toolchain";
    homepage = "https://github.com/espressif/crosstool-NG";
    license = licenses.gpl2Plus;
    platforms = [ "x86_64-linux" ];
  };
}
