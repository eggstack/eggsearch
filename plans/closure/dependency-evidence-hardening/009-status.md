# Dependency Evidence Hardening M009 — Final Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/dependency-evidence-hardening/010-final-closure-and-registry-reconciliation.md`

Source subsystem roadmap:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

Terminal corrected candidate: `12d8f9bf52217cef30e53006ee412dac25b79f03`.

Hosted CI: [run 36217887427](https://github.com/eggstack/eggsearch/actions/runs/36217887427) passed on that exact candidate.

## 1. Workstream evidence

M001-M007 remain represented by their original closure records at
`plans/closure/dependency-evidence-hardening/001-status.md` through
`007-status.md`. Those records document the historical candidates they
qualified. The post-M007 audit and the corrective work are recorded separately
in [M008 closure](008-status.md), implemented at
`3fd3076c4ad9680490b35a96759cab3f5022e6bc` and closed by the terminal
candidate above.

M008 corrected three defects: public-advisory applicability now requires
compatible artifact provenance; package identity uses ecosystem-specific
normalization with exact matching where no shared normalizer is established;
and malformed structured inputs cannot appear as Complete-empty results.
Distinct provenance and target evidence remain distinct during assessment
deduplication. The full policy and regression matrix are in `008-status.md`.

## 2. Exact-candidate qualification

All local gates below were run against terminal corrected candidate
`12d8f9bf52217cef30e53006ee412dac25b79f03`:

| Gate | Result |
|---|---|
| `make check` | Pass; 3,293 library tests, all integration suites, 4 doctests, repo hygiene, and packaging contract completed without a reported failure |
| `cargo test --locked --all-features` | Pass as the all-feature test stage of `make check` |
| Focused applicability and dependency parser suites | Covered by `make check`; focused rerun on implementation parent passed 169 security/applicability tests and 36 dependency fixture/property tests |
| `make bench-check` | Pass; optimized Criterion harness compiled with `--no-run` |
| `RUSTUP_TOOLCHAIN=nightly make fuzz-smoke` | Pass; validate_url, sanitize_pipeline, bounded_response_reader, and dependency_parse campaigns completed without a crash. The final dependency_parse campaign completed 335,994 runs. |
| Hosted repository CI | Pass; run `36217887427` on the exact terminal candidate SHA |

One earlier local `make check` attempt was interrupted by SIGKILL during the
long `bounded_command` test. The isolated suite then passed all 31 tests, and
the complete canonical gate was rerun on the same candidate and completed
through doctests, hygiene, and packaging checks. This was a transient process
interruption, not a test assertion failure.

No production source or runtime behavior changed in M009. M009 reconciles
planning state and records the already-qualified M008 candidate.

## 3. Remaining limitations and disposition

The intentional unverified cases remain as documented in M008: where
checked-in source evidence cannot establish public-registry artifact identity,
dependency-driven applicability is Unknown and does not claim affectedness or
fixedness. Conservative exact package matching may omit case-variant matches
for ecosystems without an established shared normalization rule. Neither
limitation is a correctness blocker.

No unresolved High or Medium finding remains. No dependency or binary-size
delta was introduced. M001-M007 historical closure records were not changed.

## 4. Recommendation

Recommendation: **closed**. M001-M009 are closed, the roadmap and registry
identify this record as terminal controlling evidence, and the required CI and
local qualification evidence refer to the same corrected candidate.
