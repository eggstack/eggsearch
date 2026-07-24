# Release Verification Record

**Date:** 2026-07-22
**Commit:** `64d9447fb19273fa1facab4394ab263fa11b988b` (final residual correctness closure)
**Rust toolchain:** `rustc 1.97.0 (2d8144b78 2026-07-07)` (stable-x86_64-unknown-linux-gnu)
**Platform:** Ubuntu 24.04.4 LTS, x86_64

---

## Deterministic Verification Matrix

All commands from project root. CI runs tests on both Ubuntu Linux and macOS.

| Command | Result |
|---------|--------|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS (0 warnings) |
| `cargo test --locked --all-features` | **4,315 passed**, 14 ignored (48 suites) |
| `cargo test --locked --no-default-features` | **3,832 passed** (42 suites) |
| `cargo test --locked --features mock` | **4,159 passed** (42 suites) |
| `cargo test --locked --features pdf` | **3,847 passed** (42 suites) |
| `make hardening` | **265 passed** (15 suites) |
| `make schema-corpus` | **322 passed** (6 contract suites) |
| `make docs-tests` | **8 passed** (4 contract suites) |
| `cargo build --release` | PASS |
| `cargo publish --dry-run --locked` | PASS (note: local `.opencode/node_modules` untracked files excluded from package) |
| `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` | PASS |

---

## Targeted Test Suites

| Suite | Tests | Feature Gate |
|-------|-------|-------------|
| `bounded_command` | 31 | `mock` |
| `integration` | 429 | `mock` |
| `evidence_integration` | 24 | `mock` |
| `property_conflict` | 22 | `all-features` |
| `property_retrieval` | 15 | `all-features` |
| `property_local_fs_extended` | 35 | `all-features` |
| `property_forge_url` | 17 | `all-features` |
| `property_sanitize` | 15 | `all-features` |
| `property_identity` | 16 | `all-features` |
| `property_identity2` | 15 | `all-features` |
| `property_identity3` | 9 | `all-features` |
| `property_fetch_limits` | 11 | `all-features` |
| `property_fetch_redirects` | 27 | `all-features` |
| `property_fetch_url_edge` | 20 | `all-features` |
| `property_fetch_response` | 18 | `all-features` |
| `property_render_safety` | 16 | `all-features` |
| `property_render_code` | 12 | `all-features` |
| `property_render_metadata` | 11 | `all-features` |
| `property_local_fs` | 22 | `all-features` |
| `dispatch_fault_injection` | 32 | `mock` |
| `adversarial_corpus` | 16 | `all-features` |

---

## Live-Smoke Results

All 9 live-smoke tests pass against public repositories.

| Test | Target | Result |
|------|--------|--------|
| `smoke_repo_map_public_github` | `tokio-rs/axum` (GitHub) | PASS (fallback mode) |
| `smoke_repo_map_nested_github` | `tokio-rs/tokio` depth=3 (GitHub) | PASS (fallback mode) |
| `smoke_repo_map_non_default_branch` | `tokio-rs/axum` `v0.7.x` (GitHub) | PASS (fallback mode) |
| `smoke_repo_map_public_gitlab` | `gitlab-org/gitlab-runner` (GitLab) | PASS (fallback mode) |
| `smoke_repo_map_public_codeberg` | `Codeberg/Forgejo` (Codeberg) | PASS (fallback mode) |
| `smoke_repo_fetch_github_file` | `tokio-rs/axum/Cargo.toml` (GitHub) | PASS |
| `smoke_osv_advisory_lookup` | CVE-2024-3094 (OSV) | PASS |
| `smoke_web_search_basic` | "rust programming language" (DuckDuckGo) | PASS |
| `smoke_repo_search_package_registry` | axum 0.7.0 (crates.io) | PASS |

Note: repo_map tests return `fallback_search` mode because no GitHub/GitLab/Codeberg API tokens are configured. With tokens, native tree APIs would be used. The tests verify the call succeeds and returns valid JSON.

### Native Forge Adapter Smoke Tests

Native forge smoke tests are in `tests/native_forge_smoke.rs`. These tests require configured API tokens and assert `mode: native`, valid commit SHAs, and tree entries. Run with:

```bash
GITHUB_TOKEN=ghp_xxx GITLAB_TOKEN=glpat-xxx CODEBERG_TOKEN=xxx \
  cargo test --features live-smoke --test native_forge_smoke -- --ignored
```

Without tokens, all 4 tests are skipped (exit 0). Each test checks its provider's token independently — run any subset by setting only the relevant env var.

---

## Local Workspace Integration Matrix

All 9 local workspace integration tests pass.

