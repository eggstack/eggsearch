# Release Polish Pass Plan

## Purpose

This plan is a targeted release-readiness pass for eggsearch after the recent search, fetch, repo, security, research, and evidence-bundle work. The goal is not to add another broad feature tranche. The goal is to make the current capability set easier to trust, easier to operate, and safer to expose as a public MCP server for coding agents.

Focus areas:

1. Correct provider diagnostics so humans and agents see accurate configured/enabled/available state.
2. Tighten or explicitly document the residual fetch safety boundary, especially DNS validation-to-connect behavior.
3. Fix small result-allocation correctness issues that can suppress useful local evidence.
4. Improve `web_fetch` structured output utility for agents without changing its security model.
5. Polish release-facing documentation and examples so the surface is understandable without reading source.
6. Add release verification checks and regression tests around the above items.

This pass should preserve the current stable MCP tool surface: `web_search`, `web_fetch`, `batch_fetch`, `provider_status`, `repo_search`, `repo_fetch`, `repo_map`, `security_search`, `research_search`, and `build_evidence_bundle`.

## Non-goals

Do not redesign the provider system, introduce persistent indexing, add crawler behavior, make `web_fetch` execute JavaScript, add a browser engine, or broaden local workspace search beyond the existing configured-root model.

Do not replace the existing safety framing or trust-label model. This pass may clarify, test, and polish the model, but should not remove external-untrusted labeling or prompt-injection marker handling.

Do not add new always-on external API providers. API-backed providers should remain opt-in through `[search].api` and environment-variable credentials.

## Current observations motivating the pass

The repository already has a credible release baseline: feature-matrix CI, a stable ten-tool MCP surface, bounded search/fetch defaults, structured fetch rendering, SSRF-oriented URL validation, prompt-injection framing, provider capability descriptors, search profiles, and extensive tests.

The main risks are concentrated in polish/correctness seams:

- `eggsearch doctor` and provider-health snapshots can report misleading configured/enabled state for API providers and no-key HTML providers.
- Fetch target validation resolves DNS before `reqwest` sends the request, but the actual connection uses `reqwest` resolution. That is a residual DNS-rebinding / validation-to-connect gap unless the connection path pins the validated address set.
- Local workspace search allocates `effective_max_results / 2`, which becomes zero when the caller asks for one result.
- `web_fetch` currently exposes one aggregate document chunk for many documents. That is safe, but less useful for coding agents than bounded semantic chunks with stable IDs.
- `metadata_only` avoids returning body text, but still reads a bounded body. Documentation should not imply it is a pure header-only or very cheap operation unless the implementation changes.
- README is comprehensive but too dense for release. It needs a shorter front door and deeper docs split by operator and agent concerns.

## Phase 1 — Provider diagnostics correctness

### Objective

Make `eggsearch doctor`, `provider_status`, provider health snapshots, and routing telemetry agree about provider state. The release invariant should be simple: a provider's `enabled`, `configured`, `available`, and `default` state must mean the same thing everywhere.

### Tasks

1. Audit all provider-state construction paths:
   - `src/commands/doctor.rs`
   - `src/meta/adapter.rs`
   - `src/meta/provider_diagnostics.rs`
   - `src/core/config.rs`
   - `src/core/provider.rs`
   - `src/mcp/tools.rs` provider-status response code

2. Create or reuse one central helper for provider configured-state semantics. Suggested semantics:
   - HTML scrape providers: configured when known and enabled/built; no credential required.
   - `searxng`: configured only when `[search].searxng.enabled = true` and `base_url` is non-empty and valid.
   - API-key providers: configured only when `[search].api.<id>.enabled = true`, `api_key_env` is non-empty, and the referenced environment variable resolves.
   - `osv`: configured by default when enabled because it is a no-key JSON API provider.
   - `local_workspace`: configured only when `[local].enabled = true` and at least one valid root is present.

