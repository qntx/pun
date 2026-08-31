# Makefile for Rust project using Cargo
# Kept in sync with Justfile (same targets / same check suite).

.PHONY: all default list build check run test bench clippy clippy-fix fmt fmt-check doc update deny clean

default: all
all: fmt clippy-fix deny test

list:
	@echo "Available targets: all build check run test bench clippy clippy-fix fmt fmt-check doc update deny clean"

build:
	cargo build --workspace --release --all-features

check:
	cargo check --workspace --all-features

update:
	cargo update

run:
	cargo run -p gap --release --all-features --

test:
	cargo test --workspace --all-features

bench:
	cargo bench --all-features

clippy:
	cargo +nightly clippy --workspace \
		--all-targets \
		--all-features \
		-- -D warnings

clippy-fix:
	cargo +nightly clippy --workspace \
		--fix \
		--all-targets \
		--all-features \
		--allow-dirty \
		--allow-staged \
		-- -D warnings

fmt:
	cargo +nightly fmt --all -- \
		--config unstable_features=true,group_imports=StdExternalCrate,imports_granularity=Module

fmt-check:
	cargo +nightly fmt --all -- \
		--check \
		--config unstable_features=true,group_imports=StdExternalCrate,imports_granularity=Module

doc:
	cargo +nightly doc --all-features --no-deps --open

deny:
	cargo deny check

clean:
	cargo clean
