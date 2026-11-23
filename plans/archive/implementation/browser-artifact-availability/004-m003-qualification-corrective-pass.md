# B004 — M003 Qualification Corrective Pass

Status: closed (corrective criteria and full exact-candidate M003 requalification passed; closure evidence is in `plans/closure/browser-artifact-availability/003-status.md`)
Source: `plans/subsystems/browser-artifact-availability-roadmap.md` M004
Primary class: infrastructure and corrective evidence
Baseline: `f76851324f917999152871f49d2302bef87d437d` (branch `feat/auto-20261110-0a674cdc`)
References: `plans/archive/implementation/browser-artifact-availability/003-distribution-and-live-qualification.md`; `plans/closure/browser-artifact-availability/003-status.md`; `plans/adrs/ADR-0014-browser-artifact-availability-and-render-bounds.md`; `plans/subsystems/python-pypi-distribution-roadmap.md` M003; `plans/closure/python-pypi-distribution/003-status.md`; `plans/subsystems/eggpack-release-adoption-roadmap.md` M001; `plans/closure/eggpack-release-adoption/001-status.md`.
Dependencies: Interface: M003's unchanged ten-tool schemas and qualification contract. Operational: the approved publicly reachable HTTPS origin for synthetic fixture pages, used during requalification. The full M003 acceptance suite is now requalified on exact candidate `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`; evidence is in the M003 closure record.

## Objective

Correct the typed-request declarations that fail the installed-wheel smoke, replace unsupported macOS arm64 runner labels at their source, and obtain an authorized fixture route that can exercise browser rendering without weakening the existing private-target protections. This new corrective pass did not amend the original M003 plan and was not sufficient by itself for M003 closure; the full exact-candidate acceptance evidence is recorded in the M003 closure record.

## Initial evidence

- PR candidate `f76851324f917999152871f49d2302bef87d437d` ordinary CI passed in run `43804158099`; Eggpack drift and contract checks passed in run `43804158070`.
- Python wheel, emulated ARMv7, and extracted-sdist smoke cells failed at `bindings/python/tests/test_native.py:139` in run `43804158035`. Direct comparison against the downloaded macOS arm64 wheel found exactly two missing fields in each of `BatchFetchRequest`, `WebFetchRequest`, and `ResearchSearchRequest`: `offset` and `limit`.
- Egress qualification run `43804158039` was cancelled for macOS arm64 before steps because its `macos-14` runner configuration is unsupported. The same stale label appears in `release/eggpack/github-policy.json`, the generated release workflow, and the candidate-qualification and egress workflows.
- The initial local realistic-mix run was not evidence: browser interception rejects private/local targets even when the HTTP fetch config allows them. At that time no public fixture origin had been authorized or started.

## Follow-up exact-candidate evidence

- Candidate `8faa0f15a40f336f361d523c9b45b3fe3b0106ad` passed hosted seven-target qualification (`43813413696`), the Python wheel/sdist matrix (`43808981282`), hosted CI (`43808981290`), egress qualification (`43808981328`), direct macOS arm64 install/release smoke, and live ten-tool MCP verification.
- The approved public synthetic fixture origin was used without changing browser private-target protections. The 294-sample adjusted report observed 39 expected cold browser escalations, idle teardown, post-idle relaunch, and final process cleanup, but remained non-qualifying because `max_requests = 13` tore down only after render 14. M005 tracks the confirmed counter-reset defect.
- `packaging/test-install.sh` remains blocked by the workstation's `mavis-trash: unsupported OS: Plan9` guard. PowerShell installer and release-smoke wrappers were not run because neither `pwsh` nor `powershell` is installed.

## Latest exact-candidate follow-up

- Candidate `fb99a43734bd4f2329bdfa2ba4315b403c2cd985` passes local realistic-mix qualification (294 samples, 39 expected cold browser escalations, request-13 cap teardown, request-14 relaunch), macOS arm64 installer/release smoke, and live ten-tool MCP checks. Local evidence paths are recorded in the M003 closure record.
- The exact-SHA qualifier run `43888773102` on predecessor `cdc9445ae121d4f0e0508be9df1b94c3289621f0` failed in `packaging/test-install.ps1:12`: the mapping assertion was sensitive to CRLF checkout line endings. Commit `fb99a43` normalizes line endings before matching. In qualifier `43901942424`, Windows validation job `133432522588` passed the installer/release-smoke wrappers and uploaded SHA-keyed artifact `eggpack-windows-wrapper-evidence-fb99a43734bd4f2329bdfa2ba4315b403c2cd985`. Its evidence records `source_revision=fb99a43734bd4f2329bdfa2ba4315b403c2cd985`, target `x86_64-pc-windows-msvc`, `test_install=passed`, and `release_smoke=passed`.
- On `fb99a43`, hosted CI (`43897293997`), egress (`43897294001`), and Eggpack drift (`43897293439`) passed. Qualifier `43901942424`, attempt 2, passed all seven builds, qualification stages, and consumer validations, including aarch64 macOS; Windows wrappers passed and uploaded evidence. `required_gate` job `133470803916` and `aggregate` job `133471719996` passed, with aggregate outcome `complete`; the exact-target size inventory is recorded in the M003 closure record. Python full-matrix run `43916322850`, attempt 4, completed successfully on the same SHA with all 34 jobs passing, including the dependency audit, ARMv7 and sdist smokes, and the full installed-wheel matrix. PR-associated run `43897293446`, attempt 7, also passed all 34 jobs and all displayed PR checks are green. PR #16 merged as `aea34eeb89749e3196712d9e9b9741f75c58e760` on 2026-11-16T21:05:25Z. The corrections and original M003 acceptance criteria are complete on this exact candidate; evidence is in the M003 closure record.

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

The full original M003 acceptance criteria—including the authorized realistic-mix metrics, hosted seven-target run, complete wheel/sdist matrix, installer/release wrappers, live ten-tool MCP verification, and hosted CI—passed on exact candidate `fb99a43734bd4f2329bdfa2ba4315b403c2cd985`; see the M003 closure record. No fixture was published outside the approved origin. This corrective pass is closed; PR-linked run `43897293446`, attempt 7, passed all 34 jobs. PR #16 merged as `aea34eeb89749e3196712d9e9b9741f75c58e760` on 2026-11-16T21:05:25Z. Stop if any schema or security invariant regresses.

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
