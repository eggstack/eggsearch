# AGENTS.md

## Project Overview

eggsearch is a lightweight MCP (Model Context Protocol) search/fetch server for AI agents. It queries upstream search providers, deduplicates results with reciprocal rank fusion, returns compact source cards, and fetches explicit HTTP(S) URLs on demand with bounded text extraction. The shipped generic default providers are DuckDuckGo, Startpage, and Yahoo; other built-in providers such as Brave, SearXNG, GitHub/GitLab/Gitea code/issues/releases, OSV, local workspace search, security advisory databases (GitHub Advisory, NVD, CISA KEV, RustSec), package registries (crates.io, PyPI, npm, Go Proxy, Maven Central, NuGet, RubyGems, Packagist), scholarly search (OpenAlex, Crossref, Semantic Scholar), and Sourcegraph code search are available when configured. Transport is MCP over stdio.

## Build & Verification

All commands from project root. CI pins **Rust 1.88** (`rust-version = "1.88"` in Cargo.toml). **Run `make check` to replicate the full CI suite locally.**

```bash
# Full CI gate (fmt + clippy + all tests + schema-corpus + docs + publish-check)
make check

# Individual targets
cargo fmt --check            # format check (CI fails on this)
cargo clippy --all-targets --all-features -- -D warnings  # zero warnings required
cargo test --locked --all-features    # all tests
cargo test --locked --no-default-features  # no-default compilation + tests
cargo test --locked --features mock  # mock feature tests (all)
cargo test --locked --features pdf   # pdf feature tests (all)
cargo build --release        # release build
cargo publish --dry-run --locked  # pre-publish check
```

**Critical: Integration/corpus tests require `--features mock`.** Running `cargo test` without features misses most integration tests. The CI runs tests across 4 feature combos: `--all-features`, `--no-default-features`, `--features mock`, `--features pdf`.

## Project Structure

Single library + binary crate (not a workspace). Submodules under `src/`:

```
src/
  main.rs          # binary entry point (clap, tokio main)
  lib.rs           # library root, re-exports core/meta/fetch/mcp
  config.rs        # CLI config loader
  commands/        # subcommands: doctor, search, providers, mcp, fetch
  core/            # types, config, error, query, sanitize, identity, warning
    evidence_role.rs   # unified evidence role taxonomy
    workflow_coverage.rs # workflow coverage model and WorkflowResolutionContext
    conflict.rs        # contradiction and conflict metadata (SourcedValue, ConflictEntityKey)
    retrieval_status.rs # failure and absence semantics (RetrievalAttempt)
    evidence_postprocess.rs # Phase 5 response integration: roles, coverage, conflicts, retrieval summaries
    local.rs          # centralized path-component policy (hidden, SKIP_DIRS, binary, symlinks, size)
  meta/            # MetadataSearchAdapter + vendored engines + forge tree adapter + local inventory cache
    forge_adapter.rs  # forge API client with bounded response reading, endpoint safety validation
    local_backend.rs  # local search backend with auto-build inventory on first search
    local_inventory_cache.rs # git fast path with bounded command runner
    safe_open.rs      # race-resistant file opening via openat2 with RESOLVE_BENEATH
  fetch/           # HTTP fetch client, HTML rendering, extraction, span selection
  mcp/             # MCP server (rmcp), tool definitions, server state
tests/             # integration, corpus, and contract tests
docs/
  architecture/     # deep dives: overview, core, meta, fetch, mcp, commands, testing, codegg-contract
  quickstart-codegg.md # harness integration quickstart
  codegg-integration.md # full harness integration guide
  config.md          # config defaults, provider enablement, provider_status semantics
  safety.md          # trust model, fetch safety, metadata_only behavior
  threat-model.md    # operator threat model, trust boundaries
  provider-setup.md  # provider configuration guide
  agent-workflows.md # recommended tool call sequences and recipe catalog
  tool-matrix.md     # compact tool reference table
  release.md         # authoritative release process and pre-release command sequence
  release-checklist.md # short operational checklist (links to release.md)
  release-verification.md # provisional R/E native-evidence record
plans/               # historical roadmap and phase documentation (archived, not actively maintained)
```