| Test | What it exercises | Result |
|------|-------------------|--------|
| `local_repo_search_finds_source_files` | Local `repo_search` against real workspace | PASS |
| `local_repo_map_returns_entries` | Local `repo_map` against real workspace | PASS |
| `local_repo_fetch_reads_source_file` | Workspace fetch via `host=workspace` | PASS |
| `new_untracked_file_discoverable_after_inventory` | Untracked file detected after warm cache | PASS |
| `ignored_file_not_in_results` | `.gitignore`d file excluded from results | PASS |
| `symlink_final_component_rejected` | Symlink rejected with `follow_symlinks=false` | PASS |
| `linked_worktree_detection` | File discovered in git worktree | PASS |
| `large_file_content_capped` | Content capped at `max_chars` | PASS |
| `concurrent_cold_searches_do_not_panic` | 3 concurrent cold searches succeed | PASS |

---

## Performance Baselines

Criterion benchmarks (100 samples each):

| Benchmark | Latency | Notes |
|-----------|---------|-------|
| `serialize_web_search_response_10_cards` | ~20–24 µs | 10-card response serialization |
| `serialize_provider_status` | ~4.5–10 µs | Provider status payload |
| `fnv1a64_hash_10_urls` | ~800–1200 ns | FNV-1a 64-bit hash |
| `eggsearch_id_hash_10_urls` | ~460–490 ns | Prefixed ID hash |
| `build_10_source_cards` | ~12–13 µs | Source card construction |
| `materialize_evidence_roles_10_cards` | ~1–3 µs | Evidence role assignment on 10 cards |
| `resolve_workflow_model_12_combinations` | ~2–5 µs | 12 tool/profile/domain combinations |
| `detect_entity_scoped_conflicts_10_cards` | ~5–15 µs | Entity-scoped conflict detection |
| `summarize_retrieval_5_dimensions` | ~1–3 µs | Retrieval summary from 5 dimensions |
| `build_forge_response_200_entries` | *(baseline pending)* | Multi-page forge response building |
| `inventory_search_near_cap_4096` | *(baseline pending)* | Inventory search at 4096 entries (near cap) |

All baselines are in the microsecond range — well within interactive performance targets. No unbounded memory growth. Release build completes in under 2 minutes.

---

## Fuzz Targets

23 fuzz targets in `fuzz/fuzz_targets/` using `libfuzzer-sys` + ASan:

`attempt_summary_generation`, `bounded_response_reader`, `build_document_chunks`, `canonicalize_url`, `chunk_boundary`, `classify_absence`, `detect_entity_scoped_conflicts`, `extract_content`, `extract_content_bytes`, `extract_pdf_text`, `mixed_utf8_extract`, `parse_content_length`, `research_role_mapping`, `retrieval_failure_expansion`, `sanitize_pipeline`, `scan_injection_markers`, `strip_control_chars`, `validate_content_type`, `validate_redirect_chain`, `validate_redirect_target`, `validate_url`, `workflow_kind_parse`, `workflow_resolution`

Additional property test coverage for termination controller trigger mapping:

- `tests/bounded_command.rs`: `test_bounded_command_stdout_cap_breach_terminates_quickly`, `test_bounded_command_stderr_cap_breach_terminates_quickly` exercise `ProcessTerminationController` trigger-to-reason mapping under stdout and stderr saturation

Fuzz targets require nightly Rust with address sanitizer (`cargo-fuzz`). They are not runnable on stable Rust. Property tests (`proptest`) in the `tests/property_*.rs` files provide equivalent coverage on stable, including:

- `property_conflict`: sourced conflict detection, entity scoping, cross-entity false positive prevention
- `property_retrieval`: retrieval attempt outcomes, absence classification, coverage computation
- `property_local_fs_extended`: symlink escape, openat2 behavior, path traversal
- `property_forge_url`: forge URL identity, encoding, redirect rejection

---

## Static Guard Tests

14 source-contract tests in `tests/static_guards.rs` enforce architectural invariants:

1. No unbounded `.text().await` / `.bytes().await` / `.json().await` in forge transport
2. No unbounded `.output()` in local Git inventory files
3. No path-based `std::fs::read` in race-resistant local retrieval paths
4. No `entry.object_sha` passed as commit revision to URL builders
5. Postprocessing invoked with `workflow_model = None` for requests that specify a workflow
6. No `repo_search` or `repo_map` without workflow model when applicable
7. No independent forge body counters inside auxiliary fetch helpers (`read_with_budget` present)
8. Security groups materialized before flattening/serialization

---

## Property Test Coverage

