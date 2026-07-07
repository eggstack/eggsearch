# eggsearch Production Hardening Pass

Date: 2026-07-07

## Purpose

This plan closes the remaining production-readiness gaps after the recent release corrective/audit passes. The repo has addressed many concrete correctness defects around fetch truncation, line-range clamping, batch budgeting, UTF-8 handling, provider diagnostics, path validation, and documentation contract tests. The remaining work should focus on production confidence rather than feature expansion: enforceable CI, provider drift detection, adversarial fetch coverage, stable document-rendering contracts, config/diagnostic clarity, and release verification.

The goal is to make eggsearch safe and predictable as the default search/fetch MCP server for codegg and other coding agents. Do not remove existing capabilities. Preserve the stable MCP surface:

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

## Non-goals

Do not add new public MCP tools in this pass unless absolutely required by the hardening work. Do not introduce a database, persistent daemon state, or long-running background workers. Do not make SearXNG mandatory. Do not make network-dependent tests part of the default offline CI gate. Do not weaken fetch SSRF protections, prompt-injection framing, truncation caps, local workspace boundaries, or provider-routing validation.

## Current baseline to preserve

Recent code already includes the key production primitives that should remain intact:

- `ServerState::build` validates config before constructing adapter/fetch/local state.
- `web_fetch` validates explicit HTTP(S) targets, rejects credentials, performs DNS/private-IP checks, validates redirects, and pins the outbound request to the validated resolved address set for each attempt.
- Fetch output separates raw internal text from user-facing framed/sanitized text.
- Batch fetch enforces aggregate item and character budgets.
- Local workspace fetch/search rejects traversal and symlink escapes.
- Provider status reports routability, skip reasons, capabilities, health, and recipes.
- Docs contract tests validate config snippets, provider inventory, and tool-name coverage.
- Cargo package metadata includes docs/tests/benches needed for publish checks.

This pass should add confidence around those properties, not rework them unnecessarily.

## Phase 1: CI and release-gate enforcement

### Problem

The repo has a broad CI workflow, but the latest inspected commit did not have visible workflow run/status evidence. The local `make check` gate also differs from the GitHub clippy gate: the Makefile uses `cargo clippy --all-targets --all-features -- -D warnings`, while CI currently only uses `cargo clippy --all-features -- -D warnings`. That can let warnings in test/support targets escape CI.

### Tasks

1. Align GitHub CI with the local gate.
   - Change the CI clippy step to `cargo clippy --all-targets --all-features -- -D warnings`.
   - Confirm `make check` and `.github/workflows/ci.yml` cover the same required categories: fmt, clippy, all-features tests, no-default-features tests, schema corpus, docs contract tests.
   - Add a small comment in the workflow explaining that CI intentionally mirrors `make check`.

2. Add missing release gate parity.
   - Ensure CI runs `cargo publish --dry-run --locked` on push and PR.
   - Ensure CI runs `cargo doc --all-features --no-deps` with `RUSTDOCFLAGS=-D warnings`.
   - If not already present, add `cargo check --features pdf`, `cargo test --features pdf`, `cargo check --features mock`, and `cargo test --features mock` matrix entries.

3. Add workflow visibility/branch protection notes.
   - Add `docs/release.md` or update an existing release doc with the required pre-release checks.
   - Document that `main` should require the CI jobs before release tags are cut.
   - The repo cannot enforce branch protection in code, but the documentation should list the exact required checks.

4. Add a release checklist.
   - Include local commands:
     - `cargo fmt --check`
     - `cargo clippy --all-targets --all-features -- -D warnings`
     - `cargo test --all-features`
     - `cargo test --no-default-features`
     - `cargo test --features mock --test schema_identity_registry`
     - `cargo test --features mock --test fetch_safety`
     - `cargo test --features mock --test security_applicability_corpus`
     - `cargo test --features mock --test research_evidence_corpus`
     - `cargo test --features mock --test recipes_next_actions`
     - `cargo test --features mock --test evidence_bundle_handoff`
     - `cargo test --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names`
     - `cargo publish --dry-run --locked`
   - Include expected feature-flag behavior: default build excludes PDF extraction; `--features pdf` enables PDF parsing; `mock` remains test-only.

