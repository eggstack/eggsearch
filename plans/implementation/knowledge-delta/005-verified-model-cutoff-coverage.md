# M005 — Verified Model-Cutoff Coverage and Provenance

Status: closed
Closure: `plans/closure/knowledge-delta/005-status.md` on implementation candidate `e817dfde35372aa307f5580a144d7ededcb55574`
Date: 2026-11-20
Subsystem: knowledge-delta
Milestone class: capability
Planning baseline: `16b4c2888206caeb6654ffebaa039862fc143712` on `feat/auto-20261117-044c4885`; reconciled after M004 closed on `380a55cd7ea7c0b37dc6eff53851926e96e894d0`
Source addendum: `plans/subsystems/knowledge-delta-corrective-addendum.md` M005
Closed predecessors: M001 (`plans/archive/implementation/knowledge-delta/001-cutoff-aware-knowledge-brief.md`), `plans/closure/knowledge-delta/001-status.md`, candidate `c0f71909cfe2e6016512423e55b9d00fdb7220c5`; and the M004 corrective to M001 (`plans/closure/knowledge-delta/004-status.md`, candidate `380a55cd7ea7c0b37dc6eff53851926e96e894d0`), which changed brief coverage accounting and provider-timestamp reporting but not cutoff resolution, `BUNDLED_CUTOFF_SNAPSHOT`, or `src/core/knowledge_delta.rs`.
Authority: `plans/000-long-term-specification.md` §§1–4, 7–9; accepted ADR-0006, ADR-0020; `plans/003-planning-process.md` §7.

## Corrective finding and why earlier gates missed it

`src/meta/knowledge_delta/snapshot.rs` currently contains one verified model (`gpt-6-astra`). The M001 date/alias tests exercised known, unknown and ambiguous cutoff semantics; they did not require a diverse model registry or a repeatable update procedure. A syntactically correct resolver can thus return `unknown` for nearly every real agent model. This is a *coverage/maintenance* gap, not a claim that M001 closure was invalid.

## Objective, readiness and scope

Produce a reproducible, provenance-rich offline snapshot of **verifiable** model knowledge cutoffs across relevant model families, with a safe update procedure and explicit unknowns. Keep the resolver deterministic and useful without external API calls.

**Hard dependencies:** M001 closed; ADR-0020 accepted — satisfied on baseline. **Interface:** preserve existing `SnapshotRow`, `CutoffOrigin`, `DatePrecision`, operator overrides, and caller override precedence. **Operational:** record source/license/terms assessment before distributing third-party catalog data.

In scope: a maintained source manifest, priority model ID/alias inventory, a small offline validation/generation utility (if justified), expanded bundled snapshot, evidence/freshness visibility, alias-conflict checks, and regression fixtures. Out of scope: fetching metadata during each `knowledge_brief` call, guessing cutoffs from model release dates, changing model identities on behalf of clients, scraping login walls, importing full scraped catalogs blindly, or persisting user model histories.

## Ordered work packages

1. **Source inventory and policy.** At implementation time, inspect official model cards/docs, Models.dev metadata and the OpenRouter model listing/API. Verify actual field semantics and any usage/redistribution restrictions; document canonical URLs and verification date. Separate source publication dates from *model knowledge-cutoff* dates; prefer official sources, then corroborated secondaries. Do not treat model-version dates as cutoffs.
2. **Priority coverage inventory.** Obtain currently used model identifiers from existing supported user-agent examples and CodeGG/eggui integration documentation; start with OpenAI GPT-6 variants, xAI Grok 4.x, MiniMax, DeepSeek and GLM where those exact IDs exist. Maintain at least 12 priority ID/alias lookups with a source-backed disposition: verified date, unresolved conflict, no published cutoff, or alias unconfirmed. Never invent a cutoff to satisfy a target.
3. **Deterministic import hygiene.** Store a bounded, readable manifest that mirrors `SnapshotRow`: `model_id`, aliases, `date`, `precision`, `source`, `source_url`, `origin`, `verified_at`, status and reviewer/verification note. Prefer checked-in source records and an offline converter/validator over runtime network metadata dependencies. Reject duplicate or cross-family aliases, invalid/impossible dates, undocumented precision upgrades, source precedence inversions and unbounded metadata. Keep unknown entries outside `BUNDLED_CUTOFF_SNAPSHOT` unless represented explicitly in the validated registry contract.
4. **Snapshot and diagnostics.** Expand `src/meta/knowledge_delta/snapshot.rs` only with source-verified entries; retain the current row unless new verification contradicts it and the discrepancy is documented. If source truth changes, mark the previous value as superseded in the manifest and test the precedence/fallback path. Include catalog version/as-of and exact-source provenance in `model_resolution` where compatible; avoid a schema break or a count/rename change to canonical MCP tools.
5. **Fixtures and docs.** Capture offline official-vs-secondary precedence, alias collision, unknown cutoff, inconsistent month/year/day precision, and stale or unavailable catalog cases. Update `docs/config.md`, `docs/agent-workflows.md`, `architecture/research.md`, `architecture/codegg-contract.md` and test inventory; publish operator override examples for a model with no documented cutoff.

## Verification

Focused commands on the exact candidate:
```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_brief
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test property_knowledge_delta
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test property_identity
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test mcp_tool_contract
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test static_guards
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```
Add a network-free `tests/knowledge_delta_catalog.rs` suite or equivalent with manifest validation and source fixtures; run it with `--features mock`. Test keyless/offline startup and Python/SDK parity if `model_resolution` changes.

## Acceptance and stop conditions

- Every priority model lookup is explicitly classified with dated evidence or an auditable unknown/conflict disposition; the accepted bundled snapshot grows beyond the current one verified row **without** falsely assigning a cutoff. Record actual verified coverage as numerator/denominator and named providers; do not hide unknowns.
- At least six distinct verified model IDs across at least two provider families are published **if supported by source evidence**. If not supportable, do not substitute guesses: leave milestone open, publish verified partial coverage and the source evidence showing the blocker.
- Model alias resolution and date precision are deterministic in all fixtures, and no runtime network call is introduced. Existing cutoff/classification behavior, IDs, and override precedence do not regress.
- Security, package and docs gates pass on the exact candidate; the manifest is inspectable and sources are legally usable for the intended snapshot.

**Stop:** metadata sources prohibit necessary redistribution, official vs secondary disagreements cannot be reconciled without guessing, data demands a new always-on service, or a change in ADR-0020 cutoff semantics is needed. Escalate a new decision, never silently change the accepted contract.

## Required closure evidence

Write `plans/closure/knowledge-delta/005-status.md` with source-manifest hash, model coverage table (including unknowns), actual source verification dates, conflicting evidence, changed snapshot rows, tests/guards and offline startup, docs/SDK evidence, candidate SHA and residual risks. Update the corrective addendum and registry together. Do not alter M001's historical closure.