Read `src/lib.rs` for the module map, then explore submodules as needed.

## CI Pipeline

| Job | What it runs |
|-----|-------------|
| **check** | `cargo check` × 4 feature combos |
| **test** | `cargo test --locked` × 4 feature combos |
| **clippy** | `cargo clippy --all-targets --all-features -- -D warnings` |
| **schema-corpus** | 6 regression test binaries: `schema_identity_registry`, `fetch_safety`, `security_applicability_corpus`, `research_evidence_corpus`, `recipes_next_actions`, `evidence_bundle_handoff` |
| **docs-contract** | Documentation and source-contract tests, including workflow/release guards |
| **benchmarks** | `cargo bench --locked --all-features --bench perf --no-run` |
| **fmt** | `cargo fmt --check` |
| **release-build** | `cargo build --release` |
| **publish-check** | `cargo publish --dry-run --locked` |
| **hardening** | Property tests (sanitize, identity, fetch limits, fetch redirects, fetch URL edges, render safety, render code, local FS), dispatch fault injection, and adversarial corpus validation |
| **docs** | `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` |

## Feature Flags

| Flag | Purpose |
|------|---------|
| `mock` | Test-only mock engine harness (`src/meta/mock.rs`) — **required for integration/corpus tests** |
| `pdf` | PDF text extraction via `lopdf` |
| `live-smoke` | Live network smoke tests (implies `mock`); ignored by default |

Tests MUST NOT require network access. Run live smoke tests via: `cargo test --features live-smoke --test corpus_runner -- --ignored`.

## Testing

### Test file locations

| Location | Feature gate | Purpose |
|----------|-------------|---------|
| `src/*/mod.rs` `#[cfg(test)]` | Varies | Unit tests for internal logic |
| `tests/integration.rs` | `mock` | MCP tool contracts, error handling, provider behavior |
| `tests/corpus_runner.rs` | `mock` | Multi-step workflow regression |
| `tests/schema_identity_registry.rs` | None | Schema + deterministic ID fixtures |
| `tests/fetch_safety.rs` | None | Render/sanitization safety |
| `tests/security_applicability_corpus.rs` | `mock` | Security applicability pipeline |
| `tests/security_applicability_regression.rs` | None | Range evaluation boundary regressions |
| `tests/security_applicability_phase8.rs` | None | Defensive output verification |
| `tests/research_evidence_corpus.rs` | `mock` | Research evidence regression |
| `tests/recipes_next_actions.rs` | `mock` | Workflow hint generation |
| `tests/evidence_bundle_handoff.rs` | None | Evidence bundle packaging |
| `tests/docs_config_snippets.rs` | None | TOML snippet validation against AppConfig |
| `tests/docs_provider_inventory.rs` | None | Provider ID validation against KNOWN_PROVIDER_IDS |
| `tests/docs_tool_names.rs` | None | Tool name validation against MCP tools |
| `tests/docs_safety_vocabulary.rs` | None | Safety vocabulary validation |
| `tests/config_validation.rs` | None | Config deserialization, validation, and provider resolution |
| `tests/property_sanitize.rs` | None | Property tests for sanitize module |
| `tests/property_identity.rs` | None | Property tests for identity module (source_id, canonicalize_url) |
| `tests/property_identity2.rs` | None | Property tests for identity module (fetch, suggested, batch, doc IDs) |
| `tests/property_identity3.rs` | None | Property tests for identity module (chunk, code_span, locator IDs) |
| `tests/property_fetch_limits.rs` | None | Property tests for fetch URL validation |
| `tests/property_fetch_redirects.rs` | None | Property tests for fetch redirect/credential/TLD/IP validation |
| `tests/property_fetch_url_edge.rs` | None | Property tests for URL scheme/path/length edge cases |
| `tests/property_fetch_response.rs` | None | Property tests for fetch response behavior (metadata-only, byte/char limits, credentials, redirect limit, stream errors) |
| `tests/property_render_safety.rs` | None | Property tests for sanitize module (strip_control_chars, bound_text, frame) |
| `tests/property_render_code.rs` | None | Property tests for code/diff/plaintext/CSV renderers |
| `tests/property_render_metadata.rs` | None | Property tests for TrustMarkers consistency and outline-reference bounds |
| `tests/property_local_fs.rs` | None | Property tests for filesystem path handling and scoring |
| `tests/property_local_fs_extended.rs` | None | Property tests for symlinks, path traversal, skip dirs, root containment |
| `src/meta/local_inventory_cache.rs` `#[cfg(test)]` | None | Unit tests for inventory building, invalidation, git fast path |
| `tests/dispatch_fault_injection.rs` | `mock` | Provider failure, timeout, hang, dedup, concurrency, health transitions tests |
| `tests/adversarial_corpus.rs` | None | Adversarial corpus structural validation |
| `tests/corpus/adversarial/*.json` | None | Malformed/edge-case input corpora (271+ cases across 9 files) |
| `tests/forge_adapter.rs` | None | Forge adapter unit tests (endpoint validation, nested maps, resolved ref) |