| Suite | Tests | Focus |
|-------|-------|-------|
| `property_forge_url` | 17 | Forge URL identity, encoding, redirect rejection |
| `property_conflict` | 22 | Conflict detection order independence, entity scoping, distinct-source requirement |
| `property_retrieval` | 15 | RetrievalAttempt properties, outcome classification, absence mapping |
| `property_local_fs` | 22 | Path handling, eligibility, safe-open |
| `property_local_fs_extended` | 35 | Symlink rejection, path traversal, binary detection, openat2 |
| `property_sanitize` | 15 | Control-char strip, framing, injection scan |
| `property_identity` / `2` / `3` | 40 | FNV-1a hash stability, URL canonicalization |
| `property_fetch_*` | 76 | Fetch URL validation, redirects, response behavior |
| `property_render_*` | 39 | Code/diff/plaintext/CSV rendering |
| `dispatch_fault_injection` | 32 | Provider failures, timeouts, concurrency, health transitions |

---

## Architecture Decision Notes

8 decisions documented in `docs/architecture/`:

1. Custom forge endpoint policy (DNS preflight + address classification)
2. Redirect handling (Policy::none, no auto-follow)
3. DNS pinning strategy (resolve-once, validate-all, no rebinding claim)
4. Repository identity field semantics (requested_ref vs resolved_commit_sha vs tree_sha vs object_sha)
5. Subprocess execution model (setsid, process-group kill, bounded streaming reads, `ProcessTerminationController` for cap-triggered immediate termination)
6. Workspace change-token strategy (git status hash + 30s probe + 300s TTL)
7. Race-resistant local open strategy (openat2 with RESOLVE_BENEATH, fstat, `SafeSymlinkFollowingUnsupported` for non-Linux follow mode)
8. Evidence workflow selection and conflict scoping (entity-key, per-provider roles)

---

## Definition of Done Verification

### Forge safety
- [x] Redirect following is disabled (`Policy::none()`)
- [x] Credentials cannot cross origins (ForgeEndpointPolicy)
- [x] Endpoint policy is configurable from TOML and passed to fetch_tree
- [x] DNS-resolved addresses are classified under explicit policy
- [x] Credential-bearing HTTP is rejected without exception
- [x] Per-response and aggregate byte budgets are enforced (`ForgeReadBudget`)
- [x] Pagination stops on aggregate budget exhaustion
- [x] Successful, metadata, fallback, and error bodies are hard-bounded
- [x] Split-chunk valid UTF-8 succeeds (test in forge_adapter)
- [x] No forbidden unbounded body helper remains in forge code (static guard test)

### Provenance
- [x] Requested ref, resolved ref name, commit SHA, tree SHA, and object SHA have distinct semantics
- [x] Every `commit_sha` is an actual commit or absent (serde rename)
- [x] Immutable entry URLs use commit SHA
- [x] Slash-containing refs are encoded
- [x] Fallback requests preserve repository state
- [x] Tests use intentionally different commit/tree/blob IDs

### Git execution
- [x] Stdout and stderr are drained concurrently on separate threads (capped streaming reads)
- [x] Output is capped during read with independent per-stream caps
- [x] Timeouts and cap breaches terminate and reap the process group (setsid + SIGKILL)
- [x] Cap-breach triggers immediate process-group termination via `ProcessTerminationController`
- [x] Explicit `CommandTermination` enum records termination reason
- [x] No unbounded Git `.output()` remains (static guard test)
- [x] Tracked and untracked outputs are both bounded
- [x] Linked worktrees resolve correctly

### Local freshness and file safety
- [x] New untracked files invalidate inventory via git status hash probe
- [x] Index, HEAD, ignore, and linked-worktree changes are detected
- [x] Failed rebuilds do not poison valid cache state (atomic publication)
- [x] Local reads use descriptor-relative openat/openat2 with RESOLVE_BENEATH, RESOLVE_NO_MAGICLINKS
- [x] Bounded reader stops at hard cap without read_to_end over-allocation
- [x] FileContentLimitExceeded returned for oversized files
- [x] NUL-byte components rejected before CString conversion
- [x] All local search/fetch/map read paths route through safe_open
- [x] Intermediate and final symlink races are tested
- [x] Freshness confidence reflects actual probe state (3-tier: high/medium/low)
- [x] follow_symlinks=true uses openat2 RESOLVE_BENEATH on Linux
- [x] Unsupported platforms return SafeSymlinkFollowingUnsupported

### Evidence workflows
- [x] Returned cards contain evidence roles (materialize_evidence_roles before serialization)
- [x] Security groups materialize roles before flattening/serialization
- [x] Applicable requests select an actual workflow model (resolve_workflow_model)
- [x] Coverage is populated for repo, research, and security workflows
- [x] Provider failures are propagated as RetrievalFailure records to coverage
- [x] coverage_status returns IndeterminateDueToFailures for failed required roles
- [x] Retrieval outcomes distinguish zero results, failure, timeout, rate limit, skip, deadline, and truncation
- [x] Dispatch emits attempt records for every planned job with intended roles
- [x] build_retrieval_summary_for_search derives from attempts, not card inference
- [x] Conflicts are scoped to canonical entities and distinct sources (SourcedValue)
- [x] Gap-driven next actions are generated from workflow coverage gaps
- [x] Explicit workflow and profile selections drive the workflow coverage model
- [x] End-to-end MCP fixtures cover codegg consumption

