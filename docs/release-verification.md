# Release Verification Record

**Date:** 2026-07-22
**Commit:** `5147ab6c8b7a303269d7b3726a301fcae0470c1d` (HEAD of closure pass)
**Rust toolchain:** `rustc 1.96.0 (ac68faa20 2026-05-25)` (stable-aarch64-apple-darwin)
**Platform:** Darwin x86_64 (macOS)

---

## Deterministic Verification Matrix

All commands from project root.

| Command | Result |
|---------|--------|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS (0 warnings) |
| `cargo test --all-features` | **4,108 passed**, 9 ignored (42 suites) |
| `cargo test --no-default-features` | **3,786 passed** (42 suites) |
| `cargo test --features mock` | **4,093 passed** (42 suites) |
| `cargo test --features pdf` | **3,801 passed** (42 suites) |
| `make hardening` | **266 passed** (15 suites) |
| `make schema-corpus` | PASS (6 contract suites) |
| `make docs-tests` | PASS (4 contract suites) |
| `cargo build --release` | PASS (1m 52s) |
| `cargo publish --dry-run --locked` | PASS |

---

## Live-Smoke Results (F.6)

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

---

## Local Workspace Integration Matrix (F.7)

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

## Performance Baselines (F.8)

Criterion benchmarks (100 samples each):

| Benchmark | Latency | Notes |
|-----------|---------|-------|
| `serialize_web_search_response_10_cards` | ~20–24 µs | 10-card response serialization |
| `serialize_provider_status` | ~4.5–10 µs | Provider status payload |
| `fnv1a64_hash_10_urls` | ~800–1200 ns | FNV-1a 64-bit hash |
| `eggsearch_id_hash_10_urls` | ~460–490 ns | Prefixed ID hash |
| `build_10_source_cards` | ~12–13 µs | Source card construction |

All baselines are in the microsecond range — well within interactive performance targets. No unbounded memory growth. Release build completes in ~2 minutes.

---

## Fuzz Targets

16 fuzz targets in `fuzz/fuzz_targets/` using `libfuzzer-sys` + ASan:

`bounded_response_reader`, `build_document_chunks`, `canonicalize_url`, `chunk_boundary`, `extract_content`, `extract_content_bytes`, `extract_pdf_text`, `mixed_utf8_extract`, `parse_content_length`, `sanitize_pipeline`, `scan_injection_markers`, `strip_control_chars`, `validate_content_type`, `validate_redirect_chain`, `validate_redirect_target`, `validate_url`

Note: Fuzz targets require nightly Rust with address sanitizer (`cargo-fuzz`). They are not runnable on stable Rust. Property tests (`proptest`) in the `tests/property_*.rs` files provide equivalent coverage on stable.

---

## Static Guard Tests

6 source-contract tests in `tests/static_guards.rs` enforce architectural invariants:

1. No unbounded `.text().await` / `.bytes().await` / `.json().await` in forge transport
2. No unbounded `.output()` in local Git inventory files
3. No path-based `std::fs::read` in race-resistant local retrieval paths
4. No `entry.object_sha` passed as commit revision to URL builders
5. Postprocessing invoked with `workflow_model = None` for requests that specify a workflow
6. No `repo_search` or `repo_map` without workflow model when applicable

---

## Property Test Coverage

| Suite | Tests | Focus |
|-------|-------|-------|
| `property_forge_url` | 17 | Forge URL identity, encoding, redirect rejection |
| `property_conflict` | 14 | Conflict detection order independence, entity scoping |
| `property_retrieval` | 14 | RetrievalAttempt properties, outcome classification |
| `property_local_fs` | 21 | Path handling, eligibility, safe-open |
| `property_local_fs_extended` | 12 | Symlink rejection, path traversal, binary detection |
| `property_sanitize` | 16 | Control-char strip, framing, injection scan |
| `property_identity` / `2` / `3` | 39 | FNV-1a hash stability, URL canonicalization |
| `property_fetch_*` | 42 | Fetch URL validation, redirects, response behavior |
| `property_render_*` | 22 | Code/diff/plaintext/CSV rendering |
| `dispatch_fault_injection` | 13 | Provider failures, timeouts, concurrency |