### Acceptance criteria

- `make check` and CI do not diverge in clippy coverage.
- CI includes publish dry-run and docs warning gates.
- Release documentation lists exact pre-tag checks.
- A reviewer can determine from the repo which checks must be green before release.

## Phase 2: Provider probe and provider contract hardening

### Problem

`provider_status` accepts a `probe` argument, but live provider probing is currently documented as not implemented. That makes provider diagnostics less useful for operators, especially because many providers are scrape/API adapters subject to upstream drift. For production, `provider_status` should either implement probe semantics or stop exposing a misleading argument. Prefer implementing bounded, optional probes.

### Tasks

1. Implement bounded provider probes for `provider_status` when `probe = true`.
   - Add a probe path that performs one small query per routable provider.
   - Use provider-specific safe probe queries where possible:
     - generic web: `rust language`
     - code providers: `repo:rust-lang/rust Vec`
     - package registries: package known to exist for that ecosystem, e.g. `serde`, `requests`, `react`, or provider-appropriate names.
     - advisory providers: known stable CVE/GHSA/OSV fixture if API semantics support lookup; otherwise a generic advisory query.
     - scholarly providers: stable generic query such as `rust programming language` or DOI lookup only when provider supports DOI lookup.
   - Bound per-provider probe timeout by the configured search timeout, and do not let probe failures block status generation.
   - Return probe result metadata in `provider_status`, not as unstructured warning strings only.

2. Define stable probe result fields.
   - Add fields such as:
     - `probed: bool`
     - `probe_status: "ok" | "failed" | "skipped" | "unsupported"`
     - `probe_error_class: Option<String>`
     - `probe_message: Option<String>`
     - `probe_latency_ms: Option<u64>`
     - `probe_result_count: Option<usize>`
   - Keep existing `providers`, `health`, `code_hosts`, `server_capabilities`, and recipe output backward compatible.

3. Add provider-class contract tests.
   - Offline tests should use mock engines/adapters and validate response shape.
   - Add tests that assert:
     - unknown provider IDs fail explicit routing.
     - disabled/config-missing provider IDs produce actionable skip reasons.
     - 404/not-found semantics are nonfatal for advisory/package lookup where intended.
     - rate limits classify as `rate_limited`.
     - parse failures classify as `parse_error`.
     - provider capability flags used by routing and telemetry match descriptor expectations.

4. Add optional live-smoke documentation.
   - Keep live smoke tests ignored by default.
   - Document required environment variables/API keys for API-backed providers.
   - Ensure live smoke tests do not require credentials for scrape/no-key providers.
   - Use conservative timeouts and result-count assertions to reduce flakiness.

### Acceptance criteria

- `provider_status { probe: true }` performs bounded, nonfatal provider probes and returns structured probe data.
- `provider_status { probe: false }` remains fast and network-light.
- Provider probe failure does not crash the server.
- Tests cover provider routing, failure classification, and probe response shape.
- Docs clearly distinguish configuration status, routability, cached health, and live probe status.

## Phase 3: Fetch adversarial safety coverage

### Problem

The fetch implementation has strong safety controls, but fetch is the highest-risk boundary. It needs adversarial tests for SSRF, redirect abuse, DNS edge cases, body expansion, and content-type deception. This should be handled with offline `httpmock`/local fixtures where practical.

### Tasks