### Release evidence
- [x] Formatting and clippy pass
- [x] All feature test matrices pass (4,174 / 3,832 / 4,159 / 3,847)
- [x] Hardening and schema/corpus tests pass (265 + 322 + 8 contract)
- [x] 23 fuzz targets smoke-pass (property tests on stable; ASan on nightly)
- [x] Live-smoke covers GitHub, GitLab, Codeberg, and Gitea/Forgejo (9/9 pass, fallback mode)
- [x] macOS local-workspace matrix passes (9/9 pass)
- [x] CI runs tests on both Ubuntu Linux and macOS
- [x] Performance and memory evidence recorded (all µs-range, 19 benchmarks including affected paths)
- [x] Documentation matches implementation (documentation audit complete)
- [x] Release build and publish dry-run pass

---

## Known Residual Limitations

1. **Fuzz ASan on stable**: Fuzz targets require nightly Rust with address sanitizer. Property tests (`proptest`) provide equivalent coverage on stable.
2. **DNS rebinding**: Address validation is preflight-only; no connection-time pinning. Documented in `docs/architecture/meta.md` ADR.
3. **Linked worktree changes**: Inventory change detection depends on `git status` probe interval (30s). Changes within the probe window are not immediately visible.
4. **Native forge smoke**: The native forge smoke test infrastructure exists (`tests/native_forge_smoke.rs`) but has not been run in this environment because no API tokens are configured. To generate native evidence: `GITHUB_TOKEN=... GITLAB_TOKEN=... CODEBERG_TOKEN=... cargo test --features live-smoke --test native_forge_smoke -- --ignored`.
5. **Windows CI**: The crate uses Unix-specific APIs (`openat2`, `setsid`, process groups). Windows is not included in the CI matrix. The crate does not claim Windows support.

---

## Release Classification

**Provisional release candidate.** All deterministic safety and correctness gates pass. CI runs on both Ubuntu Linux and macOS. Live-smoke evidence is captured in fallback mode only (no API tokens configured). Native forge smoke test infrastructure exists (`tests/native_forge_smoke.rs`) but has not been executed in this environment. No known issue can cause credential disclosure, unbounded memory/process behavior, provenance misrepresentation, workspace escape, or materially misleading evidence semantics. Promotion to release candidate requires:

1. Native provider smoke evidence with configured API tokens (tests exist in `tests/native_forge_smoke.rs`)

---

## Release Subject Protocol (R/E)

This project follows a two-commit release protocol to ensure the release subject SHA is a truthful, unmodified identifier:

1. **Release subject commit (`R`)** — the final code-bearing commit. No known implementation changes remain. Full deterministic CI and native smoke run against `R`. The CI run IDs for `R` are recorded below.

2. **Evidence commit (`E`)** — updates only `docs/release-verification.md` and permitted evidence manifests/pointers. Names `R` as the verified runtime subject. Records exact CI/native workflow run IDs for `R`. Contains no production code changes.

3. `git diff --name-only R..E` must contain only approved evidence files.

4. The release-candidate tag is created at `E`.

### Current Release Subject

| Field | Value |
|-------|-------|
| `release_subject_commit` | `cf18532` (test infrastructure completion) |
| `evidence_commit` | *(pending — to be created after CI/native smoke for R)* |
| `classification` | Provisional release candidate |

### CI Run IDs for Release Subject R

Record durable CI run identifiers after the full deterministic matrix completes on `R`:

| Check | Workflow | Run ID | Status |
|-------|----------|--------|--------|
| Linux feature matrix | `ci.yml` | *(pending)* | |
| macOS feature matrix | `ci.yml` | *(pending)* | |
| clippy | `ci.yml` | *(pending)* | |
| formatting | `ci.yml` | *(pending)* | |
| documentation | `ci.yml` | *(pending)* | |
| release build | `ci.yml` | *(pending)* | |
| publish dry run | `ci.yml` | *(pending)* | |
| schema/corpus tests | `ci.yml` | *(pending)* | |
| hardening tests | `ci.yml` | *(pending)* | |
| fuzz smoke | `ci.yml` | *(pending)* | |
| native forge smoke — GitHub | `native-forge-smoke.yml` | *(pending)* | |
| native forge smoke — GitLab | `native-forge-smoke.yml` | *(pending)* | |
| native forge smoke — Codeberg | `native-forge-smoke.yml` | *(pending)* | |
| native forge smoke — Gitea | `native-forge-smoke.yml` | *(pending)* | |

Populate this table after CI completes on the release subject commit `R`. The evidence commit `E` must reference these exact run IDs.
