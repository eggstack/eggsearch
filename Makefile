.PHONY: check ci fmt clippy feature-check test release-check docs-check release-build publish-check bench-check fuzz-smoke live-smoke native-forge-smoke

check: fmt clippy feature-check test

ci: check

fmt:
	cargo fmt --check

clippy:
	cargo clippy --locked --all-targets --all-features -- -D warnings

feature-check:
	cargo check --locked --no-default-features

test:
	cargo test --locked --all-features
	cargo test --locked --no-default-features

release-check: check docs-check release-build publish-check

docs-check:
	RUSTDOCFLAGS="-D warnings" cargo doc --locked --all-features --no-deps

release-build:
	cargo build --locked --release

publish-check:
	cargo publish --dry-run --locked

bench-check:
	cargo bench --locked --all-features --bench perf --no-run

fuzz-smoke:
	cd fuzz && cargo fuzz run validate_url -- -max_total_time=60
	cd fuzz && cargo fuzz run sanitize_pipeline -- -max_total_time=60
	cd fuzz && cargo fuzz run bounded_response_reader -- -max_total_time=60

live-smoke:
	cargo test --features live-smoke --test corpus_runner -- --ignored

native-forge-smoke:
	cargo test --features live-smoke --test native_forge_smoke -- --ignored
