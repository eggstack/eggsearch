# Testing Infrastructure Deep Dive

**Location:** `tests/` (54 files), `fuzz/` (23 targets)
**Purpose:** Comprehensive test suites for correctness, security, and performance.

---

## Test Categories

### Integration Tests (`tests/integration.rs`)

MCP tool input validation, provider failures, tool response shape.

**Key scenarios:**
- Input validation for all 10 tools
- Provider failure handling
- Response format verification
- Edge cases (empty queries, invalid URLs, etc.)

### Corpus Tests (`tests/corpus_runner.rs`, `tests/corpus/`)

Multi-step workflow regression tests.

**Structure:**
```
tests/corpus/
  scenarios/        # Happy-path workflow scenarios
  adversarial/      # Malformed/edge-case inputs
```

**Scenarios:**
- Web search → fetch → extract workflow
- Repo search → repo fetch → repo map workflow
- Security search → advisory lookup workflow
- Research search → evidence bundle workflow

### Property Tests (`tests/property_*.rs`)

Pure function testing with `proptest` (14 files).

**Coverage:**
- Identity functions (FNV-1a hashing)
- Sanitization pipeline
- URL canonicalization
- Version comparison
- Conflict detection
- Fetch limits validation
- Local filesystem operations

### Fault Injection (`tests/dispatch_fault_injection.rs`)

Provider failures, timeouts, concurrency testing.

**Scenarios:**
- Single provider failure
- All providers failing
- Timeout handling
- Concurrent request limits
- Panic recovery

### Security Tests

| File | Purpose |
|------|---------|
| `security_applicability_regression.rs` | Security applicability assessment regression |
| `security_applicability_phase8.rs` | Phase 8 security features |
| `fetch_safety.rs` | URL validation, SSRF prevention |

### Evidence Tests

| File | Purpose |
|------|---------|
| `evidence_bundle_handoff.rs` | Evidence bundle construction |
| `evidence_integration.rs` | Evidence workflow integration |
| `codegg_evidence_contract.rs` | Evidence contract verification |

### Config Tests

| File | Purpose |
|------|---------|
| `config_validation.rs` | Config validation rules |
| `docs_config_snippets.rs` | Documentation config snippets |

### Browser Tests (feature-gated `browser`)

| File | Purpose |
|------|---------|
| `browser_profiles.rs` | Profile management |
| `browser_transport.rs` | Transport orchestration |
| `browser_live_smoke.rs` | Live browser tests |

### Contract Tests

| File | Purpose |
|------|---------|
| `keyless_core.rs` | Keyless-core invariant |
| `docs_keyless_contract.rs` | Keyless contract documentation |
| `docs_provider_inventory.rs` | Provider inventory documentation |
| `docs_tool_names.rs` | Tool name documentation |
| `docs_safety_vocabulary.rs` | Safety vocabulary documentation |

### Other Targeted Tests

| File | Purpose |
|------|---------|
| `forge_adapter.rs` | Forge API client |
| `bounded_command.rs` | Bounded command execution |
| `conflict_source_attribution.rs` | Conflict source attribution |
| `inventory_freshness.rs` | Inventory freshness |
| `local_workspace_integration.rs` | Local workspace integration |
| `schema_identity_registry.rs` | Schema identity registry |
| `static_guards.rs` | Static guards |
| `recipes_next_actions.rs` | Recipe next actions |
| `retrieval_attempt_ledger.rs` | Retrieval attempt ledger |
| `native_forge_smoke.rs` | Native forge smoke tests |
| `native_security_attempts.rs` | Native security attempts |
| `research_evidence_corpus.rs` | Research evidence corpus |
| `research_semantic_roles.rs` | Research semantic roles |

---

## Fuzz Targets (`fuzz/fuzz_targets/`)

23 targets using `cargo-fuzz` + `libfuzzer`:

| Target | Purpose |
|--------|---------|
| `validate_url` | URL validation |
| `sanitize_pipeline` | Sanitization pipeline |
| `extract_content` | HTML content extraction |
| `extract_pdf_text` | PDF text extraction |
| `bounded_response_reader` | Bounded response reading |
| `canonicalize_url` | URL canonicalization |
| `classify_absence` | Absence classification |
| `workflow_kind_parse` | Workflow kind parsing |
| `research_role_mapping` | Research role mapping |
| ... | ... |

---

## Test Execution

### Full Suite

```bash
make check                    # fmt + clippy + no-default + all-features tests
cargo test --locked --all-features  # all tests
```

### Specific Suites

```bash
# Integration only
cargo test --locked --features mock --test integration

# Corpus regression
cargo test --locked --features mock --test corpus_runner

# Standalone tests
cargo test --locked --all-features --test security_applicability_regression
cargo test --locked --all-features --test security_applicability_phase8

# Dispatch fault injection
cargo test --locked --all-features --test dispatch_fault_injection

# Adversarial corpus
cargo test --locked --all-features --test adversarial_corpus

# Keyless-core contract
cargo test --locked --all-features --test keyless_core

# Browser tests
cargo test --locked --features browser --test browser_profiles
cargo test --locked --features browser --test browser_transport

# Live smoke tests (requires network)
cargo test --features live-smoke --test corpus_runner -- --ignored
```

---

## Test Conventions

### New File vs Extend Existing

- **New file** when testing a distinct subsystem or targeting a specific bug class
- **Extend `integration.rs`** for MCP tool input validation, provider failures, tool response shape
- **Extend `corpus_runner.rs`** for multi-step workflows
- **Unit tests** at bottom of source file for private functions

### Property Tests

Use `proptest` for pure functions:

```rust
proptest! {
    #[test]
    fn test_source_id_deterministic(url in "https://.*", title in ".*") {
        let id1 = source_id(&url, &title, None);
        let id2 = source_id(&url, &title, None);
        prop_assert_eq!(id1, id2);
    }
}
```

### Mock Engine

Feature-gated `mock`:

```rust
#[cfg(feature = "mock")]
pub mod mock {
    pub struct MockEngine;
    
    impl SearchEngine for MockEngine {
        fn search(&self, ...) -> Result<Vec<SearchResult>> {
            // Returns deterministic test data
        }
    }
}
```

---

## Code Coverage

### Lint Checks

```bash
cargo clippy --locked --all-targets --all-features -- -D warnings
```

Zero warnings required.

### Format Check

```bash
cargo fmt --check
```

CI fails on formatting violations.

---

## Performance Benchmarks

`benches/perf.rs` — criterion-based benchmarks:

- URL validation
- Sanitization pipeline
- HTML extraction
- FNV-1a hashing

---

[← Back to Overview](overview.md)
