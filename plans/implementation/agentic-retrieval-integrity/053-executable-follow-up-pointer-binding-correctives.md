# M053 — Executable Follow-Up Pointer Binding Correctives

Status: ready
Date: 2026-11-26
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective to M052)
Baseline: commit `32621b7081beb649dad82af7ec8a1848f5e2c8f4` (M052 implementation `02c4163`, post-commit evidence `51ef2d2`)
Corrects: M052, `plans/implementation/agentic-retrieval-integrity/052-agent-facing-contract-ergonomics.md`
Closure: `plans/closure/agentic-retrieval-integrity/052-status.md`
Audit: post-closure findings recorded at `32621b7` and in the M052 closure record.

Decision authority: accepted ADR-0022 §3 (an emitted pointer must be executable without reconstructing its locator); M052 §Contract and WP3 (binding only values known without ambiguity); `plans/003-planning-process.md` §7 (corrective passes are new plans).

## Objective

Correct the two P2 follow-up-pointer binding defects confirmed by the read-only
audit of M052. A package-based applicability action must carry the actual
package locator, and a single-source repository fetch action must carry a path
when that exact source card already exposes one. Keep M052 closed; this plan is
a separate corrective milestone and authorizes no code changes by itself.

## Evidence and corrective rationale

| Finding | Current evidence | Why M052 verification missed it |
|---|---|---|
| Package-only `security_search` can emit an unusable `inspect_applicability` action. The action uses `query: "<package>"`; the MCP seam binds only `req.query`, which is empty for a valid package+ecosystem request. The action omits `package` and leaves `ecosystem`/`version` as placeholders. | `src/meta/recipe_catalog.rs:235-243` builds the action; `src/mcp/tools/security_search.rs:105-128,317-331` accepts an absent query and binds only query/URL; `src/core/security.rs:1536-1552` allows an empty query when a strong identifier (including package+ecosystem) exists. `SecuritySearchArgs` has explicit package, ecosystem, and version fields. | The M052 tests and live smoke did not exercise a package-only applicability action. A non-empty query in other security requests masks the empty-query binding defect; the package locator was not asserted in the emitted template. |
| A single-source `repo_search` `fetch_top_source` action keeps `<path>` even when its one source card has `metadata.code.path`. | `src/meta/recipe_catalog.rs:167-175` creates the action; `src/mcp/tools/repo_search.rs:461-473` binds query/owner/repo and a single-card URL but no path; `SourceMetadata.code` and `CodeMetadata.path` are at `src/core/source_card.rs:293-321` and `src/core/code_metadata.rs:57-70`. | M052 exercised request-field binding and other repository actions, but not a single-source `fetch_top_source` response whose referenced card carries a known code path. |

The closure record explicitly states that these two template shapes were not
covered and that remediation belongs in a new corrective plan. No M052
requirement is reopened beyond correcting these evidenced cases.

## Contract and invariants

- Keep the nine advertised tools, twelve callable names, aliases, accepted
  argument sets, error codes, providers, default sets, retrieval behavior,
  source IDs, and trust framing unchanged.
- Keep `next_actions` and `input_template` response shapes unchanged. This
  work changes values in already-existing response fields only.
- For `inspect_applicability`, use the target tool's actual argument names:
  `package`, `ecosystem`, and `assess_applicability: true`. Include
  `version` only when the request supplied a non-empty version. Do not map a
  package value into `query`, and do not serialize an empty query. Since
  `security_search` accepts package+ecosystem as a strong identifier, the
  query field may be omitted. Emit the action only when a non-empty package and
  ecosystem form a valid locator; otherwise omit it rather than inventing a
  value or retaining an unresolved placeholder. Do not infer a package from
  prose or untrusted advisory content.
- Bind `fetch_top_source.items[0].path` only from the `CodeMetadata.path`
  attached to the exact single source card named by the action's sole
  `source_id`. Never borrow a path from another card or bind one for a
  multi-source action. If that source card has no unambiguous path, retain the
  existing safe behavior; do not add URL parsing or locator heuristics here.
- Bound values continue through the existing field sanitization and length
  policy. A rejected or empty value must not be emitted as a substitute.
- No input-schema, output-schema, advertised-description, or tool inventory
  change is expected. Therefore no fingerprint re-pin is authorized or
  expected. If implementation requires any advertised schema or description
  change, stop and split/register that scope rather than re-pinning here.
- Static recipe prose and unrelated action templates are out of scope.

## Work packages

### WP1 — Bind package applicability targets (S)

At the `security_search` response seam, construct or bind the applicability
action from the resolved request fields rather than treating `query` as a
package alias. The action is emitted only when the exact package+ecosystem pair
is present and non-empty. Include the known version when supplied; omit the
optional version otherwise. Use the current `assess_applicability: true`
behavior and the existing action reason/tool. Do not add a public argument or
change security-search validation.

Tests:

- In `tests/security_workflow.rs`, add a deterministic tool-response regression for `package: "axum"`,
  `ecosystem: "crates.io"`, no `query`, and an applicability suggestion.
  Assert the emitted action has `package` and `ecosystem`, carries a
  supplied version when present, omits an absent version, contains no empty
  `query` or unresolved placeholder, and deserializes as the existing
  `SecuritySearchArgs` shape.
- Assert the action's target remains directly callable without a query.
- In `tests/security_applicability_regression.rs`, assert a package+ecosystem
  request validates with an empty query and package without ecosystem remains
  rejected at core validation. This captures the MCP/core validation mismatch
  without loosening the security contract.
- Assert an applicability action is omitted when no complete package+ecosystem
  locator is available; an ordinary query or CVE identifier must not be
  misrepresented as a package.
