# M009 — Paced Mojeek Default-Qualification Rerun

Status: blocked (executed 2026-11-23; see `plans/closure/retrieval-quality-evidence-extension/009-status.md`)
Date: 2026-11-23
Source: `plans/subsystems/retrieval-quality-evidence-extension-roadmap.md` M009
Primary class: capability (qualification; corrective to the M008 execution protocol)
Baseline source SHA: to be recorded at run time; M008's candidate was `7ec251df2ba715705468f2eb1e5392fa87fc4d32`
References: `plans/implementation/retrieval-quality-evidence-extension/008-mojeek-default-qualification.md`; `plans/closure/retrieval-quality-evidence-extension/008-status.md`; `plans/closure/retrieval-quality-evidence-extension/evidence/mojeek-403-mechanism-controls.json`; external runner `scripts/run_retrieval_quality_external_live.py`; `docs/retrieval-quality-cross-domain-live.md`; ADR-0006; `plans/003-planning-process.md`.
Dependencies: Interface — the frozen M007 SciFact, NQ, and FEVER test query/qrel fixtures and the runner report contract. Operational — reachable keyless Mojeek, DuckDuckGo, Brave HTML, and Yahoo endpoints; confirmed on 2026-11-23. No paid provider, credential, or configured external service.

## Objective and non-goals

Re-run the M008 held-out admission gate for Mojeek with an execution protocol that does not itself
provoke Mojeek's automated-query escalation, so the outcome measures the provider rather than the
cohort's own request pressure.

The gate is unchanged. This milestone changes only how the same frozen trials are paced, and it adds
the per-attempt status capture M008 lacked.

Non-goals: do not amend M008's historical result, do not relax or re-tune the admission thresholds,
do not change the selected query IDs, qrels, or repetitions, do not change ranking, provider
parsing, routing, challenge behavior, or MCP contracts, do not add paid providers or credentials, and
do not retain raw search responses or upstream page bodies. A failed, incomplete, or refused rerun
leaves the defaults unchanged.

## Why a rerun is justified

M008 recorded thirteen Mojeek `http_status` outcomes and concluded Mojeek did not serve this egress.
The 2026-11-23 controls in
`plans/closure/retrieval-quality-evidence-extension/evidence/mojeek-403-mechanism-controls.json`
withdraw the network scope of that reading:

- the same public address served Mojeek's homepage (`200`) and `/search` with the solvable `200`
  ALTCHA wall for ordinary queries, including the held-out question `nq:test2352`;
- the shipped client returned five real results for that question with no provider failures;
- the hard `403` reproduced only for one specific probe string, from three different client
  identities, and changing the user agent or client hints changed nothing.

Sustained automated bursts remain Mojeek's documented escalation path, and M008 issued 72 attempts
without pacing. The rerun therefore tests the same question under a protocol that does not
manufacture the failure.

Mojeek's `https://www.mojeek.com/robots.txt` still declares `User-agent: *` / `Disallow: /search`.
Running this milestone is an operator decision about that stated preference, not a policy-neutral
act, and it stays opt-in and outside routine CI.

## Frozen trial design

- Dataset arms, provider arms, query IDs, and repetitions are identical to M008: `scifact`, `nq`,
  `fever`; `duckduckgo`, `brave`, `yahoo`, `mojeek`; three deterministically selected queries per
  dataset; two repetitions; ten results per query; eighteen-second per-attempt timeout; 72 provider
  attempts total with no retries or extra queries.
- Protocol id `m009-paced-v1`. The dataset selection seed stays the M008 seed
  `m008-mojeek-default-qualification-v1` so the same queries are selected; the protocol id names the
  changed execution parameters and must appear in every report.
- **Pacing.** Minimum 25 s and maximum 40 s between consecutive attempts of the same provider arm,
  derived deterministically from `(protocol id, provider, query id, repetition)` so the schedule is
  reproducible. No two attempts to the same provider within 25 s.
- **Stop rule.** Three consecutive `blocked` outcomes for one provider arm abort that arm. The
  refusals are recorded and the gate fails for that arm; the arm is not retried to push through.
- **Cooldown.** At most one full rerun per 24 h from the same egress.
- **Status capture.** Every attempt records the classified `error_class`, the HTTP status when the
  response carried one, and the attempt start time, so pacing and refusals are checkable from the
  report rather than asserted.
