# eggsearch Architecture Overview

**Version:** 0.3.5 · **Rust edition:** 2021 · **MSRV:** 1.88
**Crate type:** Single library + binary (no workspace)

eggsearch is a lightweight MCP (Model Context Protocol) search/fetch server for AI agents. It queries upstream search providers, deduplicates results with reciprocal rank fusion, returns compact source cards, and fetches explicit HTTP(S) URLs on demand with bounded text extraction. Transport is MCP over stdio only.

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI (main.rs)                        │
│  clap subcommands: doctor | search | mcp | fetch | providers│
└──────────┬──────────────────────────────────┬───────────────┘
           │                                  │
           ▼                                  ▼
┌──────────────────────┐          ┌───────────────────────────┐
│    commands/          │          │    mcp/                    │
│  CLI subcommand impls │          │  MCP server (rmcp, stdio) │
│  doctor, search,      │          │  10 tool handlers          │
│  fetch, providers     │          │  ServerState, Policy       │
└──────────┬───────────┘          └─────────────┬─────────────┘
           │                                    │
           └──────────────┬─────────────────────┘
                          ▼
              ┌───────────────────────┐
              │    MetadataSearchAdapter│ ← meta/
              │    (orchestrator)       │
              │  • engine dispatch      │
              │  • RRF aggregation      │
              │  • provider health      │
              └───────┬───────────────┘
                      │
        ┌─────────────┼─────────────────┐
        ▼             ▼                 ▼
  ┌──────────┐  ┌──────────┐     ┌───────────┐
  │ engines/ │  │ engines/ │     │ engines/  │
  │  HTML    │  │  JSON    │     │  API-key  │
  │ scrapers │  │  APIs    │     │ providers │
  └──────────┘  └──────────┘     └───────────┘

              ┌───────────────────────┐
              │      fetch/            │
              │  FetchClient (reqwest) │
              │  • SSRF protection     │
              │  • HTML extraction     │
              │  • PDF extraction      │
              │  • span selection      │
              └───────────────────────┘

              ┌───────────────────────┐
              │       core/            │
              │  Pure domain types     │
              │  config, error, query  │
              │  identity, sanitize    │
              │  source_card, warning  │
              └───────────────────────┘
