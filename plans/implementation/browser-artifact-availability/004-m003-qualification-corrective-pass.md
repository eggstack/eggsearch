# B004 — M003 Qualification Corrective Pass

Status: active (public-fixture qualification is blocked pending maintainer authorization)
Source: `plans/subsystems/browser-artifact-availability-roadmap.md` M004
Primary class: infrastructure and corrective evidence
Baseline: `f76851324f917999152871f49d2302bef87d437d` (branch `feat/auto-20261110-0a674cdc`)
References: `plans/implementation/browser-artifact-availability/003-distribution-and-live-qualification.md`; `plans/closure/browser-artifact-availability/003-status.md`; `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md`; `plans/subsystems/python-pypi-distribution-roadmap.md` M003; `plans/closure/python-pypi-distribution/003-status.md`; `plans/subsystems/eggpack-release-adoption-roadmap.md` M001; `plans/closure/eggpack-release-adoption/001-status.md`.
Dependencies: Interface: M003's unchanged ten-tool schemas and qualification contract. Operational: maintainer approval and an approved publicly reachable HTTPS origin for synthetic fixture pages. M003 remains blocked until every original acceptance condition is requalified on one exact candidate.

## Objective

Correct the typed-request declarations that fail the installed-wheel smoke, replace unsupported macOS arm64 runner labels at their source, and obtain an authorized fixture route that can exercise browser rendering without weakening the existing private-target protections. This is a new corrective pass; it does not amend the original M003 plan or imply that M003 has closed.

## Current evidence

- PR candidate `f76851324f917999152871f49d2302bef87d437d` ordinary CI passed in run `43804158099`; Eggpack drift and contract checks passed in run `43804158070`.
- Python wheel, emulated ARMv7, and extracted-sdist smoke cells failed at `bindings/python/tests/test_native.py:139` in run `43804158035`. Direct comparison against the downloaded macOS arm64 wheel found exactly two missing fields in each of `BatchFetchRequest`, `WebFetchRequest`, and `ResearchSearchRequest`: `offset` and `limit`.
- Egress qualification run `43804158039` was cancelled for macOS arm64 before steps because its `macos-14` runner configuration is unsupported. The same stale label appears in `release/eggpack/github-policy.json`, the generated release workflow, and the candidate-qualification and egress workflows.
- The attempted local realistic-mix run is not evidence: browser interception rejects private/local targets even when the HTTP fetch config allows them. No public host or tunnel has been authorized or started.

## Non-goals

- No MCP tool, JSON schema, argument, response, fingerprint, or runtime behavior change.
- No relaxation or bypass of SSRF, private-address, DNS-resolution, or browser-interception protections.
- No direct edits to `.github/workflows/release-eggpack.yml`; change its Eggpack producer inputs and regenerate it.
- No public fixture publication, tunnel, DNS change, or externally reachable server without explicit maintainer authorization.
- No M003 closure, PyPI republishing, release staging, or merge based only on these corrective changes.

## Invariants

- All ten MCP tools and their canonical schemas remain unchanged.
- Every canonical schema property remains represented by its Python request `TypedDict`.
- Browser navigation continues to reject localhost, private IP literals, and DNS answers resolving to private addresses.
- Eggpack remains the release-workflow producer authority; generated workflow drift must be checked.
- A green result on a new SHA does not transfer any earlier qualification evidence.

## Ordered work packages

1. Add the missing `offset` and `limit` optional integer declarations to the three Python request types and improve the existing contract assertion's failure message without weakening its subset invariant.
2. Set the Eggpack macOS arm64 source runner to the documented supported `macos-15` label, regenerate `.github/workflows/release-eggpack.yml` with the pinned Eggpack CLI, and update the macOS arm64 labels in `.github/workflows/qualify-eggpack-candidate.yml` and `.github/workflows/egress-feature-qualify.yml`. Add a regression assertion in the existing qualification-contract test suite so the unsupported mapping is caught.
3. Stop at the fixture permission boundary. After approval, use a controlled public HTTPS fixture origin with synthetic content only; confirm DNS resolves publicly, preserve the prescribed fixture mix and evidence shape, and retain the existing SSRF protections. Without approval and a reachable origin, record this work package as blocked and produce no qualifying metrics.
4. Run focused Python contract and workflow checks, `make producer-drift`, `make packaging-check`, and the applicable Rust contract suite. On the resulting exact candidate, rerun the complete wheel/sdist matrix and all original M003 gates; update M003's closure evidence only with results actually obtained.

## Acceptance and stop conditions

The code corrective pass is acceptable only when the Python schema contract passes against the canonical Rust schemas, the supported runner mapping is used consistently by producer and qualification workflows, Eggpack regeneration has no drift, and the focused Rust and packaging checks pass. Record any known workstation-only wrapper stop without bypassing it.

M003 remains blocked until the full original acceptance criteria pass on one exact candidate, including the authorized realistic-mix metrics, hosted seven-target run, complete wheel/sdist matrix, installer/release wrappers, live ten-tool MCP verification, and hosted CI. Stop without publishing fixtures if authorization or an approved reachable origin is absent. Stop if any schema or security invariant regresses.

## Verification commands

```bash
python -m pytest -q bindings/python/tests/test_native.py -k typed_requests
cargo test --locked --all-features --test egress_qualify_contract
make producer-drift
make packaging-check
make check
```

On this workstation, `make packaging-check` stops at `mavis-trash: unsupported OS: Plan9` inside `packaging/check-contract.sh`. Workflow pin, planning, Eggpack qualifier, and egress target contracts passed before that stop; do not bypass the workstation guard. Re-run the wrapper on a supported host and record its result.

The full wheel/sdist, candidate, realistic-mix, installer, and live-tool gates are exact-candidate requalification and are not replaced by these local checks.