### Running specific suites

```bash
cargo test --locked --features mock --test integration              # integration only
cargo test --locked --features mock --test corpus_runner            # corpus regression
cargo test --locked --all-features --test security_applicability_regression --test security_applicability_phase8  # standalone
make schema-corpus                                         # all contract tests
make docs-tests                                            # documentation contract tests
cargo test --locked --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names --test docs_safety_vocabulary
cargo test --locked --all-features --test property_sanitize --test property_identity --test property_identity2 --test property_identity3 --test property_fetch_limits --test property_fetch_redirects --test property_fetch_url_edge --test property_fetch_response --test property_render_safety --test property_render_code --test property_render_metadata --test property_local_fs --test property_local_fs_extended  # property tests
cargo test --locked --all-features --test dispatch_fault_injection  # dispatch fault injection (requires mock)
cargo test --locked --all-features --test adversarial_corpus  # adversarial corpus validation (117+ cases across 9 files)
make hardening                                              # all hardening tests
```

### Adding tests

- **New file** when testing a distinct subsystem or targeting a specific bug class
- **Extend `integration.rs`** for MCP tool input validation, provider failures, tool response shape
- **Extend `corpus_runner.rs`** for multi-step workflows
- **Unit tests** at bottom of source file for private functions
- Always run `cargo clippy --all-targets --all-features -- -D warnings` after adding
- **Property tests** in `tests/property_*.rs` for pure functions (sanitize, identity, fetch limits, render) using `proptest`
- **Adversarial corpus** in `tests/corpus/adversarial/` for malformed/edge-case inputs; validate structure in `tests/adversarial_corpus.rs`
- **Fault injection** in `tests/dispatch_fault_injection.rs` for provider failures, timeouts, concurrency, and health transitions
- **Fuzz harness** in `fuzz/` using `cargo-fuzz` + `libfuzzer` for URL validation, redirect validation, Content-Type handling, HTML extraction, PDF parsing, sanitization pipeline, document chunking, Content-Length parsing, chunk boundary splitting, mixed UTF-8 extraction, redirect chain validation, and bounded response reader (16 targets)

## Code Conventions