1. Expand fetch safety tests.
   - Add tests for:
     - embedded credentials in initial URL and redirect URL.
     - redirect to localhost literal.
     - redirect to RFC1918 IPv4 literal.
     - redirect to IPv6 loopback/unique-local/link-local.
     - relative redirect that resolves to a blocked target.
     - uppercase/lowercase content-type variants.
     - missing content-type with unsupported binary-looking body.
     - PDF magic served under misleading content-type with PDF disabled.
     - `Content-Length` larger than `max_bytes`.
     - chunked body exceeding `max_bytes`.
     - gzip/brotli decoded body exceeding `max_bytes`, if feasible with reqwest/httpmock.

2. Add DNS/address tests at the validation layer.
   - Unit-test blocked address detection directly where possible.
   - Cover:
     - `127.0.0.1`
     - `0.0.0.0`
     - `10.0.0.0/8`
     - `172.16.0.0/12`
     - `192.168.0.0/16`
     - `169.254.0.0/16`
     - IPv6 loopback `::1`
     - IPv6 unique local `fc00::/7`
     - IPv6 link-local `fe80::/10`
     - IPv4-mapped IPv6 addresses such as `::ffff:127.0.0.1` and `::ffff:192.168.1.1`.

3. Add URL parser edge cases.
   - Test unusual but valid URL forms accepted by `url::Url`.
   - Confirm policy decisions for:
     - uppercase schemes.
     - trailing-dot hostnames.
     - percent-encoded credentials-like strings in path/query.
     - usernames/passwords with empty components.
     - port-specific host resolution.
   - Do not add permissive behavior unless a test proves it is safe.

4. Verify truncation and trust markers.
   - For each oversized body case, assert `truncated` or the corresponding error is set consistently.
   - Assert `trust_markers.text_truncated` when raw text is capped for internal consumers.
   - Assert user-facing text remains framed when `sanitize_output = true`.

### Acceptance criteria

- Fetch safety tests cover SSRF primitives, redirects, address ranges, content-type deception, and size limits.
- All new tests are offline and deterministic.
- No fetch path can access local/private networks unless the corresponding config explicitly allows it.
- New tests prove existing safe behavior without weakening URL support unnecessarily.

## Phase 4: Document-rendering golden snapshots

### Problem

The fetch renderer now handles HTML, Markdown, code, JSON/text, CSV/TSV, notebooks, XML/RST/AsciiDoc, and gated PDFs. The production risk is regression in agent-facing document structure: blocks, outline, chunks, truncation flags, link flags, and line-preservation behavior.

### Tasks

1. Add fixture inputs under `tests/fixtures/fetch/` or equivalent.
   - `sample.html`
   - `sample.md`
   - `sample.rs`
   - `sample.py`
   - `sample.json`
   - `sample.csv`
   - `sample.tsv`
   - `sample.ipynb`
   - `sample.xml`
   - `sample.diff`
   - minimal PDF fixture only if size/licensing are acceptable; otherwise keep PDF tests generated in-memory or mocked.

2. Add golden assertions without brittle full-snapshot noise.
   - Prefer structural assertions over giant serialized snapshots.
   - Assert:
     - `document.kind`
     - `document.render_format`
     - `document.text_format`
     - nonempty/expected `blocks`
     - outline presence/shape where applicable.
     - chunk IDs stable enough for handoff.
     - `text_chars_returned` not exceeding caps.
     - `text_truncated`, `block_truncated`, and `link_truncated` semantics.
     - detected language for code-like files.

3. Add line-preservation checks.
   - For code/diff/Markdown-source paths, assert that line numbers or block ranges remain meaningful after truncation.
   - Ensure `repo_fetch` line spans do not shift because of trust framing or synthetic metadata.

4. Add metadata-only checks.
   - For HTML and PDF-disabled/enabled paths, assert metadata-only mode avoids expensive body construction where intended and returns coherent metadata.
   - Ensure metadata-only responses do not include body text.

### Acceptance criteria

- Each supported document kind has at least one deterministic structural regression test.
- Tests verify both legacy `text` behavior and structured `document` behavior where applicable.
- Rendering regressions are caught before release without depending on live network pages.

