# Plan 002 — Agent-Facing Schema Slimming Without Capability Loss

Status: implementation plan
Scope: eggsearch MCP request schemas and compatibility layers
Depends on: Plan 001

## Objective

Reduce the token cost and selection ambiguity of eggsearch's model-facing JSON schemas while preserving all existing capabilities, tool names, backend planners, provider controls, and advanced/operator functionality.

The key constraint is that capability must remain accessible. The change is therefore a schema/projection refactor, not feature deletion.

## Problem statement

Several current MCP tools expose many orthogonal knobs directly to the model even though ordinary agents should rarely control them. The largest concentration is `repo_search`, whose schema combines repository location, lexical/code filters, source-selection booleans, provider selection, timeouts, package/version semantics, local workspace behavior, error-investigation mode, and a broad workflow taxonomy. `research_search` and `security_search` each expose their own partially overlapping workflow vocabulary. `web_search` likewise includes infrastructure/debug fields such as explicit provider routing and timeout controls.

This increases:

- context occupied by tool definitions;
- probability of choosing the wrong semantic path;
- malformed/contradictory requests;
- documentation burden;
- adapter translation complexity in CodeGG.

## Non-goals

- Do not remove existing request fields from the backend structs in one breaking change.
- Do not merge `repo_search`, `research_search`, or `security_search` implementations.
- Do not make provider selection impossible for advanced clients.
- Do not change search/fetch safety semantics.
- Do not require a mega-tool with a large union schema.

## Design principle

Separate the **canonical agent schema** from the **full compatibility/operator schema**.

The canonical schema should contain high-value semantic controls. Advanced infrastructure controls remain accepted by the runtime for backward compatibility but should not occupy the ordinary model-facing schema when the MCP library/host path permits a projected schema.

If `rmcp` macros make separate advertised and deserialization schemas awkward, implement explicit tool definitions/router metadata rather than weakening compatibility. The runtime argument structs may remain supersets.

## Canonical schema targets

### `web_search`

Ordinary agent-facing inputs should prioritize:

- `query`
- `intent`
- `freshness` or `date_range`
- `include_domains` / `exclude_domains`
- `language` / `region` only if they materially affect current providers
- `excerpt_count`
- `max_results`

Hide or mark advanced/deferred from ordinary schema where technically possible:

- explicit `providers`
- `timeout_ms`
- safe-search infrastructure override if safe-search is better enforced by host/operator policy

The runtime must continue accepting these fields for existing callers.

### `repo_search`

Introduce one canonical semantic selector, tentatively `goal`, while retaining existing aliases during migration.

Suggested canonical values:

- `understand`
- `architecture`
- `debug`
- `migration`
- `security`
- `dependency`
- `performance`
- `compare`
- `pre_change`
- `post_change`

Map legacy controls deterministically:

- `profile=coding` → default codebase-oriented behavior
- `profile=security` → `goal=security`
- `profile=research` → broader source diversity without changing domain
- `mode=exact_error` → `goal=debug` plus exact-error behavior
- legacy `workflow` values → corresponding canonical goal

Do not silently reinterpret conflicting legacy and canonical fields. Define precedence explicitly and return a repairable validation result when incompatible values are supplied together.

Replace the forest of independent source booleans in the canonical schema with a compact source selector, for example:

```json
{"sources":["code","docs","registry","issues","pull_requests","releases","examples","changelog","migration_guides"]}
```

Workflow/goal defaults should choose sensible source sets, so the agent usually omits `sources` entirely.

The runtime compatibility layer must continue accepting the old booleans and translate them into the same internal request representation.

Keep repository/package fields available because they encode real task semantics:

- host/owner/repo/org
- path/file/language/symbol
- ecosystem/package/version/version_requirement/package_namespace
- compare_version when migration/comparison needs it
- include_local if the caller genuinely needs to override local evidence

Provider and timeout controls should move out of the ordinary schema if feasible.

### `research_search`

Normalize its workflow vocabulary against the canonical semantic vocabulary established above. Do not force the internal `ResearchWorkflow` type to become the same enum as repository workflows; use a translation layer.

