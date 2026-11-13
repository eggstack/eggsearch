# M025 — Anubis `metarefresh` Presentation Corrective

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M025; corrective to M016
Primary class: invariant (corrective)
Baseline: `fd2634e`
References: ADR-0010; `plans/implementation/agentic-retrieval-integrity/016-anubis-metarefresh-solver.md`; `plans/closure/agentic-retrieval-integrity/016-status.md`; `src/meta/challenge/anubis.rs`; `src/meta/engines/startpage.rs`; `src/mcp/tools/web_fetch.rs`; `architecture/engines.md` § Challenge solving.
Dependencies: ADR-0010 (accepted); M016 is closed. This is a new corrective plan, not an amendment to M016.

## Objective

Make the `metarefresh` solver independent of how the challenge page presents the
refresh. M016 implemented only the `<meta http-equiv="refresh">` shape and a
`challenge` field in the JSON. A live Anubis v1.28.3 deployment publishes neither:

```text
GET https://anubis.techaro.lol/docs/design/how-anubis-works/  (2026-11-13)
HTTP/2 200
refresh: 2; url=/.within.website/x/cmd/anubis/api/pass-challenge?challenge=<64-hex>&id=<uuid>&redir=%2Fdocs%2F

#anubis_challenge:
{"rules":{"algorithm":"metarefresh","difficulty":1},
 "challenge":{"issuedAt":"...","metadata":{...},"id":"<uuid>","method":"metarefresh",
              "randomData":"<128-hex>","policyRuleHash":"...","difficulty":1,
              "spent":false,"extensions":null}}
```

There is no `<meta http-equiv="refresh">` element and no `"challenge"` key. The echo
token is `challenge.randomData`, the same field the `fast` method uses. The current
parser therefore fails in two independent ways:

- `published_meta_refresh` requires a `<meta>` element and returns `malformed
  parameters` when only the `Refresh:` header is present.
- `from_metarefresh` requires a `challenge` string and returns `malformed parameters`
  for the live object, which has only `randomData`.

Even on a page that happens to include the meta element, the live object would still
be refused. M016's fixture invented both the meta element and the `challenge` field,
which is why its synthetic tests passed.

The fix is to treat the presentation as optional and the echo token as
`randomData`:

- If a meta refresh or `Refresh:` header is available, use its wait and target.
- If neither is available, derive the wait as `difficulty + 1` seconds (the value the
  upstream itself advertises) and synthesize the same-origin pass-challenge URL from
  the parsed `id`, `randomData`, and the original request URL.
- Submit `challenge=<randomData>`, `id=<id>`, and `redir=<original request>`; never
  append a `result=` parameter.

## Current implementation evidence

- `AnubisChallenge::parse` dispatches `METHOD_METAREFRESH` to
  `published_meta_refresh(&document)`; a missing meta element is
  `BOUND_MALFORMED`.
- `from_metarefresh` stores `challenge: required_str(object, "challenge")?` and
  requires the JSON `wait`, when present, to agree with the meta refresh.
- `metarefresh_url` appends `result=<token>` and, when absent, `id=<id>`. The live
  target already carries `challenge` and `id`, so the extra parameter is ignored by
  the server, but it is not the canonical contract.
- `metarefresh_wait` refuses with `wall clock exhausted` when the published wait does
  not fit the remaining budget; `attempts_used()` stays zero for this method.
- M016's closure records: "no deployment publishing `metarefresh` was reachable" and
  names a page that serves the method without `#anubis_challenge` as a stop-and-decide
  condition. The live page does serve `#anubis_challenge`; the defect is the two
  field assumptions above.

## Non-goals

- No header plumbing unless the synthesized URL proves insufficient. The solver is
  called with the challenge body and the request URL; the protocol is deterministic
  from those values.
- No new configuration key, no new method, no change to the difficulty ceiling or the
  wait policy beyond honouring the published or derived value.
- No change to the `fast`, `preact`, ALTCHA, or fetch-detection paths.
- No browser, user-agent, or retry change.
- No re-solve on a still-challenged replay.

## Invariants that cannot regress

- The submission echoes only values the challenge published: `randomData`, `id`, and
  the original request URL. No request header, cookie, user agent, or operator value
  reaches it.
- The wait is at least `difficulty * 800 ms`; the derived wait is `difficulty + 1`
  seconds; the published wait is used when it is present and usable. Every wait is
  bounded by the remaining call deadline.
- The target is same-origin, root-relative or absolute; a cross-origin or
  unresolvable target fails closed before any wait or request.
- A spent challenge, a malformed object, or a missing `randomData` fails closed before
  any wait.
