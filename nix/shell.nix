{
  alarm-clock,
  mkShell,
  just,
  rust-analyzer,
  rustfmt,
  probe-rs-tools,
  ...
}:
mkShell {
  inputsFrom = [ alarm-clock ];
  buildInputs = [
    just
    rust-analyzer
    rustfmt
    probe-rs-tools
  ];
}
