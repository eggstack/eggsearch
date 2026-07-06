# Release Corrective Verification Pass

Date: 2026-07-06
Status: handoff plan
Priority: release-blocking verification/correction
Scope: verify and tighten the post-plan release work, especially the 34-provider expansion, provider diagnostics, docs contracts, fetch conversions, and final release gates.

## Context

The release-polish plans were implemented aggressively. The P0 items appear to have landed: component-aware local path validation, provider `routable` / `skip_reason` diagnostics, documentation contract tests, provider setup docs, quickstart docs, and release checklist. P1 fetch optimization/conversions also appear to have landed. Additionally, the provider ecosystem expanded from 18 to 34 built-in backends in one large change.

The repository should now receive a corrective verification pass. The risk profile has shifted from missing functionality to integration drift: a large number of new provider modules, new provider capability fields, new document kinds, and new docs/tests must all agree.

## Objectives

1. Confirm every release-blocking P0 item is actually closed in code, tests, docs, and CI.
2. Audit all 34 provider IDs for descriptor/config/engine/docs/test consistency.
3. Verify default no-token operation remains conservative and reliable.
4. Verify the expanded provider set does not silently enable expensive, brittle, or credential-dependent providers by default.
5. Verify new fetch document kinds and conversions are bounded, schema-safe, and tested.
6. Run the full release gate on the current `main` SHA, not on intermediate commits.
7. Produce a small corrective patch set for any inconsistencies found.

## Non-Goals

Do not add more providers in this pass.
Do not add new MCP tools.
Do not widen default network behavior.
Do not default-enable PDF extraction.
Do not turn `provider_status.probe` into live probing.
Do not add live API tests to CI.
Do not implement browser rendering, JavaScript execution, or crawling.

## Phase 1: Establish Current Baseline

### Tasks

1. Record the current `main` SHA in the verification notes or commit message.
2. Run the complete local release gate:

```bash
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock --test schema_identity_registry
cargo test --features mock --test fetch_safety
cargo test --features mock --test security_applicability_corpus
cargo test --features mock --test research_evidence_corpus
cargo test --features mock --test recipes_next_actions
cargo test --features mock --test evidence_bundle_handoff
cargo test --test docs_config_snippets
cargo test --test docs_provider_inventory
cargo test --test docs_tool_names
cargo publish --dry-run --locked
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

3. Run `make check` and confirm it covers docs-contract tests.
4. Confirm CI has jobs for all release-relevant gates. If GitHub Actions did not run for the current SHA, trigger or push a no-op correction only if needed after fixes.

### Acceptance Criteria

- All commands pass on the current SHA, or failures are documented and fixed in later phases of this pass.
- CI/Makefile coverage matches the release checklist.

## Phase 2: Provider Inventory Consistency Audit

### Provider IDs to Audit

Audit every provider in `KNOWN_PROVIDER_IDS`:

- `duckduckgo`
- `brave`
- `startpage`
- `yahoo`
- `mojeek`
- `searxng`
- `brave_api`
- `github_code`
- `github_issues`
- `github_releases`
- `gitlab_code`
- `gitlab_issues`
- `gitlab_releases`
- `gitea_code`
- `gitea_issues`
- `gitea_releases`
- `osv`
- `github_advisory`
- `nvd`
- `cisa_kev`
- `rustsec`
- `local_workspace`
- `crates_io`
- `pypi`
- `npm_registry`
- `go_pkg`
- `maven_central`
- `nuget`
- `rubygems`
- `packagist`
- `openalex`
- `crossref`
- `semantic_scholar`
- `sourcegraph`

### Required Checks Per Provider

For each provider, verify:

1. It is present exactly once in `KNOWN_PROVIDER_IDS`.
2. It has the correct `ProviderKind`.
3. `requires_api_key` is accurate.
4. `provider_configured_state` matches actual engine construction requirements.
5. `built_in_provider_descriptor` reports accurate capabilities.
6. `routable` and `skip_reason` semantics match the engine builder.
7. It is either buildable as an engine or intentionally diagnostics-only/local-only.
8. It is documented in `docs/provider-setup.md` and `docs/config.md` or explicitly exempted.
9. It is covered by descriptor tests.
10. If it has a module under `src/meta/engines`, that module has fixture/mock tests or equivalent unit tests.

### Specific High-Risk Checks

#### API-key providers

Check these provider IDs:

- `brave_api`
- `github_code`
- `github_issues`
- `github_releases`
- `gitlab_code`
- `gitlab_issues`
- `gitlab_releases`
- `gitea_code`
- `gitea_issues`
- `gitea_releases`
- `github_advisory`
- `semantic_scholar`
- `sourcegraph`

Verify:

- Missing `api_key_env` yields a stable skip reason.
- Env var named but unset yields a stable skip reason.
- Env var set to empty string yields a stable skip reason.
- Gitea/Forgejo providers require `base_url`.
- Sourcegraph requires `base_url` if the implementation cannot infer a safe default. If it currently has a default, document it explicitly.
- GitHub/GitLab enterprise `base_url` behavior is documented if supported.

#### No-key JSON/API providers

Check these provider IDs:

- `nvd`
- `cisa_kev`
- `rustsec`
- `crates_io`
- `pypi`
- `npm_registry`
- `go_pkg`
- `maven_central`
- `nuget`
- `rubygems`
- `packagist`
- `openalex`
- `crossref`

Verify:

- They are not incorrectly marked `ApiKey` if no key is required.
- They are not default-enabled unless intentionally part of a profile/default provider set.
- They classify HTTP 429/rate-limit/timeout consistently.
- They respect the adapter timeout.
- They return bounded result counts.

#### Local workspace

Verify:

- `local_workspace` is diagnostics-visible but not treated as a network engine.
- It is not included in generic network engine construction.
- It is only routable when `[local].enabled = true` and roots are usable.
- The component-aware path validation tests cover double-dot filenames and real parent-directory traversal.

### Acceptance Criteria

- Add or update tests so every provider has descriptor coverage.
- Add at least one provider-status test for the default no-token config.
- Add at least one provider-status test for a token-backed provider becoming routable when env is set.
- Add at least one provider-status test for Gitea/Forgejo missing `base_url`.
- Add at least one provider-status test for a no-key JSON provider.

## Phase 3: Engine Construction and Profile Routing Audit

### Tasks

1. Audit `build_default_engines` or equivalent engine builder.
2. Confirm every configured provider that should produce an engine does so.
3. Confirm every provider skipped by construction has an explicit structured `SkippedProvider` reason.
4. Confirm provider selection profiles remain conservative:
   - `generic` uses generic/default providers only.
   - `coding` prefers code-host/local/code search providers but degrades safely.
   - `security` prefers OSV/security providers without over-querying unrelated registries unless requested.
   - `research` prefers broad source diversity without flooding all 34 providers by default.
5. Verify `default_providers` behavior is not accidentally changed by the 34-provider expansion.
6. Verify unknown provider IDs still yield validation errors or structured warnings, not panics.

### Tests to Add or Update

- Engine builder includes each no-key engine when explicitly enabled.
- Engine builder skips API-key providers with missing env vars.
- Engine builder skips base-url-required providers without base URL.
- Default config builds only intended default engines.
- Security profile does not invoke every registry by default unless intentionally configured.
- Research profile does not invoke every scholarly/backend provider by default unless intentionally configured.

### Acceptance Criteria

- Provider routing behavior is predictable and documented.
- No provider is silently inert when `provider_status` says it is routable.
- No provider is queried when `provider_status` says it is not routable.

## Phase 4: Security Search Backend Audit

### Providers

- `osv`
- `github_advisory`
- `nvd`
- `cisa_kev`
- `rustsec`

### Tasks

1. Verify `lookup_advisory` behavior for CVE, GHSA, OSV, and RustSec IDs.
2. Verify package/version advisory queries produce normalized vulnerability metadata.
3. Verify CISA KEV is modeled as exploit-status enrichment, not as a complete vulnerability database.
4. Verify severity, aliases, affected ranges, fixed versions, timestamps, and references are bounded and normalized.
5. Verify defensive guidance remains non-offensive and does not include exploit instructions.
6. Verify failure telemetry distinguishes timeout, HTTP status, rate-limit, network, and parse errors.

### Tests

Use fixture JSON/HTTP mocks only. Do not require live network or API credentials in CI.

Required fixture tests:

- CVE lookup via NVD fixture.
- GHSA lookup via GitHub advisory fixture.
- RustSec advisory fixture.
- CISA KEV CVE enrichment fixture.
- OSV package query fixture.
- Malformed provider response produces parse failure without panic.

### Acceptance Criteria

- `security_search` can aggregate native security sources without duplicate or contradictory records.
- KEV status is clearly presented as exploit-status evidence.
- No live API dependency exists in normal CI.

## Phase 5: Registry Backend Audit

### Providers

- `crates_io`
- `pypi`
- `npm_registry`
- `go_pkg`
- `maven_central`
- `nuget`
- `rubygems`
- `packagist`

### Tasks

1. Verify each backend extracts normalized package metadata:
   - package name
   - ecosystem
   - latest version if available
   - requested/resolved version if applicable
   - repository URL
   - homepage/docs URL
   - license if available
   - changelog/release links if available
   - deprecation/yanked flags if available
2. Verify package names are URL-encoded correctly.
3. Verify scoped/namespace package names work:
   - npm scoped packages such as `@scope/name`
   - Maven group/artifact coordinates
   - Packagist vendor/package names
   - Go module paths
4. Verify missing package responses are not treated as fatal adapter failures.
5. Verify all returned strings are bounded/sanitized through existing source-card sanitization.

### Tests

Add fixtures for at least:

- crates.io normal package
- PyPI normal package
- npm scoped package
- Maven group/artifact
- Go module path
- missing package response
- malformed registry response

### Acceptance Criteria

- Registry providers improve package-aware repo/security workflows without causing default search noise.
- Query encoding is tested for namespace/scoped ecosystems.

## Phase 6: Scholarly and Sourcegraph Backend Audit

### Providers

- `openalex`
- `crossref`
- `semantic_scholar`
- `sourcegraph`

### Tasks

1. Verify scholarly providers return SourceCard-compatible results with metadata:
   - title
   - URL
   - DOI when available
   - authors when available and bounded
   - publication year/date
   - venue/source
   - abstract/snippet bounded by configured caps
2. Verify DOI lookup capability claims match actual behavior.
3. Verify Sourcegraph requires and documents base URL/token expectations.
4. Verify Sourcegraph query mapping respects repo/org/path/language/symbol hints if advertised.
5. Verify search results have stable source metadata and ranking behavior.

### Tests

Use fixtures/mocks for:

- OpenAlex query response.
- Crossref query response.
- Semantic Scholar query response with missing token behavior.
- Sourcegraph code search response.
- Sourcegraph missing base URL/token behavior.

### Acceptance Criteria

- Research mode receives structured scholarly evidence without schema drift.
- Sourcegraph is not advertised as supporting filters that are not actually enforced.

## Phase 7: Fetch Conversion and Document Schema Audit

### New/Changed Areas

- HTML render deduplication.
- `DocumentKind::Notebook`.
- `DocumentKind::Csv`.
- `DocumentKind::Xml`.
- `DocumentKind::Rst`.
- `DocumentKind::AsciiDoc`.
- CSV/TSV renderer.
- Notebook renderer.
- XML/RST/AsciiDoc detection.
- Low-power profile docs.

### Tasks

1. Verify `DocumentKind` serialization remains schema-compatible.
2. Regenerate/update schema snapshots if needed.
3. Verify clients can tolerate new document kind strings.
4. Verify HTML text/document output is unchanged except for performance.
5. Verify notebook conversion never executes code and skips/bounds outputs.
6. Verify CSV/TSV conversion is bounded by max chars/bytes and handles quoted fields.
7. Verify XML/RST/AsciiDoc are detected but rendered conservatively.
8. Verify metadata-only mode skips expensive rendering for all new document kinds where appropriate.
9. Verify low-power config snippets parse in docs contract tests.

### Tests

- HTML before/after output equivalence regression.
- Notebook with markdown/code/large output.
- Notebook invalid JSON.
- CSV with quoted fields and truncation.
- TSV with many rows and truncation.
- XML MIME type and extension detection.
- RST and AsciiDoc extension detection.
- Metadata-only path for non-HTML structured text.

### Acceptance Criteria

- New conversions are bounded and non-executing.
- Schema tests are intentionally updated and passing.
- No duplicate HTML rendering is reintroduced.

## Phase 8: Documentation and Claims Audit

### Tasks

1. Verify README provider summary matches actual default behavior.
2. Verify `docs/provider-setup.md` states which providers are default-enabled vs available when configured.
3. Remove any inaccurate “enabled by default” statements for providers that are merely built-in.
4. Verify `docs/config.md` examples parse and reflect actual key names.
5. Verify `docs/quickstart-codegg.md` has a working no-token path and a working token-backed path.
6. Verify `docs/release-checklist.md` exactly matches Makefile/CI names.
7. Verify docs avoid implying live crawling/browser rendering.
8. Verify Gitea/Forgejo, Sourcegraph, SearXNG, and GitLab self-managed base URL behavior is explicit.

### Acceptance Criteria

- Docs-contract tests pass.
- A human reader can configure default web search, GitHub coding search, Gitea/Forgejo, SearXNG, local workspace, security search, and low-power mode without reading source.

## Phase 9: Manual Smoke Matrix

Run these manually against a local build.

### Default no-token config

- `eggsearch mcp stdio` starts.
- `provider_status` reports generic providers routable and token-backed providers non-routable with skip reasons.
- `web_search` returns results or graceful provider failures.
- `web_fetch` fetches a simple HTML page.
- `web_fetch` with markdown extraction works.
- `batch_fetch` respects item/char caps.

### GitHub-token config

- `github_code`, `github_issues`, `github_releases`, and `github_advisory` become routable when configured and env var is set.
- Missing env var produces skip reason.

### Gitea/Forgejo config

- Missing base URL produces skip reason.
- Present base URL and token makes provider routable.

### Local workspace config

- `repo_search` can include local results.
- `repo_fetch` can fetch a file named `foo..bar.rs`.
- `repo_fetch` rejects `../secret.rs`.

### Security config

- `security_search` handles a CVE query.
- `security_search` handles a package/version query.
- KEV enrichment is visible when available.

### Fetch conversion smoke

- CSV fetch renders table preview.
- Notebook fetch renders markdown/code cells but not large outputs.
- XML fetch is accepted and classified.

## Phase 10: Final Release Gate

After fixes, run:

```bash
make check
cargo publish --dry-run --locked
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

Then confirm CI passes on the final pushed SHA.

## Deliverables

The corrective pass should produce:

1. Any code fixes needed for provider routability, descriptors, or engine construction.
2. Any tests needed to cover provider inventory and new backend behavior.
3. Any docs corrections needed for default behavior and provider requirements.
4. Any schema/corpus updates required by new document kinds or provider capabilities.
5. A concise commit message summarizing exact commands run and results.

## Release Decision Criteria

The repo is ready to tag only when:

- Full local release gate passes on current `main`.
- GitHub CI passes on the same SHA.
- Every provider in `KNOWN_PROVIDER_IDS` has descriptor, docs, and test coverage.
- Default no-token config remains functional.
- Token-backed providers degrade with clear skip reasons.
- New fetch conversions are bounded and non-executing.
- `provider_status` accurately reflects actual engine build/routing behavior.