- `metarefresh` consumes no `SolveBudget` attempts; `attempts_used()` remains zero.
- One submission per solve; no redirect following; the response body is bounded.
- Ten MCP tools and the refusal vocabulary are unchanged; no cookie or token appears
  in logs, errors, warnings, `Debug`, or fixtures.

## Expected production changes

1. **Optional presentation.** Change `AnubisChallenge::parse` for
   `METHOD_METAREFRESH` so the meta refresh is optional. Parse the `Refresh:` header
   only if the call site can supply it; otherwise proceed with the JSON-derived
   values. The two accepted presentations are:
   - `<meta http-equiv="refresh" content="N; url=...">`;
   - no presentation at all, in which case the wait is `difficulty + 1` seconds and
     the target is synthesized.
2. **Echo token.** Use `challenge.randomData` as the echoed token. Accept a legacy
   `challenge` field only when `randomData` is absent, and only for backward
   compatibility with a captured page; a page carrying both must use `randomData`.
3. **Submission builder.** Add a `metarefresh_submission` builder that takes the
   base URL, the validated base prefix, the request URL, the challenge ID, and the
   random data, and returns:
   `{base}{prefix}/.within.website/x/cmd/anubis/api/pass-challenge?redir=<request URL>&challenge=<randomData>&id=<id>`.
   Validate same-origin, reject control characters and traversal, and never append
   `result=`.
4. **Wait derivation.** In `from_metarefresh`, store the published wait when present
   (meta or header) and a derived `difficulty + 1` seconds otherwise. If a JSON
   `wait` is present alongside a published wait, keep the agreement check; if the
   published wait is absent, accept the JSON wait when it is bounded. `metarefresh_wait`
   still refuses a requirement beyond the remaining budget.
5. **Tests.**
   - Live-shaped header-only fixture with no meta element and `randomData`, asserting
     the synthesized URL byte-for-byte and the derived wait.
   - No-presentation fixture with neither meta nor header, same assertion.
   - Existing meta fixture remains green as a regression.
   - A page with both `randomData` and a legacy `challenge` uses `randomData`.
   - Spent, missing `randomData`, cross-origin target, and wait-beyond-budget cases
     fail closed with zero requests.
   - Engine-level test with a header-only page: one submission, one replay.
   - `attempts_used() == 0` after a successful solve.
6. **Docs.** `architecture/engines.md` § Challenge solving (both presentations and
   the derived wait), `tests/fixtures/engines/README.md` (live observation date and
   URL), `CHANGELOG.md`.

## Ordered work packages

1. Optional presentation and `randomData` echo in the parser and challenge struct.
2. Submission builder and wait derivation, with unit tests.
3. Engine and fetch wiring tests with the header-only shape.
4. Live record, documentation, and the closure record.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --lib meta::challenge
cargo test --locked --all-features --lib meta::engines::startpage
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Live, recorded rather than asserted: `web_fetch` against
`https://anubis.techaro.lol/docs/design/how-anubis-works/` with
`[search.challenge] enabled = true`, difficulty ceiling at least 1, attempts at
least 1000, and enough timeout for the wait. Record whether the returned body is the
documentation page, the wait observed, and the challenge method. If the site stops
publishing `metarefresh`, record that instead and keep the synthetic evidence.

## Compatibility and closure evidence

No MCP tool, response-schema, or configuration-key change. A page carrying the legacy
`challenge` field without `randomData` remains accepted for backward compatibility;
a page carrying both uses `randomData`. The meta-tag path remains a tested regression,
so a deployment that still publishes it does not change behaviour. The derived wait is
`difficulty + 1` seconds, so the shipped `timeout_ms = 2000` may need raising even for
difficulty 1; this is the same operator consequence M016 recorded, not a new knob.

Required closure evidence: the implementation commit; the focused and full test
outputs; the live `web_fetch` record against the v1.28.3 page (or the exact reason it
was unreachable, with the synthetic evidence retained); the header-only and
presentation-less fixtures; the documentation updates; and the residual risk if the
live shape drifts again. The record must state the observed token field and wait
source, not just that the tests passed.

## Acceptance and stop conditions

- A header-only `metarefresh` page with no meta element solves and submits to the
  synthesized URL with `challenge=<randomData>` and `id=<id>`.
- A page with neither meta nor header solves with the derived `difficulty + 1`
  second wait.
- The existing meta-tag fixture still solves with its published wait.
- A live-shaped page with `randomData` and no `challenge` field is not rejected as
  malformed.
- A spent, cross-origin, or over-budget page performs zero submissions.
- Stop and return for a decision if the live page requires a different token field,
  a different wait unit, or a target that cannot be reconstructed from the published
  values; record the exact shape rather than guessing.
