.PHONY: check test clippy fmt doc schema-corpus docs-tests publish-check live-smoke release-build hardening

# Full offline quality gate (all CI checks)
check: fmt clippy test-all test-no-default test-mock test-pdf hardening schema-corpus docs-tests release-build docs publish-check

# Format check
fmt:
	cargo fmt --check

# Clippy
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# All tests
test-all:
	cargo test --locked --all-features

# No default features
test-no-default:
	cargo test --locked --no-default-features

# Mock feature tests
test-mock:
	cargo test --locked --features mock

# PDF feature tests
test-pdf:
	cargo test --locked --features pdf

# Property tests and adversarial corpus validation
hardening:
	cargo test --locked --all-features --test property_sanitize --test property_identity --test property_identity2 --test property_identity3 --test property_fetch_limits --test property_fetch_redirects --test property_fetch_url_edge --test property_fetch_response --test property_render_safety --test property_render_code --test property_render_metadata --test property_local_fs --test property_local_fs_extended --test dispatch_fault_injection --test adversarial_corpus

# Schema/fixture corpus tests (all new contract tests)
schema-corpus:
	cargo test --locked --features mock --test schema_identity_registry
	cargo test --locked --features mock --test fetch_safety
	cargo test --locked --features mock --test security_applicability_corpus
	cargo test --locked --features mock --test research_evidence_corpus
	cargo test --locked --features mock --test recipes_next_actions
	cargo test --locked --features mock --test evidence_bundle_handoff

# Documentation contract tests
docs-tests:
	cargo test --locked --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names --test docs_safety_vocabulary

# Release build
release-build:
	cargo build --release

# Documentation build (warnings denied)
docs:
	RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps

# Live smoke tests (requires network, ignored by default)
live-smoke:
	cargo test --features live-smoke --test corpus_runner -- --ignored

# Dry-run publish check
publish-check:
	cargo publish --dry-run --locked
