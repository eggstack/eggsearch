# Testing

**Location:** `tests/` (76 `*.rs` suites), `src/**/tests` (module unit tests), `benches/perf.rs`, `fuzz/fuzz_targets/` (22 targets)
**Purpose:** Behavioral contracts first, property tests for pure functions, fault injection for dispatch, corpus regression for multi-step workflows, fuzz for adversarial input, Criterion for characterization.

Full per-suite counts live in `docs/test-inventory.md`. Suite placement rules live in `skills/eggsearch-dev/SKILL.md`. This document explains how the layers fit together and which commands are canonical.

---

## Suite count and layout

`tests/*.rs` holds 76 suites (mid-70s to low-80s as suites are added or merged; treat `docs/test-inventory.md` as the count source of truth). Layout:

| Location | Purpose |
|----------|---------|
| `tests/mcp_tools.rs`, `web_search_integration.rs`, `web_fetch_integration.rs`, `provider_routing.rs`, `provider_probe_conformance.rs`, `repo_workflow.rs`, `research_workflow.rs`, `security_workflow.rs`, `evidence_contract.rs` | Behavioral MCP/workflow contracts |
| `tests/corpus_runner.rs` + `tests/corpus/scenarios/` | Multi-step workflow regression |
| `tests/corpus/adversarial/` + `tests/adversarial_corpus.rs` | Malformed-input structural validation |
| `tests/property_*.rs` (16 suites) | Pure-function `proptest` coverage |
| `tests/dispatch_fault_injection.rs` | Provider failure/timeout/concurrency/panic |
| `tests/docs_*.rs`, `tests/static_guards.rs`, `tests/schema_identity_registry.rs` | Documentation, hygiene, and identity contracts |
| `tests/mcp_tool_contract.rs`, `mcp_schema_slimming.rs`, `mcp_2026_protocol.rs`, `mcp_projection.rs` | Tool consolidation contracts |
| `tests/tool_surface_evaluation.rs`, `tests/tool_surface_live.rs` | Tool-selection corpus + opt-in live-model comparison |
| `tests/mcp_http.rs` | Loopback Streamable HTTP lifecycle and transport hardening |
| `packaging/test-install.sh`, `packaging/test-install.ps1` | Installer behavior (not `tests/`) |
| `src/**/mod.rs` unit tests | Private-function coverage at the bottom of the source file |

---

## Behavioral suite naming

Use behavioral suite names. Historical phase-suite names are retired; do not introduce new ones.