- **No comments** unless explicitly requested
- **Formatter:** `cargo fmt` (standard rustfmt). CI checks `cargo fmt --check`.
- **Linter:** `cargo clippy --all-targets --all-features -- -D warnings` — zero warnings.
- **Error handling:** `core` defines `CoreError`/`CoreResult<T>` via `thiserror`. Adapter returns `WebSearchResponse` (never errors; partial failures are soft). MCP tools return `Result<serde_json::Value, ToolError>`.
- **Deterministic IDs:** SourceCard IDs, suggested fetches, and grouping use content-derived FNV-1a hashes (`src/core/identity.rs`). Never use random IDs for stable output types.
- **Sanitization:** All untrusted text flows through `src/core/sanitize.rs` (3 tiers: control-char strip, framing, injection scan). Production defaults `sanitize_output = true`; tests default to `false`.
- **Forge safety:** Forge API client uses `Policy::none()` redirect policy (redirects rejected). `ForgeEndpointPolicy` controls loopback, private network, and HTTPS requirements. `validate_base_url()` rejects embedded credentials, resolves DNS names to classify all resolved addresses, classifies literal IPv4/IPv6 addresses, and blocks HTTP with API keys. All forge response bodies (tree, metadata, error previews) are read through `read_bounded_body()` with a hard byte cap. Error-body previews use a separate 8KB cap with control-character sanitization. URL path components are percent-encoded for GitHub and Gitea/Forgejo endpoints. `ForgeReadBudget` tracks aggregate bytes across all requests within a single tool invocation (operation-wide, not per-response); pagination stops on aggregate budget exhaustion.
- **Bounded git execution:** `run_bounded_command()` drains stdout and stderr concurrently with independent capped reads, creates a new process group via `setsid()`, and kills the process group on timeout. Cap breaches (stdout or stderr limit exceeded) trigger immediate process group termination. `CommandTermination` enum records the termination reason (`Exited`, `TimedOut`, `StdoutLimitExceeded`, `StderrLimitExceeded`, `SpawnFailed`, `Signaled`). Untracked file counts derive from bounded `git ls-files -z --others` output. `git status`, `git check-ignore`, and other auxiliary Git commands also use bounded execution.
- **Evidence postprocessing:** `evidence_postprocess.rs` populates evidence roles (materialized onto cards), workflow coverage (selected per tool/profile/domain), retrieval summaries, and structured conflicts on all result conversion paths. Rate limiting is classified as provider failure, not policy skip. Security responses include workflow coverage and conflict metadata. All new fields are additive and optional.
- **Attempt outcomes vs absence kinds:** `RetrievalAttemptOutcome` (success, failure, timeout, rate limit, skip, truncation) and `EvidenceAbsenceKind` (no evidence, provider failed, deadline, insufficient, indeterminate, not applicable) are related but distinct. An attempt outcome describes what happened during retrieval; an absence kind describes the impact on evidence coverage. One outcome can map to different absence kinds depending on the workflow model and role requirements.
- **Capability partitioning:** `dispatch_subqueries` deduplicates and partitions intended roles per provider. Supported roles execute in one provider call; unsupported roles receive a separate `SkippedCapabilityUnavailable` attempt. `NotApplicable` is reserved for operations that do not apply to the request, and `SkippedByPolicy` is reserved for deliberate policy or routing suppression.
- **Provider-scoped advisories:** `AdvisoryCapabilities` declares native advisory operations. Scoped lookups return one terminal outcome per selected provider, preserve the executing provider ID, surface errors and deadlines, and never invoke unsupported no-op methods. Native operations honor the request's resolved provider set.
- **Truncation evidence:** Exact candidate-limit saturation without provider metadata is `LimitReachedUnknown`, with `truncated = false` and a separate summary counter. Confirmed truncation requires Eggsearch or provider evidence.

## Key Architecture

