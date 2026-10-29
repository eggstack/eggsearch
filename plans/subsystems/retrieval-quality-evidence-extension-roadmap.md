# Retrieval Quality Evidence Extension Roadmap

Status: active
Source: corrective follow-up to closed deterministic retrieval quality M006
References: `plans/implementation/deterministic-retrieval-quality/006-comparative-qualification-and-closure.md`; `plans/closure/deterministic-retrieval-quality/006-status.md`; ADR-0006.

## Purpose

Extend the retrieval-quality evidence without reopening or rewriting M006. The
M006 report correctly records the limits of its small synthetic corpus and
unavailable SearXNG/browser runtime. This follow-up adds independently judged
real retrieval tasks, repeated keyless live observations, an actual configured
SearXNG comparison, paired browser/PDF extraction cases, and a real CodeGG
consumer acceptance. All live and consumer runs remain opt-in and outside
routine CI.

## Invariants and non-goals

- Keep the closed M001-M006 evidence immutable; no ranking, provider selection,
  feature default, MCP tool, or CodeGG policy changes are authorized here.
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

## Acceptance and handoff

Register implementation under
`plans/implementation/retrieval-quality-evidence-extension/001-expanded-live-and-consumer-acceptance.md`.
Record source SHA, corpus/annotation fingerprints, provider configuration,
trial windows and budgets, feature/runtime versions, model route and usage,
per-case judgments and failures, plus explicit unavailable/blocked arms in
`plans/closure/retrieval-quality-evidence-extension/001-status.md`.
Do not revise the M006 comparison report's historical findings; link the
extension report as a later evidence window. Any algorithm or production
policy change requires a separate plan and independently held-out evidence.
