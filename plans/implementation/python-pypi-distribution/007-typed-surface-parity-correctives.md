# M007 — Python Typed-Surface Parity Correctives

Status: active
Date: 2026-11-27
Subsystem: Python PyPI distribution and prebuilt SBC wheels
Milestone class: invariant (corrective to the typed binding surface qualified by M006)
Baseline: `main` `699f909` (2026-11-27; source version 0.4.5)
Corrects: `c86d3a6` (agentic M054/M055 retrieval-integrity corrections) changed the canonical Rust tool surface without mirroring the Python typed surface qualified by `plans/closure/python-pypi-distribution/006-status.md`.
Closure: `plans/closure/python-pypi-distribution/007-status.md`

## Objective

Restore parity between the canonical Rust tool schemas and the typed surface in
`bindings/python/python/eggsearch/types.py`, so the hosted `Python native qualification`
workflow returns green on `main`. This is a binding-only corrective; the Rust tool surface,
schemas, workflows, and versions are correct and are not reopened.

## Findings

| Finding | Evidence |
|---|---|
| F1 `provider_status` advertises `response_detail`, but `ProviderStatusRequest` omits it. | Hosted `Python native qualification` run `46067192070` on `699f909`, job `Rebuild and smoke extracted sdist`: `AssertionError: provider_status request type is missing schema properties: ['response_detail']`. The canonical schema properties are `probe`, `recipe_detail`, and `response_detail`. |
| F2 `fetch` advertises `web`, `repo`, and `site` items, but the item test still asserts only `{WebFetchItem, RepoFetchItem}`. | Same run and job: `Extra items in the left set: SiteFetchItem`. The canonical `fetch` schema names all three item types and carries the `SiteQueryNormalization` definition. |
| F3 `SiteFetchItem` exists in `eggsearch.types`, but `eggsearch/__init__.py` does not re-export it, so importing it from the package fails. | Follow-up hosted run `46069918230` on `3a9d649`, job `Rebuild and smoke extracted sdist`: `ImportError: cannot import name 'SiteFetchItem' from 'eggsearch'`. The package exports `WebFetchItem` and `RepoFetchItem`, so the site class is the only missing item export. |

All 26 failing jobs in that run report the same two assertions (each: 2 failed, 26 passed).
The other three main workflows on the same commit succeeded.

## Why M006 verification did not catch it

`python-native-qualification.yml` runs only on pushes to `main`, pull requests to `main`, and
manual dispatch. The 28-commit branch that M057 fast-forwarded into `main` was validated with
dispatched `CI` runs only, so no Python qualification ran after `c86d3a6` changed
`provider_status` and `fetch`. The first push to `main` exposed the accumulated drift in one
run. Process guard: dispatch `Python native qualification` on the branch before
fast-forwarding `main` across Rust tool-surface changes.

## Work packages

- WP1 Mirror the canonical field: add `response_detail: ResponseDetail | None` to
  `ProviderStatusRequest`.
- WP2 Restore the item union and its export: re-export `SiteFetchItem` from `eggsearch`, assert
  it in `FetchItem`, and assert its exact field set (`type`, `url`, `limit`, `include_subdomains`,
  `link_discovery`, `normalize_query`) against the Rust site variant.
- WP3 Prove locally before pushing: compare every `REQUEST_TYPES` entry against the canonical
  `tools/list` schemas from `mcp stdio` (the same data `tool_contracts()` returns), then confirm
  the hosted run.

## Acceptance

- Both failing assertions pass locally against the canonical schema.
- Hosted `Python native qualification` is green on the corrective commit.
- Fingerprint `d7968115e378f708` unchanged; no `src/`, schema, tool, argument, provider,
  workflow, version, or default-set change.

## Non-goals and stop conditions

No typed field beyond the canonical schema; no changes to `src/`, `bindings/python/src`,
`.github/workflows/`, package versions, or published artifacts. Stop and register a separate
decision if parity requires a Rust surface change rather than a binding mirror.

## Verification

- Local typing harness over `types.py` plus the canonical `tools/list` schemas.
- `cargo test --locked --all-features --test mcp_tool_contract` (fingerprint unchanged).
- `python3 packaging/check-planning-consistency.py`, `git diff --check`.
- Hosted `Python native qualification` on the corrective commit.
