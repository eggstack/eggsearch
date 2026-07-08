# Milestone 8 Plan: Final Public-Release Polish

## Objective

Prepare eggsearch for a coherent public release after the release-tightening work is complete. This is the final closeout pass. It should avoid new feature work and focus on public-facing consistency, packaging, release notes, examples, and final verification.

The release should present eggsearch as a stable MCP metasearch/fetch tool for coding agents, security research, and deep research, with bounded fetch, strong provider diagnostics, and explicit trust boundaries.

## Scope

In scope:

- README polish;
- changelog/release notes;
- Cargo metadata audit;
- package include/exclude audit;
- docs cross-link audit;
- examples audit;
- CLI help sanity check;
- MCP tool schema stability check;
- final release checklist;
- tag readiness;
- public caveat review.

Out of scope:

- new MCP tools;
- new providers;
- major architecture refactors;
- live probe implementation;
- crawler/JS/browser fetch expansion;
- persistent provider health;
- post-release roadmap implementation.

## Current State

The repository now has the core release-hardening pieces:

- stable documented MCP tool set;
- explicit fetch network policy;
- boundary tests for special-use address ranges;
- raw-text MCP omission;
- provider skip codes;
- provider health views;
- architecture docs;
- release gate docs;
- planned operator threat model and provider setup docs.

Final polish should make public materials coherent and remove any stale plan/release-candidate language from user-facing docs.

## Workstream 1: README Public-Release Audit

### Goal

Ensure the README is concise, accurate, and useful as the first public entry point.

### Review checklist

Verify README includes:

- one-sentence project description;
- intended use with coding agents/codegg;
- stable MCP tool list;
- install/build instructions;
- minimal run example;
- basic config pointer;
- safety summary;
- provider setup pointer;
- threat model pointer;
- release/docs links;
- license.

Verify README does not include:

- stale plan language;
- references to unimplemented features as if they exist;
- overbroad safety claims;
- raw local paths from development machines;
- CI claims unless currently true;
- secrets or real API keys.

### Suggested README shape

Keep README short. Push details to docs:

```markdown
## Safety

eggsearch treats remote web/provider content as `external_untrusted` and local workspace content as provenance-trusted only. Fetch is explicit, bounded, no-JS, no-crawl, and blocks private/reserved network targets by default. See `docs/threat-model.md` and `docs/safety.md`.
```

### Acceptance criteria

- README is accurate and not bloated.
- README links to detailed docs rather than duplicating them.
- No feature is advertised beyond current implementation.

## Workstream 2: Changelog and Release Notes

### Goal

Create release notes that accurately describe the shipped behavior.

### Steps

1. Inspect `CHANGELOG.md` or create one if absent.
2. Add a release section for the target version.
3. Summarize user-facing changes:
   - stable MCP tool set;
   - fetch safety hardening;
   - special-use IP blocking;
   - provider skip codes;
   - provider health views;
   - raw-text MCP omission/internal-only handling;
   - docs improvements;
   - release-gate/CI improvements.
4. Include breaking or compatibility notes:
   - raw text not included in MCP output by default, if that is new;
   - provider status response has added fields;
   - stricter fetch address blocking may reject URLs that previously passed;
   - `provider_status.probe` remains deferred/reserved.
5. Include known caveats:
   - live provider behavior can drift;
   - no JS rendering/crawling;
   - PDF extraction optional/config-gated;
   - provider health is process-local.

### Acceptance criteria

- Release notes match actual code.
- Caveats are explicit.
- No planned features are described as shipped.

## Workstream 3: Cargo Metadata and Package Audit

### Goal

Ensure crates.io package metadata and included files are correct.

### Steps

Inspect `Cargo.toml`:

- `version`;
- `edition`;
- `rust-version`;
- `license`;
- `repository`;
- `homepage` if present;
- `documentation`;
- `readme`;
- `description`;
- `keywords`;
- `categories`;
- feature flags;
- docs.rs metadata;
- package include/exclude list.

Run:

```bash
cargo package --locked --list
cargo publish --dry-run --locked
```

Inspect package list for:

- required source files;
- README;
- license;
- docs referenced by README;
- examples;
- no target/build artifacts;
- no `.env`/secret/config local files;
- no excessive plan artifacts if plans are not intended for package publication.

Decision point: decide whether `plans/` should be included in published crate packages. Plans are useful in repo but may not belong in crates.io package. If current `include` includes `plans/`, decide intentionally.

### Acceptance criteria

- Dry-run succeeds.
- Package contents are intentional.
- Cargo metadata accurately describes release.

## Workstream 4: Docs Cross-Link and Staleness Audit

### Goal

Make docs navigable and remove stale claims.

### Files to audit

- `README.md`
- `docs/safety.md`
- `docs/threat-model.md` if added
- `docs/provider-setup.md`
- `docs/config.md`
- `docs/tool-matrix.md`
- `docs/agent-workflows.md`
- `docs/release.md`
- `docs/release-checklist.md`
- `docs/architecture/overview.md`
- `docs/architecture/fetch.md`
- `docs/architecture/meta.md`
- `docs/architecture/mcp.md`
- `docs/architecture/core.md`
- `docs/architecture/codegg-contract.md`
- `AGENTS.md`
- `.skills/*` if present

