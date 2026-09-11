# Testing Infrastructure Deep Dive

**Location:** `tests/` (behavioral suites plus corpus/contract/property/adversarial), `fuzz/` (22 targets)
**Purpose:** Comprehensive test suites for correctness, security, and performance.

---

## Test Categories

### Behavioral Integration Suites (`tests/mcp_tools.rs`, `web_search_integration.rs`, etc.)

MCP tool input validation, provider failures, tool response shape, partitioned by behavioral contract (no historical mega-suite):

- `mcp_tools.rs` — stable tool surface registration
- `web_search_integration.rs` — web search validation, sanitization, intent reranking
- `web_fetch_integration.rs` — fetch extraction, truncation, safety
- `provider_routing.rs` — provider routing, code-host rewrites, diagnostics
- `repo_workflow.rs` — repository evidence discovery
- `research_workflow.rs` — multi-source research discovery
- `security_workflow.rs` — advisory retrieval and safety
- `evidence_contract.rs` — batch fetch and evidence packaging

### Streamable HTTP Tests (`tests/mcp_http.rs`)

Loopback lifecycle, health identity, current and legacy protocol flows, shared
tool definitions, request hardening, session cleanup, and shutdown behavior.

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

Pure function testing with `proptest` (16 files).

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

### Provider Probe Conformance (`tests/provider_probe_conformance.rs`)

Mock-backed deterministic coverage for the shared probe service:

- configured/routable success, missing-key/config skips, unknown-provider skips
- timeout, HTTP error (including 429 with `http_status`), parse, network, panic containment
- cooldown interaction and explicit-request-after-degraded semantics
- bounded/sanitized messages, credential non-leakage, narrow-request budget observance
- descriptor source-of-truth for native (`exa`/`tavily`) versus local domain filtering

### Security Tests

| File | Purpose |
|------|---------|
| `security_applicability_regression.rs` | Security applicability assessment regression |
| `security_applicability_corpus.rs` | Security applicability assessment corpus (version/range, dependency relations) |
| `security_applicability_contract.rs` | Security applicability assessment contract (formerly phase-8 suite) |
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
| `docs_provider_inventory.rs` | Provider inventory documentation (code-derived from `KNOWN_PROVIDER_IDS`) |
| `docs_tool_names.rs` | Tool name documentation (code-derived from `src/mcp/server.rs`) |
| `docs_safety_vocabulary.rs` | Safety vocabulary documentation |
| `provider_capability_contract.rs` | Provider native-capability enforcement contract (brave_api/exa/tavily/firecrawl, domain-filter exclusivity) |

### Behavioral Regression Contracts (historical phase suites, renamed)

| File | Purpose |
|------|---------|
| `provider_request_contract.rs` | `EngineSearchRequest` fidelity (formerly `phase1_provider_contract`) |
| `extract_fetch_contract.rs` | Excerpt bounds/merge/sanitization and fetch cache/focus controls (formerly `phase2_extract_fetch`) |
| `provider_workstream_regression.rs` | Provider inventory, capability matrix, closure invariants (formerly `phase5_closure`) |
| `structured_local_code_intelligence.rs` | Structured parsing, ranking, fallback, budgets, repo-map enrichment (formerly `phase13_structured_code`) |
| `batch_fetch_retrieval.rs` | Mixed focused/unfocused batch, truncation, isolation, round-trip (formerly `phase14_batch_focus`) |

### Tool Consolidation Contracts (Plans 001-005)

| File | Purpose |
|------|---------|
| `mcp_tool_contract.rs` | Canonical registry parity (aliases, discovery text, sanitization, fingerprint determinism) |
| `mcp_schema_slimming.rs` | Slimmed ordinary schema size budget with legacy-field acceptance |
| `mcp_2026_protocol.rs` | Structured results, output schemas, repairable error contract |
| `mcp_projection.rs` | Response-detail projection (failure-vs-absence, trust, conflicts, truncation, bundle identity, byte reduction) |

### Tool-Surface Evaluation (`tests/tool_surface_evaluation.rs`, `tests/tool_surface_live.rs`)

Deterministic agentic tool-surface regression gate over the labeled
corpus in `tests/fixtures/tool_surface/cases.json` (43 fixtures across
generic web, repo, fetch, exact-error, security, research, evidence,
diagnostics, and ambiguity categories; every stable tool is an expected
primary at least once). The runner scores each query against the live
tool definitions, then asserts description/total/instructions/compact
byte budgets, top-1/recall@3/MRR thresholds, forbidden-primary
exclusion, known-tool/next-action reference validity, and synthetic
Layer 2 hydration mechanics (next actions stay within the ten stable
tools and compact discovery stays below full definitions). Baseline
fingerprint and per-category accuracy print with `-- --nocapture` or
`make eval-tool-surface`. `tool_surface_live.rs` keeps the Layer 3
report contract in CI and holds one `#[ignore]`d manual multi-model
comparison entry point (`EGGSEARCH_EVAL_MODEL`, `-- --ignored`).

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

### Provider-Specific Tests

| File | Purpose |
|------|---------|
| `exa.rs` | Exa semantic search adapter |
| `tavily.rs` | Tavily search adapter |
| `firecrawl_developer.rs` | Firecrawl developer adapter |

---

## Fuzz Targets (`fuzz/fuzz_targets/`)

22 targets using `cargo-fuzz` + `libfuzzer`:

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
| ... | (9 shown; full list of 22 in `docs/test-inventory.md`) |

---

## Test Execution

### Full Suite

```bash
make check                    # fmt + clippy + no-default + all-features tests
cargo test --locked --all-features  # all tests
```

### Specific Suites

```bash
# Behavioral suites
cargo test --locked --features mock --test web_search_integration

# Corpus regression
cargo test --locked --features mock --test corpus_runner

# Standalone tests
cargo test --locked --all-features --test security_applicability_regression
cargo test --locked --all-features --test security_applicability_contract

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
- **Extend behavioral suites** (`mcp_tools`, `web_search/web_fetch` integration, `provider_routing`, `repo/research/security` workflow, `evidence_contract`) for MCP tool input validation, provider failures, tool response shape
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
