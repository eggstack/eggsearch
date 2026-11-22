# M029 — Unified Fetch Tool and Legacy Alias Compatibility

Status: closed

Class: capability

Source roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md

Relevant decision: accepted ADR-0016

Planning baseline: 82c862aa6024724a228a83ee2ea91323ab17c704 (branch feat/auto-20261117-044c4885; clean worktree at planning time)

## Objective

Replace the two advertised fetch entries web_fetch and batch_fetch with one canonical fetch MCP tool. Make the common case ergonomic: one FetchItem object for a single target or a non-empty array of FetchItem objects for multiple targets, with web and repo variants. Reduce the canonical tool-list context, keep repo_fetch available as the specialist, and preserve existing callers through unadvertised web_fetch and batch_fetch aliases that retain legacy request and response behavior.

The desired public inventory contains exactly nine canonical tools. This milestone is not complete until the full and compact serialized MCP advertisements are smaller than the ten-tool baseline and the nine-tool surface is demonstrated end-to-end.

## Authority and dependencies

- Interface dependency: ADR-0016 is accepted and defines the nine-tool target, input union, common result envelope, and alias boundary.
- Hard dependencies: none. Existing web, batch, repo, and fetch-addressing implementations provide stable execution paths; M029 integrates them without reopening their behavior.
- Authority order: plans/000-long-term-specification.md as amended by ADR-0016, ADR-0016, this plan, then current code evidence. Do not infer permission to change non-fetch tool contracts.
- The closed tool-surface-consolidation roadmap and its M001–M007 closure records remain closed and unchanged.

## Current implementation evidence

- src/mcp/tools/web_fetch.rs defines WebFetchArgs with a top-level URL and web controls including extraction mode, links, PDF, cache policy, focus, rendering, browser profile, screenshot, document offset/limit, and response detail.
- src/mcp/tools/batch_fetch.rs defines BatchFetchArgs with a Vec of target items and request-wide item count, per-item and aggregate character limits, timeout, continue-on-error, collection offset/limit, and response detail.
- src/core/batch_fetch.rs defines the tagged Web and Repo variants. The web item already runs through the web fetch path; the repo item carries structured locator and selection controls.
- BatchFetchResult and BatchFetchResponse already provide an ordered per-item result envelope, aggregate counts, warning fields, telemetry, and collection-addressing fields. Reuse or narrowly generalize these shapes for canonical fetch rather than introducing competing output models.
- src/mcp/server.rs registers web_fetch and batch_fetch as tool methods. src/mcp/tool_contract.rs and src/mcp/output_schema.rs define their canonical metadata and output schemas; src/mcp/projection.rs dispatches per-tool response projections.
- src/sdk.rs publishes a ten-name ToolName inventory and direct execution arms for both old names. bindings/python/python/eggsearch/types.py and __init__.py expose separate typed requests, responses, and convenience methods for web_fetch and batch_fetch.
- Contract tests include tests/mcp_tools.rs, tests/mcp_tool_contract.rs, tests/mcp_schema_slimming.rs, tests/docs_tool_names.rs, tests/tool_surface_evaluation.rs, tests/mcp_projection.rs, and tests/static_guards.rs. The canonical tool names and downstream descriptions are also recorded in docs/tool-matrix.md, docs/codegg-integration.md, and skills/eggsearch-mcp/SKILL.md.

## Contract to implement

### Input

The required items property accepts either one FetchItem object or a non-empty array of FetchItem objects. A FetchItem uses a closed discriminator: type is web or repo. Normalize a scalar to a one-element ordered internal collection before validation and execution. Reject missing items, empty arrays, unknown target variants, and malformed target-specific fields before any remote or local I/O.

The web variant carries the current web-fetch target controls. The repo variant carries the current structured repository target controls already available in batch_fetch. Request-wide controls preserve current batch semantics: maximum item count, per-item and total character budgets, timeout, continue-on-error, collection offset/limit, and response detail. Keep document offset/limit on the web FetchItem and collection offset/limit at the top level so the scopes remain unambiguous. Preserve web PDF, cache, focus, render, browser-profile, and screenshot controls; preserve repo host/ref/path/span/symbol/block/local-preference controls. Keep existing defaulting, validation, and hard caps.

### Output

Canonical fetch always returns the common batch-style envelope, even when items is one object. Use a results array in original request order and the shared aggregate counts, truncation state, total characters, warnings, structured warnings, telemetry, and collection-addressing metadata. Each result carries the shared item fields: index, target type, label, stable ID where available, ok, response or error, chars_returned, and truncated. Successful response retains the existing target-specific web or repo payload inside response. A one-item fetch is a one-element results envelope, not the direct web_fetch response.

Keep MCP image content blocks out of structured JSON. For requested web screenshots, preserve ADR-0015's bounded capture, profile/network policy, input ordering, and unambiguous item association; never screenshot repo targets or persist image bytes.

### Compatibility boundary