- A temporary, credential-free qualification config enabling only these four keyless HTML providers
  while retaining the shipped three-provider default list; every trial explicitly selects one
  provider. Record the config fingerprint.

## Work package 1 — runner protocol support (implementation)

`scripts/run_retrieval_quality_external_live.py` currently exposes `--query-count`,
`--repetitions`, `--seed`, `--max-results`, `--timeout-seconds`, and `--max-provider-queries`, and
records only `error_class` per provider failure (`{"id": …, "error_class": …}`). It has no pacing
and no status capture, which is why M008 could not distinguish refusal classes.

1. Add a protocol option group: `--protocol-id`, `--min-interval-seconds` (default 25),
   `--max-interval-seconds` (default 40), and `--consecutive-blocked-stop` (default 3). Defaults
   preserve today's behaviour only when the protocol flags are absent; a paced run must pass a
   protocol id and be branded in the report.
2. Wait the deterministic per-attempt interval before each attempt, and record the planned and actual
   interval plus the attempt start time in the report.
3. Record `http_status` per attempt, parsing it from the CLI provider-failure message when the
   engine reports one (for example `status 403`), and never storing a response body.
4. Abort an arm on the consecutive-blocked stop rule and mark the arm `aborted` with the refusal
   count; do not silently convert an aborted arm into a zero-result arm.
5. Add runner unit tests for the schedule determinism, the stop rule, and status extraction from the
   two message shapes the engines emit.

## Predeclared admission gate

Identical to M008. All conditions are required; otherwise the defaults do not change.

1. Trial integrity: all 12 reports identify the same source SHA, release binary SHA, runner
   fingerprint, protocol id, selected query IDs, seed, result limit, timeout, and pacing parameters;
   the recorded intervals satisfy the declared minimum; no paid providers or credentials were used;
   every attempt and every non-success outcome is represented.
2. Mojeek operations: at least 17/18 outcomes valid (`results` or `zero_results`), at least 5/6 valid
   in each dataset, and at least 4/6 non-empty in each dataset. Timeouts, provider errors, aborted
   arms, and command errors count as invalid.
3. Comparable controls: each existing default provider has at least 4/6 valid outcomes in each
   dataset; a Mojeek match counts as unique only on a query/repetition where all three controls have
   valid outcomes and none matched that corpus document.
4. Held-out contribution: at least three distinct query/document pairs across at least two dataset
   families, with at least one pair unique under condition 3.
5. Reporting: per-arm valid/error/empty/aborted counts, matched qrel pairs and denominators, hit@10,
   macro and micro recall@10, query-level unique contributions, latency distribution, refusal
   reasons, and limitations. No qrel match is never read as proof of irrelevance.

## Ordered work packages

1. Implement and test the runner protocol support (work package 1).
2. Freeze the candidate: build `cargo build --release --locked --no-default-features --bin eggsearch`,
   record the source SHA, binary SHA-256, runner fingerprint, config fingerprint, protocol id, and
   the resolved attempt schedule.
3. Run the twelve arms once each, sequentially, honouring the pacing, stop rule, and 24 h cooldown.
4. Score, record the gate matrix, and either change defaults plus tests and user-facing
   documentation together, or record M009 blocked with the defaults untouched.
5. Reconcile M009, the roadmap, and the registry in the same change as the closure record.

## Acceptance and stop conditions

- The runner change passes `cargo test` for the affected suites and the repository hygiene checks,
  and a dry run on local fixtures shows the declared intervals and the stop rule firing.
- A paced run is valid only if the report itself demonstrates the intervals; a report without timing
  evidence fails condition 1.
- Defaults change only when every gate condition passes on the exact candidate.
- **Stop:** a third consecutive blocked outcome ends the arm; a refused arm is not retried within
  24 h; no gate threshold, query ID, or qrel is revised after observing results.

## Execution outcome

Executed 2026-11-23 under `m009-paced-v1`. Mojeek met its own reliability condition with 18/18 valid,
18/18 non-empty outcomes and no refusals — against M008's 5/18 — which confirms the pacing premise.

The gate still failed, on the controls rather than on Mojeek: Yahoo, a shipped default, answered
`500` to all eighteen attempts, so no Mojeek trial had all three controls valid and no paired or
unique contribution could be established. Condition 4 also failed on its own terms with one distinct
qrel pair across one dataset family. Defaults are unchanged and Mojeek remains disabled. See
`plans/closure/retrieval-quality-evidence-extension/009-status.md`.
