# Eggsearch Long-Term Specification

Status: normative long-term specification

This document defines what eggsearch is becoming and what must remain true
across releases and implementation strategies. It MUST remain stable during
ordinary feature implementation and MAY be amended only per
`plans/003-planning-process.md` §2.1.

The keywords MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, and MAY are normative.

## 1. Product identity

Eggsearch is a lightweight MCP search/fetch server for AI agents: live
metasearch with RRF dedup, bounded fetch, and deterministic evidence bundles.

- Single library + binary crate, not a workspace.
- Stable contract is 10 MCP tools plus CLI; the Rust module tree is an
  implementation detail with no semver library guarantee (see `src/lib.rs`).
- Transports: client-owned `mcp stdio` or explicit loopback-only `mcp serve`.
  Non-loopback/LAN/public MCP exposure is a non-goal.
- Dependency flow: `core <- meta <- mcp <- commands`; `fetch` is independent
  of `meta`.

## 2. Stable MCP tool contract

The 10 stable tools are:

- `web_search`
- `web_fetch`
- `batch_fetch`
- `provider_status`
- `repo_search`
- `repo_fetch`
- `repo_map`
- `security_search`
- `research_search`
- `build_evidence_bundle`

The tool set MUST remain exactly 10 unless the tool-matrix, docs contract
tests, and CodeGG integration docs move in the same change (enforced by
`tests/static_guards.rs`). MCP tools MUST call the `MetadataSearchAdapter`,
never engines directly. New tools REQUIRE a dedicated module under
`src/mcp/tools/` with shared validation in `common.rs` and goal/workflow
resolution in `canonical.rs`, registered in `src/mcp/server.rs`.

## 3. Retrieval architecture

The target pipeline remains:

```text
discover -> rank -> select -> bounded fetch -> deterministic evidence handoff
```

- Metasearch fan-out with RRF aggregation and deterministic tie ordering.
- Unsupported provider capabilities stay explicit: dispatch emits
  capability-skip attempts, never silent omissions.
- Domain workflows (`repo`/`research`/`security`) share
  `WorkflowExecution`/`RetrievalAttemptSet`/`FetchCandidateBuilder` primitives
  but keep typed planners, grouping, and suggested-fetch builders. They MUST
  NOT be flattened into one generic workflow.
- Stable IDs are content-derived FNV-1a (`src/core/identity.rs`). Random UUIDs
  are forbidden; ID semantics MUST NOT change.
- All untrusted text MUST be sanitized through `src/core/sanitize.rs` /
  `sanitize_field()`.

## 4. Trust and safety bounds

- All untrusted I/O MUST be bounded: forge responses only via
  `read_bounded_body()`, never bare `.text()`/`.bytes()`; bounded git
  execution only via `run_bounded_command()` with process-group kill on
  timeout/cap breach.
- `commit_sha` comes from `resolved_ref`, never the entry object SHA.
- `CacheScope::Profile` uses the opaque profile ID, never the display name.
- Invalid explicit browser path is `ExplicitPathInvalid`; fallback to
  auto-discovery is forbidden.
- Outbound HTTP ownership: `eggfetch-core` owns transport, pooling, TLS,
  decompression, total deadlines, and redirect mechanics. Eggsearch owns
  SSRF/retry/truncation policy. No eggsearch-local reqwest client or
  compatibility facade is permitted.
- Optional `egress` feature is a listener-free HTTP/SOCKS proxy-chain route
  for provider upstreams only. Prebuilt/default binaries exclude it. Dynamic
  `FetchClient` targets MUST remain direct with resolved-address pinning.

## 5. Provider model

- 37 known provider IDs (`KNOWN_PROVIDER_IDS` in `src/core/provider.rs`),
  covering HTML scrape, JSON API, API-key, advisory, registry, and scholarly
  sources. `local_workspace` is served by the local backend, not an engine.
- New engines MUST implement `SearchEngine::search(&EngineSearchRequest)`.
- New providers MUST declare the 24-flag `ProviderCapabilities`, add the ID to
  `KNOWN_PROVIDER_IDS`, document native-vs-local enforcement in
  `docs/provider-setup.md`, and extend
  `tests/provider_capability_contract.rs`.
- Native enforcement matrix (authoritative; `AGENTS.md` mirrors it):
  `brave_api` natively enforces safe-search, freshness/date-range, language,
  region, news; `exa` natively enforces freshness/date-range, domain filters,
  result timestamps; `tavily` natively enforces safe-search, freshness/
  date-range, language, region, domain filters, news. Domain filters are
  natively enforced only by providers advertising `supports_domain_filters`
  (currently `exa`, `tavily`); all other domain filtering is local
  approximation.
- Tests MUST pass keyless: missing credentials are provider-scoped skips,
  never global failures. Tests MUST NOT require network.

## 6. Distribution and operations

- Seven-target release matrix with exact 16-asset assembly (seven binaries,
  seven checksums, two installers), machine-checked via
  `packaging/release-targets.txt`, `packaging/release-inputs.txt`, the release
  workflow, installers, updater, and install docs (`make packaging-check`).
- Multiple binary SKUs are forbidden. Either one canonical default/release
  binary includes a feature, or the feature remains a documented source-build
  opt-in.
- `integrate` prints by default and mutates only with `--apply` (atomic,
  backed up, `eggsearch` entry only). Never register `target/debug` binaries.
- `CHANGELOG.md` entries are append-only history.

## 7. Performance and footprint

Optimization is optimization-with-equivalence. It MUST NOT change the
ten-tool surface, ranking weights, trust/SSRF semantics, cache policy, batch
budget semantics, public Rust compatibility, browser/PDF availability,
provider coverage, or integration verification merely to improve benchmark or
binary-size numbers.

- Hot paths use shared immutable inventory/cache ownership and score/select
  candidates once; deterministic tie ordering is preserved.
- Fetch timeout overrides reuse the shared transport for equal/shorter values
  and build one widened client for longer values; batch setup remains one
  adjustment per call.

## 8. Non-goals

- Recursive crawling or autonomous browser interaction.
- Provider-generated answers, summaries, deep-research agents, or
  schema-generation layers.
- A new general-purpose `site_map` MCP tool unless a future evidence-based
  plan promotes it.
- Mandatory vector/embedding local indexing.
- Mandatory LSP/rust-analyzer local-search dependency.
- Full PDF layout/OCR work unless separately planned.
- apt/RPM/Homebrew/Winget/Chocolatey/Scoop/MSI/PKG pipelines; containers as
  the primary install mechanism; unattended/background auto-update scheduling.
- MCPB as a required distribution mechanism.

## 9. Verification gates

- `make check` is the canonical gate: fmt + clippy + no-default check +
  all-features tests + hygiene + packaging-check.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` with
  zero warnings required.
- `mock` feature is REQUIRED for integration/corpus tests; plain `cargo test`
  misses most of them.
- Ordinary files MUST stay under 1,600 lines / 80 KB; larger modules carry
  explicit ratchet ceilings and next-slice notes in
  `architecture/maintenance.md`.