- tools/list, canonical ToolContract inventory, SDK canonical tool_names/ToolName::ALL, output-schema inventory, canonical aliases/next_actions, and host-facing docs advertise fetch and exactly eight unchanged tools. Neither web_fetch nor batch_fetch appears in canonical discovery or next_actions.
- A direct tools/call using web_fetch accepts the existing WebFetchArgs shape and returns the existing direct web response. A direct tools/call using batch_fetch accepts the existing BatchFetchArgs shape and returns the existing batch response. Both route through the shared implementation and preserve old JSON behavior; do not silently change them to the new envelope or add deprecation warnings.
- The SDK execute interface continues to accept both old names. Preserve source-compatible SDK enum/call surfaces where feasible; expose only the nine canonical names through canonical enumeration. Existing Python web_fetch and batch_fetch methods and request/response TypedDicts remain callable; add a typed fetch method and FetchItem/FetchRequest/FetchResponse definitions.
- repo_fetch remains separately advertised and behaviorally unchanged.

## Scope and non-goals

In scope: canonical fetch arguments and execution path; explicit legacy-name dispatch; shared result projection and schema; SDK and Python binding parity; the code-derived tool count and static guards; the cross-surface schema, docs, and tool-selection evaluation updates needed for nine canonical tools.

Out of scope: search/ranking or provider behavior; repo_fetch removal or redesign; any other tool rename, addition, removal, or schema change; changing budgets, truncation, cache, retry, SSRF, redirect, browser, PDF, workspace, trust, sanitizer, stable-ID, or Mode-Off behavior; retiring the old names from direct execution; changing default features, CLI, packaging, or releases; reopening the closed tool-surface roadmap; changes outside the declared production/test/docs surfaces.

## Invariants that cannot regress

- All MCP and SDK paths call existing policy-owning adapters/executors; the unified tool must not bypass validation, sanitization, trust framing, or bounded I/O.
- All items are validated before I/O; order, stable item identity, per-item and aggregate budgets, partial-failure semantics, warnings, telemetry, offsets, and output projection remain deterministic.
- Web-only and repo-only policies stay typed. Web URLs remain HTTP(S)-validated and SSRF-safe; forge/workspace locators retain bounded path/ref rules. Mixed arrays cannot relax either policy.
- Existing web_fetch and batch_fetch direct callers receive their legacy response shape. Legacy names are never silently surfaced as recommended next actions or canonical discovery names.
- repo_fetch and the other seven non-fetch tool contracts remain unchanged. The canonical inventory is nine, not ten or eleven.
- The canonical full and compact serialized tool-list schemas are each smaller than the captured ten-tool baseline. Record exact serialized byte counts and existing fingerprint/evaluation output; do not substitute an unmeasured assertion about context savings.
- Screenshot capture remains opt-in, bounded, out of JSON, non-persistent, and associated with its source item as required by ADR-0015.

## Ordered work packages

1. Inspect the current rmcp router and server listing path. Confirm a finite explicit tools/call compatibility route can accept web_fetch and batch_fetch without placing either in tools/list. Preserve closed-world unknown-name rejection. Stop for a focused architecture revision if the framework forces aliases into canonical discovery or requires a broad generic router rewrite.
2. Define FetchItem, FetchArgs, and FetchResponse at the narrowest shared layer. Generate the JSON Schema union for one item or a non-empty array; pin required fields, discriminator values, item and aggregate controls, and distinct document-versus-collection addressing. Reuse current web/repo executors. Keep full request validation ahead of I/O and preserve deterministic input order.
3. Register fetch as the only canonical fetch contract. Update server descriptions, canonical names, output schema, projection and next_actions. Keep legacy methods or adapter routes outside canonical contracts and tool discovery; translate old requests and return their old response values.
4. Update src/sdk.rs so canonical enumeration is nine names and fetch is executable, while direct execution and existing SDK entry points accept old names with old results. Update Python TypedDicts, typed fetch wrapper, and existing web_fetch/batch_fetch wrappers. Verify installed-wheel contract parity, not only source types.
5. Update tests and static guards to derive and assert the nine-name canonical inventory, strict next_actions, unadvertised aliases, actual alias calls, single and mixed multi-target results, error and budget behavior, projection, schema, screenshots, and SDK/Python parity. Update docs/tool-matrix.md, docs/codegg-integration.md, skills/eggsearch-mcp/SKILL.md, tool-surface evaluation fixtures, and test inventory entries where suite or contract descriptions change. Update architecture ownership text only if implementation changes a contributor rule; keep AGENTS.md an index.
6. Measure full and compact tools/list bytes/fingerprints and run the tool-surface evaluation against the pre-change baseline. If either advertised schema is not smaller, revise the fetch schema or descriptions in-scope and repeat measurement; do not close on count reduction alone.

## Expected ownership surfaces

