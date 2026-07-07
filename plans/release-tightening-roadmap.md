# Release Tightening Roadmap

## Purpose

This roadmap is a closure plan for moving eggsearch from release-candidate quality to production-ready public release quality. It intentionally avoids expanding the stable MCP tool surface. The current release surface is already broad enough for codegg and coding-agent use: live web metasearch, bounded fetch, batch fetch, provider diagnostics, repo search/fetch/map, security search, research search, and deterministic evidence bundles.

The goal of this line of work is to close the remaining release-risk items found during the current repo review:

1. fetch and SSRF hardening around special-use IP ranges;
2. provider-status and routing diagnostics precision;
3. fetch response schema and raw-text exposure clarity;
4. provider health observability and cooldown semantics;
5. release-gate verification on the exact release commit;
6. operator threat-model and provider-setup documentation;
7. final public-release polish.

Treat this as a production hardening pass, not a feature roadmap.

## Current Baseline

The repository is already in a relatively strong shape:

- `README.md` documents the ten stable MCP tools and the safety posture.
- `Cargo.toml` has release metadata, a minimal default feature set, optional `pdf`, and test-only `mock`/`live-smoke` features.
- `Makefile` defines a local release-style gate with formatting, clippy, all-features tests, no-default tests, schema/corpus tests, and documentation contract tests.
- `.github/workflows/ci.yml` mirrors the same release gate with feature matrix checks, clippy, fmt, release build, publish dry-run, and docs build.
- `docs/release.md` documents the authoritative release sequence and requires direct CI verification on the release commit.
- Fetch is already explicit, bounded, no-JS, no-crawl, no-recursive-fetch, and blocks localhost/private network access by default.
- Provider inventory is broad and includes generic web, repo/code, issue/release, security, package registry, scholarly, local workspace, and Sourcegraph-style providers.

The remaining issues are mainly closure and precision issues. The most important code-level risk is special-use address handling in fetch. The most important release-process risk is that the latest release-candidate commit still needs direct green CI verification before tagging.

## Non-goals

Do not add new MCP tools in this line of work.

Do not add additional provider backends unless a small change is required to fix an existing provider-status inconsistency.

Do not add crawling, recursive fetch, JavaScript rendering, browser automation, or page interaction. The explicit single-URL fetch posture is a security and predictability strength.

Do not make live smoke tests mandatory in default CI. Live provider behavior is third-party state and should remain opt-in. A failed live smoke test should block release only when investigation shows a local regression.

Do not introduce persistent provider-health state unless it is small, optional, and explicitly documented. Process-local health is acceptable for this release if the semantics are precise.

## Milestone 1: Fetch and SSRF Hardening Closure

### Objective

Make fetch target validation production-grade by explicitly blocking non-global/special-use address ranges by default for direct URLs, redirected URLs, and DNS-resolved targets.

### Motivation

The fetch path already validates HTTP(S) scheme, embedded credentials, localhost/private literals, DNS resolution, redirect targets, and address pinning. The gap is policy completeness. Current IPv4 blocking relies heavily on standard-library helpers and does not explicitly cover all relevant special-use ranges. Comments and behavior must align.

### Required work

- Replace the current narrow IPv4 predicate with an explicit deny policy for non-global/special-use networks.
- Cover at least:
  - `0.0.0.0/8`;
  - loopback `127.0.0.0/8`;
  - RFC1918 ranges `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`;
  - carrier-grade NAT `100.64.0.0/10`;
  - link-local `169.254.0.0/16`;
  - IETF protocol assignments and documentation/test ranges where appropriate, including `192.0.0.0/24`, `192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`;
  - benchmarking `198.18.0.0/15`;
  - multicast `224.0.0.0/4`;
  - reserved `240.0.0.0/4`;
  - broadcast `255.255.255.255`.
- Extend IPv6 blocking documentation and tests for loopback, unspecified, unique-local, link-local, multicast, documentation, and IPv4-mapped forms.
- Ensure the same validation path is used for initial URLs, rewritten raw code-host URLs, redirected URLs, and DNS-resolved targets.
- Verify that address pinning still reuses the same validated resolution result for the outbound request.
- Update `docs/safety.md` with exact semantics of `allow_private_network` and `allow_localhost`.

### Acceptance criteria

- Direct fetch to any blocked literal address fails by default.
- Redirect to any blocked literal or DNS-resolved address fails by default.
- DNS resolution to a blocked address fails by default.
- IPv4-mapped IPv6 addresses are evaluated through the IPv4 policy.
- `allow_localhost` does not accidentally imply broad private network access.
- `allow_private_network` does not accidentally imply localhost access unless explicitly intended and documented.
- Unit and integration tests cover the special-use policy.
- Comments and documentation accurately describe implemented behavior.