### Staleness checks

Search for and inspect:

- `TODO`;
- `TBD`;
- `planned`;
- `future`;
- `not implemented`;
- `probe`;
- `raw_text`;
- `allow_private_network`;
- `provider_status`;
- `skip_code`;
- `health_views`;
- `localhost`;
- `private network`.

Not every occurrence is bad. The goal is to ensure caveats are intentional and clear.

### Acceptance criteria

- Docs cross-link cleanly.
- Stale claims are removed or rephrased as explicit caveats.
- `provider_status.probe` is consistently described as reserved/deferred.
- Safety docs and architecture docs agree.

## Workstream 5: CLI and MCP Example Audit

### Goal

Ensure examples match actual CLI and MCP schema.

### CLI examples to verify

Run or verify help output for:

```bash
eggsearch --help
eggsearch doctor --help
eggsearch doctor --probe --help
eggsearch providers --help
eggsearch providers --json
eggsearch search --help
eggsearch fetch --help
eggsearch mcp stdio --help
```

Adjust docs if flags differ.

### MCP examples to verify

For each stable tool, ensure docs contain either compact examples or a schema pointer:

- `web_search`;
- `web_fetch`;
- `batch_fetch`;
- `provider_status`;
- `repo_search`;
- `repo_fetch`;
- `repo_map`;
- `security_search`;
- `research_search`;
- `build_evidence_bundle`.

Check that examples use current field names:

- `max_results` versus old variants;
- `extract_mode` values;
- provider `skip_code` and `health_views`;
- repo locator fields;
- evidence bundle fields.

### Acceptance criteria

- No public example uses stale flags or schema fields.
- CLI help and docs agree.
- MCP examples are minimal and copyable.

## Workstream 6: Schema and Backward-Compatibility Review

### Goal

Ensure final response schemas are stable enough for codegg and external MCP clients.

### Steps

1. Run schema identity tests.
2. Inspect schema changes from release-hardening commits.
3. Confirm new fields are additive where possible:
   - `skip_code`;
   - `health_views`;
   - raw-text internal metadata if not serialized to MCP.
4. Confirm removed/hidden fields are intentional and documented.
5. Confirm deterministic IDs did not change unexpectedly.
6. Confirm warning code names and provider skip-code names are stable.

### Acceptance criteria

- Schema identity tests pass.
- Intentional schema changes are documented in changelog.
- codegg-facing contract docs match current schema.

## Workstream 7: Final Release Verification

### Goal

Tie together Milestone 5 results and public-release readiness.

### Required commands

Run the full release gate from Milestone 5 on the final commit.

At minimum:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock --test schema_identity_registry
cargo test --features mock --test recipes_next_actions
cargo test --features mock --test evidence_bundle_handoff
cargo test --all-features --test fetch_safety
cargo test --all-features --test docs_config_snippets --test docs_provider_inventory --test docs_tool_names
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo publish --dry-run --locked
```

Also confirm GitHub CI on exact final SHA if available.

### Acceptance criteria

- Local gate passes.
- CI gate passes or limitation is documented.
- Release checklist records exact commit SHA.

## Workstream 8: Tagging and Post-Release Checklist

### Goal

Prepare for a clean release tag and public announcement.

### Steps

1. Confirm version in `Cargo.toml`.
2. Confirm changelog version section.
3. Confirm release notes.
4. Confirm exact commit SHA.
5. Confirm tag name, e.g. `v0.3.4` or next version.
6. Create signed tag if project policy requires it.
7. Publish only after dry-run and CI are clean.
8. After publish, verify docs.rs build and crate page.
9. Update any downstream codegg references if needed.

### Acceptance criteria

- Tag and release notes point to the verified commit.
- crates.io/docs.rs state is checked after release.
- codegg integration docs are not left stale.

## Public Release Quality Bar

Before release, the project should satisfy:

- no known security-policy mismatch in fetch;
- no stale provider setup claims;
- no undocumented provider skip-code behavior;
- no undocumented trust-boundary escape hatch;
- no public example using old schema;
- no release-gate drift;
- no missing license/readme/package metadata;
- no unverified CI/local gate.

## Regression Risks

### Risk: Final docs polish accidentally changes semantics

Mitigation: docs-only changes should be reviewed for overclaiming. Run docs-contract tests.

### Risk: Package excludes needed docs

Mitigation: inspect `cargo package --list` and README links.

### Risk: Changelog describes plans as shipped

Mitigation: base changelog on actual commits and tests, not roadmap language.

### Risk: CI status unavailable

Mitigation: document local gate output and direct GitHub UI status. Fix workflow triggers if possible.

## Deliverables

- README polish patch.
- Changelog/release notes patch.
- Cargo metadata/package include audit patch if needed.
- Docs cross-link/staleness patch.
- CLI/MCP example corrections.
- Schema contract verification.
- Final release checklist with exact SHA and gate results.

## Definition of Done

This milestone is complete when public docs, examples, changelog, Cargo metadata, package contents, schema contract, and release checklist all describe the same shipped behavior; the full local release gate passes; GitHub CI is verified or explicitly documented as unavailable; and the repository is tag-ready without additional feature or architecture work.
