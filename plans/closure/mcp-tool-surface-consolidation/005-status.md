# Tool-Surface Consolidation M005 — CodeGG Disclosure Closure Status

Status: closed (eggsearch-side contract; CodeGG downstream tracked as handoff)

Source implementation plan:

- `plans/implementation/mcp-tool-surface-consolidation/005-codegg-progressive-disclosure-integration.md`

Source subsystem roadmap:

- `plans/subsystems/tool-surface-consolidation-roadmap.md`

Applicable ADR:

- `plans/adrs/ADR-0001-ten-tool-surface-and-adapter-ownership.md`

Repository baseline reviewed: M001 metadata stable; M002-M004 final contracts.

Interface dependency: M001 metadata (satisfied); completion against M002-M004 final contracts (satisfied at `5a99bc2`).

Implementation commit (eggsearch side):

- `5a99bc258e9378319ca1439dc7bdc53832b1ba73` — CodeGG progressive-disclosure contract (aliases, keywords, discovery text, next-action sanitization, recipe graph)

Closure candidate: `aab2e776b5177c3451713db4c737f1da8650318b` (no
disclosure-contract change since `5a99bc2`; M006/M007 add only harness
and guards).

## 1. Executive finding

M005 is closed for the eggsearch-side contract this repository owns.
The canonical registry now carries every field CodeGG's existing
catalog/BM25/deferred-tool surface needs (aliases, keywords,
`discovery_text()`, disclosure hints, related/next graphs, content
fingerprint with discovery metadata); `next_actions` are sanitized
(known tools only, capped) and the recipe graph matches the handoff
(`repo_map` after `repo_search`, `repo_fetch` after
`research_search`). Raw eggsearch tools stay hidden by default behind
stable native wrappers. The CodeGG-repo implementation (compact
`tool_search`, hydration, next-action-guided disclosure, palettes,
cache keying) is a downstream handoff owned by `dbowm91/codegg` and is
tracked, not claimed, here.

## 2. Requirement-to-evidence matrix (eggsearch-side scope)

| Requirement (plan acceptance) | Evidence | Result |
|---|---|---|
| Every capability retained and callable | Ten tools registered; M001-M004 suites green | pass |
| Compact discovery metadata available (no full schemas by default) | `discovery_text()` + purpose/use-when/not-for/keywords/aliases per tool; `discovery_text_supports_representative_tool_queries` | pass |
| Selective hydration supported | `is_known_tool()`, aliases never colliding with canonical names, fingerprint covering discovery metadata for cache correctness | pass |
| Next actions can guide follow-ups without widening authority | `sanitize_next_actions` (unknown ignored, reason-less dropped, capped at 5); `runtime_next_actions_reference_known_tools_only`; M006 Layer 2 hydration mechanics | pass |
| Initial context materially smaller | M001 descriptions + M002 schemas + M004 projections compose to 73037 definitions / 1614 instructions (M006 report) | pass |
| Existing web/repo/research/security/batch/evidence integration tests pass | `mcp_tools`, `web_search/fetch_integration`, `repo/research/security_workflow`, `batch_fetch_retrieval`, `evidence_contract`, `codegg_evidence_contract` green | pass |

CodeGG-repo workstreams (A compact `tool_search`, B hydration, C
next-action disclosure, D palettes, E structured-result
modernization, F cache correctness) are implemented in `dbowm91/codegg`
against this contract; eggsearch evidence here is the contract plus
the handoff docs, not CodeGG's test suite.

## 3. Production implementation evidence (eggsearch)

- `src/mcp/tool_contract.rs`: discovery-only `aliases`, expanded BM25
  `keywords`, `discovery_text()` (name + domain + disclosure + purpose
  + use-when/not-for + keywords + aliases + related/next), and
  `is_known_tool()`; content fingerprint extended with discovery
  metadata for cache correctness.
- `src/core/workflow.rs` (+ coverage): `sanitize_next_actions`
  (known-tools-only, `MAX_NEXT_ACTIONS` cap, reason required);
  applied in `recipe_catalog` and gap-driven builders.
