# Retrieval Quality Evidence Extension Roadmap

Status: closing (M007); blocked (M008); blocked (M009); blocked (M010)
Source: corrective follow-up to closed deterministic retrieval quality M006
References: `plans/archive/implementation/deterministic-retrieval-quality/006-comparative-qualification-and-closure.md`; `plans/closure/deterministic-retrieval-quality/006-status.md`; ADR-0006.

## Purpose

Extend the retrieval-quality evidence without reopening or rewriting M006. The
M006 report correctly records the limits of its small synthetic corpus and
unavailable SearXNG/browser runtime. This follow-up adds independently judged
real retrieval tasks, repeated keyless live observations, an actual configured
SearXNG comparison, paired browser/PDF extraction cases, and a real CodeGG
consumer acceptance. All live and consumer runs remain opt-in and outside
routine CI.

## Invariants and non-goals

- Keep the closed M001-M006 evidence immutable. M007 is measurement-only and
  authorizes no ranking, provider selection, feature default, MCP tool, or CodeGG
  policy changes. Any such production change needs a separate registered milestone
  and independently held-out evidence; M008 registers the Mojeek default gate.
- Keep routine CI offline and deterministic. Live retrieval and model-backed
  consumer evaluation require explicit manual dispatch and bounded budgets.
- Separate corpus annotations from evaluator implementation and report
  annotator provenance, disagreements, adjudication, and held-out labels.
- Do not store upstream page bodies, credentials, or user search history.
- A provider outage or unconfigured connector is not a quality score. SearXNG,
  browser, or PDF evidence may not be reported as passed until actually run.

## Milestone

| Milestone | Class | Dependencies | Exit |
|---|---|---|---|
| M007 — expanded retrieval evidence and consumer acceptance | infrastructure | Operational: configured keyless providers, reachable SearXNG baseline, Chrome, PDF fixtures, and authorized CodeGG model route | Independently judged real query corpus; repeated bounded keyless trials; observed SearXNG baseline; paired HTML/browser/PDF outcomes; repeated CodeGG task acceptance; exact candidate provenance and honest limitations |
| M008 — Mojeek generic-default provider qualification | capability | Interface: frozen M007 held-out test fixtures and external runner; Operational: reachable keyless Mojeek and current generic defaults | Predeclared paired held-out gate passes on exact candidate; only then change defaults/tests/docs and verify default fan-out, otherwise retain defaults and record the negative/incomplete outcome |
| M009 — paced Mojeek default-qualification rerun | capability (qualification; corrective to the M008 execution protocol) | Hard M008 gate (blocked, unchanged); Interface: frozen fixtures and runner; Operational: reachable keyless Mojeek (confirmed 2026-11-23) | The identical M008 gate re-run under a predeclared paced protocol that records intervals and per-attempt status, with a consecutive-blocked stop rule; defaults change only if every condition passes, and a refused or aborted arm is recorded rather than re-pushed |

## Acceptance and handoff

Register implementation under
`plans/implementation/retrieval-quality-evidence-extension/001-expanded-live-and-consumer-acceptance.md`
and the active follow-up
`plans/implementation/retrieval-quality-evidence-extension/002-cross-domain-extraction-and-resource-qualification.md`.
Record source SHA, corpus/annotation fingerprints, provider configuration,
trial windows and budgets, feature/runtime versions, model route and usage,
per-case judgments and failures, plus explicit unavailable/blocked arms in
`plans/closure/retrieval-quality-evidence-extension/001-status.md`.
Do not revise the M006 comparison report's historical findings; link the
extension report as a later evidence window. M008 is a separate qualification
plan, not an amendment to M007, and authorizes a Mojeek default change only if
its predeclared held-out gate passes. Any other algorithm or production policy
change requires a separate plan and independently held-out evidence.

## Status note (2026-11-23)

M008 remains blocked and the defaults remain `[duckduckgo, brave, yahoo]`. A bounded keyless capture
identified the response behind the thirteen `http_status` observations: `HTTP 403` with Mojeek's own
363-byte "your network appears to be sending automated queries" page. They are corrected to `blocked`
in `plans/closure/retrieval-quality-evidence-extension/008-status.md`, and the classification defect
that hid the page is corrected by M049 under the agentic retrieval integrity roadmap, which changes no
default, enablement, or credential.

The refusal is **query-keyed, not network-wide and not user-agent-wide**. Follow-up controls on this
egress served Mojeek's homepage and served `/search` with the solvable `200` ALTCHA wall for ordinary
queries, including the held-out question `nq:test2352`, for which the shipped client returned five
real results with no provider failures. Only the probe string reproduced the `403`, from three
different client identities. The cohort's own 72 rapid attempts remain the most likely escalation
trigger, so the negative result may partly measure the run's request pressure; a paced, bounded rerun
is a reasonable next step but is not authorised by this note. Mojeek's `robots.txt` still declares
`Disallow: /search`, so any rerun remains an operator decision.

M009 was authorised and executed under that protocol on 2026-11-23 and is **blocked**. Mojeek met its
own reliability condition (18/18 valid, 18/18 non-empty, no refusals, against M008's 5/18), which
confirms the pacing premise, but the gate failed on the controls: Yahoo answered `500` to all
eighteen attempts, so no trial had three valid controls and no paired or unique match could be
established. Condition 4 also failed independently with one distinct qrel pair across one dataset
family. Defaults remain `[duckduckgo, brave, yahoo]` and Mojeek remains disabled.

The binding constraint is now the gate, not the provider: a browser-free keyless candidate cannot
serve Yahoo, so condition 3 cannot pass in that candidate class. Resolving it needs a new registered
milestone that either qualifies with a browser-enabled candidate or predeclares a control set limited
to providers the tested configuration can actually answer. See
`plans/closure/retrieval-quality-evidence-extension/009-status.md`.

M010 answered that question and is **blocked** on relevance. With the `browser` feature included and
`[fetch.browser] enabled = true`, Yahoo went from 0/18 to 18/18, so condition 3's pairing requirement
was satisfied for the first time: all twelve arms scored 18/18 valid, 18/18 non-empty, no refusals,
no aborts, minimum observed interval 25.478 s. Mojeek still produced only one distinct qrel pair across
one dataset family with no unique match (hit@10 2/18, micro recall 2/20) against a floor of three pairs
across two families, while DuckDuckGo produced five pairs and Brave and Yahoo four each. The gate was
not re-tuned after the fact. Mojeek is operationally sound at 1.7 s p50; it does not clear the
minimum-evidence floor on this sample, and defaults remain `[duckduckgo, brave, yahoo]`. See
`plans/closure/retrieval-quality-evidence-extension/010-status.md`.

**Disposition (2026-11-23).** Mojeek remains disabled by default. Browser rendering is already the
shipped default (`default = ["pdf", "browser"]`, with `fetch.browser.enabled` resolving from the
compiled feature), and `default_features_ship_pdf_and_browser` now guards that; the M008/M009
candidates were built with `--no-default-features`, which was the anomaly. The Python wheel remains
browser-free by ADR-0014 §7. No further Mojeek gate attempt is planned without a new milestone.