3. Fix `eggsearch doctor` provider capability summary:
   - Do not compute API provider `enabled` only from `[search].providers`.
   - Do not mark every non-SearXNG provider as configured.
   - Include API providers from `[search].api` even when they are not in `[search].providers`.
   - Keep the separate `api_providers` credential block if useful, but ensure it does not contradict the main provider capability table.

4. Fix `ProviderHealthRegistry::all_snapshots` configured-state logic:
   - HTML scrape/no-key built-ins should not appear `configured = false` solely because they are absent from `api_configured`.
   - SearXNG configured-state should reflect SearXNG base URL state, not merely enabled adapter state.
   - API providers should reflect env-var availability.

5. Ensure explicit provider errors remain strict:
   - Unknown explicit provider should fail validation.
   - Known but unavailable explicit provider should fail in strict contexts.
   - Profile/default routing may skip unavailable or cooldown providers with telemetry.

### Acceptance criteria

- `eggsearch doctor` reports coherent provider state for default config.
- `eggsearch doctor` reports `brave_api`/GitHub/GitLab/Gitea providers as unconfigured when enabled but missing env vars.
- `eggsearch doctor` reports no-key HTML providers as configured when enabled.
- `provider_status` and doctor use compatible state vocabulary.
- Unit tests cover default config, configured API provider with env var, enabled API provider with missing env var, disabled SearXNG, enabled SearXNG without URL, enabled SearXNG with valid URL, and local workspace disabled/enabled cases.

### Suggested tests

- `doctor_default_provider_state_is_coherent`
- `doctor_api_provider_missing_env_is_unconfigured`
- `doctor_api_provider_with_env_is_configured`
- `provider_health_snapshots_mark_html_providers_configured`
- `provider_status_and_doctor_configured_state_match`

## Phase 2 — Fetch safety boundary clarification and hardening

### Objective

Make the fetch safety model precise and tested. Either close the DNS validation-to-connect gap or document it explicitly as a residual risk with operator guidance.

### Tasks

1. Audit the current fetch pipeline:
   - `validate_url`
   - `validate_fetch_target`
   - code-host source-file rewrites
   - redirect validation
   - body-size bounding
   - content-type and PDF gates

2. Add regression tests for existing intended safety properties:
   - Reject `file://` and non-HTTP(S) schemes.
   - Reject embedded credentials.
   - Reject localhost literals.
   - Reject private IPv4 literals.
   - Reject IPv6 loopback, unique-local, link-local, unspecified, and IPv4-mapped private addresses.
   - Reject redirect targets that resolve to private/localhost addresses.
   - Preserve code-host rewrite validation through the same target-validation path.

3. Decide on DNS rebinding mitigation level:
   - Preferred: implement a connect path that uses the validated resolved address set, or a custom resolver path that ensures the actual connection cannot resolve to an address that was not validated.
   - Acceptable short-term release path: document the residual validation-to-connect race in `docs/safety.md`, with clear guidance that `web_fetch` should run in a network sandbox when used against adversarial URLs.

4. If implementing hardening, keep behavior bounded and simple:
   - Do not introduce persistent DNS caches.
   - Do not allow redirects to bypass validation.
   - Do not allow code-host raw rewrites to bypass validation.
   - Ensure HTTPS host/SNI/certificate validation still uses the original hostname, not a mismatched IP-only URL.

5. Add structured warnings or docs for permissive network config:
   - `allow_private_network = true`
   - `allow_localhost = true`
   - Explain that these are operator-only escape hatches and should remain false for general MCP exposure.

### Acceptance criteria

- Existing fetch safety behavior is covered by focused tests.
- DNS validation-to-connect behavior is either hardened or explicitly documented.
- README safety language does not overclaim SSRF protection beyond the implementation.
- `docs/safety.md` clearly explains trust labels, prompt-injection framing, no-JS/no-crawl behavior, private-network defaults, redirects, code-host rewrites, PDFs, and residual risks.

## Phase 3 — Local workspace result allocation fix

### Objective

Ensure local workspace search remains useful for low-result-count requests and does not accidentally allocate zero local results.

### Tasks

