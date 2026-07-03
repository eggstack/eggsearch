.PHONY: check test clippy fmt doc schema-corpus

# Full offline quality gate (all CI checks)
check: fmt clippy test-all test-no-default schema-corpus

# Format check
fmt:
	cargo fmt --check

# Clippy
clippy:
	cargo clippy --all-features -- -D warnings

# All tests
test-all:
	cargo test --all-features

# No default features
test-no-default:
	cargo test --no-default-features

# Schema/fixture corpus tests (all new contract tests)
schema-corpus:
	cargo test --features mock --test schema_identity_registry
	cargo test --features mock --test fetch_safety
	cargo test --features mock --test security_applicability_corpus
	cargo test --features mock --test research_evidence_corpus
	cargo test --features mock --test recipes_next_actions
	cargo test --features mock --test evidence_bundle_handoff

# Live smoke tests (requires network, ignored by default)
live-smoke:
	cargo test --features live-smoke --test corpus_runner -- --ignored

# Dry-run publish check
publish-check:
	cargo publish --dry-run
