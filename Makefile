.PHONY: check fmt fmt-check lint test coverage bench run

check: fmt-check lint test

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test --all-features

# Requer: cargo install cargo-llvm-cov
coverage:
	cargo llvm-cov --all-features --summary-only --fail-under-lines 89

bench:
	cargo bench

run:
	cargo run --release
