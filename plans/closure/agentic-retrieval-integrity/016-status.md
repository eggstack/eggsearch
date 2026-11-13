# M016 Anubis `metarefresh` Solver — Closure Status

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/016-anubis-metarefresh-solver.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Policy: `plans/adrs/ADR-0010-challenge-solver-coverage-and-cost-policy.md`

Planning baseline: `dace41c3a75307ea35b745d695a11630ec6495c0`

Implementation candidate and closure commit: the commit that carries this record.

## Result

`AnubisMethod::Metarefresh` and `METHOD_METAREFRESH` are added to
`src/meta/challenge/anubis.rs`. The method performs no hashing, consumes no attempt from
`SolveBudget`, and submits only values the challenge itself published.

Parsed shape, synthetic reconstruction (no live `metarefresh` deployment was reachable):

```html
<meta http-equiv="refresh" content="2; url=/metarefresh-check">
<script id="anubis_challenge" type="application/json">
{"rules":{"algorithm":"metarefresh","difficulty":1},
 "challenge":{"id":"01a1cc17-…","method":"metarefresh","challenge":"published-token",
              "redir":"/search?q=rust","difficulty":1,"spent":false}}
</script>
```

- The wait comes from the meta refresh's leading numeric field in seconds; the URL field is the
  submission target. A JSON `wait` field, when present, must agree with the meta refresh or the
  challenge is malformed.
- The answer token is the published `challenge` string, echoed verbatim and percent-encoded for
  transport. The submission appends `result=<token>` and, when the target does not already carry
  it, `id=<published id>`.
- A missing meta refresh, a non-numeric/negative/non-finite wait, a missing token, a conflicting
  `wait`, and a target that is missing, unresolvable, or cross-origin all fail closed as
  `malformed parameters` before any wait or request.
- `metarefresh_wait` refuses with `wall clock exhausted` when the requirement does not fit the
  remaining budget; `verify` takes the shared `Instant` deadline and returns `wall clock exhausted`
  instead of sending when the remaining time is zero.

The `0.8 × difficulty` wait is *not* implemented or derived. The fixture uses the published value
in the served document, as the plan requires; the hypothesis was only a comparison point and no
live page was observed to compare against.

## Verification

```bash
cargo fmt --all -- --check                    # clean
cargo clippy --locked --all-targets --all-features -- -D warnings   # clean
cargo check --locked --no-default-features    # clean
cargo test --locked --all-features --lib meta::challenge            # 52 passed
cargo test --locked --all-features --lib meta::engines::startpage   # 40 passed
cargo test --locked --all-features                                  # 6225 passed / 28 ignored
```

Solver-level evidence: `metarefresh_dispatches_and_echoes_only_published_values`,
`metarefresh_malformed_pages_fail_closed`, `metarefresh_wait_never_exceeds_the_budget`,
`metarefresh_above_ceiling_is_blocked_without_a_request`,
`metarefresh_submits_the_published_echo_once`.

Engine-level evidence: `metarefresh_challenge_solves_and_replays_once` (one submission, one
replay), `metarefresh_wait_beyond_budget_is_blocked_without_a_submission` (zero submissions),
`disabled_metarefresh_keeps_blocked_with_one_request`.

Echo-rule evidence: the submission URL in
`metarefresh_dispatches_and_echoes_only_published_values` is asserted byte-for-byte as
`{origin}/metarefresh-check?id=…&result=published-token-123`; no request header, cookie, user agent,
or operator value reaches it. The builder takes only the challenge and the resolved published
target.

`attempts_used() == 0` is asserted after a successful solve, proving `max_attempts` does not bound
this method.

## Live status

No deployment publishing `method = "metarefresh"` was reachable on 2026-11-13. The method is
**unproven in the field**; every observation above is synthetic. Startpage currently publishes
`fast` at difficulty 6. A live page that publishes the wait in an ambiguous unit, publishes no
answer token, publishes a cross-origin target, uses a different token field name, or serves the
method without `#anubis_challenge` is a stop-and-decide condition, not something this record claims
to have handled.

## Residual risk

- Field drift in the served `metarefresh` shape (wait unit, token field, target) is untested.
- The shipped `timeout_ms = 2000` cannot honour a `0.8 × difficulty` wait above difficulty 2; an
  operator must raise `timeout_ms` for harder `metarefresh` pages. This is a documented
  consequence, not a new knob.
- The method's wait is a server-published value echoed back; a malicious upstream could publish an
  arbitrary wait up to the configured ceiling. The wall-clock ceiling and the response deadline
  bound it.
