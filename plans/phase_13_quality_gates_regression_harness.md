# Phase 13: Quality Gates and Regression Harness

## Objective

Build a durable regression harness around eggsearch's agent-facing behavior. The server now has enough structured output that ordinary unit tests are not sufficient. This phase should protect MCP schemas, warning codes, identity stability, suggested-fetch reason codes, recipe contracts, fetch/document safety behavior, security applicability, research evidence analysis, local workspace safety, and evidence bundle handoff.

The goal is to make future changes safer for codegg and other harnesses by turning the expected behavior into fixtures and contract tests.

## Current context

Prior phases added:

- ten stable MCP tools;
- structured warnings;
- deterministic IDs;
- workflow recipes and next actions;
- code-aware fetch metadata;
- security applicability/remediation;
- research claims/conflicts/gaps;
- local workspace identity/trust;
- CI skeleton.

The risk is now schema drift and subtle semantic regressions.

## Non-goals

- Do not require live network tests in default CI.
- Do not snapshot entire huge responses when targeted snapshots are enough.
- Do not make tests brittle around timestamps, provider ordering outside deterministic order, or host-specific local paths.
- Do not use LLMs in tests.

## Workstream 1: MCP schema contract tests

### Required behavior

Generate or validate schemas for all public MCP tool args/responses. Contract tests should catch accidental field renames, enum rename changes, or incompatible default behavior.

### Scope

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

### Implementation guidance

- Store schema snapshots under `tests/snapshots/schemas/` or similar.
- Normalize ordering before snapshotting.
- Clearly separate breaking intentional changes from accidental changes.
- If full schema snapshotting is too noisy, snapshot selected critical paths: enums, response top-level fields, warning codes, reason codes.

### Tests

- All tool args deserialize known valid fixtures.
- All tool responses serialize known minimal fixtures.
- No stable enum serialized name changes unintentionally.
- Provider status supports `recipe_detail = none/summary/full` fixture calls.

## Workstream 2: Golden identity tests

### Required behavior

Lock down deterministic IDs for stable inputs:

- source IDs;
- fetch IDs;
- suggested fetch IDs;
- batch fetch IDs;
- locator IDs;
- document/chunk IDs;
- code span IDs;
- evidence bundle IDs.

### Tests

- Golden fixtures for each ID type.
- Cross-entity namespace test: same input string under different entity types produces different ID prefixes/hash inputs.
- URL canonicalization fixtures for default ports, fragments, percent encoding, query preservation, and `www.` policy.
- Code span ID changes with line range and symbol; stable with unrelated metadata changes.

## Workstream 3: Warning-code and reason-code registry tests

### Required behavior

Machine-readable codes are now part of the agent contract. They need registry-style tests.

### Scope

- `WarningCode` variants and serialized snake_case names.
- `AgentWarning` severity/default action mapping.
- repo suggested-fetch reason codes.
- security suggested-fetch reason codes.
- research suggested-fetch reason codes.
- next-action reason codes.
- evidence gap kinds.
- research gap kinds.
- security remediation categories.

### Tests

- Every code serializes to expected snake_case.
- No duplicate code strings.
- Every generated suggested fetch has reason code except explicit `unknown` fallback.
- Every next-action reason code is documented.
- Docs list and tests agree.

## Workstream 4: Fetch safety fixture suite

### Required fixtures

Add offline fixtures for:

- HTML with headings, links, scripts, prompt injection text.
- Markdown with headings, code fences, links, prompt injection text.
- Plain text with long content and truncation.
- Source files in Rust/Python/TS/Go with imports and symbols.
- Binary-like file fixture.
- PDF-disabled behavior fixture where applicable.
- Local workspace temp tree with traversal/symlink cases.

### Tests

- HTML/Markdown section extraction is deterministic.
- Link caps emit warnings.
- Prompt-injection markers emit structured warnings with IDs.
- Code context extraction returns expected imports/enclosing symbols.
- Local traversal/symlink escapes are rejected.
- Binary/unsupported content does not return junk text.

## Workstream 5: Security applicability regression corpus

### Required corpus scenarios

Add scenario fixtures for:

- affected exact version;
- unaffected exact version;
- unknown range syntax;
- missing version -> insufficient evidence;
- lockfile transitive dependency;
- manifest direct dependency;
- fixed version available -> upgrade remediation;
- no fixed version -> manual review/monitor;
- KEV present;
- KEV absent-not-proof;
- exploit discussion source -> defensive urgency only.

### Tests

- Applicability status and confidence match fixture expectations.
- Remediation category matches expected category.
- Fixed versions appear only when advisory metadata supplies them.
- Text safety validation does not include exploit instructions.
- Unknown/insufficient evidence never becomes `not_affected`.

## Workstream 6: Research evidence regression corpus

### Required scenarios

- architecture decision with primary docs and counterpoint;
- library comparison with benchmark source;
- migration planning with changelog/release source;
- security research with advisory source;
- only secondary sources;
- stale source set;
- conflicting evidence unresolved;
- no primary source found.

### Tests

- Claims are specific retrieval-state statements.
- Claim confidence reflects source quality/count.
- Conflicts link to correct source IDs.
- Evidence gaps match missing source classes.
- Suggested fetches prioritize primary/counterpoint/benchmark sources.
- Evidence bundle preserves research claims/conflicts.

## Workstream 7: Workflow recipe and next-action tests

### Required behavior

Workflow recipes should be treated as API contract.

### Tests

- Exactly the expected built-in recipe IDs exist unless intentionally changed.
- Every recipe tool name is a registered MCP tool.
- Every recipe support status is stable for mocked capability sets.
- Summary detail omits steps/fallbacks/trust notes.
- Full detail includes steps/fallbacks/trust notes.
- None detail omits recipes.
- Next-action hints are capped at 5 and reference valid tool names.
- Next-action input templates deserialize when placeholders are replaced with fixture values.

## Workstream 8: Evidence bundle handoff tests

### Required behavior

Evidence bundles are the handoff primitive for agents. They must preserve enough metadata without exploding size.

### Tests

- Bundle preserves source/fetch IDs.
- Bundle preserves code-span IDs and selected spans.
- Bundle preserves security verdict/remediation metadata when provided.
- Bundle preserves research claims/conflicts when provided.
- Bundle preserves local workspace trust/dirty metadata.
- Bundle gap analysis detects missing fetches, all-external-untrusted, missing tests/examples/manifests, local dirty/mismatch, and security/research gaps.
- Bundle output obeys max source/fetch/char limits.

## Workstream 9: CI and test ergonomics

### Required behavior

CI should execute stable offline gates:

- fmt;
- clippy all targets/all features;
- tests all features;
- tests no default features;
- schema/fixture corpus tests;
- optional publish dry-run.

Live/network tests should be ignored by default and documented.

### Acceptance criteria

- Developers can run one documented command or small command set for all offline checks.
- CI covers no-default-features if supported.
- Fixture tests do not require network.
- Snapshot updates require deliberate command/process.

## Workstream 10: Documentation

Update docs for:

- regression harness layout;
- how to add a new fixture;
- how to update snapshots intentionally;
- live smoke test policy;
- stable code registries;
- what counts as a breaking schema change.

## Acceptance criteria

- MCP tool schemas and key enums have contract coverage.
- Deterministic IDs have golden tests.
- Warning/reason codes have registry tests.
- Fetch safety behavior has offline fixtures.
- Security and research behavior have regression corpora.
- Evidence bundle handoff is covered end-to-end.
- CI runs the offline quality gate.
