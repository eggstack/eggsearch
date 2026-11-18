# ADR-0016: Unified Fetch Discovery and Legacy Alias Compatibility

Status: accepted

Date: 2026-11-17

Accepted: 2026-11-17

Decision owners: project maintainers

Related specification sections: plans/000-long-term-specification.md §1, §2, §7; plans/003-planning-process.md §2.1

Related decisions: ADR-0001, ADR-0006, ADR-0011

Affected subsystem roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md

## Context and forces

The current advertised inventory uses separate web_fetch and batch_fetch entries. A caller must choose between two fetch tools before deciding whether it has one target or several, and the two schemas repeat web-fetch controls. batch_fetch already accepts both web and repository targets, while web items use the same web-fetch execution path. The split therefore adds discovery and schema cost without defining distinct fetch safety boundaries.

The decision must make the common fetch path obvious, support explicit web and repository targets, retain the existing specialist repo_fetch, preserve old direct calls and SDK/Python callers, and avoid changing URL, workspace, forge, browser, PDF, cache, budget, trust, or timeout policy. The advertised count decrease from ten to nine is intentional.

## Considered alternatives

1. Keep the ten advertised names and rely on descriptions or presets. Rejected: agents still choose between separate single-web and multi-target fetch tools, and both schemas remain in canonical discovery.
2. Advertise fetch in addition to web_fetch and batch_fetch. Rejected: this increases the inventory and preserves the cognitive and context burden.
3. Replace the two advertised entries with one canonical fetch operation and keep explicit unadvertised compatibility routes for the old names. Selected: one discovery choice handles a single target or a set of targets; legacy callers retain their old argument and result contracts.
4. Use a mode flag with separate top-level web and repository payloads. Rejected: it duplicates shape selection and adds a mode discriminator instead of using one typed item union.

## Decision

1. The canonical MCP inventory is exactly nine advertised tools after M029. The canonical fetch tool replaces web_fetch and batch_fetch in tools/list and canonical tool metadata. The other eight names remain unchanged. repo_fetch remains the existing specialist operation.
2. The fetch request requires items, whose value is either one FetchItem object or a non-empty array of FetchItem objects. Each FetchItem is tagged by type with the web or repo variant. A scalar object is normalized to one ordered item internally; an array uses the same executor and result handling. Empty arrays are invalid before any fetch I/O.
3. The web variant carries the existing web-fetch controls and bounded web-target behavior. The repo variant carries the existing batch repository locator fields and bounded repo-fetch behavior. Top-level controls retain the established multi-item budgets, timeout, continue-on-error, collection offset/limit, and response-detail semantics. Per-document web offset/limit remain attached to the web target so document addressing is not confused with collection addressing.
4. Canonical fetch returns one common batch-style response envelope for every request, including a scalar single-item request. It contains an ordered results array and the established aggregate fields, warnings, telemetry, and collection-addressing metadata. Each result uses the shared per-item envelope: original index, target type, label, stable ID where available, success flag, either the existing target response or an error, returned character count, and truncation state. The inner successful web or repo response retains its current target-specific fields. The single-item form therefore returns a one-element results array rather than a different top-level response shape.
5. web_fetch and batch_fetch remain explicit, callable compatibility aliases for tools/call and the native SDK execute interface. web_fetch accepts its current top-level WebFetchArgs shape and returns its current direct web-fetch response. batch_fetch accepts its current BatchFetchArgs shape and returns its current batch response. Alias calls use the shared underlying fetch execution and policy; they do not appear in tools/list, canonical tool names, SDK discovery lists, canonical schemas, or next_actions. Compatibility dispatch is finite and name-based, not a permissive fallback for unknown tools. No deprecation warning or alias-specific behavior change is added.
6. Python and SDK convenience calls for web_fetch and batch_fetch remain callable with their existing request and response types. A canonical fetch operation and its typed request/result types are added. Canonical tool-name inventories report nine names and do not include the legacy aliases.
7. All other tool contracts remain unchanged. In particular, the change does not add or remove a distinct capability, alter repo_fetch, change any workflow planner, expose aliases through next_actions, or weaken adapter ownership, validation, sanitization, SSRF, bounds, trust, stable IDs, or error semantics.

## Supersession and scope

This ADR supersedes only the exact-ten inventory, no-removal/rename, and fixed-name constraints of ADR-0001 and ADR-0006 to the extent necessary to replace the advertised web_fetch and batch_fetch entries with fetch. It does not reopen or amend the closed tool-surface-consolidation roadmap or its M001–M007 closure records. It does not change any other canonical tool or authorize future tool additions, removals, or renames. The nine-tool target and alias boundary are implemented and qualified only through M029.

The long-term specification is amended to state the nine-tool target. Historical plans and closure records that describe the former ten-tool baseline remain accurate historical evidence and are not rewritten.

## Consequences and tradeoffs

Agents see one canonical fetch choice for explicit web or repository targets and one request grammar for single and multiple items. The inventory loses one advertised entry, and removal of the duplicated web_fetch/batch_fetch discovery schemas is expected to reduce tool-list context. M029 must measure full and compact serialized advertisements against the ten-tool baseline; if the canonical schemas do not reduce the serialized advertisement size, the milestone is not complete and the schema must be simplified without weakening target coverage or bounds.

Legacy aliases are intentionally callable without being discoverable. Consumers that rely on tools/list rather than an existing direct call must migrate to fetch. The compatibility path must preserve legacy request and response behavior so that old SDK/Python callers do not need a coordinated cutover.

## Compatibility and migration

No alias is removed from dispatch. Old tool names remain executable but are not canonical advertised names. Existing web_fetch and batch_fetch inputs and output JSON remain structurally compatible, including aliases already accepted by their current request types. The new fetch output has a single shared outer results envelope; it is not substituted for the legacy outputs. This is a canonical MCP discovery/schema change and must be reflected in tool-matrix, docs contract tests, CodeGG integration documentation, SDK/Python contract surfaces, and tool-surface evaluation fixtures in M029.

## Security and operational implications

The unified handler must validate every item and all request-level bounds before starting remote or local work. The web variant keeps the existing HTTP(S)-only, redirect, resolved-address, browser-profile authorization, PDF, cache, screenshot, and timeout controls. The repo variant keeps the existing forge/workspace path validation, bounded reads, ref resolution, and trust rules. Mixed requests preserve input order and existing partial-failure behavior. Screenshot image blocks remain out of JSON, bounded, unpersisted, and associated with the correct web item. No new dependency, network fallback, route, crawl, or retry policy is authorized.

## Verification and status

ADR-0016 is accepted and authorizes planning and implementation of the contract, not implementation or closure. M029 is the owning capability milestone. It must update the code-derived inventory guards and all affected docs, exercise aliases over the actual MCP call path while proving their absence from tools/list and next_actions, preserve SDK/Python parity, and record exact-candidate full and compact schema-size evidence. Closure requires the M029 record, roadmap status, and registry status to be updated together on the exact candidate SHA.