- Preserve the existing applicability reason code, priority, source IDs, and
  sanitization behavior for all other `security_search` actions.

Acceptance: the package-only request produces a directly callable follow-up
whose known locator fields exactly match the request, with no placeholder or
empty-query defect. Stop if this requires a new public field, error code,
provider, or change to retrieval/applicability semantics.

### WP2 — Bind the single-source repository path (S)

Extend the existing request/response binding context in `src/core/workflow.rs`
with source-ID-keyed path data, or an equivalent narrow mechanism. In
`repo_search`, collect `metadata.code.path` alongside the already collected
URL for each stable source ID. The binder may fill `path` only for an action
with exactly one matching source ID, preserving the current single-candidate
rule used for URL binding. Do not change owner/repo/ref resolution or the
multi-source `fetch_multiple_focused` selection behavior.

Tests:

- Add a deterministic `repo_search` response fixture containing one code
  source card with `metadata.code.path`; assert the `fetch_top_source`
  repo item contains that exact path plus the already-bound owner/repo fields.
- Assert the generated item round-trips through the existing `fetch` argument
  deserializer without changing its locator.
- Add binder coverage proving a path from source A is never applied to an
  action naming source B or to a multi-source action; preserve existing URL
  single/multiple-candidate tests.
- Cover a card without `metadata.code.path` to ensure no guessed path is
  introduced and unrelated template fields are unchanged.

Acceptance: when a single action references a source card with one known code
path, the path is bound from that card and the emitted repo item remains
callable. No path is copied across sources, and no repository search, fetch,
identity, or path-parsing behavior is otherwise changed.

### WP3 — Contract documentation and candidate closure evidence (S)

Update the existing follow-up-pointer explanation in `architecture/mcp.md`
and its concise summary in `docs/tool-matrix.md` to distinguish request
package/ecosystem binding from the free-text query field and to state that a
single-source repo path is bound only from that card's structured code metadata.
Add one `CHANGELOG.md` `Fixed` entry for the two pointer corrections.
Update `docs/test-inventory.md` and `architecture/testing.md` if their
suite totals or ownership entries change; do not create a new suite merely for
these regressions. No `AGENTS.md` change is expected.

Acceptance: documentation agrees with the emitted response behavior, the
canonical inventory and schema fingerprint remain unchanged, and closure
record evidence names both regression tests and records any deviations or
unverified live observations.

## Ordering and dependencies

1. WP1 — security applicability target binding.
2. WP2 — source-card path binding; independent of WP1.
3. WP3 — documentation, full verification, and closure record.

Dependencies:

- Hard: none.
- Interface: M052's closed pointer contract and accepted ADR-0022 §3.
- Operational: none. Network/live checks are supplemental only; routine tests
  must remain keyless and network-independent.

M053 is dependency-ready. M049 remains a separate active milestone and is not a
prerequisite.

## Verification

Focused, on the candidate:

```bash
RUSTUP_TOOLCHAIN=1.89.0 cargo fmt --all
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib 'binding_tests::'
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test mcp_tools
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test security_workflow
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test security_applicability_regression
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test repo_workflow
```

Canonical and documentation gates:

```bash
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
RUSTUP_TOOLCHAIN=1.89.0 make packaging-check
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test static_guards planning_status
python3 packaging/check-planning-consistency.py
git diff --check
```

Live, supplemental only: if a keyless release-binary run is available, capture
the exact package-only `security_search` and single-source `repo_search`
responses over MCP and save the emitted `next_actions` in the closure record.
Do not make network access a CI requirement; if the live probe cannot produce
the relevant result shape, record it as unverified and rely on the deterministic
regression tests for the contract gate.

## Compatibility and migration

The change is limited to values inside existing `next_actions` fields. Hosts
that previously received placeholders receive concrete request/source metadata
instead. No caller must change an input, and no tool schema, alias, inventory,
provider, default, output field, error code, or MCP fingerprint changes. The
existing `suggested_fetches` contract and all other M052 binding rules remain
in force.

## Acceptance criteria

- A package+ecosystem `security_search` call without `query` emits a
  callable `inspect_applicability` action with the package locator and no
  empty query or unresolved known-value placeholder.
- The applicability action includes a known version and omits a missing one;
  it is not emitted when its required package+ecosystem locator is unavailable.
- A single-source `repo_search` `fetch_top_source` action binds the exact
  `metadata.code.path` from its referenced card.
- A missing or ambiguous path is never fabricated or taken from a different
  source card; multi-source actions do not inherit a single-card path.
- Regression tests fail against the prior behavior and pass on the candidate.
- Nine advertised tools, twelve callable names, aliases, arguments, error
  codes, providers, defaults, trust behavior, and fingerprint remain unchanged.
- `make check`, `make docs-check`, `make packaging-check`, planning
  consistency, and `git diff --check` pass on the candidate.
- A closure record at
  `plans/closure/agentic-retrieval-integrity/053-status.md` records the
  candidate SHA, requirement-to-evidence mapping, tests/gates, unchanged
  contract evidence, deviations, and residual risks. M052 remains closed.

## Stop conditions and non-goals

Stop and register a separate decision/plan if either repair requires a tool or
alias change, public argument or response-field change, new provider/default,
new error code, retrieval behavior change, or MCP schema/description re-pin.
Do not broaden this milestone into general locator inference, URL parsing,
security package inference from advisory prose, static workflow-recipe edits,
other follow-up templates, or a redesign of `NextActionBindings` beyond the
narrow source/request values needed here.

This plan repairs only the two audited defects. It does not reopen M052 or
claim implementation, test, live, or closure evidence before those steps are
completed on a candidate.
