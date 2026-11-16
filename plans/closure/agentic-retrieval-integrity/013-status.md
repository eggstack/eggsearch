# M013 Challenge Solver Conformance Corrections — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/013-challenge-solver-conformance-corrections.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Policy: `plans/adrs/ADR-0010-challenge-solver-coverage-and-cost-policy.md`

Planning baseline: `dace41c3a75307ea35b745d695a11630ec6495c0`

Implementation candidate and closure commit: the commit that carries this record.

## Result

All five conformance defects are corrected in `src/meta/challenge/anubis.rs` and
`src/meta/challenge/mod.rs`, with the test module moved to
`src/meta/challenge/anubis_tests.rs` through `#[path]` so `anubis.rs` stays inside the
ordinary-file limits (764 lines / 27 KB; the test file is separate and private).

| # | Defect | Correction | Regression test |
|---|---|---|---|
| 1 | `anubis_base_prefix` ignored | `script#anubis_base_prefix` is parsed as a JSON string, validated, normalised, and joined before `PASS_CHALLENGE_PATH`; `unsafe base prefix` is the new bound | `base_prefix_defaults_to_empty_and_normalises_one_trailing_slash`, `base_prefix_refuses_unsafe_values`, `base_prefix_is_redacted_in_debug`, `prefixed_challenge_solves_under_the_published_prefix` |
| 2 | `slow` refused as unsupported | `METHOD_SLOW` dispatches to the `fast` path with no new variant; the unregistered-method negatives now use a synthetic `quantum` label | `slow_is_an_alias_of_fast`, `slow_method_solves_through_the_fast_path` |
| 3 | `spent` unchecked | `challenge.spent` is parsed as a tri-state; `true` refuses as `already spent`, a non-boolean is malformed, absent means unspent | `spent_challenge_is_refused_before_any_work`, `spent_challenge_is_blocked_without_a_submission` |
| 4 | whole deadline burned | `CANDIDATE_NANOS = 130` from the recorded release measurement; `expected_candidates`, `compact_count`, and `check_projection` refuse before hashing with the projection and the operative ceiling in the sentence | `projection_arithmetic_is_exact`, `projection_refuses_before_hashing`, `projection_breach_blocks_with_zero_requests`, `projection_breach_is_blocked_without_a_submission` |
| 5 | published `redir` unvalidated | `resolved_redir` accepts only a same-origin absolute URL or a single-slash root-relative path without `..` or control characters; everything else falls back to the request URL | `redir_is_validated_before_it_reaches_the_query`, `preact_cross_origin_redir_falls_back_to_the_request_url` |

## Baseline failure evidence

The regression tests were authored against the corrected code. Their baseline failure is
established by the pre-change source, not by re-running the suite at `dace41c3`:

- `slow` was the *asserted* negative control at baseline (`unknown_method_fails_closed`), so the
  baseline behaviour is directly recorded in the old test.
- The prefix script was never read, so the prefix path and Debug-redaction assertions had no
  implementation to satisfy.
- `spent` was never read, so a spent page parsed and was submitted.
- No projection existed; `check_projection` did not compile against the baseline.
- `resolved_redir` returned any non-empty published string, so the cross-origin and scheme table
  could not pass.

This is recorded as an evidence gap rather than presented as an executed baseline run.

## Verification

```bash
cargo fmt --all -- --check                    # clean
cargo clippy --locked --all-targets --all-features -- -D warnings   # clean
cargo check --locked --no-default-features    # clean
cargo test --locked --all-features --lib meta::challenge            # 52 passed
cargo test --locked --all-features --lib meta::engines::startpage   # 40 passed
cargo test --locked --all-features                                  # 6225 passed / 28 ignored
cargo test --locked --all-features --test engine_parse_conformance  # 6 passed
python3 packaging/check-planning-consistency.py                     # ok
```

The shipped Startpage shape is unchanged: `enabled_challenge_solves_and_replays_once` passes
without edits to its expectations, including the five pass-challenge query parameters and
`elapsedTime=1`.

## Behaviour changes

- An unusable prefix now reports `unsafe base prefix` instead of a false `pass cookie rejected`.
- A spent challenge reports `already spent` instead of spending the budget.
- A `slow` deployment solves instead of reporting a false `unsupported scheme`.
- An unprojectable challenge is refused before hashing, naming the projection and the ceiling.


Hosted CI: run `43452679850` on `dc352ae` passed all three jobs — Ubuntu `ci`, the Windows
all-features check, and the macOS canonical verification plus filesystem-portability tests.

## Residual risk

- No prefixed, `slow`, or `preact` deployment was reachable on 2026-11-13; those paths are
  synthetic-fixture verified only. `fast` remains the only live-observed method.
- The `CANDIDATE_NANOS` constant is a release-build figure; debug builds are about two orders of
  magnitude slower and their projection refusals are not a valid performance baseline.
- Percent-encoded traversal in a published `redir` is not decoded locally and is left to the
  server's normalisation, as recorded in the plan.
- ADR-0010's `Related:` line already named the correct `013-challenge-solver-conformance-corrections.md`
  and `016-anubis-metarefresh-solver.md` files at this candidate, so no correction was needed; the
  plan's work-package note was stale on that point.