| Suite | Contract |
|-------|----------|
| `mcp_tools` | Stable tool surface: registration, input validation for all 10 tools, response shape, edge cases (empty queries, invalid URLs) |
| `web_search_integration` | Web search validation, sanitization, intent reranking, excerpt bounds |
| `web_fetch_integration` | Fetch extraction, truncation, metadata-only mode, safety bounds |
| `provider_routing` | Provider routing, code-host rewrites, diagnostics, capability-skip telemetry |
| `provider_probe_conformance` | Shared probe service: configured/routable success, missing-key and config skips, unknown-provider skips, timeout, HTTP error (including 429 with `http_status`), parse and network failures, panic containment, cooldown interaction, explicit-request-after-degraded semantics, bounded/sanitized messages, credential non-leakage, narrow-request budget observance, descriptor source-of-truth for native versus local domain filtering |
| `repo_workflow` | Repository evidence discovery end to end |
| `research_workflow` | Multi-source research discovery end to end |
| `security_workflow` | Advisory retrieval, applicability assessment, safety handling |
| `evidence_contract` | Batch fetch isolation, evidence packaging, bundle handoff shape |
| `extract_fetch_contract` | Excerpt bounds and merge, focus ranking and caps, fetch cache policy and max-age controls |
| `batch_fetch_retrieval` | Mixed focused/unfocused batch, UTF-8 boundaries, failure isolation, suggested-fetch round-trip |
| `provider_request_contract` | `EngineSearchRequest` fidelity, date/domain validation, wire compression behavior |
| `provider_capability_contract` | Native-capability enforcement matrix (native versus local filtering) |
| `provider_workstream_regression` | Provider inventory, capability descriptors, URL dedup with stable IDs |
| `structured_local_code_intelligence` | Structured parsing, definition ranking, regex fallback, budgets, repo-map enrichment |
| `mcp_tool_contract`, `mcp_schema_slimming`, `mcp_2026_protocol`, `mcp_projection` | Registry parity, schema size budgets, structured error contract, response-detail projection |
| `fetch_safety` | URL validation and SSRF prevention at the fetch boundary |
| `forge_adapter` | Forge endpoint validation, redirect policy, URL construction from `resolved_ref` |
| `local_workspace_integration`, `inventory_freshness`, `retrieval_attempt_ledger`, `evidence_bundle_handoff`, `evidence_integration` | Workspace, inventory lifecycle, ledger validation, bundle pipeline |
| `config_validation`, `docs_config_snippets` | Config rules and doc snippet validity |
| `exa`, `tavily`, `firecrawl_developer` | Provider-specific adapter behavior |
| `security_applicability_regression`, `security_applicability_corpus`, `security_applicability_contract` | Applicability pipeline regression, corpus, and contract |
| `research_evidence_corpus`, `research_semantic_roles` | Research evidence regression and role mapping |
| `conflict_source_attribution`, `recipes_next_actions`, `schema_identity_registry` | Conflict attribution, hint generation, schema and ID fixtures |
| `keyless_core`, `docs_keyless_contract` | Keyless-core runtime invariant and its doc contract |
| `egress_routing`, `egress_qualify_contract` | Egress route behavior and egress qualification CI contract |
| `bounded_command` | Bounded git command execution (caps, timeout, termination) |

### Where new tests go

- New file for a distinct subsystem or a specific bug class.
- Extend the behavioral suites above for MCP tool input validation, provider failures, and tool response shape. Do not create a parallel mega-suite.
- Extend `corpus_runner.rs` plus `tests/corpus/scenarios/` for multi-step workflows.
- Pure functions get `proptest` files (`tests/property_*.rs`), not behavioral suites.
- Provider failure modes go in `dispatch_fault_injection.rs`, not scattered across workflow suites.
- Packaging behavior belongs under `packaging/test-install.sh` and `packaging/test-install.ps1`, never under `tests/`.
- Multi-step regressions go in `corpus_runner.rs`; single-step edge cases stay in the owning behavioral suite.

---

## Mock feature harness

`mock` is a test-only engine harness (`src/meta/mock.rs`). It is required for integration and corpus tests; plain `cargo test` without features misses most of them.

```bash
cargo test --locked --features mock --test web_search_integration
cargo test --locked --features mock --test repo_workflow
cargo test --locked --features mock --test corpus_runner
```

Feature flags in `Cargo.toml`:

| Flag | Purpose |
|------|---------|
| `mock` | Deterministic mock engine; required for integration/corpus tests |
| `pdf` | PDF text extraction via `lopdf` |
| `browser` | Headless rendering via `chromiumoxide` |
| `egress` | Opt-in listener-free HTTP/SOCKS proxy-chain route for provider upstreams only; excluded from default binaries |
| `live-smoke` | Live network smoke tests; implies `mock`; manual opt-in only |

Browser suites are feature-gated separately:

```bash
cargo test --locked --features browser --test browser_profiles
cargo test --locked --features browser --test browser_transport
```

---

## Keyless rule

Tests must pass keyless. CI blanks all credential environment variables, so missing credentials are provider-scoped skips, never global failures. A suite that fails only because no API key is present is a bug in the suite, not a missing credential. Provider-scoped outcomes (skip, capability-unavailable, degraded) must be asserted explicitly where the suite covers unconfigured providers. `keyless_core` and `docs_keyless_contract` pin this invariant.

---

## Live-smoke opt-in

Network tests are never part of the default gate. `live-smoke` implies `mock` and marks its tests `#[ignore]`d:

```bash
cargo test --locked --features live-smoke --test corpus_runner -- --ignored
cargo test --locked --all-features --test tool_surface_live -- --ignored
```

`tool_surface_live` additionally supports a manual multi-model comparison entry point gated by `EGGSEARCH_EVAL_MODEL`. It keeps the Layer 3 report contract in CI and holds exactly one ignored comparison test for operator-driven runs. Native forge smoke suites follow the same pattern and require operator-provided credentials plus fixture configuration; they are maintainer-only diagnostics, never release evidence.

---

## Corpus runner

`tests/corpus_runner.rs` is the multi-step regression gate. Scenarios in `tests/corpus/scenarios/` cover search-to-fetch-to-extract flows across web, repo, security, and research workflows, including ranking, fallback, and applicability edge cases. Each scenario is a deterministic JSON fixture executed against the mock harness. Add a scenario when a bug spans more than one tool call; keep single-call regressions in the owning behavioral suite.

---

## Property tests

16 `proptest` suites cover pure functions only and run without network access:

| Suite | Focus |
|-------|-------|
| `property_sanitize`, `property_render_safety` | `strip_control_chars`, `bound_text`, `scan_injection_markers`, `frame` |
| `property_identity`, `property_identity2`, `property_identity3` | FNV-1a IDs, canonicalization, cross-type non-collision |
| `property_fetch_limits`, `property_fetch_redirects`, `property_fetch_url_edge` | Scheme, length, IP classification, TLD and port policy |
| `property_fetch_response` | `FetchClient` credentials, metadata-only mode, caps, timeout, redirect limit |
| `property_render_code` | Code, diff, plaintext, and CSV renderers |
| `property_render_metadata` | `TrustMarkers` merge laws, outline-reference bounds |
| `property_local_fs`, `property_local_fs_extended` | Path joining, traversal, symlinks, root containment |
| `property_forge_url` | Forge URL builders and credential rejection |
| `property_conflict` | Entity-scoped conflict detectors |
| `property_retrieval` | Attempt ledger, absence kinds, truncation evidence |

Pattern: assert invariants (determinism, idempotency, bounds, non-panic on arbitrary input), not single examples.

```bash
cargo test --locked --all-features --test property_sanitize
cargo test --locked --all-features --test property_identity
cargo test --locked --all-features --test property_fetch_limits
cargo test --locked --all-features --test property_retrieval
```

---

## Fault injection and adversarial corpus

`dispatch_fault_injection` (requires `mock`) pins soft-failure semantics: partial failure yields partial results, all-failure yields empty results, panics and hangs are contained, output ordering is deterministic across runs, health transitions and cooldowns behave, and concurrency never exceeds the provider limit.

`adversarial_corpus` validates the JSON corpora under `tests/corpus/adversarial/` (malformed HTML, extended markup, structured text, URL edges, sanitization edges, identity edges, PDF edges, filesystem edges). The runner asserts structural validity; the behavioral suites assert the handling.

```bash
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test adversarial_corpus
```

---

## Documentation contract tests

`tests/docs_tool_names.rs` derives from `src/mcp/server.rs` and `tests/docs_provider_inventory.rs` derives from `KNOWN_PROVIDER_IDS`. Never invent tool-like or provider names in prose; keep docs in agreement with the code. `static_guards` enforces module ownership fail-closed. `docs_config_snippets`, `docs_safety_vocabulary`, and `docs_keyless_contract` pin the remaining doc promises.

```bash
cargo test --locked --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names --test docs_safety_vocabulary --test static_guards
```

---

## Fuzz harnesses

22 `cargo-fuzz` + `libfuzzer` targets are registered as `[[bin]]` entries in `fuzz/Cargo.toml` (the source of truth), covering URL validation, redirect chains, content-type classification, HTML and byte-level extraction, mixed-UTF-8 handling, PDF extraction, the sanitize pipeline and its stages, chunk boundaries, document chunking, the production bounded chunk-append path, workflow parsing and resolution, absence classification, conflict detection, and retrieval bookkeeping. Fuzz-only dependencies never enter the runtime graph.

