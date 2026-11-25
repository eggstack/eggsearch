# M005 — Instruction-Signal Screening

Status: closed (no-go; rejected by ADR-0026 Decision 3)
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

## Decision — no-go (ADR-0026 Decision 3)

Rejected. No `injection_signals` field ships, and no corpus was assembled.
The rejection is a design decision, not a failed measurement: the detector class
cannot meet the bar, and most of the proposed signal already exists.

1. **The signal already exists.** `scan_injection_markers`
   (`src/core/sanitize.rs:221`) detects override phrases, system/assistant role
   prefixes, and ChatML tags, records byte offsets, and `TrustMarkers` reports
   the counts. The marginal capability here is spans and excerpts, not detection.
2. **Invisible-character evasion is already handled.** Tier-1 sanitisation
   strips bidi, zero-width, and control characters, and the marker scan strips
   them before matching so `ign<U+200B>ore` still matches.
3. **The precision bar is a model-class target.** Quoted attacks in security
   articles, API docs, and legal text are ordinary benign content. At most two
   reviewed false positives on 200 benign pages with at least 80 percent recall
   is not reachable with fixed patterns, and model calls are forbidden here.
4. **Hidden text is an extraction gap.** `aria-hidden="true"` is skipped
   (`src/fetch/render/blocks.rs:342`), but `display:none` and
   `visibility:hidden` text survives because extraction has no CSS engine.
   That is a deterministic fidelity defect, not instruction semantics.
5. **Homoglyph mixing cannot be separated from legitimate multilingual text**
   without a model.
6. **A weak field implies a guarantee.** The honest contract is
   `external_untrusted` plus framing plus marker counts. Adding spans and
   excerpts would grow the payload and overstate what the server knows.

Consequences: the fetch trust contract is unchanged; the marker scan's scope and
limits should be stated in `docs/safety.md`; and CSS-hidden text or
confusable-script handling, if wanted, belongs to a separate sanitisation plan
that is not presented as instruction detection.

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

## Corpus and success criteria (retained as the rejected bar)

These thresholds were not measured. They record what a future proposal would
have had to clear; the decision above rejects the detector class rather than
claiming a measurement that was never taken.

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

## Ordered work packages (not executed)

No work package below was started; the record is retained so a future proposal
knows what the spike would have required.

1. Corpus assembly and a labelled fixture manifest (paths and provenance, no
   third-party content that cannot be redistributed).
2. Signal implementations with per-signal unit tests.
3. Measurement harness producing precision, recall, and latency.
4. Go/no-go review recorded in this plan.
5. Only on go: schema field, docs, and integration with the payload budget.

## Verification

No code, corpus, or latency measurement was produced. The existing marker-scan
unit tests in `src/core/sanitize.rs` continue to cover the signal that does
ship, and the accepted decision is recorded in ADR-0026.

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

ADR-0026 Decision 3, the duplicate-signal and precision analysis above, and the
roadmap and registry status change. No corpus manifest, precision table, or
latency measurement exists because the milestone was rejected before that work
was justified.