- `src/meta/recipe_catalog.rs`: `repo_search` follow-ups include
  `repo_map`; `research_search` follow-ups include `repo_fetch`, so
  the guided-disclosure graph matches the handoff examples
  (`web_search` → fetch/batch; `repo_search` →
  fetch/map/batch; `research_search` → fetch/evidence; `security_search`
  → fetch/evidence).
- `tests/mcp_tool_contract.rs` extensions: alias/discovery/
  sanitization, runtime next-action validity, fingerprint determinism.
- Docs: compact discovery, run-bounded hydration, next-action guided
  disclosure, role palettes, structured-result retention, and
  fingerprint caching across `docs/codegg-integration.md`,
  `architecture/codegg-contract.md`, `docs/tool-matrix.md`,
  `docs/agent-workflows.md`, `docs/quickstart-codegg.md`,
  `architecture/mcp.md`; legacy profile/mode/workflow examples
  modernized to canonical goal/include.

## 4. Verification executed

Against candidate `aab2e77`:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --test mcp_tool_contract
cargo test --locked --all-features --test tool_surface_evaluation
cargo test --locked --all-features --test recipes_next_actions
cargo test --locked --all-features --test codegg_evidence_contract
cargo test --locked --all-features
make bench-check
make docs-check
```

Outcomes:

- `mcp_tool_contract`: 14 passed (incl. disclosure, alias, sanitization, fingerprint coverage)
- `tool_surface_evaluation` Layer 2: synthetic hydration reaches fetch + evidence tools; unknown targets ignored
- `recipes_next_actions`: pass (template keys valid, next-action tools known, priorities bounded, hints capped)
- `codegg_evidence_contract`: pass (handoff shape)
- `cargo test --locked --all-features`: 81 suites ok, 0 failed
- `make bench-check`, `make docs-check`: pass

## 5. Invariant review

Raw `mcp__eggsearch__*` tools remain hidden by default; deferred tools
discoverable but never forced; hydration/monotonicity is a CodeGG
policy property filtered through the same ceilings the contract
supports — no eggsearch change widens execution authority.

## 6. Failure and recovery review

Malicious/unknown next-action names ignored, never hydrated;
reason-less actions dropped; hydration caps deterministic. Legacy
structured-content fallback retained for older eggsearch versions.

## 7. Migration and compatibility review

Additive discovery metadata only; no wire or tool-name change. Cache
fingerprint covers name, description, input schema,
annotations/disclosure metadata, wire aliases, and surface revision,
so hydration state applies cleanly over cached base surfaces.

## 8. Security review

No new execution surface; disclosure hints advisory only; aliases are
discovery-only and can never be invoked as wire names (guard asserts
no alias collides with a canonical tool).

## 9. Documentation and operations

Handoff notes in `docs/codegg-integration.md` and
`architecture/codegg-contract.md` cover 2026 negotiation,
`structuredContent` preference, `outputSchema` validation,
`isError`-vs-transport distinction, and fingerprint caching with
backward-compatible fallback.

## 10. Residual findings

| Severity | Finding |
|---|---|
| Low, tracked handoff | Full CodeGG `tool_search`/hydration/palette/cache implementation and its metrics (initial vs hydrated bytes, search-call counts, first-tool accuracy, malformed/repair rates, completion) live in `dbowm91/codegg` and are verified by that repo's suites, not claimed here. |
| None high/medium (eggsearch scope) | No eggsearch-side regression identified. |

## 11. Roadmap disposition

M005 moves to closed (eggsearch-side contract) with this record as
controlling evidence. With M001-M004 closing in the same sequence and
M006/M007 already closed, the tool-surface workstream is complete
pending only the registry/roadmap updates in the same commit.

## 12. Registry updates

Covered in the same closure commit: M005 marked closed with closure
record `plans/closure/mcp-tool-surface-consolidation/005-status.md`.