Smoke-run the three key targets with:

```bash
make fuzz-smoke
```

which runs `validate_url`, `sanitize_pipeline`, and `bounded_response_reader` for 60 seconds each. Longer campaigns use `cargo +nightly fuzz run <target> -- -max_total_time=300`. Seed corpora stay minimal (a handful of distinct-path inputs per target, no network-derived seeds, 8 KB per file) and crash artifacts are promoted into deterministic regression tests (property file, adversarial JSON entry, or behavioral suite) rather than re-fuzzed blindly. Full harness design, seed rules, and the crash promotion process live in `architecture/hardening.md`.

---

## Benchmarks

`benches/perf.rs` is a Criterion harness (single `perf` bench target, `harness = false`). It characterizes URL validation, the sanitization pipeline, HTML extraction, FNV-1a hashing, local inventory candidate selection, timeout-client adjustment, derived-cache hits, compact MCP projections, and cached tool-definition access. Results are characterization evidence on exact candidates, never CI thresholds. The deterministic gate is compile-only:

```bash
make bench-check
```

which runs `cargo bench --locked --all-features --bench perf --no-run`.

---

## Tool-surface evaluation

`tests/tool_surface_evaluation.rs` runs a deterministic tool-selection regression over the labeled corpus in `tests/fixtures/tool_surface/cases.json` (43 fixtures across generic web, repo, fetch, exact-error, security, research, evidence, diagnostics, and ambiguity categories; every stable tool is an expected primary at least once). The runner scores each query against the live tool definitions, then asserts description, total, instructions, and compact byte budgets, top-1/recall@3/MRR thresholds, forbidden-primary exclusion, known-tool and next-action reference validity, and synthetic Layer 2 hydration mechanics (next actions stay within the ten stable tools and compact discovery stays below full definitions). The baseline fingerprint and per-category accuracy print with `-- --nocapture` or `make eval-tool-surface`:

```bash
cargo test --locked --all-features --test tool_surface_evaluation
cargo test --locked --all-features --test tool_surface_evaluation -- --nocapture
make eval-tool-surface
```

`tests/tool_surface_live.rs` keeps the Layer 3 report contract in CI (fingerprint, config, deltas) and holds one `#[ignore]`d manual multi-model comparison entry point driven by `EGGSEARCH_EVAL_MODEL`:

```bash
cargo test --locked --all-features --test tool_surface_live -- --ignored
```

## Transport, egress, and browser suites

`tests/mcp_http.rs` covers the loopback Streamable HTTP lifecycle: health identity, current and legacy protocol flows, shared tool definitions, request hardening, session cleanup, and shutdown behavior. It runs under `--all-features`:

```bash
cargo test --locked --all-features --test mcp_http
```

`tests/egress_routing.rs` covers the opt-in proxy-chain route: config validation (including IPv6 literals), credential redaction, dialer and SSRF gate, deterministic HTTP/SOCKS/multi-hop composition, pool reuse, cancellation and deadline handling, HTTPS TLS/SNI, gzip/Brotli decode with body limits, authenticated proxies with credential non-forwarding, malformed-proxy fail-closed, and routed redirects. Proxy composition tests sit under the `egress` feature; the shape and validation tests run without it. `tests/egress_qualify_contract.rs` pins the egress qualification CI contract (exact target-set equality, per-target egress check, non-publishing, MSRV 1.89, route-seam trigger and symbol coverage) and is mirrored by `packaging/check-egress-qualify-contract.sh` inside `make packaging-check`.

Browser suites are gated by the `browser` feature:

| Suite | Focus |
|-------|-------|
| `browser_profiles` | Profile management: opaque profile IDs, never display names, explicit-path validation (`ExplicitPathInvalid` with no auto-discovery fallback) |
| `browser_transport` | Transport orchestration: startup, navigation, timeouts, shutdown |
| `browser_live_smoke` | Live browser tests; ignored by default, manual opt-in only |