## Milestone 2: Provider Status and Routing Diagnostics Cleanup

### Objective

Make provider diagnostics precise enough for an agent or operator to distinguish disabled, unknown, unconfigured, missing-credential, unhealthy, and non-routable providers without parsing prose.

### Motivation

The provider model now exposes IDs, API provider IDs, provider capabilities, configured state, routable state, and skip reasons. The remaining gap is semantic precision. A known-but-disabled provider should not be described as unknown. A missing API-key environment variable should not be collapsed into a generic provider-not-configured message when a more useful cause is known.

### Required work

- Introduce or normalize a machine-readable skip-reason taxonomy.
- Suggested skip reason codes:
  - `disabled`;
  - `unknown_provider`;
  - `not_built`;
  - `not_configured`;
  - `missing_api_key_env`;
  - `api_key_env_unset`;
  - `invalid_base_url`;
  - `local_backend_unavailable`;
  - `feature_not_compiled`;
  - `cooldown`;
  - `provider_error`.
- Preserve a human-readable `skip_reason` message if desired, but add or standardize a canonical code field for MCP/CLI consumers.
- Make `enabled`, `configured`, `routable`, and `default` mean one thing consistently across CLI output, MCP `provider_status`, docs, and tests.
- Update provider-status tests to cover disabled known providers, unknown configured providers, API providers with missing env var names, API providers with unset env vars, SearXNG missing base URL, and local workspace disabled/unavailable states.
- Update docs-provider inventory tests and examples.

### Acceptance criteria

- Known disabled providers report `disabled`, not `unknown_provider`.
- API providers with missing `api_key_env` and API providers with unset environment variables are distinguishable.
- SearXNG with `enabled = true` but no usable base URL is reported distinctly.
- Local workspace unavailable state is distinct from disabled state.
- CLI JSON and MCP provider-status output use the same state vocabulary.
- Docs describe the state vocabulary.

## Milestone 3: Fetch Response Schema and Raw Text Exposure Audit

### Objective

Ensure fetch-related tool outputs are bounded, predictable, and explicit about raw text, rendered text, truncation, and intended consumer use.

### Motivation

The fetch client intentionally keeps a larger internal `raw_text` budget for consumers such as `repo_fetch`, while the user-facing `text` field is bounded by the request cap. This is useful but can become a token-budget and trust-boundary footgun if raw text is returned broadly without clear metadata or mode semantics.

### Required work

- Audit MCP serialization and response types for:
  - `web_fetch`;
  - `batch_fetch`;
  - `repo_fetch`;
  - `build_evidence_bundle`;
  - any tests/fixtures that include fetch payloads.
- Decide and document whether `raw_text` is public output, internal output, or mode-gated output.
- If `raw_text` remains public, add explicit metadata:
  - `raw_text_chars_returned`;
  - `raw_text_truncated`;
  - `raw_text_cap`;
  - `raw_text_intended_for` or equivalent docs language.
- If `raw_text` should be internal, ensure it is omitted or hidden from default MCP responses while preserving internal line/span selection behavior.
- Ensure `document.blocks`, `document.chunks`, `text`, and `raw_text` have separately testable truncation behavior.
- Verify prompt-injection framing and trust markers are not accidentally bypassed by raw fields in agent-facing output.
- Audit PDF fetch behavior for clear distinctions among not-compiled, disabled, unsupported, no extractable text, extraction error, and truncation.

### Acceptance criteria

- Default fetch outputs cannot unexpectedly balloon because of internal raw extraction fields.
- Every returned text-bearing field has a clear cap and truncation signal.
- Response schema makes raw versus rendered versus document-block text distinction obvious.
- Tests cover small requested `max_chars` with larger internal selection needs.
- Tests cover `metadata_only`, `text`, and `markdown` modes.
- PDF response errors and truncation states are machine-distinguishable.

## Milestone 4: Provider Health Observability and Cooldown Semantics

### Objective

Make provider health state visible, testable, and documented, while keeping the implementation process-local unless there is a clear reason to persist it.

### Motivation

The adapter already records successes, explicit failures, and inferred timeouts in a process-local provider health registry. That is a good base, but production operators and coding agents need to understand how health affects routability and how to interpret degraded provider state.

### Required work

- Define provider health lifecycle semantics:
  - what counts as success;
  - what counts as failure;
  - how empty-success responses are treated;
  - how HTTP 429/rate-limit is classified;
  - how timeouts are inferred;
  - whether cooldown affects routing or diagnostics only;
  - whether health resets on process restart.