## Phase 5: Config validation and documentation simplification

### Problem

The config model is powerful but broad. Some parsed fields are intentional no-ops, and provider/profile setup is complex. For production, operators need clear minimal/recommended configs and fail-fast validation. Future commands should not accidentally load unvalidated config.

### Tasks

1. Centralize config validation semantics.
   - Audit all CLI command paths and tests for config loading.
   - Prefer validating config in the CLI config loader by default, or add clearly named functions:
     - `load_validated`
     - `load_unvalidated_for_tests_or_migration`
   - Ensure every production command uses validated config.
   - Avoid breaking tests that intentionally construct invalid configs; migrate those to direct `AppConfig` construction or unvalidated helpers.

2. Improve validation messages.
   - Make config errors actionable and include the exact TOML path.
   - Ensure API-provider errors distinguish:
     - unknown provider id.
     - enabled without `api_key_env`.
     - env var name set but not present.
     - provider known but not routable.
   - Decide whether enabled API provider with missing env var should warn or hard-fail. For production, prefer hard-fail when explicitly enabled in config, but do not hard-fail merely because docs mention an optional provider.

3. Simplify documentation.
   - Add or update docs with three canonical configurations:
     - minimal default local MCP config.
     - codegg recommended config.
     - low-power/Raspberry Pi config.
   - Put advanced provider/API examples in a separate provider setup page.
   - Put no-op compatibility fields (`[search].live.user_agent`, `[search].live.respect_robots_txt`) in a clearly labeled compatibility/no-op section, or remove them if backwards compatibility is not needed.

4. Add config snippet contract tests for the new examples.
   - Mark snippets with existing config-test metadata.
   - Ensure all docs examples parse and validate as appropriate.
   - For examples that require env vars, either use parse-only metadata or test with controlled env vars.

### Acceptance criteria

- Production command paths do not use unvalidated config.
- Docs expose a small recommended config surface before advanced options.
- No-op fields are not presented as working behavior.
- All documented TOML snippets are covered by docs contract tests.

## Phase 6: Agent-facing response contracts and evidence quality

### Problem

The MCP surface is broad and structured, but production agents need stable response contracts. Search and fetch responses should make provenance, trust, truncation, source quality, and next actions machine-actionable. Some of this already exists; this phase tightens consistency.

### Tasks

1. Audit response schemas for all ten MCP tools.
   - Ensure every tool that returns external content includes trust markers/warnings where appropriate.
   - Ensure every search-like response includes provider queried/failed/skipped data.
   - Ensure every fetch-like response includes truncation and final URL metadata.
   - Ensure every evidence-producing response has stable IDs.

2. Add source quality tiers.
   - Without adding model judgment, add deterministic `source_quality` or equivalent metadata where feasible:
     - official docs.
     - repository source.
     - issue/PR discussion.
     - release/changelog.
     - package registry.
     - advisory database.
     - scholarly/source-of-record.
     - generic web/community.
     - unknown.
   - Keep this deterministic and derived from provider/result kind/URL classification.

3. Strengthen next-action hints.
   - Ensure `next_actions` recommend `web_fetch`, `repo_fetch`, `repo_map`, or `build_evidence_bundle` with specific stable IDs/locators where possible.
   - Avoid vague suggestions that require the agent to infer missing fields.
   - Add regression tests for next-action shape on representative responses.

4. Add schema identity/compatibility tests.
   - Ensure new fields are additive.
   - Update schema corpus tests if response fixtures intentionally evolve.
   - Preserve old field names unless a migration note is added.

### Acceptance criteria

- Agents can distinguish source quality, trust level, truncation, provider failure, and next recommended operation from structured fields.
- New fields are additive and documented.
- Existing schema-corpus tests pass after fixture updates.

## Phase 7: Local workspace hardening and performance sanity

### Problem