Canonical agent-facing fields should be approximately:

- `query`
- `domain`
- `goal`/`workflow`
- `depth`
- `compare_targets`
- `constraints`
- `source_types` when the caller needs to override defaults
- `freshness`

Prefer a source-set/list over multiple `include_*` booleans. Existing booleans remain accepted as compatibility aliases.

### `security_search`

Keep the fields that make this tool genuinely distinct:

- query
- package/ecosystem/version
- CVE/GHSA/OSV/RustSec identifiers
- severity threshold
- applicability assessment
- dependency files

Normalize generic workflow selection to a smaller security-centric semantic surface. Most calls should not need a generic ten-value workflow field because the tool itself already establishes the domain.

Convert optional retrieval decorations (`include_kev`, exploit context, defensive guidance, vendor advisories) into either sensible defaults or a compact `include` set. Preserve old fields as aliases.

### Fetch tools

Do not aggressively redesign `batch_fetch`; its tagged `web`/`repo` item model is already clear. Review only for schema-size savings through shared definitions and description shortening.

`web_fetch`, `repo_fetch`, and `repo_map` should keep explicit locators. Advanced cache/render/browser/PDF controls may remain because they expose genuinely different fetch behavior, but descriptions should separate common inputs from advanced inputs.

## Internal implementation

### 1. Add canonicalization helpers

Introduce shared translation helpers under an appropriate core/MCP boundary, for example:

- `canonicalize_repo_search_args`
- `canonicalize_research_search_args`
- `canonicalize_security_search_args`

They should convert canonical and legacy representations into the existing typed request structs.

Avoid adding additional branching inside the large planner/dispatch modules.

### 2. Define conflict rules

For every canonical field with legacy aliases, document and test:

- equivalent combinations accepted;
- canonical value wins only when the legacy value is absent or equivalent;
- incompatible combinations return an actionable repair message;
- unknown values enumerate canonical accepted values without dumping implementation aliases unnecessarily.

### 3. Measure schema size

Add a deterministic schema-size test/benchmark that serializes all advertised MCP tool definitions and records:

- bytes per tool;
- total schema bytes;
- total description bytes;
- property count per tool.

Set a regression threshold after the slimming implementation. The threshold should be strict enough to prevent accidental return to the current oversized surface but allow additive fields when justified.

### 4. Keep full capability reachable

Document one supported path for advanced callers to discover/use every legacy field. Depending on MCP/library constraints this may be:

- additive advanced schema metadata;
- a host-side full-schema mode;
- compatibility acceptance of non-advertised JSON properties;
- separate operator docs/examples.

Do not add a second public tool merely to expose advanced arguments unless no lower-cost compatibility mechanism exists.

## Testing

Add table-driven translation tests for all legacy-to-canonical mappings, especially:

- `repo_search` profile/mode/workflow combinations;
- source boolean → source-set translation;
- exact-error behavior;
- security review and migration workflow aliases;
- research source include booleans;
- security include booleans;
- canonical/legacy conflict errors;
- old CodeGG request fixtures.

Update schema-contract snapshots only after verifying that all historical request fixtures still deserialize and execute equivalently.

## Acceptance criteria

- All ten tools and all existing backend capabilities remain callable.
- Ordinary advertised schemas are materially smaller; target at least a 30% reduction in serialized definition bytes for the four search tools combined unless measurement shows a better justified threshold.
- `repo_search` no longer asks an ordinary agent to reason independently over `profile`, `mode`, a broad `workflow` enum, and many source booleans.
- `research_search` and `security_search` use vocabulary consistent with the same high-level task semantics.
- Existing CodeGG compatibility fixtures continue to pass.
- Legacy clients using old fields remain supported for at least one documented deprecation window.
- New validation errors tell an agent how to repair conflicting or invalid semantic controls.

## Verification

```bash
cargo test --all-features
cargo test --features mock provider_workstream_regression
cargo test --features mock provider_request_contract
make docs-check
make bench-check
```

Include before/after schema-size numbers in the closure/status document for this plan.