Primary production surfaces: src/mcp/server.rs; src/mcp/tool_contract.rs; src/mcp/output_schema.rs; src/mcp/projection.rs; src/mcp/tools/web_fetch.rs; src/mcp/tools/batch_fetch.rs; src/mcp/tools/batch_fetch_item.rs; src/core/batch_fetch.rs; src/sdk.rs; bindings/python/python/eggsearch/types.py; bindings/python/python/eggsearch/__init__.py.

Test and evaluation surfaces: tests/mcp_tools.rs; tests/mcp_tool_contract.rs; tests/mcp_schema_slimming.rs; tests/mcp_projection.rs; tests/docs_tool_names.rs; tests/tool_surface_evaluation.rs; tests/static_guards.rs; tests/fixtures/tool_surface/; bindings/python/tests/test_native.py; relevant test-inventory documentation.

Host-facing docs: docs/tool-matrix.md; docs/codegg-integration.md; skills/eggsearch-mcp/SKILL.md. Keep existing unrelated migration notes and closed historical closure records untouched.

No new dependency is expected. Split modules if needed to keep ordinary files under the 1,600-line / 80-KB limits; record any unavoidable ratchet change and next-slice note under architecture/maintenance.md as part of the implementation, not this planning-only change.

## Verification commands

Focused Rust checks, all offline and keyless:

~~~text
cargo test --locked --all-features --test mcp_tools
cargo test --locked --all-features --test mcp_tool_contract
cargo test --locked --all-features --test mcp_schema_slimming
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test docs_tool_names
cargo test --locked --all-features --test tool_surface_evaluation
cargo test --locked --all-features --test static_guards
make eval-tool-surface
~~~

Python binding checks:

~~~text
cd bindings/python && python3 -m pytest tests/test_native.py
cd bindings/python && pyright python/eggsearch
~~~

Run installed-wheel smoke/schema parity in the repository's Python CI workflow on the exact candidate, since source-only tests do not prove the built extension contract. Run the canonical repo gates after focused fixes:

~~~text
make check
make docs-check
~~~

Do not use plain cargo test as the integration gate. Do not require live network or credentials. If a live MCP smoke is used to prove tools/call alias behavior, use local/mock fixtures and capture the exact-candidate evidence; ordinary CI remains network-free.

## Acceptance criteria

1. Canonical tools/list and all canonical inventories contain exactly nine names, with fetch replacing web_fetch and batch_fetch; all other eight names are unchanged.
2. fetch accepts a valid single web item, single repo item, mixed non-empty arrays, and ordered multi-target arrays. Empty arrays, invalid discriminators/fields, and over-limit requests fail before network or local reads.
3. The one-item canonical response is the common results envelope. Multi-item results preserve original order, target labels/types, per-item outcomes, aggregate warnings/telemetry/budgets, address windows, and partial-failure behavior.
4. Canonical web and repo target results match their existing executors for content, warnings, error classification, trust markers, sanitization, identity, and bounds; repo_fetch remains unchanged.
5. Direct web_fetch and batch_fetch calls through MCP and SDK/Python still accept legacy request forms and return their legacy response shapes. The actual tools/call compatibility path succeeds even though tools/list omits these names; unknown names remain rejected.
6. web_fetch and batch_fetch are absent from canonical tool metadata, output-schema inventory, SDK/Python canonical name lists, and all next_actions. fetch is the only advertised fetch target in descriptions and docs.
7. Full and compact serialized tool-list sizes are each below the exact ten-tool baseline; record byte counts, fingerprints, test outputs, and tool-surface evaluation results.
8. Web/repo, PDF, browser, opt-in screenshot, local workspace, mixed success/failure, truncation, pagination, and response-detail cases pass focused tests. Screenshot policy remains ADR-0015-compliant.
9. Focused tests, installed-wheel contract parity, make check, and make docs-check pass on the exact candidate. Documentation and static guards agree with the nine-tool contract.
10. The M029 closure record maps every criterion to exact-candidate evidence, lists failures and not-run checks, and is committed with the roadmap and registry status update.

## Stop conditions

Stop and report a design blocker rather than weakening compatibility or safety if rmcp cannot route the two names without advertising them; if legacy outputs cannot be preserved while sharing execution; if the input union loses web/repo field coverage or introduces ambiguous offset/budget semantics; if mixed-item validation can start I/O before rejecting an invalid item; if the nine-tool schema does not reduce full and compact advertisement size; or if any change requires reopening the closed tool-surface roadmap. Do not add a new tool, broaden dynamic routing, relax static guards, or change unrelated contracts to get a green test.

## Required closure evidence

Create plans/closure/agentic-retrieval-integrity/029-status.md with the exact implementation SHA, requirement-to-evidence table, before/after canonical inventories, full and compact serialized schema byte counts/fingerprints, MCP alias call and tools/list evidence, SDK and installed-wheel Python parity, budget/truncation/order/error evidence, screenshot and security invariants, all commands and outcomes, docs/static-guard state, unresolved findings, and final recommendation. Update the roadmap M029 status and plans/registry.md in the same closure commit. Qualification does not transfer to another SHA.
