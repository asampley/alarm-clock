set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

help:
	just --list

clean:
	cargo clean

build:
	cargo build --release

run: build
	cargo run --release

install-tools: install-rust-esp install-probe-rs

install-probe-rs:
	cargo +stable install probe-rs-tools

install-rust-esp:
	cargo +stable install espup
	espup install

[unix]
setup-rust-esp:
	. ~/export-esp.sh