- **Single crate:** Library + binary in one package. `src/lib.rs` re-exports `core`, `fetch`, `mcp`, `meta`.
- **Adapter pattern:** `MetadataSearchAdapter` wraps all search engines, handles RRF aggregation, sanitization, and provider health. MCP tools call the adapter, never engines directly.
- **Provider model:** `ProviderKind` enum (`HtmlScrape`, `JsonApi`, `ApiKey`, `Local`). Capability flags are conservative — HTML scrapers report `ProviderCapabilities::none()`.
- **Profiles:** `SearchProfile` (`generic`, `coding`, `security`, `research`) influence provider selection. Profiles are advisory; unavailable providers are skipped with warnings, not errors.
- **Config:** `$XDG_CONFIG_HOME/eggsearch/config.toml`. Root type is `AppConfig` with `SearchSection`, `FetchSection`, and `LocalConfig`.
- **Transport:** MCP over stdio only. Server instructions are in `EGGSEARCH_INSTRUCTIONS` constant in `mcp/server.rs`.
- **Nested repository maps:** `repo_map` returns both `root_entries` (backward-compatible root-only) and `entries` (all retained entries within max_depth). Depth calculation: root = 1, `src/lib.rs` = 2.
- **Local auto-build:** Local workspace inventory is built automatically on first search (auto-build on cache miss). `inventory_truncated` is propagated from inventory roots into search results.
- **Entry revalidation:** `validate_entry()` in `local_inventory_cache.rs` is called before every content read in both inventory and fallback search paths, skipping stale/deleted/oversized entries.
- **Bounded git execution:** `run_bounded_command()` in `local_inventory_cache.rs` enforces timeout (5s), stdout cap (16MB), and stderr cap (64KB) on Git subprocess invocations. Stdout and stderr are drained concurrently using separate threads with independent byte caps. Creates a new process group via `setsid()` and kills the process group on timeout. Cap breaches (stdout or stderr limit exceeded) trigger immediate process group termination via `ProcessTerminationController`. `CommandTermination` enum records the reason: `Exited`, `TimedOut`, `StdoutLimitExceeded`, `StderrLimitExceeded`, `SpawnFailed`, or `Signaled`. Untracked file counts derive from bounded `git ls-files -z --others` output. `git status`, `git check-ignore`, and other auxiliary Git commands also use bounded execution.
- **Freshness confidence:** `FreshnessConfidence` enum (`high`/`medium`/`low`) in `core/local.rs` is computed from inventory age and propagated through `InventoryTelemetry`, `RepoMapResponse`, and `LocalRepoMatch`. Inventory freshness is also checked via a `git status --porcelain=v2` hash (change token) stored alongside the inventory, detecting untracked file creation, staging, branch switches, and ignore-rule changes without waiting for the age-based TTL.
- **Race-resistant local file opening:** `safe_open.rs` provides `safe_open_relative()` which uses descriptor-relative file opening via `openat`/`openat2` with `O_NOFOLLOW`. On Linux, it attempts `openat2` with `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS`, falling back to `openat` with `O_NOFOLLOW` on older kernels. For `follow_symlinks=true` on Linux, uses `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS` (omitting `RESOLVE_NO_SYMLINKS`). On non-Linux Unix platforms, `follow_symlinks=true` returns `SafeSymlinkFollowingUnsupported`. Each path component is opened relative to the parent directory descriptor, eliminating TOCTOU races between validation and open.
- **Evidence workflow selection and conflict scoping:** `resolve_workflow_model()` maps tool name, profile, and research domain to a deterministic `WorkflowCoverageModel` defining required/recommended/optional evidence roles for each of 10 core workflows. `ConflictEntityKey` (entity type + canonical ID + field) provides composite grouping for conflict detection, preventing unrelated sources from being compared. Both are wired into all result conversion paths via `evidence_postprocess.rs`.
- **Semantic research subquery intent:** Research subqueries carry typed `intended_roles` derived from `ResearchSourceType`, not from opaque `rq_*` labels. The planner emits roles that dispatch and postprocessing consume directly.
- **Multi-role failure expansion:** Retrieval failures for research subqueries expand across all `intended_roles` — failure conversion never assumes a single role per subquery.
- **Native security attempt participation:** Native security lookups (CVE/GHSA/OSV/RustSec/KEV) produce `RetrievalAttempt` records that participate in the retrieval ledger alongside web-search results.
- **Conflict source scoping:** Conflict source IDs identify only the disagreeing cards, not entire entity groups.
- **Release evidence R/E protocol:** Release evidence uses a two-commit protocol: release subject commit `R` (code-bearing) and evidence commit `E` (only docs/manifests). `docs/release-verification.md` records both `R` and `E` and the CI run IDs for `R`. Classification remains provisional until native and CI evidence is present.
- **Native smoke tests are distinct from fallback:** Native forge smoke tests (`tests/native_forge_smoke.rs`) exercise the adapter path directly with configured API tokens. Live-smoke fallback tests are diagnostic only. Release evidence requires the manual fail-closed workflow, exact release-subject checkout, required credentials and fixture variables, a passing native assertion, structured evidence, and exact pass from every required provider job.
- **DNS validation is preflight-only:** DNS address classification happens before connection. No connection-time DNS pinning is enforced. Documented in `docs/architecture/meta.md`.
- **Windows is unsupported:** The crate uses Unix-specific APIs (`openat2`, `setsid`, process groups). Windows is not included in the CI matrix and is not claimed as supported.