1. Locate all local-result budget calculations in repo/research/security flows.
2. Fix `effective_max_results / 2` style calculations so positive caller budgets allocate at least one result when local search is enabled.
3. Preserve upper bounds; do not allow local search to return more than the requested budget merely because of the minimum.
4. Add tests for `max_results = 1`, `max_results = 2`, and larger values with local search enabled.
5. Ensure grouped results still respect `max_per_group` and final truncation behavior.

### Acceptance criteria

- `repo_search` with local workspace enabled and `max_results = 1` can return one local candidate when it is the best match.
- No request can exceed configured or requested result caps because of the minimum allocation.
- Tests cover low-budget local search behavior.

## Phase 4 — `web_fetch` chunk utility polish

### Objective

Improve agent usability of `web_fetch` documents by exposing bounded, stable, useful chunks instead of a single aggregate chunk for most documents.

### Tasks

1. Define chunking rules for `FetchDocument.chunks`:
   - Stable `chunk_id` values based on block ranges, e.g. `chunk_000`, `chunk_001`.
   - Respect configured/existing character budgets.
   - Prefer heading boundaries for HTML/Markdown prose.
   - Prefer code-block or line-range boundaries for source code/diffs/JSON/TOML/YAML.
   - Preserve `block_start` and `block_end` correctly.
   - Include `heading_path` derived from active outline/heading context.

2. Implement a small deterministic chunk builder:
   - Input: blocks, outline, max chunk chars, optional document kind.
   - Output: non-overlapping chunks with valid block ranges.
   - Avoid model summarization or semantic rewriting.

3. Keep legacy fields stable:
   - `text` remains populated as bounded extracted text.
   - `document.blocks` remains present for structured consumers.
   - Existing block kinds and outline format remain stable.

4. Add tests for:
   - HTML with multiple headings.
   - Large source file.
   - Markdown document.
   - Diff/patch rendering.
   - Truncated documents.
   - Empty or metadata-only responses.

5. Documentation:
   - Explain that chunks are deterministic slices, not summaries.
   - Explain how coding agents should use `chunks`, `outline`, and `blocks`.

### Acceptance criteria

- Multi-section documents produce multiple bounded chunks with stable IDs.
- Chunk block ranges are valid and non-overlapping.
- Chunks do not exceed the chosen chunk budget except where a single block itself exceeds that budget and has already been bounded.
- Existing tests depending on `text`, `blocks`, and `outline` still pass.

## Phase 5 — Metadata-only behavior decision

### Objective

Align `metadata_only` implementation and documentation so users know what cost and behavior to expect.

### Tasks

1. Audit `web_fetch` metadata-only path.
2. Decide whether to optimize or document current behavior.
3. If optimizing:
   - For HTML, stop after a bounded early read sufficient to detect title/meta description where feasible.
   - Preserve max-byte safety and content-type gates.
   - Do not add streaming parser complexity unless necessary.
4. If documenting:
   - State that `metadata_only` suppresses returned body text and structured body blocks, but may still read a bounded response body to extract metadata safely.
   - Do not describe it as header-only.
5. Add tests that assert metadata-only output has no body text/document blocks but still returns title/description when available.

### Acceptance criteria

- Docs and behavior agree.
- `metadata_only` responses are predictable and bounded.
- No regression in HTML title/description extraction.

## Phase 6 — Documentation split and release-facing README cleanup

### Objective

Make the release approachable for both humans and coding agents without hiding advanced features.

### Tasks

1. Keep README focused on:
   - What eggsearch is and is not.
   - Install from crates.io and build from source.
   - MCP stdio quick start.
   - Minimal CLI examples.
   - Stable MCP tool list.
   - Tool-selection guide: `web_search` vs `repo_search` vs `security_search` vs `research_search` vs `web_fetch` vs `batch_fetch` vs `build_evidence_bundle`.
   - Safety model summary.
   - Links to deeper docs.

2. Add `docs/config.md`:
   - Minimal default config.
   - Coding-agent config with GitHub/GitLab/Gitea providers.
   - Security-search config with OSV and generic fallback.
   - Research-search config.
   - Local workspace config.
   - SearXNG config.
   - Fetch limits and private-network knobs.
   - No-op/reserved config fields clearly labeled.