```

The four top-level library modules (`core`, `meta`, `fetch`, `mcp`) plus the `commands` binary module form the entire codebase. `core` has zero external dependencies beyond serialization — it defines the canonical data model. `meta` wraps all search engines behind the adapter boundary. `fetch` handles outbound HTTP. `mcp` exposes everything through 10 stable MCP tools.

---

## Module Map

| Module | Path | Responsibility | Deep Dive |
|--------|------|----------------|-----------|
| **core** | `src/core/` | Pure domain types, config model, error types, identity system, sanitization, warnings, source cards, quality heuristics, security/research/repo/local/package/evidence types | [core.md](core.md) |
| **meta** | `src/meta/` | Metasearch adapter + 34 vendored search engines. RRF aggregation, query planning, provider health, result grouping, suggested fetches, local workspace backend | [meta.md](meta.md) |
| **fetch** | `src/fetch/` | HTTP fetch client, HTML content extraction, PDF extraction, span selection, SSRF protection, code-host URL rewriting | [fetch.md](fetch.md) |
| **mcp** | `src/mcp/` | MCP server over stdio (rmcp), 10 tool definitions, shared server state, policy enforcement | [mcp.md](mcp.md) |
| **commands** | `src/commands/` | CLI subcommands: doctor, search, mcp, fetch, providers | [commands.md](commands.md) |
| **testing** | `tests/` | Integration, corpus, schema/contract, and documentation contract tests | [testing.md](testing.md) |

### Subsystem Deep Dives

| Subsystem | Files | Responsibility | Deep Dive |
|-----------|-------|----------------|-----------|
| **config** | `src/core/config.rs`, `src/config.rs` | TOML config model, provider resolution, validation, CLI loading | [config.md](config.md) |
| **engines** | `src/meta/engines/` (38 files) | Vendored search engine implementations, `SearchEngine` trait, result normalization | [engines.md](engines.md) |
| **security** | `src/core/security.rs`, `src/meta/security_*.rs` | Security advisory search, version applicability, remediation, KEV enrichment | [security.md](security.md) |
| **research** | `src/core/research.rs`, `src/meta/research_*.rs` | Research evidence discovery, claims, conflicts, gaps, workflow scaffolding | [research.md](research.md) |
| **evidence & workflow** | `src/core/evidence_*.rs`, `src/core/workflow*.rs`, `src/core/conflict.rs`, `src/core/retrieval_status.rs` | Evidence bundles, role taxonomy, workflow recipes, conflict detection, retrieval tracking | [evidence-workflow.md](evidence-workflow.md) |
| **local workspace** | `src/core/local.rs`, `src/meta/local_*.rs`, `src/meta/safe_open.rs` | Filesystem search, cached inventory, git-aware fast path, race-resistant file opening | [local-workspace.md](local-workspace.md) |

### Supporting Documentation

| Document | Purpose |
|----------|---------|
| [codegg-contract.md](codegg-contract.md) | Stable MCP response contract for harness developers (deterministic IDs, warnings, trust model, next actions, security applicability, research evidence) |
| [hardening.md](hardening.md) | Property testing, adversarial corpus, fuzz harness, crash promotion |
| [../../docs/threat-model.md](../../docs/threat-model.md) | Operator threat model, trust boundaries, prompt-injection handling, configuration escape hatches |
| [../../docs/safety.md](../../docs/safety.md) | Fetch safety, blocked address ranges, sanitization tiers, trust markers |
| [../../docs/config.md](../../docs/config.md) | Config defaults, provider requirements, profile examples |
| [../../docs/tool-matrix.md](../../docs/tool-matrix.md) | Compact tool reference with trust semantics |

---

## MCP Tools (10)

| Tool | Category | Purpose |
|------|----------|---------|
| `web_search` | Search | Live metasearch over configured upstream providers |
| `web_fetch` | Fetch | Bounded extraction of one explicit HTTP(S) URL |
| `batch_fetch` | Fetch | Bounded batch fetch over URLs or repo locators |
| `provider_status` | Utility | Diagnostic report of provider config, health, capabilities, recipes |
| `repo_search` | Search | Structured repository evidence discovery with grouped bundles |
| `repo_fetch` | Fetch | Structured repository file fetch by locator with line ranges and symbols |
| `repo_map` | Fetch | Repository structure discovery (important files and directories) |
| `security_search` | Search | Security vulnerability and advisory search with normalized metadata |
| `research_search` | Search | Research-oriented multi-source evidence discovery with claims and conflicts |
| `build_evidence_bundle` | Utility | Package selected evidence into a portable container for multi-agent handoff |

Tools are defined in `src/mcp/tools.rs`. The MCP server uses `rmcp` with `tool_router` proc macros.

---

## Provider Ecosystem

**34 known providers** across 4 kinds:

| Kind | Examples | Capability |
|------|----------|------------|
| `HtmlScrape` | DuckDuckGo, Startpage, Yahoo, Mojeek | Generic web search (conservative capabilities) |
| `JsonApi` | SearXNG, OSV, NVD, CISA KEV, RustSec, package registries, scholarly | Structured APIs, richer results |
| `ApiKey` | Brave API, GitHub/GitLab/Gitea code/issues/releases, Semantic Scholar, Sourcegraph | Requires authentication, richest results |
| `Local` | local_workspace | Filesystem-based workspace search |

**Capability flags** (24 boolean flags): `supports_safe_search`, `supports_freshness`, `supports_language`, `supports_region`, `supports_domain_filters`, `supports_news`, `supports_code_search`, `supports_repo_filter`, `supports_org_filter`, `supports_path_filter`, `supports_language_filter`, `supports_symbol_hint`, `supports_issue_search`, `supports_release_search`, `supports_result_timestamps`, `supports_security_search`, `supports_package_metadata`, `supports_advisory_lookup_by_id`, `supports_advisory_lookup_by_package`, `supports_exploit_kev_status`, `supports_scholarly_search`, `supports_doi_lookup`, `supports_repo_indexing`, `supports_structured_changelog`. Capability flags are conservative — HTML scrapers report `ProviderCapabilities::none()`.

**4 search profiles** influence provider selection:
- `generic` — broad web search (DuckDuckGo, Startpage, Yahoo)
- `coding` — code-focused (adds GitHub, GitLab, Gitea, Sourcegraph)
- `security` — vulnerability-focused (adds OSV, NVD, CISA KEV, RustSec)
- `research` — scholarly (adds OpenAlex, Crossref, Semantic Scholar)

Profiles are advisory; unavailable providers are skipped with warnings, never errors. Non-routable providers include a machine-readable `skip_code` (13 variants: `unknown_provider`, `disabled_by_user`, `missing_api_key`, `missing_searxng_config`, `missing_base_url`, `invalid_base_url`, `missing_local_backend`, `credential_not_configured`, `credential_env_missing`, `credential_invalid`, `cooldown_active`, `not_built`, `unknown`).

---

## Data Flow

### Search Flow (web_search / repo_search / security_search / research_search)

```
1. Policy check (mode == Live?)
2. Query validation
3. Provider resolution (resolve_providers / resolve_profile_providers)
4. SearchPlan construction (planner.rs / repo_planner.rs / research_planner.rs / error_planner.rs)
5. Bounded parallel dispatch across engines (dispatch.rs)
6. RRF aggregation (adapter.rs)
7. SourceCard construction with deterministic FNV-1a IDs (identity.rs)
8. Sanitization (sanitize.rs) — 3 tiers: control chars, framing, injection scan
9. Quality metadata (quality.rs) — confidence, relevance, authority, freshness, evidence strength
10. Evidence postprocessing (evidence_postprocess.rs) — roles, coverage, retrieval summaries, conflicts
11. Result grouping (grouping.rs / repo_grouping.rs / research_grouping.rs / security_grouping.rs)
12. Suggested fetches (suggested_fetches.rs / research_suggested_fetches.rs / security_suggested_fetches.rs)
13. Fetch ranking (fetch_ranking.rs) — deterministic scoring pipeline
14. Next-action hints (recipe_catalog.rs) — up to 5 hints per response
15. Structured warnings (warning.rs) — 50+ machine-readable codes
```

### Fetch Flow (web_fetch / repo_fetch / batch_fetch)

```
1. Policy check (fetch_enabled?)
2. URL validation (limits.rs) — SSRF, localhost, private-network
3. Code-host URL rewriting (code_host_fetch.rs) — GitHub/GitLab/Codeberg → raw
4. HTTP request (reqwest, FetchClient)
5. Redirect revalidation (each redirect re-checked against SSRF rules)
6. Content detection (detect.rs) — HTML, markdown, code, PDF, plain text
7. Extraction (extract.rs) — text, links (15+ kinds), metadata
8. HTML rendering (render/) — blocks, outline, chunks
9. Span selection (span.rs) — symbol/line-range expansion for repo_fetch
10. Sanitization (sanitize.rs) — 3 tiers
11. Document construction (document.rs) — 16 document kinds
12. Response with trust markers
```

### Local Workspace Flow

```
1. Git worktree discovery (local_inventory.rs)
2. Remote URL normalization + identity matching
3. File inventory construction with caching (local_inventory_cache.rs)
   - Auto-build on first search (cache miss triggers build)
   - Git-aware fast path via `git ls-files -z --cached --others --exclude-standard`
   - Bounded command runner: 5s timeout, 16MB stdout / 64KB stderr caps, concurrent pipe drainage (stdout thread + stderr main), kill-on-timeout watchdog thread
   - Native directory walking fallback
   - XXH3 fingerprinting for change detection
