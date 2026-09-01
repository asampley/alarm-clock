{
  alarm-clock,
  mkShell,
  just,
  rust-analyzer,
  probe-rs-tools,
  ...
}:
mkShell {
  inputsFrom = [ alarm-clock ];
  buildInputs = [
    just
    rust-analyzer
    probe-rs-tools
  ];
}