## MCP Tools (10 total)

`web_search`, `web_fetch`, `batch_fetch`, `provider_status`, `repo_search`, `repo_fetch`, `repo_map`, `security_search`, `research_search`, `build_evidence_bundle`.

Tools are defined in `src/mcp/tools.rs`. The MCP server uses `rmcp` crate with `tool_router` proc macros.

Search tools return `evidence_role` on source cards and `conflict_metadata` when sources disagree.

## Publishing

```bash
make publish-check  # runs cargo publish --dry-run --locked
```

Pre-publish: clippy clean, tests pass, fmt clean, version bumped in Cargo.toml, CHANGELOG.md updated.

The authoritative pre-release command sequence, required CI checks, and
live-smoke policy live in [`docs/release.md`](docs/release.md). The short
operational checklist lives in [`docs/release-checklist.md`](docs/release-checklist.md).
The `Makefile`, `.github/workflows/ci.yml`, and `docs/release.md` are kept in
sync; if you change a flag or command in one, update the others in the same
commit.

## Pitfalls

- **Forgetting `--features mock`** — integration/corpus tests won't compile without it
- **Adding random UUIDs** to stable output types — use FNV-1a hashes via `src/core/identity.rs`
- **Bypassing sanitization** — all untrusted text must flow through `sanitize.rs` or `sanitize_field()`
- **Hardcoding provider lists** — use `resolve_providers()` which validates enabled/known status
- **Changing deterministic IDs** — breaks regression corpus tests and cross-tool deduplication
- **Missing `cargo fmt`** — CI will fail on `cargo fmt --check`
- **Bypassing forge response bounds** — all forge API responses must use `read_bounded_response()`; no `.text().await` or `.bytes().await` without a prior hard bound
- **Changing commit_sha semantics** — `commit_sha` must come from `resolved_ref` (actual commit SHA), not from entry object SHA
- **Using opaque rq_* labels as the sole source of role inference** — research planner now provides typed intended roles via `intended_roles`; do not infer roles from `rq_*` subquery IDs
- **Using .first() on intended_roles for failure conversion** — must expand across all roles when converting retrieval failures
- **Silently discarding native advisory lookup errors** — all lookups (CVE, GHSA, OSV, RustSec, KEV) produce `RetrievalAttempt` records in the retrieval ledger
- **Treating limit saturation as proof of truncation** — use `TruncationEvidence`; `LimitReachedUnknown` does not set `truncated` or `has_truncation`
- **Allowing native smoke skips to promote a release** — missing credentials, fixture refs, malformed evidence, or missing provider outputs must fail the manual release workflow