- Expose compact provider health in `provider_status` if not already present in sufficient detail.
- Suggested health fields:
  - `health_status`;
  - `failure_count`;
  - `last_error_class`;
  - `last_error_message` or a redacted/bounded version;
  - `cooldown_until` if cooldown exists;
  - `last_success_at` and `last_failure_at` if timestamp support is acceptable;
  - `routable_now`.
- Add tests for state transitions:
  - success after failure resets or improves state;
  - timeout records timeout class;
  - 429 records rate-limited class;
  - panic during dispatch is converted to provider failure;
  - provider that never responds before deadline is recorded as timeout;
  - cooldown state appears in provider-status output.
- Update CLI and MCP docs with health-state examples.

### Acceptance criteria

- Provider health behavior is documented and test-backed.
- `provider_status` can explain why an enabled/configured provider is currently not being used.
- Cooldown, if implemented, is visible and deterministic enough for tests.
- Restart semantics are explicitly documented as process-local or persistent.
- Agent-facing warnings remain concise and do not leak unbounded provider error text.

## Milestone 5: Release Gate Verification and CI Trustworthiness

### Objective

Confirm that the exact release-candidate commit is green under the documented gate before tagging.

### Required work

- Confirm GitHub Actions run on the exact release SHA.
- Confirm required checks are enabled or manually documented if branch protection cannot be configured from repo code.
- Run or verify the documented local gate:
  - `cargo fmt --check`;
  - `cargo clippy --all-targets --all-features -- -D warnings`;
  - `cargo test --all-features`;
  - `cargo test --no-default-features`;
  - schema/corpus tests;
  - docs-contract tests;
  - `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps`;
  - `cargo publish --dry-run --locked`.
- Ensure `Makefile`, `.github/workflows/ci.yml`, and `docs/release.md` remain in sync.

### Acceptance criteria

- Green CI on exact commit.
- Local gate passes or documented equivalent is attached to release notes.
- No release-gate drift.

## Milestone 6: Operator Threat Model and Safety Documentation

### Objective

Make the trust boundary and safety model explicit for public operators and agent integrators.

### Required work

- Add or expand `docs/threat-model.md` / `docs/safety.md`.
- Document untrusted remote content, local workspace provenance, prompt-injection framing, SSRF model, no-JS/no-crawl fetch, credential handling, provider drift, and PDF caveats.
- Include safe and unsafe examples for agent consumption.

### Acceptance criteria

- A new operator can understand what eggsearch does and does not protect against without reading source code.
- Local trusted provenance is clearly distinguished from instruction trust.

## Milestone 7: Provider Setup Matrix and Failure-Mode Documentation

### Objective

Make provider configuration and expected failure modes legible.

### Required work

- Add or refine a provider setup matrix.
- Include provider ID, capability class, default state, credential requirement, config path, env var example, rate-limit expectations, and live-smoke status.
- Add example configurations for default install, codegg repo search, security search, and research search.

### Acceptance criteria

- Operators can configure common profiles without source-code inspection.
- Provider failure modes are documented in the same vocabulary used by `provider_status`.

## Milestone 8: Final Public-Release Polish

### Objective

Cut a clean public release candidate with coherent docs, metadata, changelog, and examples.

### Required work

- Audit README, docs, examples, changelog, Cargo metadata, and release docs.
- Confirm version and changelog match.
- Ensure no stale plan-only language appears in public docs.
- Confirm docs.rs metadata and crate package include paths are correct.
- Tag only after milestone acceptance criteria are met.

### Acceptance criteria

- No stale public documentation.
- No misleading provider or safety claims.
- No CI/release gate drift.
- Release notes accurately describe shipped behavior.

## Recommended Implementation Sequence

1. Complete Milestone 1 alone. It is the highest-priority security-hardening item.
2. Complete Milestones 2 through 4 together or in close sequence because provider diagnostics, routing state, and health state are coupled.
3. Complete Milestones 5 through 7 as release-operations and operator-documentation hardening.
4. Complete Milestone 8 as a final closeout pass with no new feature work.

## Definition of Done for the Roadmap

This roadmap is complete when:

- `make check` passes;
- GitHub CI is green on the exact release commit;
- fetch blocks special-use network targets by default across direct, redirect, DNS, and IPv4-mapped IPv6 paths;
- provider-status skip reasons are precise and machine-readable;
- fetch response raw-text behavior is documented, bounded, and tested;
- provider health is visible, deterministic, and documented;
- operator safety and provider setup docs are sufficient for public use;
- changelog, README, docs, Cargo metadata, and release process are aligned.
