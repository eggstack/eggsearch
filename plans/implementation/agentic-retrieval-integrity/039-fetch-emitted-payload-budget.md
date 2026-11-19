# M039 — Fetch Emitted-Payload Budget and Structural Degradation

Status: ready
Date: 2026-11-19
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective to M036)
Decision authority: M036 (unified fetch readable-text budget) and ADR-0016 (canonical nine-tool
inventory). No new ADR: this completes the bound M036 established for the readable measure while
preserving the requested text.

References: `plans/000-long-term-specification.md` §2 (stable MCP tool contract) and §3
(retrieval architecture); `architecture/fetch.md` (batch returned-character budget);
`architecture/codegg-contract.md` (fetch result contract).

## Objective

Make `max_total_chars` bound what the caller actually receives. The requested text keeps
priority: before an item's text is clipped or the item is omitted, the batch envelope drops
optional structural representations — `document.blocks`/`document.chunks`, then `document`,
then `lines` — and reports the degradation. The envelope reports both the readable measure
(`chars_returned`, unchanged) and the emitted payload (`payload_chars`), so an agent can budget
context truthfully.

## Evidence

Measured on 2026-11-19 against candidate `76c1156a2c5418d4b9dab4493eddd9232bdd7b8a`, using the
all-features debug binary, MCP `stdio`, the default `max_total_chars` (50,000; cap 120,000), and
the default full schema profile:

1. A three-item repo batch (two eight-line windows and a README) reported
   `total_chars_returned: 12,223`; the serialized payload was **186,812 bytes**.
2. `rust-lang/rust` `library/std/src/lib.rs` lines 1-8: `text` 443 characters,
   `chars_returned` 712, `document` **64,158 bytes**. The document's
   `text_chars_returned` is **30,271** — the whole file — carried in `blocks` (~31.9 KB) and
   `chunks` (~31.9 KB).
3. The same repository's `src/tools/rustfmt/README.md`: `text` 10,574,
   `chars_returned` 10,863, `document` 28,917 bytes plus `lines` 17,875 bytes.
4. A 100-character window of `https://www.rust-lang.org/`: 197 reported characters,
   11,058-byte payload, `document` 5,408 bytes (window-scoped, but structurally duplicated).
5. The same three-item batch with `response_detail: "compact"` is **19,753 bytes**, because
   compact removes `document`/`lines`.
6. `ResponseDetail::Diagnostic` returns the payload unchanged, and the full schema profile
   defaults to diagnostic, so the default path has no payload bound.
7. `truncate_batch_result_to_budget` clips only `text`; it never degrades structural fields,
   and `chars_returned` excludes them by M036's design.

## Scope

In scope:

- a per-item emitted-payload measure and aggregate enforcement in the batch envelope;
- the structural degradation ladder and its markers;
- additive payload telemetry (`payload_chars`, `total_payload_chars`, degradation markers);
- one structured warning per degraded item;
- tests and documentation for those contracts.

Out of scope:

- changing `max_total_chars`, its cap, or any shipped default;
- `repo_fetch`'s own response shape or budget;
- redefining `response_detail` projections or the readable measure;
- a new tool or argument;
- browser, PDF, challenge, cache, and workspace authorization behavior;
- re-opening M036's omission outcome or its warning taxonomy.

## Invariants

- The requested `text` is never clipped while a droppable structural field remains.
- A successful item's emitted payload fits the remaining aggregate budget under the documented
  measure, within the envelope's documented per-item overhead.
- `chars_returned` keeps its M036 readable-text meaning; `payload_chars` is additive,
  deterministic, and a conservative upper bound on the projected payload.
- Structural degradation is explicit: a marker names what was dropped, and one structured
  warning names the item and the budget; nothing is silently lost.
- Omission keeps M036's contract: `ok: false`, `response: null`, `chars_returned: 0`, a
  budget error, and one warning.
- Result order, stable IDs, labels, trust markers, sanitization, and deterministic tie ordering
  are unchanged.
- The wire schema is additive; no new dependency; `cargo fmt` clean; ordinary files under
  1,600 lines / 80 KB.

## Expected production changes

- `src/mcp/tools/batch_fetch.rs`: the payload measure, the degradation ladder, and the
  warning.
- `src/core/batch_fetch.rs`: additive per-item and aggregate payload fields and the
  degradation marker on `BatchFetchResult`/`BatchFetchResponse`.
- `src/mcp/projection.rs`: retain the additive fields in compact and standard projections.
- `tests/batch_fetch_retrieval.rs`, `tests/mcp_projection.rs`, and
  `tests/extract_fetch_contract.rs`: regression coverage.
