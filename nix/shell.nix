{
  alarm-clock,
  mkShell,
  espup,
  just,
  rust-analyzer,
  rustup,
  probe-rs-tools,
  ... }:
mkShell {
  withInputsFrom = [ alarm-clock ];
  buildInputs = [
    espup
    just
    rust-analyzer
    rustup
    probe-rs-tools
  ];

  shellHook = ''
    espup install --toolchain-version 1.92
    . ~/export-esp.sh
  '';
}
