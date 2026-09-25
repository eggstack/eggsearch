# ADR-0001: Ten-Tool MCP Surface and Adapter Ownership

Status: accepted

Date: 2026-09-25

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md#2`
- `plans/000-long-term-specification.md#3`

Affected subsystem roadmaps:

- `plans/subsystems/tool-surface-consolidation-roadmap.md`
- `plans/subsystems/maintenance-codegg-quality-roadmap.md`

## Context

Eggsearch exposes metasearch, fetch, and evidence capabilities to untrusted
agent callers. Tool sprawl would fragment trust review and CodeGG integration;
a single mega-tool would collapse distinct evidence budgets and domain policy
into one ambiguous surface.

## Decision drivers

- Stable CodeGG integration against a fixed tool inventory.
- Distinct budgets for discovery, fetch, repo/security/research workflows,
  and evidence packaging.
- One enforceable ownership boundary for validation and response shape.

## Considered options

### Option A — Fixed ten-tool surface with adapter ownership

Keep exactly the 10 documented tools. All tools call
`MetadataSearchAdapter`, never engines directly. Validation and response
shape live in `src/mcp/tools/` with shared `common.rs`/`canonical.rs`.

### Option B — Open-ended tool growth

Allow new tools per provider or workflow without a contract gate.

### Option C — Single mega-tool

Collapse search/fetch/evidence into one tool with a mode flag.

## Decision

Option A. The surface stays exactly 10 tools unless the tool-matrix, docs
contract tests, and CodeGG integration docs move in the same change.
`static_guards.rs` enforces the count and the adapter-only call path.

## Consequences

Positive: stable downstream contract; reviewable trust boundary; code-derived
inventories stay truthful.

Negative: genuinely new evidence classes require a coordinated multi-surface
change.

Neutral: disclosure/schema slimming happens inside the fixed surface (see the
tool-surface consolidation workstream), not by adding tools.

## Compatibility and migration

No migration. Additive input/output fields remain backward-compatible; tool
removal or renaming requires an explicit compatibility plan.

## Security implications

New tools bypassing the adapter would bypass sanitization, bounds, and
capability-skip accounting. The guard fails closed on direct engine calls.

## Verification

`tests/docs_tool_names.rs`, `tests/mcp_tool_contract.rs`, and
`tests/static_guards.rs` prove registry parity and ownership.

## Supersession

None.