3. Add `docs/providers.md`:
   - Provider IDs.
   - Provider kind: HTML scrape, JSON API, API key, local.
   - Required configuration.
   - Capability matrix.
   - Freshness semantics: provider-side freshness vs result timestamps.
   - Recommended provider profiles for generic/coding/security/research.

4. Add `docs/safety.md`:
   - External content is untrusted.
   - Prompt-injection framing and marker scanning.
   - No crawling, no JavaScript.
   - Fetch URL restrictions.
   - Redirect validation.
   - Private network/localhost defaults.
   - DNS validation-to-connect behavior and residual risks.
   - Local workspace trust boundary.
   - Security-search scope: advisory retrieval, not exploitability determination.

5. Add `docs/tool-recipes.md`:
   - Common agent workflows.
   - Repository API understanding workflow.
   - Exact compiler/runtime error workflow.
   - CVE/package vulnerability workflow.
   - Deep research workflow.
   - Evidence bundle handoff workflow.

6. Optional README polish:
   - Add CI badge.
   - Add `cargo publish --dry-run` note for maintainers.
   - Add short examples for opencode/codegg MCP configuration if that config shape is stable enough.

### Acceptance criteria

- README is shorter and release-oriented.
- Advanced schema/output examples are moved or mirrored into docs pages.
- New docs reflect actual config defaults and implementation boundaries.
- No docs claim that reserved/no-op fields change runtime behavior.

## Phase 7 — Release verification and changelog polish

### Objective

Make release readiness reproducible.

### Tasks

1. Add or update release checklist documentation:
   - `cargo fmt --check`
   - `cargo clippy --all-features -- -D warnings`
   - `cargo test --all-features`
   - `cargo test --no-default-features`
   - `cargo test --features mock`
   - `cargo test --features pdf`
   - `cargo build --release`
   - `cargo publish --dry-run`

2. Ensure CI covers the relevant matrix.
3. Add a `CHANGELOG.md` release entry if absent or stale:
   - Stable MCP tool surface.
   - Search profiles.
   - Repo/code search support.
   - Fetch rendering and code-host raw rewrites.
   - Security/advisory search.
   - Research search.
   - Evidence bundles.
   - Safety boundaries and defaults.

4. Verify crate packaging:
   - `Cargo.toml` include list contains all required docs or intentionally excludes them.
   - If docs are important to crates.io users, add `docs/**/*.md` to package include.
   - Verify README links resolve under crates.io rendering.

5. Verify generated docs:
   - `cargo doc --all-features --no-deps`
   - Confirm public crate docs do not expose confusing internal-only test constructors beyond intended feature gates.

### Acceptance criteria

- Release checklist is documented.
- Changelog accurately represents current capabilities.
- `cargo publish --dry-run` passes.
- Docs included in crate package if README links to them.

## Implementation order

Use this order to reduce regression risk:

1. Provider diagnostics fixes and tests.
2. Local result allocation fix and tests.
3. Fetch safety tests and either hardening or explicit docs.
4. Fetch chunking improvements.
5. Metadata-only docs/behavior alignment.
6. Documentation split and README cleanup.
7. Changelog and release verification.

## Required regression commands

Run at minimum:

```bash
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock
cargo test --features pdf
cargo build --release
cargo publish --dry-run
```

If fetch safety internals change, also run any focused fetch safety and schema/corpus tests:

```bash
cargo test --features mock --test fetch_safety
cargo test --features mock --test schema_identity_registry
cargo test --features mock --test recipes_next_actions
cargo test --features mock --test evidence_bundle_handoff
```

## Final handoff notes

This should be treated as a release polish pass, not a feature expansion. The repo already has substantial capability. The work should bias toward making current behavior accurate, documented, and testable.

The most important success condition is that a coding agent can call `provider_status`, choose the correct search/fetch workflow, understand when a provider degraded to generic search, fetch explicit URLs safely, and package evidence without ambiguous or misleading metadata.
