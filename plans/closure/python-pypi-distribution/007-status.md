# M007 Closure — Python Typed-Surface Parity Correctives

Status: closed
Date: 2026-11-27
Candidate: `057a86d` on `main`, with precursor corrective commits `3a9d649` and `ec54c73`
Plan: `plans/implementation/python-pypi-distribution/007-typed-surface-parity-correctives.md` (now `closed`; this record closes it)
Baseline: `699f909` (`main`, 2026-11-27; source version 0.4.5)
Corrects: `c86d3a6` (agentic M054/M055) reached `main` without a Python typed-surface update; the first `main` push exposed the drift.

## Requirement-to-evidence matrix

| Finding | Correction | Evidence | Result |
|---|---|---|---|
| F1 `provider_status` advertises `response_detail`; `ProviderStatusRequest` omitted it. | Added `response_detail: ResponseDetail | None`. | Local harness compares all nine `REQUEST_TYPES` against canonical `tools/list` properties: `REQUEST_SCHEMA_PARITY_OK`; `provider_status` properties `probe`, `recipe_detail`, `response_detail`. Hosted run `46072717384` green. | Pass |
| F2 `fetch` advertises `web`, `repo`, and `site` items; the item test asserted only web and repo. | Included `SiteFetchItem` in `FetchItem` and asserted its six fields. | Harness `FETCH_ITEM_UNION_OK`; Python site fields equal the canonical variant. Hosted run `46072717384` green. | Pass |
| F3 `SiteFetchItem` was not re-exported from `eggsearch`. | Added the import and `__all__` entry in `eggsearch/__init__.py`. | Harness `PACKAGE_EXPORTS_OK 13` (every name imported by the test is bound by the package). Hosted run `46072717384` green. | Pass |
| F4 The `WebFetchItem` field assertion omitted `attributes`, `include_selectors`, and `exclude_selectors`. | Refreshed the set and added `test_fetch_item_field_sets_match_canonical_rust_schema`, which derives each field set from the canonical `FetchItem` union. | Harness `ITEM_SCHEMA_PARITY_OK {'web': 20, 'repo': 21, 'site': 6}`. Hosted run `46072717384` green. | Pass |

## Hosted chronology

| Run | Commit | Outcome |
|---|---|---|
| `46067192070` | `699f909` | 26 jobs failed with the same two assertions (F1, F2). CI, egress, and drift succeeded. |
| `46069918230` | `3a9d649` | `Rebuild and smoke extracted sdist` failed on the missing `SiteFetchItem` export (F3). |
| `46071404018` | `ec54c73` | `Emulated ARMv7 installed-wheel smoke` failed on the stale web field set (F4). |
| `46072717384` | `057a86d` | Success: 41/41 jobs. `46072717414` CI success; `46072717344` Release drift guard success. Egress qualification is path-filtered and did not run for the binding-only commits. |

## Verification

- Local harness over `types.py` and canonical `mcp stdio` `tools/list`: request schema parity, item union, per-item field parity, package exports.
- `python3 -m py_compile` on the three changed package files.
- `cargo test --locked --all-features --test mcp_tool_contract`: 23/23, fingerprint `d7968115e378f708` unchanged.
- `python3 packaging/check-planning-consistency.py`: `planning-consistency: ok`; `git diff --check` clean.
- Hosted `Python native qualification` `46072717384`: 41/41 jobs success.

## Invariants held

- No `src/`, schema, tool, argument, provider, default-set, workflow, or version change; fingerprint unchanged.
- Published 0.4.4 artifacts untouched; source version remains 0.4.5.
- All corrections are binding-side mirrors of the existing canonical schema.

## Known limitations

- The Python typing tests run only in hosted `Python native qualification`, not in `make check`, so local gates cannot catch this class of drift.
- The process guard (dispatch `Python native qualification` on a branch before fast-forwarding `main` across tool-surface changes) is recorded in the plan and is not enforced by a guard.
- Three pushes were needed because the stale expectations were layered (union, export, field set); the schema-derived test added here removes the field-set layer.

## Recommendation

Closed. M007 is `closed` in the plan, `plans/registry.md`, and the roadmap, committed with this record. Python M004 remains blocked on native ARMv7 evidence and is unaffected.
