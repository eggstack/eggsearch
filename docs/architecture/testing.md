# Testing Strategy Deep Dive

**Path:** `tests/` and `src/*/mod.rs`
**Purpose:** Comprehensive test coverage across unit, integration, corpus, schema/contract, and documentation tests.

---

## Test Categories

### Unit Tests

**Location:** Bottom of each source file (`#[cfg(test)]`)

- Test private functions and internal logic
- No feature flags required
- Run with: `cargo test --all-features`

### Integration Tests

**Location:** `tests/integration.rs`
**Feature gate:** `mock`

- MCP tool contracts, error handling, provider behavior
- Uses mock engine harness
- Run with: `cargo test --features mock --test integration`

### Corpus Tests

**Location:** `tests/corpus_runner.rs`
**Feature gate:** `mock`

- Multi-step workflow regression tests
- Tests agent workflow recipes end-to-end
- Run with: `cargo test --features mock --test corpus_runner`

### Schema/Contract Tests

**Location:** `tests/schema_identity_registry.rs`, `tests/fetch_safety.rs`, `tests/security_applicability_corpus.rs`, `tests/research_evidence_corpus.rs`, `tests/recipes_next_actions.rs`, `tests/evidence_bundle_handoff.rs`

- 6 regression test binaries
- Validate schema stability, deterministic IDs, safety properties
- Run with: `make schema-corpus`

### Documentation Contract Tests

**Location:** `tests/docs_config_snippets.rs`, `tests/docs_provider_inventory.rs`, `tests/docs_tool_names.rs`, `tests/docs_safety_vocabulary.rs`

- Validate docs snippets against actual types
- Ensure config examples, provider lists, tool names, safety vocabulary match code
- Run with: `cargo test --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names --test docs_safety_vocabulary`

### Config Validation Tests

**Location:** `tests/config_validation.rs`

- Config deserialization, validation, provider resolution
- Run with: `cargo test --all-features --test config_validation`

### Security Applicability Tests

**Location:** `tests/security_applicability_regression.rs`, `tests/security_applicability_phase8.rs`

- Range evaluation boundary regressions
- Defensive output verification
- Run with: `cargo test --all-features --test security_applicability_regression --test security_applicability_phase8`

---

## Feature Flags

| Flag | Purpose | Required For |
|------|---------|-------------|
| `mock` | Test-only mock engine harness (`src/meta/mock.rs`) | Integration/corpus tests |
| `pdf` | PDF text extraction via `lopdf` | PDF-specific tests |
| `live-smoke` | Live network smoke tests (implies `mock`) | Manual live tests only |

**Critical:** Integration/corpus tests require `--features mock`. Running `cargo test` without features misses most integration tests.

---

## CI Pipeline

The CI runs tests across 4 feature combos:

1. `--all-features`
2. `--no-default-features`
3. `--features mock`
4. `--features pdf`

### CI Jobs

| Job | What it runs |
|-----|-------------|
| check | `cargo check` × 4 feature combos |
| test | `cargo test --locked` × 4 feature combos |
| clippy | `cargo clippy --all-targets --all-features -- -D warnings` |
| schema-corpus | 6 regression test binaries (Makefile uses `--locked`, CI does not) |
| docs-contract | 4 documentation contract tests (Makefile uses `--locked`, CI does not) |
| fmt | `cargo fmt --check` |
| release-build | `cargo build --release` |
| publish-check | `cargo publish --dry-run --locked` |
| docs | `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` |

---

## Running Specific Suites

```bash
cargo test --locked --features mock --test integration              # integration only
cargo test --locked --features mock --test corpus_runner            # corpus regression
cargo test --locked --all-features --test security_applicability_regression --test security_applicability_phase8  # standalone
make schema-corpus                                         # all contract tests
make docs-tests                                            # documentation contract tests
cargo test --locked --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names --test docs_safety_vocabulary
```

---

## Mock Engine

The `mock` feature enables `src/meta/mock.rs` — a test-only search engine that returns predictable results without network access.

All integration/corpus tests use the mock engine. **Tests MUST NOT require network access.**

Live smoke tests are run separately:
```bash
cargo test --features live-smoke --test corpus_runner -- --ignored
```

---

## Full CI Gate

```bash
make check
```

This runs the complete CI suite locally:
1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --locked --all-features`
4. `cargo test --locked --no-default-features`
5. `cargo test --locked --features mock`
6. `cargo test --locked --features pdf`
7. Schema-corpus tests
8. Documentation contract tests
9. `cargo build --release`
10. `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps`
11. `cargo publish --dry-run --locked`

---

**Back to:** [overview.md](overview.md)