- `architecture/fetch.md`, `docs/features.md`, `docs/tool-matrix.md`,
  `docs/test-inventory.md`, and a `CHANGELOG.md` entry.

## Compatibility and migration

- Additive fields only. `chars_returned` does not change meaning; `payload_chars` is new.
- A diagnostic caller reading a window of a large file may lose the structural `document`/`lines`
  when the budget is tight. The requested text survives, the loss is marked, and a larger
  `max_total_chars` or a re-fetch restores it.
- No configuration, cache, storage, or dependency change. No migration.

## Dependencies

- Hard: M036 (closed), which established the readable measure, truncate-before-omit, and the
  failed-omission contract.
- Interface: ADR-0016 and M029 (unified `fetch`).
- Soft: none.

## Work packages

1. **Paper first.** Update `architecture/fetch.md` to define the emitted-payload measure, the
   degradation order, the markers, the warning, and the relationship between `chars_returned`
   and `payload_chars`. This lands before code.
2. **One payload measure.** Measure the serialized item result (response plus its visible
   metadata, excluding the envelope's own arrays and counts) in Unicode scalar values, before
   the envelope projection, as a conservative upper bound: projection only removes fields, so a
   bound measured before it also holds after it. Document the envelope overhead and the
   compact-projection over-count. Keep it deterministic.
3. **Degrade before clipping.** When the item exceeds the remaining budget, apply the ladder in
   order:
   `document.blocks`+`document.chunks` → `document` → `lines` → clip `text` → omit.
   Keep `document.text_chars_returned`, `kind`, and `metadata` for as long as they fit, and
   mark each drop on the item.
4. **Truthful telemetry.** Add `payload_chars` per item and `total_payload_chars` to the
   envelope; retain them in compact and standard projections. `chars_returned` stays the
   readable measure.
5. **Warning.** Emit one structured warning naming the item and the dropped field(s), with the
   stable prefix `batch_payload_budget_degraded`. Map it to the existing
   `fetch_content_truncated` code unless a new `WarningCode` is documented in the same change.
6. **Tests.** Extend the suites with:
   - a windowed read of a large repo file at the default budget returns the requested `text`,
     `ok: true`, and an emitted payload inside the budget;
   - a whole-file repo read whose structure exceeds the budget keeps `text` and drops
     `document`/`lines` with markers;
   - a three-item batch has `total_payload_chars <= max_total_chars` and every item remains
     readable;
   - the ladder order is observable: text is never clipped while a droppable field remains;
   - an item that still cannot fit even its locator and text is omitted under M036's contract;
   - compact and standard projections retain `payload_chars` and the markers;
   - `repo_fetch`'s own response is unchanged.
7. **Documentation.** `architecture/fetch.md`, `docs/features.md`, `docs/tool-matrix.md`
   (the `fetch` envelope wording), `docs/test-inventory.md`, and `CHANGELOG.md`.

## Verification

```bash
cargo test --locked --all-features --test batch_fetch_retrieval
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test extract_fetch_contract
cargo test --locked --all-features --test repo_workflow
cargo test --locked --all-features --test static_guards
make check
make docs-check
./packaging/check-repo-hygiene.sh
```

## Acceptance criteria

- A successful batch never emits item payloads that exceed `max_total_chars` under the
  documented measure, within the documented envelope overhead.
- A windowed read of a large repo file at the default budget returns the requested text; the
  full structural document is not silently included beyond the budget.
- Any dropped `document`/`lines` is visible in an additive marker and one structured warning.
- `chars_returned` is unchanged for every item; `payload_chars` and
  `total_payload_chars` are present, agree with the measured pre-projection payload, and bound
  the projected payload.
- Omitted items keep M036's failed outcome; `repo_fetch` and every shipped default are
  unchanged.
- The broad gate passes with 0 failures.

## Stop conditions

Stop and record rather than working around any of these:

- exact payload measurement requires serializing every item twice in a way that breaks the batch
  deadline; choose a conservative structural estimate, document the error bound, and record the
  tradeoff;
- dropping `document` breaks a documented offset/addressing contract; keep the minimal
  `text_chars_returned`/`kind` skeleton and record the residual schema tradeoff;
- the additive fields cannot remain backward compatible with the existing output schema; defer
  `payload_chars` to a follow-up and enforce the bound without it, recording the missing
  telemetry;
- a shipped default must change to satisfy the criterion (that needs its own measured
  acceptance).

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/039-status.md` with the exact candidate SHA, a
requirement-to-evidence matrix, the before/after payload measurements for the three probes above,
the suite results, the broad-gate counts, the chosen measure and degradation ladder, and
residual risk (including the envelope overhead and any deferred telemetry).
