.PHONY: check ci fmt clippy feature-check test hygiene packaging-check release-check release-candidate-check docs-check release-build publish-check bench-check fuzz-smoke live-smoke eval-tool-surface native-forge-smoke-github native-forge-smoke-gitlab native-forge-smoke-codeberg native-forge-smoke-gitea native-forge-smoke-all

check: fmt clippy feature-check test hygiene packaging-check

ci: check

fmt:
	cargo fmt --check

clippy:
	cargo clippy --locked --all-targets --all-features -- -D warnings

feature-check:
	cargo check --locked --no-default-features

test:
	cargo test --locked --all-features

hygiene:
	./packaging/check-repo-hygiene.sh

packaging-check:
	./packaging/check-contract.sh

release-check: check release-candidate-check docs-check release-build publish-check

release-candidate-check:
	./packaging/release-validate.sh candidate

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

eval-tool-surface:
	cargo test --locked --all-features --test tool_surface_evaluation -- --nocapture

native-forge-smoke-github:
	cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored native_github

native-forge-smoke-gitlab:
	cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored native_gitlab

native-forge-smoke-codeberg:
	cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored native_codeberg

native-forge-smoke-gitea:
	cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored native_gitea

native-forge-smoke-all:
	cargo test --locked --features live-smoke --test native_forge_smoke -- --ignored