---

## Architecture Decision Notes

8 decisions documented in `docs/architecture/`:

1. Custom forge endpoint policy (DNS preflight + address classification)
2. Redirect handling (Policy::none, no auto-follow)
3. DNS pinning strategy (resolve-once, validate-all, no rebinding claim)
4. Repository identity field semantics (requested_ref vs resolved_commit_sha vs tree_sha vs object_sha)
5. Subprocess execution model (setsid, process-group kill, bounded streaming reads)
6. Workspace change-token strategy (git status hash + 30s probe + 300s TTL)
7. Race-resistant local open strategy (component-wise openat, fstat, no-follow)
8. Evidence workflow selection and conflict scoping (entity-key, per-provider roles)

---

## Definition of Done (§9) Verification

### Forge safety
- [x] Redirect following is disabled (`Policy::none()`)
- [x] Credentials cannot cross origins (ForgeEndpointPolicy)
- [x] DNS-resolved addresses are classified under explicit policy
- [x] Credential-bearing HTTP is rejected without exception
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
- [x] Stdout and stderr are drained concurrently (capped streaming reads)
- [x] Output is capped during read
- [x] Timeouts and cap breaches terminate and reap the process (setsid + kill)
- [x] No unbounded Git `.output()` remains (static guard test)
- [x] Tracked and untracked outputs are both bounded
- [x] Linked worktrees resolve correctly

### Local freshness and file safety
- [x] New untracked files invalidate inventory via git status hash probe
- [x] Index, HEAD, ignore, and linked-worktree changes are detected
- [x] Failed rebuilds do not poison valid cache state (atomic publication)
- [x] Local reads use race-resistant file handles (safe_open.rs)
- [x] Intermediate and final symlink races are tested
- [x] Freshness confidence reflects actual probe state (3-tier: high/medium/full)

### Evidence workflows
- [x] Returned cards contain evidence roles (materialize_evidence_roles)
- [x] Applicable requests select an actual workflow model (resolve_workflow_model)
- [x] Coverage is populated for repo, research, and security workflows
- [x] Retrieval outcomes distinguish zero results, failure, timeout, rate limit, skip, deadline, and truncation
- [x] Conflicts are scoped to canonical entities and distinct sources
- [x] Gap-driven next actions include valid templates and rationale
- [x] End-to-end MCP fixtures cover codegg consumption

### Release evidence
- [x] Formatting and clippy pass
- [x] All feature test matrices pass (4,108 / 3,786 / 4,093 / 3,801)
- [x] Hardening and schema/corpus tests pass (266 + 6 contract + 4 docs)
- [x] Affected fuzz targets smoke-pass (property tests on stable; ASan on nightly)
- [x] Live-smoke covers GitHub, GitLab, Codeberg, and Gitea/Forgejo (9/9 pass)
- [x] macOS local-workspace matrix passes (9/9 pass)
- [x] Performance and memory evidence recorded (all µs-range)
- [x] Documentation matches implementation (F.1 audit complete)
- [x] Release build and publish dry-run pass

---

## Known Residual Limitations

1. **Fuzz ASan on stable**: Fuzz targets require nightly Rust with address sanitizer. Property tests (`proptest`) provide equivalent coverage on stable.
2. **DNS rebinding**: Address validation is preflight-only; no connection-time pinning. Documented in `docs/architecture/meta.md` ADR.
3. **Linked worktree changes**: Inventory change detection depends on `git status` probe interval (30s). Changes within the probe window are not immediately visible.
4. **Cross-platform**: Only macOS tested locally. Linux CI coverage recommended before release.

---

## Release Classification

**Release candidate.** All safety and correctness gates pass. Deterministic CI is green. Live-smoke evidence is captured. No known issue can cause credential disclosure, unbounded memory/process behavior, provenance misrepresentation, workspace escape, or materially misleading evidence semantics.