```bash
cargo test --locked --features browser --test browser_profiles
cargo test --locked --features browser --test browser_transport
```

## Hygiene and packaging contract

`make hygiene` runs the deterministic `packaging/check-repo-hygiene.sh`: no forbidden transcript or log artifacts, no ANSI escapes in unexpected root text artifacts, no tracked build outputs (`target/`, `node_modules/`), no oversized unexpected root blobs over 100 KiB (allowlist: `Cargo.lock`, `CHANGELOG.md`, `README.md`, `AGENTS.md`, `LICENSE`), and no tracked editor or temp files. `make packaging-check` runs `packaging/check-contract.sh`, which keeps `packaging/release-targets.txt`, `packaging/release-inputs.txt`, the release workflow, the egress qualification matrix, installers, the updater, and the install docs synchronized. Installer behavior itself is exercised by `packaging/test-install.sh` (Unix) and `packaging/test-install.ps1` (Windows), not by `cargo test`.

`tests/static_guards.rs` enforces module ownership fail-closed (MCP tools call the `MetadataSearchAdapter`, never engines directly; new tools live in dedicated modules under `src/mcp/tools/` and register in `src/mcp/server.rs`). `tests/schema_identity_registry.rs` pins schema and deterministic-ID fixtures so contract drift fails loudly.

```bash
make hygiene
make packaging-check
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features --test schema_identity_registry
```

## Canonical commands

`make check` is the canonical gate:

```bash
make check  # fmt + clippy + no-default check + all-features tests + hygiene + packaging-check
```

Target breakdown from the `Makefile`:

| Target | Command |
|--------|---------|
| `fmt` | `cargo fmt --check` (CI fails on violations) |
| `clippy` | `cargo clippy --locked --all-targets --all-features -- -D warnings` (zero warnings) |
| `feature-check` | `cargo check --locked --no-default-features` (compile-only) |
| `test` | `cargo test --locked --all-features` |
| `hygiene` | `./packaging/check-repo-hygiene.sh` |
| `packaging-check` | `./packaging/check-contract.sh` |

Release adds documentation, build, and publish gates:

```bash
make release-check  # check + docs + release build + publish dry-run
```

which runs `release-candidate-check`, `docs-check` (`RUSTDOCFLAGS="-D warnings" cargo doc`), `release-build`, and `publish-check` (`cargo publish --dry-run`). MSRV is pinned by `rust-version = "1.89"` in `Cargo.toml`; edition 2021.

Individual suites:

```bash
cargo test --locked --features mock --test web_search_integration
cargo test --locked --features mock --test web_fetch_integration
cargo test --locked --features mock --test provider_routing
cargo test --locked --features mock --test repo_workflow
cargo test --locked --features mock --test research_workflow
cargo test --locked --features mock --test security_workflow
cargo test --locked --features mock --test evidence_contract
cargo test --locked --features mock --test corpus_runner
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test provider_probe_conformance
cargo test --locked --all-features --test extract_fetch_contract
cargo test --locked --all-features --test batch_fetch_retrieval
cargo test --locked --all-features --test property_sanitize
cargo test --locked --all-features --test property_retrieval
cargo test --locked --all-features --test adversarial_corpus
cargo test --locked --all-features --test tool_surface_evaluation
cargo test --locked --all-features --test tool_surface_live -- --ignored
cargo test --locked --all-features --test mcp_http
cargo test --locked --all-features --test fetch_safety
cargo test --locked --all-features --test forge_adapter
```

Suite startup and client-integration rendering have their own entry points outside `tests/`:

```bash
cargo test --locked --all-features startup::tests
eggsearch startup instructions
eggsearch startup status --json
eggsearch integrate list --json
eggsearch integrate codex --transport stdio
```

Keep `docs/test-inventory.md` and `skills/eggsearch-dev/SKILL.md` in sync when adding or renaming suites.

---

[← Back to Overview](overview.md)