Local workspace support is valuable for codegg, but it must remain bounded, safe, and fast on low-power machines. Recent path-validation fixes should be backed by more systematic tests and lightweight performance checks.

### Tasks

1. Expand local workspace safety tests.
   - Reject absolute paths.
   - Reject parent traversal components.
   - Reject symlink escapes.
   - Accept filenames containing `..` where not a parent component.
   - Accept nested normal paths and single-dot components where safe.
   - Confirm `prefer_local` does not bypass remote/local trust separation.

2. Add local indexing bounds tests.
   - Verify `max_file_bytes` is respected.
   - Verify `max_indexed_files` is respected.
   - Verify binary-file detection is case-insensitive.
   - Verify large directories are bounded and return warnings/metadata rather than unbounded work.

3. Add lightweight performance benches or smoke metrics.
   - Use existing `benches/perf.rs` if applicable.
   - Add local-workspace fixture with many small files and a few large files.
   - Measure rough behavior of repo_map/repo_search local path under low-power assumptions.
   - Do not make benchmark timing a CI failure unless using deterministic operation-count assertions.

### Acceptance criteria

- Local workspace cannot escape configured roots.
- Local indexing/search remains bounded by config.
- Performance tests or benches provide a baseline for future regressions.

## Phase 8: Release verification and closure

### Tasks

1. Run the full local release gate.
   - `make check`
   - `cargo test --all-features`
   - `cargo test --no-default-features`
   - `cargo test --features pdf`
   - `cargo publish --dry-run --locked`
   - `cargo doc --all-features --no-deps` with warnings denied.

2. Run optional live smoke tests.
   - No-key providers only by default.
   - API-backed providers only when corresponding env vars are present.
   - Record failures as provider drift unless they indicate local regression.

3. Verify packaging.
   - Ensure package include list covers required docs, tests, benches, README, LICENSE, CHANGELOG.
   - Confirm docs.rs metadata builds with all features.
   - Confirm README badges and install instructions match current crate metadata.

4. Update changelog/release notes.
   - Summarize hardening improvements.
   - Note any new response fields as additive.
   - Note feature flags and default PDF-disabled behavior.
   - Note provider probe behavior and live-smoke limitations.

5. Final manual checks.
   - Run `eggsearch providers --json`.
   - Run `eggsearch doctor` and, if implemented, `eggsearch doctor --probe`.
   - Run `eggsearch mcp stdio` startup once with default config.
   - Run one local `web_search`, one `web_fetch`, one `repo_search`, and one `provider_status` using mock/offline tests or safe live examples.

### Acceptance criteria

- All required gates pass.
- CI is green and visible on the release commit.
- Changelog/release docs reflect new hardening work.
- No capability regression from the current MCP surface.
- The repo is ready for a production-oriented tagged release or a final release-candidate pass.

## Suggested implementation order

1. CI parity and release docs.
2. Config validation/documentation simplification.
3. Fetch adversarial tests.
4. Document-rendering golden tests.
5. Provider probe implementation and provider contracts.
6. Agent response contract/source-quality additions.
7. Local workspace hardening/performance sanity.
8. Final release verification.

This order front-loads release safety and low-risk tests before response-shape changes. Provider probing can be implemented after the safety scaffolding is stronger because it touches live-network behavior.

## Review checklist

Before considering this pass complete, verify:

- No public MCP tool was removed or renamed.
- Existing fields remain backward compatible unless a changelog note explains the migration.
- `sanitize_output = true` remains the default for search and fetch.
- `web_fetch` remains explicit and non-crawling.
- Fetch still rejects private/localhost access by default.
- Local workspace results remain `local_trusted` but not instruction-trusted.
- Provider probe failures are diagnostic, not fatal.
- Live smoke tests are opt-in/ignored and do not make default CI flaky.
- Docs examples are parse/validation tested.
- `cargo publish --dry-run --locked` passes on the final commit.
