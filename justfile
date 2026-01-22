set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

help:
	just --list

clean:
	cargo clean

build *params:
	cargo build --release {{params}}

run *params: (build params)
	cargo run --release {{params}}

# If you're not using nix you can install through cargo
install-tools: install-rust-esp install-probe-rs

# If you're not using nix you can install through cargo
install-probe-rs:
	cargo +stable install probe-rs-tools

# If you're not using nix you can install through cargo
install-rust-esp:
	cargo +stable install espup
	espup install

[unix]
setup-rust-esp:
	. ~/export-esp.sh
