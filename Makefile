.PHONY: install check test test-coverage build

install:
	asdf install
	cargo install cargo-llvm-cov
	rustup component add llvm-tools-preview

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings

test:
	cargo test

test-coverage:
	cargo llvm-cov --cobertura --output-path coverage.xml

build:
	cargo build --release
