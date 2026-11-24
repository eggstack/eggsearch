# M005 — Instruction-Signal Screening

Status: proposed (research spike; no production behavior until measured)
Date: 2026-11-24
Subsystem: web acquisition expansion
Milestone class: capability (research-first)
Baseline: `d932115d1f86af51710747d243455792ff63a6fb` on `docs/user-facing-polish-20261123`
Decision authority: `architecture/fetch.md` (sanitisation and trust markers); ADR-0006

References: `plans/003-planning-process.md` §2.4; `docs/safety.md`; `src/core/sanitize.rs`; `src/mcp/excerpt_economy.rs` (bounded passage framing).

## Objective

Tell an agent when fetched text contains content that addresses a model as if it
were an instruction, with deterministic evidence and no blocking, so the caller
can raise its own guard. This is a signal, not a filter.

## Why this is a spike

Sanitisation already removes control characters and unsafe markup, and every
fetched document is marked `external_untrusted`. What is missing is a
measurable answer to "does this page contain instruction-shaped text". A
detector that fires on ordinary documentation is worse than none, so precision
must be measured before any schema field ships.

## Candidate signals (deterministic, no model)

- Imperative second-person addressing combined with tool, credential, or
  system-prompt vocabulary.
- Text hidden by CSS (`display:none`, zero font size, off-screen positioning,
  `aria-hidden`) that survives extraction.
- Runs of zero-width or bidirectional control characters, and homoglyph mixing
  inside instruction-shaped sentences.
- Long base64 or hex blobs adjacent to instruction vocabulary.
- Instruction-shaped text inside HTML comments or `<template>` content.
- Known override patterns ("ignore previous instructions") as bounded literals.

Each signal produces a kind, a character span, and a short excerpt. Nothing is
rewritten and nothing is removed.

## Contract (only if the spike passes)

Additive optional field on fetched documents:

```text
injection_signals: [{ kind, span_start, span_end, excerpt }]
```

Capped at 8 signals and 256 characters per excerpt, ordered by span start, and
covered by the existing payload budget. Trust markers are unchanged; the field
is evidence, not a verdict, and never blocks or alters extracted text.

## Corpus and success criteria

- A benign corpus of at least 200 captured pages across docs, blogs, forums,
  legal text, and API references: zero signals is the target, and every hit must
  be reviewed as a documented false positive.
- An injection corpus assembled from published prompt-injection samples plus
  synthetic variants: at least 80 percent detection at the signal level, with
  every miss classified by reason.
- Latency budget: under 5 ms per 100k characters on the existing extraction
  path.

If the benign corpus produces more than two reviewed false positives, or
detection stays below 80 percent, the milestone is rejected and the field is
not shipped.

## Ordered work packages

1. Corpus assembly and a labelled fixture manifest (paths and provenance, no
   third-party content that cannot be redistributed).
2. Signal implementations with per-signal unit tests.
3. Measurement harness producing precision, recall, and latency.
4. Go/no-go review recorded in this plan.
5. Only on go: schema field, docs, and integration with the payload budget.

## Verification

Corpus harness results, per-signal tests, latency measurement, and a regression
test that a clean page produces an empty signal list. `make check`;
`make docs-check`.

## Acceptance and stop conditions

- Benign false positives within the review budget, detection at or above the
  threshold, latency inside budget.
- **Stop:** any signal requires a model, network access, or unbounded scanning;
  or the field would change trust markers or block content.

## Non-goals

- Blocking, quarantining, rewriting, or redacting content.
- Accusation, scoring, or a numeric risk value.
- Model-based classification.
- Screening tool output, prompts, or conversation state.

## Closure evidence

Corpus manifest, precision and recall table, latency measurement, false-positive
review notes, the go/no-go decision, and either the shipped field evidence or a
recorded rejection.