4. Inventory-first search: candidate filtering → bounded content reads → scoring
5. SymbolBackend trait for regex-based symbol matching
6. SourceCard conversion with trust = local_trusted
7. File classification: is_generated, is_vendor, is_test, is_example, is_config, is_lockfile
8. Telemetry: backend used, inventory age, files considered/read, bytes read
9. inventory_truncated propagated from inventory roots into search results
```

---

## Cross-Cutting Concerns

### Deterministic Identity System

All stable output types use FNV-1a 64-bit content-derived hashes, never random UUIDs. This enables cross-tool deduplication and regression testing.

| Entity | Prefix | Key Fields |
|--------|--------|------------|
| Source card | `src_` | provider_id + url + title + source_kind |
| Suggested fetch | `suggested_` | url + group + priority |
| Fetch result | `fetch_` | url + text_prefix |
| Code span | `span_` | url + language + line_start + line_end + symbol |
| Evidence bundle | `bundle_` | goal + source_ids + fetch_ids |
| Locator | `loc_` | host + owner + repo + ref_name + path |
| Document | `doc_` | url + title + kind |
| Document chunk | `chunk_` | doc_id + chunk_index |

URLs are canonicalized before hashing (lowercase scheme/host, strip `www.`, default ports, fragments, normalize percent-encoding). Versioned input prefix: `eggsearch-id-v1\0`.

See [core.md](core.md#identity-system) for details.

### Three-Tier Sanitization

All untrusted text flows through sanitization before reaching agents:

| Tier | When Active | What It Does |
|------|-------------|--------------|
| Tier 1 | Always | Strip control chars (NUL, CR, ASCII controls, bidi, zero-width) + length bound |
| Tier 2 | `sanitize_output = true` | Frame text in `<<<EXTERNAL_UNTRUSTED>>>` delimiters |
| Tier 3 | `sanitize_output = true` | Scan for 7 prompt-injection marker patterns |

See [../../docs/safety.md](../../docs/safety.md) and [core.md](core.md#sanitization) for details.

### Warning System

50+ machine-readable `WarningCode` variants with stable `snake_case` strings, 4 severity levels (info/notice/warning/error), and recommended actions. `WarningAccumulator` deduplicates by `(code, provider_ids, result_ids, source_ids)`.

See [core.md](core.md#warnings) for details.

### Quality Heuristics

Deterministic per-result quality metadata computed from URL/domain heuristics and structured result metadata:
- **Confidence**: High/Medium/Low/Unknown
- **Relevance**: Exact/Strong/Partial/Weak/Unknown
- **Authority**: Primary/Official/Maintainer/PackageRegistry/Community/NewsOrBlog/Unknown
- **Freshness**: Current/Recent/Historical/Stale/Undated/Unknown
- **EvidenceStrength**: ExactCodeSpan/ExactIdentifier/StructuredMetadata/SnippetOnly/UrlOnly/Unknown

See [core.md](core.md#quality-heuristics) for details.

### Trust Model

- `external_untrusted` — All web/remote content. Treat as data, never instructions.
- `local_trusted` — Local workspace content. Provenance-trusted, not instruction-trusted.
- `unknown` — Default to `external_untrusted` behavior.

Three sanitization tiers + trust markers in every response. See [../../docs/threat-model.md](../../docs/threat-model.md) for the full operator threat model.

### Provider Health Tracking

Per-provider health via `ProviderHealthRegistry` (process-local, `Mutex<BTreeMap>`):
- Success resets failures, clears cooldown, records latency
- 3 consecutive failures → cooldown (rate-limited: 60s, timeout: 15s, transport: 30s, panic: 30s)
- Cooldown cleared immediately on success
- Cooled-down providers skipped for routing but never skipped when explicitly requested

See [meta.md](meta.md#provider-health) for details.

### Bounded Everything

Most resources are bounded: timeouts, max_results, max_chars, max_bytes, redirect limits, link caps, import scan limits, batch sizes, PDF pages, concurrency, forge tree/pagination response bytes, forge error-body previews (8KB cap via `read_error_preview()`), and forge metadata lookups (bounded response reading). `ForgeReadBudget` tracks aggregate bytes across all requests within a single tool invocation (operation-wide, not per-response); pagination stops on aggregate budget exhaustion. The untracked-file count (`git ls-files --others`) is read through `run_bounded_command_for_inventory()` with a configurable cap. File opening in the primary search path uses `safe_open.rs` with descriptor-relative `openat2` and `RESOLVE_BENEATH`; inventory fallback paths use `std::fs::read()` with size capping. Defaults are safe for general MCP exposure.

### Forge Endpoint Safety

Forge API base URLs are validated by `validate_base_url()` before use: embedded credentials are rejected, DNS names are resolved to classify all resolved addresses, literal IPv4/IPv6 addresses are classified, and HTTP with API keys is rejected. `ForgeEndpointPolicy` controls loopback (`allow_loopback`), private network (`allow_private_network`), and HTTPS requirements (`require_https`). All forge response bodies are read through bounded response readers; error-body previews use `read_error_preview()` with an 8KB cap. Forge API clients use `Policy::none()`, rejecting all redirects. See [meta.md](meta.md#forge-adapter) for details.

---

## Key Architectural Decisions

1. **Adapter pattern** — `MetadataSearchAdapter` is the single boundary between MCP tools and search engines. Engine types never leak past this module. This enables testing, swapping engines, and adding new providers without changing tool implementations.

2. **Deterministic IDs** — All stable output types use FNV-1a 64-bit hashes with versioned prefix (`eggsearch-id-v1\0`). URL canonicalization prevents trivial differences from producing different IDs. This enables cross-tool deduplication and regression testing.

3. **Soft failures** — Adapter returns `WebSearchResponse` with warnings, never errors. Partial provider failures are surfaced as warnings. This matches the agent-oriented use case where partial results are better than none.

4. **No persistent state** — Server starts and runs without any index, database, or filesystem state (except config). All state is process-local.

5. **RRF aggregation** — Reciprocal Rank Fusion merges results from multiple providers. `score(d) = Σ 1/(k + rank_i(d))` where k=60.

6. **Profile-based routing** — 4 profiles influence provider selection. Degraded profiles fall back to defaults with warnings.

7. **Three-tier sanitization** — Untrusted text is always stripped/bounded (Tier 1), optionally framed (Tier 2), and optionally scanned for injection markers (Tier 3).

8. **Inventory-first search** — Local workspace search uses a cached file inventory to avoid repeated full-tree walks. Git-aware fast path (`git ls-files -z --cached --others --exclude-standard`) is preferred when available; native directory walking is the fallback. Inventory is auto-built on first search (cache miss). A `git status --porcelain=v2` hash (`status_hash`) is stored alongside the inventory, detecting untracked file creation, staging, branch switches, and ignore-rule changes. Per-file validation via XXH3 fingerprinting (path + size + mtime) detects changes between inventory build and search time. Freshness confidence is based on `FRESHNESS_PROBE_INTERVAL` (30s): inventory age < 30s = High, age < rebuild TTL with unchanged status_hash = Medium, else Low.

9. **Bounded subprocess execution** — `run_bounded_command()` in `local_inventory_cache.rs` enforces timeout (5s), stdout cap (16MB), and stderr cap (64KB) on Git subprocess invocations. Stdout and stderr are drained concurrently using separate threads, each with independent byte caps. Creates a new process group via `setsid()` and kills the process group on timeout using a watchdog thread. Cap breaches (stdout or stderr limit exceeded) trigger immediate process group termination via `ProcessTerminationController`. `CommandTermination` enum records the reason: `Exited`, `TimedOut`, `StdoutLimitExceeded`, `StderrLimitExceeded`, `SpawnFailed`, or `Signaled`. This prevents zombie processes, memory exhaustion from large outputs, and indefinite hangs from misbehaving Git commands.

10. **Workspace change-token strategy** — The `status_hash` (XXH3 hash of `git status --porcelain=v2 -z --untracked-files=normal` output) provides lightweight change detection between inventory builds. The 30-second `FRESHNESS_PROBE_INTERVAL` avoids redundant status checks on rapid successive searches. When the status hash matches, the `index_mtime` fallback check is skipped (status_hash is authoritative). When the status hash is unavailable (e.g., non-git directory or truncated output), the fallback `index_mtime` check applies.

11. **Race-resistant local file opening** — `safe_open.rs` provides `safe_open_relative()` which uses descriptor-relative file opening via `openat`/`openat2` with `O_NOFOLLOW`. On Linux, it attempts `openat2` with `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS`, falling back to `openat` with `O_NOFOLLOW` on older kernels. For `follow_symlinks=true` on Linux, uses `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS` (omitting `RESOLVE_NO_SYMLINKS`) to let the kernel enforce containment while allowing symlinks. On non-Linux Unix platforms, `follow_symlinks=true` returns `SafeSymlinkFollowingUnsupported` because no race-safe containment primitive is available. Each path component is opened relative to the parent directory descriptor, eliminating TOCTOU races between validation and open. The final file descriptor is verified via `fstat` for regular file type and size limits.

12. **Evidence workflow selection and conflict scoping** — `resolve_workflow_model()` maps tool name, profile, and research domain to a deterministic `WorkflowCoverageModel` defining required/recommended/optional evidence roles for each of 10 core workflows. `ConflictEntityKey` (entity type + canonical ID + field) provides composite grouping for conflict detection, preventing unrelated sources from being compared. Conflict source IDs identify only the disagreeing cards, not entire entity groups. Evidence roles are materialized onto all source cards via `materialize_evidence_roles()`. `RetrievalAttempt` tracks per-provider outcomes (success, failure, timeout, rate limit, skip, truncation) for attempt-derived retrieval summaries. Attempt outcomes and absence kinds are related but distinct: outcomes describe what happened during retrieval, while absence kinds describe the impact on evidence coverage. Both are wired into all result conversion paths via `evidence_postprocess.rs`.

13. **Semantic research subquery intent** — Research planner subqueries carry typed `intended_roles` derived from `ResearchSourceType`, flowing from planner through dispatch into postprocessing. This replaces opaque `rq_*` label inference with explicit role semantics.

14. **Native security attempt collection** — Native advisory lookups (CVE/GHSA/OSV/RustSec/KEV) produce `RetrievalAttempt` records that merge into the retrieval summary alongside web-search results. Lookup failures are not silently discarded; they appear as retrieval-attempt entries.

15. **Multi-role failure expansion** — Retrieval failures for research subqueries expand across all `intended_roles` on the subquery, not just a single role. This prevents incomplete failure attribution when a subquery targets multiple evidence dimensions.

16. **Manual release** — Release cadence is maintainer-controlled. `make release-check` is the local packaging gate; `cargo publish --locked` publishes to crates.io. GitHub Actions has no publication role. Optional provider conformance does not block core releases.

17. **Native smoke tests are distinct from fallback** — Native forge smoke tests (`tests/native_forge_smoke.rs`) exercise the adapter path directly with configured API tokens. Live-smoke tests (`--features live-smoke`) use fallback mode. Native smoke tests are maintainer-only diagnostics, not release evidence.

18. **DNS validation is preflight-only** — DNS address classification happens before connection. No connection-time DNS pinning is enforced. Documented in `docs/architecture/meta.md`.

19. **Windows is unsupported** — The crate uses Unix-specific APIs (`openat2`, `setsid`, process groups). Windows is not included in the CI matrix and is not claimed as supported.

---

## Feature Flags

| Flag | Purpose |
|------|---------|
| `mock` | Test-only mock engine harness (`src/meta/mock.rs`) — **required for integration/corpus tests** |
| `pdf` | PDF text extraction via `lopdf` |
| `live-smoke` | Live network smoke tests (implies `mock`); ignored by default |

---

## Build & Verification

```bash
make check            # routine gate (fmt + clippy + no-default compile check + all-features tests)
make release-check    # release gate (routine + docs + release-build + publish-dry-run)
cargo fmt --check     # format check
cargo clippy --all-targets --all-features -- -D warnings  # zero warnings required
cargo test --all-features  # all tests
cargo test --features mock  # mock feature tests (integration + corpus)
cargo build --release  # release build
cargo publish --dry-run  # pre-publish check
```

---

## Deep Dives

For detailed analysis of each component:

### Core Modules

1. [core.md](core.md) — Domain types, config, identity, sanitization, warnings, source cards, quality, security/research/repo/local/evidence types
2. [meta.md](meta.md) — Metasearch adapter, 34 engines, RRF aggregation, query planning, provider health, local workspace
3. [fetch.md](fetch.md) — HTTP client, content extraction, SSRF protection, code-host rewriting, span selection, PDF
4. [mcp.md](mcp.md) — MCP server, 10 tool definitions, state management, policy enforcement
5. [commands.md](commands.md) — CLI subcommands (doctor, search, mcp, fetch, providers)

### Subsystem Deep Dives

6. [config.md](config.md) — TOML config model, provider resolution, validation, CLI loading
7. [engines.md](engines.md) — Search engine trait, 34 engine implementations, shared infrastructure
8. [security.md](security.md) — Security advisory search, version applicability, remediation, KEV
9. [research.md](research.md) — Research evidence discovery, claims, conflicts, gaps, workflows
10. [evidence-workflow.md](evidence-workflow.md) — Evidence bundles, role taxonomy, workflow recipes, conflict detection, retrieval tracking
11. [local-workspace.md](local-workspace.md) — Filesystem search, cached inventory, git-aware fast path, race-resistant file opening

### Testing & Hardening

12. [testing.md](testing.md) — Test strategy, CI pipeline, feature flags, mock engine
13. [hardening.md](hardening.md) — Property testing, adversarial corpus, fuzz harness, crash promotion
14. [codegg-contract.md](codegg-contract.md) — Stable MCP response contract (deterministic IDs, warnings, trust model, next actions, security applicability, research evidence, local workspace metadata)

### External References

15. [../../docs/threat-model.md](../../docs/threat-model.md) — Operator threat model, trust boundaries, prompt-injection handling, configuration escape hatches, recommended host-agent policy
16. [../../docs/safety.md](../../docs/safety.md) — Fetch safety, blocked address ranges, sanitization tiers, trust markers
17. [../../docs/config.md](../../docs/config.md) — Config defaults, provider requirements, profile examples
18. [../../docs/tool-matrix.md](../../docs/tool-matrix.md) — Compact tool reference with trust semantics
19. [../../docs/agent-workflows.md](../../docs/agent-workflows.md) — Recommended tool call sequences and recipe catalog
20. [../../docs/provider-setup.md](../../docs/provider-setup.md) — Provider configuration guide
